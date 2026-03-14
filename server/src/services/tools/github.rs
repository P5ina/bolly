use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn repos_dir(workspace_dir: &Path, instance_slug: &str) -> PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("repos")
}

fn repo_dir(workspace_dir: &Path, instance_slug: &str, repo: &str) -> PathBuf {
    let safe_name = repo.replace('/', "--");
    repos_dir(workspace_dir, instance_slug).join(safe_name)
}

fn run_git(args: &[&str], cwd: &Path, token: &str) -> Result<String, ToolExecError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo")
        // Use git config env vars to rewrite https://github.com/ URLs to include the token
        .env("GIT_CONFIG_COUNT", "1")
        .env(
            "GIT_CONFIG_KEY_0",
            format!("url.https://x-access-token:{token}@github.com/.insteadOf"),
        )
        .env("GIT_CONFIG_VALUE_0", "https://github.com/")
        .output()
        .map_err(|e| ToolExecError(format!("failed to run git: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(ToolExecError(format!(
            "git {} failed (exit {}):\n{stderr}",
            args.first().unwrap_or(&""),
            output.status.code().unwrap_or(-1)
        )))
    }
}

fn run_gh(args: &[&str], cwd: &Path, token: &str) -> Result<String, ToolExecError> {
    let output = Command::new("gh")
        .args(args)
        .current_dir(cwd)
        .env("GH_TOKEN", token)
        .env("NO_COLOR", "1")
        .output()
        .map_err(|e| ToolExecError(format!("failed to run gh: {e} — is gh CLI installed?")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(ToolExecError(format!(
            "gh {} failed (exit {}):\n{stderr}",
            args.first().unwrap_or(&""),
            output.status.code().unwrap_or(-1)
        )))
    }
}

fn validate_repo(repo: &str) -> Result<(), ToolExecError> {
    if !repo.contains('/') || repo.contains("..") || repo.contains(' ') {
        return Err(ToolExecError(format!(
            "invalid repo format: '{repo}' — expected 'owner/repo'"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// github_clone
// ---------------------------------------------------------------------------

pub struct GithubCloneTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    token: String,
}

impl GithubCloneTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, token: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            token: token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GithubCloneArgs {
    /// Repository in "owner/repo" format (e.g. "facebook/react").
    pub repo: String,
}

impl Tool for GithubCloneTool {
    const NAME: &'static str = "github_clone";
    type Error = ToolExecError;
    type Args = GithubCloneArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "github_clone".into(),
            description: "Clone a GitHub repo (or pull latest). Returns local path.".into(),
            parameters: openai_schema::<GithubCloneArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        validate_repo(&args.repo)?;

        let dir = repo_dir(&self.workspace_dir, &self.instance_slug, &args.repo);

        if dir.join(".git").exists() {
            // Already cloned — pull latest
            log::info!("[github_clone] pulling {}", args.repo);
            run_git(&["fetch", "--all", "--prune"], &dir, &self.token)?;
            let default_branch = run_git(
                &["symbolic-ref", "refs/remotes/origin/HEAD", "--short"],
                &dir,
                &self.token,
            )
            .unwrap_or_else(|_| "origin/main".into());
            let branch = default_branch.trim().trim_start_matches("origin/");
            run_git(&["checkout", branch], &dir, &self.token)?;
            run_git(&["pull", "--ff-only"], &dir, &self.token)?;
            let log = run_git(&["log", "--oneline", "-5"], &dir, &self.token)?;
            Ok(format!(
                "Pulled latest for {repo}.\nLocal path: {path}\nBranch: {branch}\n\nRecent commits:\n{log}",
                repo = args.repo,
                path = dir.display(),
            ))
        } else {
            // Fresh clone (shallow for speed)
            log::info!("[github_clone] cloning {}", args.repo);
            let parent = repos_dir(&self.workspace_dir, &self.instance_slug);
            fs::create_dir_all(&parent)
                .map_err(|e| ToolExecError(format!("failed to create repos dir: {e}")))?;

            let url = format!("https://github.com/{}.git", args.repo);
            let dir_name = args.repo.replace('/', "--");
            run_git(
                &["clone", "--depth", "50", &url, &dir_name],
                &parent,
                &self.token,
            )?;

            // Configure git identity
            let _ = run_git(
                &["config", "user.name", "bolly"],
                &dir,
                &self.token,
            );
            let _ = run_git(
                &["config", "user.email", "bolly@localhost"],
                &dir,
                &self.token,
            );

            let log = run_git(&["log", "--oneline", "-5"], &dir, &self.token)?;
            Ok(format!(
                "Cloned {repo}.\nLocal path: {path}\n\nRecent commits:\n{log}",
                repo = args.repo,
                path = dir.display(),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// github_branch
// ---------------------------------------------------------------------------

pub struct GithubBranchTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    token: String,
}

impl GithubBranchTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, token: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            token: token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GithubBranchArgs {
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Name of the new branch to create.
    pub branch: String,
    /// Base branch to create from (default: repo's default branch).
    pub base: Option<String>,
}

impl Tool for GithubBranchTool {
    const NAME: &'static str = "github_branch";
    type Error = ToolExecError;
    type Args = GithubBranchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "github_branch".into(),
            description: "Create a new git branch. Repo must be cloned first.".into(),
            parameters: openai_schema::<GithubBranchArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        validate_repo(&args.repo)?;
        let dir = repo_dir(&self.workspace_dir, &self.instance_slug, &args.repo);
        if !dir.join(".git").exists() {
            return Err(ToolExecError(format!(
                "repo not cloned yet — use github_clone first"
            )));
        }

        // Fetch latest
        run_git(&["fetch", "--all"], &dir, &self.token)?;

        let base = args.base.as_deref().unwrap_or("HEAD");
        run_git(
            &["checkout", "-b", &args.branch, base],
            &dir,
            &self.token,
        )?;

        Ok(format!(
            "Created and switched to branch '{}' (based on {base})\nPath: {}",
            args.branch,
            dir.display()
        ))
    }
}

// ---------------------------------------------------------------------------
// github_commit_push
// ---------------------------------------------------------------------------

pub struct GithubCommitPushTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    token: String,
}

impl GithubCommitPushTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, token: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            token: token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GithubCommitPushArgs {
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Commit message.
    pub message: String,
    /// Specific files to stage (relative to repo root). If empty, stages all changes.
    #[serde(default)]
    pub files: Vec<String>,
}

impl Tool for GithubCommitPushTool {
    const NAME: &'static str = "github_commit_push";
    type Error = ToolExecError;
    type Args = GithubCommitPushArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "github_commit_push".into(),
            description: "Stage, commit, and push. Won't push to main/master directly.".into(),
            parameters: openai_schema::<GithubCommitPushArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        validate_repo(&args.repo)?;
        let dir = repo_dir(&self.workspace_dir, &self.instance_slug, &args.repo);
        if !dir.join(".git").exists() {
            return Err(ToolExecError("repo not cloned yet".into()));
        }

        // Check current branch — refuse main/master
        let branch = run_git(&["branch", "--show-current"], &dir, &self.token)?;
        let branch = branch.trim();
        if branch == "main" || branch == "master" {
            return Err(ToolExecError(format!(
                "refusing to push directly to '{branch}' — create a branch first with github_branch"
            )));
        }

        // Stage
        if args.files.is_empty() {
            run_git(&["add", "-A"], &dir, &self.token)?;
        } else {
            let mut cmd_args = vec!["add", "--"];
            let files: Vec<&str> = args.files.iter().map(|f| f.as_str()).collect();
            cmd_args.extend(files);
            run_git(&cmd_args, &dir, &self.token)?;
        }

        // Check if there's anything to commit
        let status = run_git(&["status", "--porcelain"], &dir, &self.token)?;
        if status.trim().is_empty() {
            return Ok("nothing to commit — working tree is clean".into());
        }

        // Commit
        run_git(&["commit", "-m", &args.message], &dir, &self.token)?;

        // Push
        run_git(
            &["push", "-u", "origin", branch],
            &dir,
            &self.token,
        )?;

        let hash = run_git(&["rev-parse", "--short", "HEAD"], &dir, &self.token)?;
        Ok(format!(
            "Committed and pushed to '{branch}'\nCommit: {}\nRepo: {}",
            hash.trim(),
            args.repo
        ))
    }
}

// ---------------------------------------------------------------------------
// github_create_pr
// ---------------------------------------------------------------------------

pub struct GithubCreatePrTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    token: String,
}

impl GithubCreatePrTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, token: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            token: token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GithubCreatePrArgs {
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Pull request title.
    pub title: String,
    /// Pull request body/description (markdown).
    pub body: String,
    /// Base branch for the PR (default: repo's default branch).
    pub base: Option<String>,
    /// Create as draft PR.
    #[serde(default)]
    pub draft: bool,
}

impl Tool for GithubCreatePrTool {
    const NAME: &'static str = "github_create_pr";
    type Error = ToolExecError;
    type Args = GithubCreatePrArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "github_create_pr".into(),
            description: "Create a GitHub pull request. Branch must be pushed first.".into(),
            parameters: openai_schema::<GithubCreatePrArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        validate_repo(&args.repo)?;
        let dir = repo_dir(&self.workspace_dir, &self.instance_slug, &args.repo);
        if !dir.join(".git").exists() {
            return Err(ToolExecError("repo not cloned yet".into()));
        }

        let mut gh_args = vec![
            "pr", "create",
            "--repo", &args.repo,
            "--title", &args.title,
            "--body", &args.body,
        ];

        if let Some(ref base) = args.base {
            gh_args.push("--base");
            gh_args.push(base);
        }

        if args.draft {
            gh_args.push("--draft");
        }

        let result = run_gh(&gh_args, &dir, &self.token)?;
        Ok(result.trim().to_string())
    }
}

// ---------------------------------------------------------------------------
// github_issues
// ---------------------------------------------------------------------------

pub struct GithubIssuesTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    token: String,
}

impl GithubIssuesTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, token: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            token: token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GithubIssuesArgs {
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Filter by state: "open", "closed", or "all". Default: "open".
    pub state: Option<String>,
    /// Filter by labels (comma-separated).
    pub labels: Option<String>,
    /// Maximum number of issues to return. Default: 20.
    pub limit: Option<u32>,
}

impl Tool for GithubIssuesTool {
    const NAME: &'static str = "github_issues";
    type Error = ToolExecError;
    type Args = GithubIssuesArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "github_issues".into(),
            description: "List issues on a GitHub repository. Returns issue number, title, state, labels, and assignees.".into(),
            parameters: openai_schema::<GithubIssuesArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        validate_repo(&args.repo)?;

        let state = args.state.as_deref().unwrap_or("open");
        let limit = args.limit.unwrap_or(20).min(100).to_string();

        let cwd = repos_dir(&self.workspace_dir, &self.instance_slug);
        fs::create_dir_all(&cwd).ok();

        let mut gh_args = vec![
            "issue", "list",
            "--repo", &args.repo,
            "--state", state,
            "--limit", &limit,
            "--json", "number,title,state,labels,assignees,createdAt",
        ];

        let labels_str;
        if let Some(ref labels) = args.labels {
            labels_str = labels.clone();
            gh_args.push("--label");
            gh_args.push(&labels_str);
        }

        run_gh(&gh_args, &cwd, &self.token)
    }
}

// ---------------------------------------------------------------------------
// github_read_issue
// ---------------------------------------------------------------------------

pub struct GithubReadIssueTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    token: String,
}

impl GithubReadIssueTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, token: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            token: token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GithubReadIssueArgs {
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Issue number.
    pub number: u64,
}

impl Tool for GithubReadIssueTool {
    const NAME: &'static str = "github_read_issue";
    type Error = ToolExecError;
    type Args = GithubReadIssueArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "github_read_issue".into(),
            description: "Read a specific GitHub issue with its full body and comments.".into(),
            parameters: openai_schema::<GithubReadIssueArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        validate_repo(&args.repo)?;

        let cwd = repos_dir(&self.workspace_dir, &self.instance_slug);
        fs::create_dir_all(&cwd).ok();

        let number = args.number.to_string();
        run_gh(
            &[
                "issue", "view", &number,
                "--repo", &args.repo,
                "--json", "number,title,body,state,labels,assignees,comments,createdAt",
            ],
            &cwd,
            &self.token,
        )
    }
}
