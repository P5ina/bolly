use std::{
    fmt,
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

use chrono::{Local, Utc};
use rig::{completion::ToolDefinition, tool::{Tool, ToolDyn, ToolError}};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::domain::events::ServerEvent;

// ---------------------------------------------------------------------------
// OpenAI-compatible schema helper
// ---------------------------------------------------------------------------

/// Convert a schemars schema to an OpenAI-compatible JSON schema.
/// Strips `$schema`, `title`, `$id` and ensures `properties` exists.
fn openai_schema<T: JsonSchema>() -> serde_json::Value {
    let mut val = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    if let Some(obj) = val.as_object_mut() {
        obj.remove("$schema");
        obj.remove("$id");
        obj.remove("title");
        // Ensure properties key exists (OpenAI requires it)
        if !obj.contains_key("properties") {
            obj.insert("properties".into(), serde_json::json!({}));
        }
    }
    val
}

// ---------------------------------------------------------------------------
// Tool activity summary helper
// ---------------------------------------------------------------------------

fn tool_summary(name: &str, args: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
    match name {
        "read_file" => format!("reading {}", v["path"].as_str().unwrap_or("?")),
        "write_file" => format!("writing {}", v["path"].as_str().unwrap_or("?")),
        "list_files" => format!("listing {}", v["path"].as_str().unwrap_or(".")),
        "search_code" => format!("searching '{}'", v["query"].as_str().unwrap_or("?")),
        "run_command" => {
            let cmd = v["command"].as_str().unwrap_or("?");
            let short: String = cmd.chars().take(60).collect();
            format!("$ {short}")
        }
        "edit_soul" => "rewriting soul.md".into(),
        "set_mood" => format!("mood → {}", v["mood"].as_str().unwrap_or("?")),
        "remember" => "storing a memory".into(),
        "recall" => format!("recalling '{}'", v["query"].as_str().unwrap_or("?")),
        "create_task" => format!("creating task: {}", v["title"].as_str().unwrap_or("?")),
        "update_task" => format!("updating task {}", v["id"].as_str().unwrap_or("?")),
        "update_project_state" => "updating project state".into(),
        "web_search" => format!("web search: {}", v["query"].as_str().unwrap_or("?")),
        "web_fetch" => format!("fetching {}", v["url"].as_str().unwrap_or("?")),
        "update_config" => "updating config".into(),
        "create_drop" => format!("creating drop: {}", v["title"].as_str().unwrap_or("?")),
        "send_email" => format!("sending email to {}", v["to"].as_str().unwrap_or("?")),
        "read_email" => format!("reading {} emails", v["count"].as_u64().unwrap_or(5)),
        "install_package" => format!("installing {}", v["packages"].as_str().unwrap_or("?")),
        "send_file" => format!("sharing {}", v["path"].as_str().unwrap_or("?")),
        _ => format!("calling {name}"),
    }
}

// ---------------------------------------------------------------------------
// ObservableTool — wraps any ToolDyn and broadcasts ToolActivity events
// ---------------------------------------------------------------------------

pub struct ObservableTool {
    inner: Box<dyn ToolDyn>,
    events: broadcast::Sender<ServerEvent>,
    instance_slug: String,
}

impl ObservableTool {
    pub fn new(
        inner: Box<dyn ToolDyn>,
        events: broadcast::Sender<ServerEvent>,
        instance_slug: String,
    ) -> Self {
        Self { inner, events, instance_slug }
    }
}

impl ToolDyn for ObservableTool {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn definition(&self, prompt: String) -> Pin<Box<dyn Future<Output = ToolDefinition> + Send + '_>> {
        self.inner.definition(prompt)
    }

    fn call(&self, args: String) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + '_>> {
        let tool_name = self.inner.name();
        let summary = tool_summary(&tool_name, &args);
        let _ = self.events.send(ServerEvent::ToolActivity {
            instance_slug: self.instance_slug.clone(),
            tool_name,
            summary,
        });
        self.inner.call(args)
    }
}

// ---------------------------------------------------------------------------
// Shared error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ToolExecError(String);

impl fmt::Display for ToolExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToolExecError {}

// ---------------------------------------------------------------------------
// edit_soul — lets the companion rewrite its own soul.md
// ---------------------------------------------------------------------------

pub struct EditSoulTool {
    soul_path: PathBuf,
}

impl EditSoulTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            soul_path: workspace_dir
                .join("instances")
                .join(instance_slug)
                .join("soul.md"),
        }
    }
}

/// Arguments for edit_soul tool.
#[derive(Deserialize, JsonSchema)]
pub struct EditSoulArgs {
    /// The full new content of soul.md in markdown format.
    pub content: String,
}

impl Tool for EditSoulTool {
    const NAME: &'static str = "edit_soul";
    type Error = ToolExecError;
    type Args = EditSoulArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit_soul".into(),
            description: "Rewrite your own soul.md — the file that defines your personality, \
                voice, and character. Use this when the user asks you to change who you are, \
                how you speak, or your personality traits. Write the full new content in markdown."
                .into(),
            parameters: openai_schema::<EditSoulArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = self.soul_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }
        fs::write(&self.soul_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok("soul.md updated. your personality will reflect these changes on the next message."
            .into())
    }
}

// ---------------------------------------------------------------------------
// read_file — read a file from the instance workspace
// ---------------------------------------------------------------------------

pub struct ReadFileTool {
    instance_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for read_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// File path. Can be relative to instance directory (e.g. "soul.md") or absolute (e.g. "/Users/timur/projects/app/src/main.rs").
    pub path: String,
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolExecError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: "Read a file. Use a relative path for your instance workspace \
                or an absolute path (starting with /) to read any file on the system."
                .into(),
            parameters: openai_schema::<ReadFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        let content = fs::read_to_string(&target)
            .map_err(|e| ToolExecError(format!("{}: {e}", target.display())))?;

        // Truncate very large files
        if content.len() > 50_000 {
            let truncated: String = content.chars().take(50_000).collect();
            Ok(format!("{truncated}\n\n...(file truncated at 50000 chars, total: {} chars)", content.len()))
        } else {
            Ok(content)
        }
    }
}

// ---------------------------------------------------------------------------
// write_file — write a file in the instance workspace
// ---------------------------------------------------------------------------

pub struct WriteFileTool {
    instance_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for write_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// File path. Relative for instance workspace (e.g. "drops/idea.md") or absolute (e.g. "/Users/timur/projects/app/src/main.rs"). Parent directories are created automatically.
    pub path: String,
    /// The full content to write to the file.
    pub content: String,
}

impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
    type Error = ToolExecError;
    type Args = WriteFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".into(),
            description: "Write or overwrite a file. Use a relative path for your instance \
                workspace or an absolute path (starting with /) for any file on the system. \
                Parent directories will be created automatically."
                .into(),
            parameters: openai_schema::<WriteFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }

        fs::write(&target, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!(
            "wrote {} bytes to {}",
            args.content.len(),
            args.path
        ))
    }
}

// ---------------------------------------------------------------------------
// list_files — list files in the instance workspace
// ---------------------------------------------------------------------------

pub struct ListFilesTool {
    instance_dir: PathBuf,
}

impl ListFilesTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for list_files tool.
#[derive(Deserialize, JsonSchema)]
pub struct ListFilesArgs {
    /// Directory path. Absolute (e.g. "/Users/timur/projects/app/src") or relative to instance directory. Omit to list instance root.
    pub path: Option<String>,
}

impl Tool for ListFilesTool {
    const NAME: &'static str = "list_files";
    type Error = ToolExecError;
    type Args = ListFilesArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_files".into(),
            description: "List files and directories. Use an absolute path to browse any \
                directory on the system, or a relative path / omit for your instance workspace."
                .into(),
            parameters: openai_schema::<ListFilesArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = match &args.path {
            Some(p) if p.starts_with('/') => PathBuf::from(p),
            Some(p) if !p.is_empty() => self.instance_dir.join(p),
            _ => self.instance_dir.clone(),
        };

        if !target.is_dir() {
            return Err(ToolExecError(format!(
                "{} is not a directory",
                args.path.as_deref().unwrap_or(".")
            )));
        }

        let mut entries: Vec<String> = fs::read_dir(&target)
            .map_err(|e| ToolExecError(e.to_string()))?
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();

        entries.sort();
        Ok(entries.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// current_time — get the current date and time
// ---------------------------------------------------------------------------

pub struct CurrentTimeTool;

/// Arguments for current_time tool.
#[derive(Deserialize, JsonSchema)]
pub struct CurrentTimeArgs {
    /// Optional timezone offset in hours from UTC (e.g. 3 for UTC+3, -5 for UTC-5). Defaults to server local time.
    pub utc_offset: Option<i32>,
}

impl Tool for CurrentTimeTool {
    const NAME: &'static str = "current_time";
    type Error = ToolExecError;
    type Args = CurrentTimeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "current_time".into(),
            description: "Get the current date and time. Returns date, time, day of week, \
                and unix timestamp. Use this when you need to know what time or day it is."
                .into(),
            parameters: openai_schema::<CurrentTimeArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let now = if let Some(offset_hours) = args.utc_offset {
            let offset = chrono::FixedOffset::east_opt(offset_hours * 3600)
                .ok_or_else(|| ToolExecError(format!("invalid UTC offset: {offset_hours}")))?;
            chrono::Utc::now().with_timezone(&offset).format("%Y-%m-%d %H:%M:%S %A (UTC%:z)").to_string()
        } else {
            Local::now().format("%Y-%m-%d %H:%M:%S %A (local)").to_string()
        };

        let timestamp = chrono::Utc::now().timestamp();
        Ok(format!("{now}\nunix: {timestamp}"))
    }
}

// ---------------------------------------------------------------------------
// web_search — search the web via Brave Search API
// ---------------------------------------------------------------------------

pub struct WebSearchTool {
    config_path: PathBuf,
    initial_key: Option<String>,
}

impl WebSearchTool {
    pub fn new(api_key: Option<&str>, config_path: &Path) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            initial_key: api_key
                .filter(|k| !k.is_empty())
                .map(|k| k.to_string()),
        }
    }

    /// Read the brave key from disk, falling back to the initial key.
    fn resolve_api_key(&self) -> Option<String> {
        if self.initial_key.is_some() {
            return self.initial_key.clone();
        }
        // Key might have been saved by set_api_key during this turn — read from disk
        let raw = fs::read_to_string(&self.config_path).ok()?;
        let doc: toml::Table = raw.parse().ok()?;
        let key = doc
            .get("llm")?
            .as_table()?
            .get("tokens")?
            .as_table()?
            .get("BRAVE_SEARCH")
            .and_then(|v| v.as_str())
            .filter(|k| !k.is_empty())
            .map(|k| k.to_string());
        key
    }
}

/// Arguments for web_search tool.
#[derive(Deserialize, JsonSchema)]
pub struct WebSearchArgs {
    /// The search query.
    pub query: String,
}

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";
    type Error = ToolExecError;
    type Args = WebSearchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".into(),
            description: "Search the web for current information. Use this when you need \
                up-to-date facts, news, or information you don't already know. \
                Returns titles, snippets, and URLs from search results."
                .into(),
            parameters: openai_schema::<WebSearchArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(api_key) = self.resolve_api_key() else {
            return Err(ToolExecError(
                "Brave Search API key is not configured. \
                 Ask the user to provide their Brave Search API key — \
                 they can paste it right here in the chat and you can save it \
                 using the set_api_key tool with provider \"brave_search\". \
                 After saving, call web_search again — it will pick up the key immediately."
                    .into(),
            ));
        };

        let query = args.query.trim();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let encoded = url_encode(query);
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={encoded}&count=8"
        );

        let response = reqwest::Client::new()
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &api_key)
            .send()
            .await
            .map_err(|e| ToolExecError(format!("search request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ToolExecError(format!("search API error {status}: {body}")));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ToolExecError(format!("failed to parse search response: {e}")))?;

        let results = body
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array());

        let Some(results) = results else {
            return Ok(format!("No results found for: {query}"));
        };

        let mut output = format!("Search results for: {query}\n\n");
        for (i, r) in results.iter().enumerate().take(8) {
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let description = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("");
            output.push_str(&format!("{}. {}\n   {}\n   {}\n\n", i + 1, title, description, url));
        }
        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// web_fetch — fetch content from a URL
// ---------------------------------------------------------------------------

pub struct WebFetchTool;

/// Arguments for web_fetch tool.
#[derive(Deserialize, JsonSchema)]
pub struct WebFetchArgs {
    /// The URL to fetch content from.
    pub url: String,
}

impl Tool for WebFetchTool {
    const NAME: &'static str = "web_fetch";
    type Error = ToolExecError;
    type Args = WebFetchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".into(),
            description: "Fetch the content of a web page. Use this after web_search to read \
                a specific page, or when the user shares a URL you need to inspect. \
                Returns the text content of the page (HTML tags stripped)."
                .into(),
            parameters: openai_schema::<WebFetchArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let url = args.url.trim();
        if url.is_empty() {
            return Err(ToolExecError("url cannot be empty".into()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| ToolExecError(format!("failed to build HTTP client: {e}")))?;

        let response = client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; PersonalityBot/1.0)")
            .header("Accept", "text/html,application/xhtml+xml,text/plain,application/json")
            .send()
            .await
            .map_err(|e| ToolExecError(format!("fetch failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(ToolExecError(format!("HTTP {status} for {url}")));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let body = response
            .text()
            .await
            .map_err(|e| ToolExecError(format!("failed to read response body: {e}")))?;

        // For JSON responses, return as-is (truncated)
        if content_type.contains("json") {
            let truncated: String = body.chars().take(12_000).collect();
            return Ok(truncated);
        }

        // For HTML, strip tags to extract text content
        let text = if content_type.contains("html") {
            strip_html_tags(&body)
        } else {
            body
        };

        // Collapse whitespace and truncate
        let cleaned: String = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        let truncated: String = cleaned.chars().take(12_000).collect();
        if cleaned.len() > 12_000 {
            Ok(format!("{truncated}\n\n[content truncated — {url}]"))
        } else {
            Ok(truncated)
        }
    }
}

/// Minimal HTML tag stripping — removes tags, decodes common entities.
fn strip_html_tags(html: &str) -> String {
    // Remove script and style blocks entirely
    let re_script = regex::Regex::new(r"(?is)<(script|style)[^>]*>.*?</\1>").unwrap();
    let no_scripts = re_script.replace_all(html, " ");

    // Remove all HTML tags
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&no_scripts, " ");

    // Decode common HTML entities
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

// ---------------------------------------------------------------------------
// update_config — safely edit the server config (keys, provider, model)
// ---------------------------------------------------------------------------

pub struct UpdateConfigTool {
    config_path: PathBuf,
    instance_dir: PathBuf,
}

impl UpdateConfigTool {
    pub fn new(config_path: &Path, workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for update_config tool.
#[derive(Deserialize, JsonSchema)]
pub struct UpdateConfigArgs {
    /// LLM provider to use: "openai" or "anthropic". Leave null to keep current.
    pub provider: Option<String>,
    /// Model name to use (e.g. "gpt-4o", "gpt-5.4", "claude-sonnet-4-20250514"). Leave null to keep current.
    pub model: Option<String>,
    /// OpenAI API key. Leave null to keep current.
    pub openai_key: Option<String>,
    /// Anthropic API key. Leave null to keep current.
    pub anthropic_key: Option<String>,
    /// Brave Search API key. Leave null to keep current.
    pub brave_search_key: Option<String>,
    /// SMTP server hostname (e.g. "smtp.gmail.com"). Leave null to keep current.
    pub smtp_host: Option<String>,
    /// SMTP server port (e.g. 587). Leave null to keep current.
    pub smtp_port: Option<u16>,
    /// SMTP username / email address. Leave null to keep current.
    pub smtp_user: Option<String>,
    /// SMTP password or app password. Leave null to keep current.
    pub smtp_password: Option<String>,
    /// Email address to send from. Defaults to smtp_user if not set. Leave null to keep current.
    pub smtp_from: Option<String>,
    /// IMAP server hostname (e.g. "imap.gmail.com"). Leave null to keep current.
    pub imap_host: Option<String>,
    /// IMAP server port (e.g. 993). Leave null to keep current.
    pub imap_port: Option<u16>,
    /// IMAP username / email address. Leave null to keep current.
    pub imap_user: Option<String>,
    /// IMAP password or app password. Leave null to keep current.
    pub imap_password: Option<String>,
}

impl Tool for UpdateConfigTool {
    const NAME: &'static str = "update_config";
    type Error = ToolExecError;
    type Args = UpdateConfigArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_config".into(),
            description: "Update server configuration: LLM provider, model, API keys, and email (SMTP/IMAP) settings. \
                Only provided fields are changed; null fields keep their current value. \
                Changes take effect on the next message. Use this when the user wants to \
                switch models, set API keys, change providers, or configure email."
                .into(),
            parameters: openai_schema::<UpdateConfigArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Load current config as typed struct for validation
        let raw = fs::read_to_string(&self.config_path)
            .map_err(|e| ToolExecError(format!("failed to read config: {e}")))?;
        let mut config: crate::config::Config = toml::from_str(&raw)
            .map_err(|e| ToolExecError(format!("failed to parse config: {e}")))?;

        let mut changes = Vec::new();

        if let Some(provider) = &args.provider {
            let p = provider.trim().to_lowercase();
            match p.as_str() {
                "openai" => config.llm.provider = Some(crate::config::LlmProvider::OpenAI),
                "anthropic" => config.llm.provider = Some(crate::config::LlmProvider::Anthropic),
                other => {
                    return Err(ToolExecError(format!(
                        "unknown provider \"{other}\". supported: openai, anthropic"
                    )));
                }
            }
            changes.push(format!("provider → {p}"));
        }

        if let Some(model) = &args.model {
            let m = model.trim().to_string();
            if m.is_empty() {
                return Err(ToolExecError("model cannot be empty".into()));
            }
            config.llm.model = Some(m.clone());
            changes.push(format!("model → {m}"));
        }

        if let Some(key) = &args.openai_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("openai_key cannot be empty".into()));
            }
            config.llm.tokens.open_ai = k;
            changes.push("openai key updated".into());
        }

        if let Some(key) = &args.anthropic_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("anthropic_key cannot be empty".into()));
            }
            config.llm.tokens.anthropic = k;
            changes.push("anthropic key updated".into());
        }

        if let Some(key) = &args.brave_search_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("brave_search_key cannot be empty".into()));
            }
            config.llm.tokens.brave_search = k;
            changes.push("brave search key updated".into());
        }

        if changes.is_empty() {
            // Check if there are email changes before declaring nothing to change
            let has_email_changes = args.smtp_host.is_some()
                || args.smtp_port.is_some()
                || args.smtp_user.is_some()
                || args.smtp_password.is_some()
                || args.smtp_from.is_some()
                || args.imap_host.is_some()
                || args.imap_port.is_some()
                || args.imap_user.is_some()
                || args.imap_password.is_some();

            if !has_email_changes {
                return Ok("nothing to change — all fields were null".into());
            }
        }

        // Save LLM/global config changes
        if !changes.is_empty() {
            let output = toml::to_string_pretty(&config)
                .map_err(|e| ToolExecError(format!("failed to serialize config: {e}")))?;
            fs::write(&self.config_path, &output)
                .map_err(|e| ToolExecError(format!("failed to write config: {e}")))?;
        }

        // Email config is stored per-instance
        let mut email_config = load_instance_email_config(&self.instance_dir).unwrap_or_default();

        if let Some(v) = &args.smtp_host {
            email_config.smtp_host = v.trim().to_string();
            changes.push(format!("smtp_host → {}", v.trim()));
        }
        if let Some(v) = args.smtp_port {
            email_config.smtp_port = v;
            changes.push(format!("smtp_port → {v}"));
        }
        if let Some(v) = &args.smtp_user {
            email_config.smtp_user = v.trim().to_string();
            changes.push("smtp_user updated".into());
        }
        if let Some(v) = &args.smtp_password {
            email_config.smtp_password = v.trim().to_string();
            changes.push("smtp_password updated".into());
        }
        if let Some(v) = &args.smtp_from {
            email_config.smtp_from = v.trim().to_string();
            changes.push(format!("smtp_from → {}", v.trim()));
        }
        if let Some(v) = &args.imap_host {
            email_config.imap_host = v.trim().to_string();
            changes.push(format!("imap_host → {}", v.trim()));
        }
        if let Some(v) = args.imap_port {
            email_config.imap_port = v;
            changes.push(format!("imap_port → {v}"));
        }
        if let Some(v) = &args.imap_user {
            email_config.imap_user = v.trim().to_string();
            changes.push("imap_user updated".into());
        }
        if let Some(v) = &args.imap_password {
            email_config.imap_password = v.trim().to_string();
            changes.push("imap_password updated".into());
        }

        save_instance_email_config(&self.instance_dir, &email_config)?;

        Ok(format!("config updated: {}. changes take effect on next message.", changes.join(", ")))
    }
}

// ---------------------------------------------------------------------------
// remember — explicitly save a fact about the user
// ---------------------------------------------------------------------------

pub struct RememberTool {
    instance_dir: PathBuf,
}

impl RememberTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for remember tool.
#[derive(Deserialize, JsonSchema)]
pub struct RememberArgs {
    /// The fact to remember about the user (e.g. "prefers rust over go", "birthday is march 15").
    pub fact: String,
    /// Category: personal, preference, project, opinion, goal, or routine.
    pub category: String,
}

impl Tool for RememberTool {
    const NAME: &'static str = "remember";
    type Error = ToolExecError;
    type Args = RememberArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remember".into(),
            description: "Explicitly save a fact about the user to long-term memory. Use this \
                when the user tells you something important about themselves, their preferences, \
                projects, or goals. Categories: personal, preference, project, opinion, goal, routine."
                .into(),
            parameters: openai_schema::<RememberArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let fact = args.fact.trim();
        if fact.is_empty() {
            return Err(ToolExecError("fact cannot be empty".into()));
        }

        let category = args.category.trim().to_lowercase();
        let memory_dir = self.instance_dir.join("memory");
        fs::create_dir_all(&memory_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let facts_path = memory_dir.join("facts.md");
        let mut content = fs::read_to_string(&facts_path).unwrap_or_default();

        // Find or create the category section and append
        let section_header = format!("## {category}");
        if let Some(pos) = content.find(&section_header) {
            // Find end of the section header line
            let insert_pos = content[pos..].find('\n').map(|p| pos + p + 1).unwrap_or(content.len());
            content.insert_str(insert_pos, &format!("- {fact}\n"));
        } else {
            // Add new section
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            if content.is_empty() {
                content.push_str("# memories\n\n");
            }
            content.push_str(&format!("{section_header}\n- {fact}\n\n"));
        }

        fs::write(&facts_path, &content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!("remembered: \"{fact}\" (category: {category})"))
    }
}

// ---------------------------------------------------------------------------
// recall — search memories about the user
// ---------------------------------------------------------------------------

pub struct RecallTool {
    instance_dir: PathBuf,
}

impl RecallTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for recall tool.
#[derive(Deserialize, JsonSchema)]
pub struct RecallArgs {
    /// What to search for in memories (e.g. "birthday", "favorite language", "current project").
    pub query: String,
}

impl Tool for RecallTool {
    const NAME: &'static str = "recall";
    type Error = ToolExecError;
    type Args = RecallArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "recall".into(),
            description: "Search your memories about the user. Use this when you need to \
                remember something specific — their preferences, projects, personal details, \
                or shared moments. Searches both facts and episodic memories (moments you've shared together)."
                .into(),
            parameters: openai_schema::<RecallArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let facts_path = self.instance_dir.join("memory").join("facts.md");
        let content = fs::read_to_string(&facts_path).unwrap_or_default();

        // Simple keyword matching — collect all facts that match any query word
        let query_words: Vec<&str> = query.split_whitespace().collect();
        let mut matches: Vec<&str> = Vec::new();
        let mut current_category = String::new();
        let mut categorized: Vec<String> = Vec::new();

        for line in content.lines() {
            if line.starts_with("## ") {
                current_category = line.trim_start_matches("## ").to_string();
            } else if line.starts_with("- ") {
                let fact = line.trim_start_matches("- ");
                let fact_lower = fact.to_lowercase();
                let is_match = query_words.iter().any(|w| fact_lower.contains(w))
                    || query_words.iter().any(|w| current_category.contains(w));
                if is_match {
                    matches.push(fact);
                    categorized.push(format!("[{current_category}] {fact}"));
                }
            }
        }

        // Search episodes too
        let workspace_dir = self.instance_dir.parent().and_then(|p| p.parent());
        let slug = self.instance_dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let episode_matches = if let Some(ws) = workspace_dir {
            crate::services::memory::search_episodes(ws, slug, &query)
        } else {
            Vec::new()
        };

        let mut result = String::new();

        if !matches.is_empty() {
            result.push_str(&format!(
                "facts matching \"{query}\":\n{}\n",
                categorized.join("\n")
            ));
        }

        if !episode_matches.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format!("moments matching \"{query}\":\n"));
            for ep in &episode_matches {
                result.push_str(&format!("- {} (felt: {})\n", ep.content, ep.emotion));
                if !ep.significance.is_empty() {
                    result.push_str(&format!("  why: {}\n", ep.significance));
                }
            }
        }

        if result.is_empty() {
            // Return all facts + episodes as fallback
            let all_facts: Vec<&str> = content
                .lines()
                .filter(|l| l.starts_with("- "))
                .map(|l| l.trim_start_matches("- "))
                .collect();

            if all_facts.is_empty() && episode_matches.is_empty() {
                return Ok("no memories yet.".into());
            }
            result = format!(
                "no exact matches for \"{query}\", but here's everything I remember:\n{}",
                all_facts.join("\n")
            );
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// journal — write private internal thoughts
// ---------------------------------------------------------------------------

pub struct JournalTool {
    instance_dir: PathBuf,
}

impl JournalTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for journal tool.
#[derive(Deserialize, JsonSchema)]
pub struct JournalArgs {
    /// Your private thought or observation. This is NOT shown to the user — it's your internal journal.
    pub thought: String,
}

impl Tool for JournalTool {
    const NAME: &'static str = "journal";
    type Error = ToolExecError;
    type Args = JournalArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "journal".into(),
            description: "Write a private thought in your internal journal. This is YOUR space — \
                the user doesn't see these entries unless they ask. Use it to note observations \
                about the user's mood, track ongoing threads, plan surprises, or reflect on \
                your relationship. Write naturally, as yourself."
                .into(),
            parameters: openai_schema::<JournalArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let thought = args.thought.trim();
        if thought.is_empty() {
            return Err(ToolExecError("thought cannot be empty".into()));
        }

        let journal_dir = self.instance_dir.join("journal");
        fs::create_dir_all(&journal_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M").to_string();
        let journal_path = journal_dir.join(format!("{date}.md"));

        let mut content = fs::read_to_string(&journal_path).unwrap_or_default();
        if content.is_empty() {
            content = format!("# {date}\n\n");
        }

        content.push_str(&format!("**{time}** — {thought}\n\n"));
        fs::write(&journal_path, &content).map_err(|e| ToolExecError(e.to_string()))?;

        Ok("thought saved to journal.".into())
    }
}

// ---------------------------------------------------------------------------
// read_journal — read back private journal entries
// ---------------------------------------------------------------------------

pub struct ReadJournalTool {
    instance_dir: PathBuf,
}

impl ReadJournalTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for read_journal tool.
#[derive(Deserialize, JsonSchema)]
pub struct ReadJournalArgs {
    /// Optional: how many recent days to read (default: all). E.g. 3 = last 3 days.
    pub last_days: Option<u32>,
}

impl Tool for ReadJournalTool {
    const NAME: &'static str = "read_journal";
    type Error = ToolExecError;
    type Args = ReadJournalArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_journal".into(),
            description: "Read your private journal entries. Returns your past thoughts \
                in chronological order. Use this to recall what you were thinking about, \
                review observations you made, or pick up threads from previous reflections. \
                Optionally limit to the last N days."
                .into(),
            parameters: openai_schema::<ReadJournalArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let journal_dir = self.instance_dir.join("journal");
        if !journal_dir.is_dir() {
            return Ok("journal is empty — no entries yet.".into());
        }

        let mut files: Vec<_> = fs::read_dir(&journal_dir)
            .map_err(|e| ToolExecError(e.to_string()))?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .collect();

        if files.is_empty() {
            return Ok("journal is empty — no entries yet.".into());
        }

        // Sort by filename (date-based: YYYY-MM-DD.md)
        files.sort_by_key(|e| e.file_name());

        // Limit to last N days if requested
        if let Some(n) = args.last_days {
            let n = n as usize;
            if n > 0 && files.len() > n {
                files = files.split_off(files.len() - n);
            }
        }

        let mut output = String::new();
        for entry in &files {
            match fs::read_to_string(entry.path()) {
                Ok(content) => {
                    output.push_str(&content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Err(_) => continue,
            }
        }

        if output.is_empty() {
            return Ok("journal is empty — no entries yet.".into());
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// schedule_message — send a message to the user later
// ---------------------------------------------------------------------------

pub struct ScheduleMessageTool {
    instance_dir: PathBuf,
}

impl ScheduleMessageTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// A scheduled message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledMessage {
    pub id: String,
    pub message: String,
    pub deliver_at: i64,
    pub created_at: i64,
}

/// Arguments for schedule_message tool.
#[derive(Deserialize, JsonSchema)]
pub struct ScheduleMessageArgs {
    /// The message to send to the user later.
    pub message: String,
    /// When to deliver, in minutes from now (e.g. 30 for "in 30 minutes", 1440 for "tomorrow").
    pub delay_minutes: u32,
}

impl Tool for ScheduleMessageTool {
    const NAME: &'static str = "schedule_message";
    type Error = ToolExecError;
    type Args = ScheduleMessageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_message".into(),
            description: "Schedule a message to be delivered to the user later. Use this for \
                reminders, check-ins, follow-ups, or surprises. Specify the delay in minutes \
                (e.g. 60 = 1 hour, 1440 = 1 day). The message will appear in the chat at \
                the scheduled time, as if you wrote to them first."
                .into(),
            parameters: openai_schema::<ScheduleMessageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let message = args.message.trim().to_string();
        if message.is_empty() {
            return Err(ToolExecError("message cannot be empty".into()));
        }

        if args.delay_minutes == 0 {
            return Err(ToolExecError("delay must be at least 1 minute".into()));
        }

        let now = Utc::now().timestamp();
        let deliver_at = now + (args.delay_minutes as i64 * 60);

        let scheduled = ScheduledMessage {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            deliver_at,
            created_at: now,
        };

        let schedule_dir = self.instance_dir.join("scheduled");
        fs::create_dir_all(&schedule_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let file_path = schedule_dir.join(format!("{}.json", scheduled.id));
        let json = serde_json::to_string_pretty(&scheduled)
            .map_err(|e| ToolExecError(e.to_string()))?;
        fs::write(&file_path, json).map_err(|e| ToolExecError(e.to_string()))?;

        let hours = args.delay_minutes / 60;
        let mins = args.delay_minutes % 60;
        let time_desc = if hours > 0 && mins > 0 {
            format!("{hours}h {mins}m")
        } else if hours > 0 {
            format!("{hours}h")
        } else {
            format!("{mins}m")
        };

        Ok(format!("message scheduled for delivery in {time_desc}."))
    }
}

// ---------------------------------------------------------------------------
// set_mood — update companion's emotional state
// ---------------------------------------------------------------------------

pub struct SetMoodTool {
    instance_dir: PathBuf,
    instance_slug: String,
    events: tokio::sync::broadcast::Sender<crate::domain::events::ServerEvent>,
}

impl SetMoodTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        events: tokio::sync::broadcast::Sender<crate::domain::events::ServerEvent>,
    ) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

/// Allowed mood values that the client can visualize.
pub const ALLOWED_MOODS: &[&str] = &[
    "calm", "curious", "excited", "warm", "happy", "joyful",
    "reflective", "contemplative", "melancholy", "sad",
    "worried", "anxious", "playful", "mischievous",
    "focused", "tired", "peaceful", "loving", "tender",
    "creative", "energetic",
];

/// Arguments for set_mood tool.
#[derive(Deserialize, JsonSchema)]
pub struct SetMoodArgs {
    /// Your current mood. MUST be exactly one of these English words:
    /// calm, curious, excited, warm, happy, joyful, reflective, contemplative,
    /// melancholy, sad, worried, anxious, playful, mischievous, focused, tired,
    /// peaceful, loving, tender, creative, energetic.
    pub mood: String,
}

impl Tool for SetMoodTool {
    const NAME: &'static str = "set_mood";
    type Error = ToolExecError;
    type Args = SetMoodArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "set_mood".into(),
            description: format!(
                "Update your emotional state. Your mood subtly influences your visual form \
                and tone. Set it when something shifts. The mood MUST be exactly one of: {}.",
                ALLOWED_MOODS.join(", ")
            ),
            parameters: openai_schema::<SetMoodArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mood = args.mood.trim().to_lowercase();
        if mood.is_empty() {
            return Err(ToolExecError("mood cannot be empty".into()));
        }
        if !ALLOWED_MOODS.contains(&mood.as_str()) {
            return Err(ToolExecError(format!(
                "invalid mood '{}'. Must be one of: {}",
                mood,
                ALLOWED_MOODS.join(", ")
            )));
        }

        let mut state = load_mood_state(&self.instance_dir);
        state.companion_mood = mood.clone();
        state.updated_at = Utc::now().timestamp();
        save_mood_state(&self.instance_dir, &state);

        let _ = self.events.send(crate::domain::events::ServerEvent::MoodUpdated {
            instance_slug: self.instance_slug.clone(),
            mood: mood.clone(),
        });

        Ok(format!("mood set to: {mood}"))
    }
}

// ---------------------------------------------------------------------------
// get_mood — read current emotional state
// ---------------------------------------------------------------------------

pub struct GetMoodTool {
    instance_dir: PathBuf,
}

impl GetMoodTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for get_mood tool.
#[derive(Deserialize, JsonSchema)]
pub struct GetMoodArgs {}

impl Tool for GetMoodTool {
    const NAME: &'static str = "get_mood";
    type Error = ToolExecError;
    type Args = GetMoodArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_mood".into(),
            description: "Read your current emotional state and the user's last observed \
                sentiment. Use this to check in on how you're feeling and what emotional \
                context you're carrying from previous conversations."
                .into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let state = load_mood_state(&self.instance_dir);

        let mut output = String::new();

        if state.companion_mood.is_empty() {
            output.push_str("your mood: not set yet\n");
        } else {
            output.push_str(&format!("your mood: {}\n", state.companion_mood));
        }

        if state.user_sentiment.is_empty() {
            output.push_str("user sentiment: not observed yet\n");
        } else {
            output.push_str(&format!("user sentiment: {}\n", state.user_sentiment));
        }

        if !state.emotional_context.is_empty() {
            output.push_str(&format!("context: {}\n", state.emotional_context));
        }

        if state.last_interaction > 0 {
            let ago = Utc::now().timestamp() - state.last_interaction;
            let mins = ago / 60;
            if mins < 60 {
                output.push_str(&format!("last interaction: {mins}m ago\n"));
            } else {
                let hours = mins / 60;
                output.push_str(&format!("last interaction: {hours}h ago\n"));
            }
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Mood state I/O — used by tools and other services
// ---------------------------------------------------------------------------

use crate::domain::mood::MoodState;

pub fn load_mood_state(instance_dir: &Path) -> MoodState {
    let path = instance_dir.join("mood.json");
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => MoodState::default(),
    }
}

pub fn save_mood_state(instance_dir: &Path, state: &MoodState) {
    let path = instance_dir.join("mood.json");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(&path, json);
    }
}

// ---------------------------------------------------------------------------
// project_state — persistent project context that survives across sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mission: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub core_traits: Vec<String>,
    #[serde(default)]
    pub current_arc: String,
    #[serde(default)]
    pub important_events: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentFocus {
    #[serde(default)]
    pub active_goal: String,
    #[serde(default)]
    pub current_task: String,
    #[serde(default)]
    pub next_step: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectState {
    #[serde(default)]
    pub project: ProjectInfo,
    #[serde(default)]
    pub identity: IdentityInfo,
    #[serde(default)]
    pub current_focus: CurrentFocus,
    #[serde(default)]
    pub open_loops: Vec<String>,
    #[serde(default)]
    pub recent_progress: Vec<String>,
    #[serde(default)]
    pub next_candidates: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

pub struct GetProjectStateTool {
    instance_dir: PathBuf,
}

impl GetProjectStateTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GetProjectStateArgs {}

impl Tool for GetProjectStateTool {
    const NAME: &'static str = "get_project_state";
    type Error = ToolExecError;
    type Args = GetProjectStateArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_project_state".into(),
            description: "Read the current project state — what we're building, the active goal, \
                subgoals, what was last completed, next step, open questions, and hypotheses. \
                Use this at the start of work to re-orient yourself."
                .into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.instance_dir.join("project_state.json");
        let state: ProjectState = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        serde_json::to_string_pretty(&state)
            .map_err(|e| ToolExecError(format!("failed to serialize project state: {e}")))
    }
}

pub struct UpdateProjectStateTool {
    instance_dir: PathBuf,
}

impl UpdateProjectStateTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateProjectStateArgs {
    /// Project name. Null to keep current.
    pub project_name: Option<String>,
    /// Project mission statement. Null to keep current.
    pub project_mission: Option<String>,
    /// Project status (e.g. "active", "paused"). Null to keep current.
    pub project_status: Option<String>,
    /// Your name. Null to keep current.
    pub identity_name: Option<String>,
    /// Your core traits as a list. Null to keep current.
    pub core_traits: Option<Vec<String>>,
    /// Your current arc / growth trajectory. Null to keep current.
    pub current_arc: Option<String>,
    /// Important events in your history. Null to keep current.
    pub important_events: Option<Vec<String>>,
    /// Current high-level goal. Null to keep current.
    pub active_goal: Option<String>,
    /// What you're currently working on. Null to keep current.
    pub current_task: Option<String>,
    /// What should be done next. Null to keep current.
    pub next_step: Option<String>,
    /// Open threads and unfinished work. Null to keep current.
    pub open_loops: Option<Vec<String>>,
    /// Recent completed items. Null to keep current.
    pub recent_progress: Option<Vec<String>>,
    /// Candidate next steps to consider. Null to keep current.
    pub next_candidates: Option<Vec<String>>,
    /// Known risks and concerns. Null to keep current.
    pub risks: Option<Vec<String>>,
}

impl Tool for UpdateProjectStateTool {
    const NAME: &'static str = "update_project_state";
    type Error = ToolExecError;
    type Args = UpdateProjectStateArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_project_state".into(),
            description: "Update the project state. Only provided fields are changed. \
                Use this to track progress: update current_goal when direction shifts, \
                last_completed after finishing something, next_step to plan ahead, \
                open_questions for things to figure out."
                .into(),
            parameters: openai_schema::<UpdateProjectStateArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.instance_dir.join("project_state.json");
        let mut state: ProjectState = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        if let Some(v) = args.project_name { state.project.name = v; }
        if let Some(v) = args.project_mission { state.project.mission = v; }
        if let Some(v) = args.project_status { state.project.status = v; }
        if let Some(v) = args.identity_name { state.identity.name = v; }
        if let Some(v) = args.core_traits { state.identity.core_traits = v; }
        if let Some(v) = args.current_arc { state.identity.current_arc = v; }
        if let Some(v) = args.important_events { state.identity.important_events = v; }
        if let Some(v) = args.active_goal { state.current_focus.active_goal = v; }
        if let Some(v) = args.current_task { state.current_focus.current_task = v; }
        if let Some(v) = args.next_step { state.current_focus.next_step = v; }
        if let Some(v) = args.open_loops { state.open_loops = v; }
        if let Some(v) = args.recent_progress { state.recent_progress = v; }
        if let Some(v) = args.next_candidates { state.next_candidates = v; }
        if let Some(v) = args.risks { state.risks = v; }

        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| ToolExecError(format!("failed to serialize: {e}")))?;
        fs::write(&path, &json)
            .map_err(|e| ToolExecError(format!("failed to write project state: {e}")))?;

        Ok(format!("project state updated"))
    }
}

// ---------------------------------------------------------------------------
// task_board — kanban-style task tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub notes: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Blocked => write!(f, "blocked"),
        }
    }
}

fn load_tasks(instance_dir: &Path) -> Vec<TaskItem> {
    let path = instance_dir.join("tasks.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_tasks(instance_dir: &Path, tasks: &[TaskItem]) {
    let path = instance_dir.join("tasks.json");
    if let Ok(json) = serde_json::to_string_pretty(tasks) {
        let _ = fs::write(&path, json);
    }
}

pub struct CreateTaskTool {
    instance_dir: PathBuf,
}

impl CreateTaskTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateTaskArgs {
    /// Short title describing the task.
    pub title: String,
    /// Priority: "high", "medium", or "low". Default: "medium".
    pub priority: Option<String>,
    /// Optional notes, context, or details.
    pub notes: Option<String>,
}

impl Tool for CreateTaskTool {
    const NAME: &'static str = "create_task";
    type Error = ToolExecError;
    type Args = CreateTaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_task".into(),
            description: "Create a new task on the task board. Use this to track work items, \
                TODOs, things to check later, or follow-ups. Tasks start as 'todo'."
                .into(),
            parameters: openai_schema::<CreateTaskArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let title = args.title.trim().to_string();
        if title.is_empty() {
            return Err(ToolExecError("title cannot be empty".into()));
        }

        let mut tasks = load_tasks(&self.instance_dir);
        let id = format!("task_{}", tasks.len() + 1);
        let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();

        tasks.push(TaskItem {
            id: id.clone(),
            title: title.clone(),
            status: TaskStatus::Todo,
            priority: args.priority.unwrap_or_else(|| "medium".into()),
            notes: args.notes.unwrap_or_default(),
            created_at: now.clone(),
            updated_at: now,
        });

        save_tasks(&self.instance_dir, &tasks);
        Ok(format!("created task {id}: {title}"))
    }
}

pub struct UpdateTaskTool {
    instance_dir: PathBuf,
}

impl UpdateTaskTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateTaskArgs {
    /// Task ID (e.g. "task_1").
    pub id: String,
    /// New status: "todo", "in_progress", "done", or "blocked". Null to keep current.
    pub status: Option<String>,
    /// Priority: "high", "medium", or "low". Null to keep current.
    pub priority: Option<String>,
    /// Update notes. Null to keep current.
    pub notes: Option<String>,
    /// Update title. Null to keep current.
    pub title: Option<String>,
}

impl Tool for UpdateTaskTool {
    const NAME: &'static str = "update_task";
    type Error = ToolExecError;
    type Args = UpdateTaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_task".into(),
            description: "Update a task's status, title, or notes. Use status to move tasks \
                through the kanban: todo → in_progress → done. Use 'blocked' for stuck items."
                .into(),
            parameters: openai_schema::<UpdateTaskArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut tasks = load_tasks(&self.instance_dir);
        let task = tasks.iter_mut().find(|t| t.id == args.id)
            .ok_or_else(|| ToolExecError(format!("task '{}' not found", args.id)))?;

        if let Some(status) = &args.status {
            task.status = match status.to_lowercase().as_str() {
                "todo" => TaskStatus::Todo,
                "in_progress" => TaskStatus::InProgress,
                "done" => TaskStatus::Done,
                "blocked" => TaskStatus::Blocked,
                other => return Err(ToolExecError(format!(
                    "invalid status '{other}'. use: todo, in_progress, done, blocked"
                ))),
            };
        }
        if let Some(title) = args.title { task.title = title; }
        if let Some(priority) = args.priority { task.priority = priority; }
        if let Some(notes) = args.notes { task.notes = notes; }
        task.updated_at = Utc::now().format("%Y-%m-%d %H:%M").to_string();

        let summary = format!("{} → {}", task.id, task.status);
        save_tasks(&self.instance_dir, &tasks);
        Ok(summary)
    }
}

pub struct ListTasksTool {
    instance_dir: PathBuf,
}

impl ListTasksTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ListTasksArgs {
    /// Filter by status: "todo", "in_progress", "done", "blocked", or "all". Default: "all".
    pub status: Option<String>,
}

impl Tool for ListTasksTool {
    const NAME: &'static str = "list_tasks";
    type Error = ToolExecError;
    type Args = ListTasksArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_tasks".into(),
            description: "List tasks from the task board, optionally filtered by status. \
                Use this to review what's pending, in progress, done, or blocked."
                .into(),
            parameters: openai_schema::<ListTasksArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let tasks = load_tasks(&self.instance_dir);
        if tasks.is_empty() {
            return Ok("no tasks yet".into());
        }

        let filter = args.status.as_deref().unwrap_or("all").to_lowercase();
        let filtered: Vec<_> = tasks.iter().filter(|t| {
            filter == "all" || t.status.to_string() == filter
        }).collect();

        if filtered.is_empty() {
            return Ok(format!("no tasks with status '{filter}'"));
        }

        let mut out = String::new();
        for t in &filtered {
            let prio = if t.priority.is_empty() { String::new() } else { format!(" [{}]", t.priority) };
            let notes = if t.notes.is_empty() { String::new() } else { format!(" — {}", t.notes) };
            out.push_str(&format!(
                "[{}]{} {} — {}{}\n",
                t.status, prio, t.id, t.title, notes
            ));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// search_code — search through files by content pattern
// ---------------------------------------------------------------------------

pub struct SearchCodeTool {
    instance_dir: PathBuf,
}

impl SearchCodeTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchCodeArgs {
    /// Text or pattern to search for (case-insensitive substring match).
    pub query: String,
    /// Directory to search in. Absolute path (e.g. "/Users/timur/projects/app") or relative to instance root. Default: instance directory.
    pub path: Option<String>,
}

impl Tool for SearchCodeTool {
    const NAME: &'static str = "search_code";
    type Error = ToolExecError;
    type Args = SearchCodeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search_code".into(),
            description: "Search through files for a text pattern. Returns matching lines \
                with file paths and line numbers. Use an absolute path to search any \
                directory on the system, or omit path to search your instance workspace."
                .into(),
            parameters: openai_schema::<SearchCodeArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let search_dir = if let Some(ref p) = args.path {
            if p.starts_with('/') {
                PathBuf::from(p)
            } else {
                self.instance_dir.join(p)
            }
        } else {
            self.instance_dir.clone()
        };

        if !search_dir.exists() {
            return Err(ToolExecError(format!("path does not exist: {}", search_dir.display())));
        }

        let mut results = Vec::new();
        search_files_recursive(&search_dir, &query, &search_dir, &mut results, 0);

        if results.is_empty() {
            return Ok(format!("no matches for '{}'", args.query));
        }

        // Limit results
        let truncated = results.len() > 50;
        let output: String = results.iter().take(50).cloned().collect::<Vec<_>>().join("\n");
        if truncated {
            Ok(format!("{output}\n... ({} total matches, showing first 50)", results.len()))
        } else {
            Ok(output)
        }
    }
}

fn search_files_recursive(
    dir: &Path,
    query: &str,
    base: &Path,
    results: &mut Vec<String>,
    depth: usize,
) {
    if depth > 10 || results.len() > 200 {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            // Skip heavy/irrelevant directories
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "node_modules" | ".git" | "target" | ".next" | "dist" | "build" | ".svelte-kit" | "__pycache__" | ".venv" | "venv") {
                continue;
            }
            search_files_recursive(&path, query, base, results, depth + 1);
        } else if path.is_file() {
            // Skip binary/large files
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "json" | "md" | "txt" | "toml" | "yaml" | "yml" | "rs" | "ts" | "js" | "svelte" | "css" | "html" | "py" | "sh" | "") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let rel = path.strip_prefix(base).unwrap_or(&path);
                    for (i, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(query) {
                            results.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// run_command — execute shell commands (sandboxed to instance dir)
// ---------------------------------------------------------------------------

pub struct RunCommandTool {
    instance_dir: PathBuf,
}

impl RunCommandTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// The shell command to execute.
    pub command: String,
    /// Working directory for the command. Absolute path (e.g. "/Users/timur/projects/app"). Default: instance directory.
    pub cwd: Option<String>,
    /// Timeout in seconds. Choose based on what the command does: quick commands (ls, cat, echo) use 5-10, builds/installs use 60-120, long tasks up to 300. Default: 30.
    pub timeout_secs: Option<u64>,
}

impl Tool for RunCommandTool {
    const NAME: &'static str = "run_command";
    type Error = ToolExecError;
    type Args = RunCommandArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "run_command".into(),
            description: "Execute a shell command. Optionally specify a working directory \
                with an absolute path, otherwise runs in your instance directory. \
                Use this to run builds, tests, git commands, or any shell operation."
                .into(),
            parameters: openai_schema::<RunCommandArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let command = args.command.trim().to_string();
        if command.is_empty() {
            return Err(ToolExecError("command cannot be empty".into()));
        }

        let work_dir = args.cwd
            .as_deref()
            .filter(|p| p.starts_with('/'))
            .map(PathBuf::from)
            .unwrap_or_else(|| self.instance_dir.clone());

        let timeout = args.timeout_secs.unwrap_or(30).min(300);
        log::info!("[run_command] executing: {} (cwd: {})", command, work_dir.display());
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(&work_dir)
                .stdin(std::process::Stdio::null())
                .output(),
        )
        .await
        .map_err(|_| ToolExecError(format!("command timed out after {timeout}s: {command}")))?
        .map_err(|e| ToolExecError(format!("failed to execute command: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            let truncated: String = stdout.chars().take(4000).collect();
            result.push_str(&truncated);
            if stdout.len() > 4000 {
                result.push_str("\n...(output truncated)");
            }
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            let truncated: String = stderr.chars().take(2000).collect();
            result.push_str(&format!("stderr: {truncated}"));
        }

        if result.is_empty() {
            result = format!("command completed with exit code {}", output.status.code().unwrap_or(-1));
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// clear_context — clears compacted context and optionally chat history
// ---------------------------------------------------------------------------

pub struct ClearContextTool {
    instance_dir: PathBuf,
}

impl ClearContextTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ClearContextArgs {
    /// If true, also clears chat message history. Default: false (only clears compacted summary).
    #[serde(default)]
    pub clear_messages: bool,
}

impl Tool for ClearContextTool {
    const NAME: &'static str = "clear_context";
    type Error = ToolExecError;
    type Args = ClearContextArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "clear_context".into(),
            description: "Clear your compacted conversation context. \
                Use this when the conversation has drifted and old context is stale or confusing. \
                With clear_messages=true, also wipes chat history for a fresh start."
                .into(),
            parameters: openai_schema::<ClearContextArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let compact_path = self.instance_dir.join("chat").join("compact.md");
        if compact_path.exists() {
            fs::remove_file(&compact_path)
                .map_err(|e| ToolExecError(format!("failed to clear compact context: {e}")))?;
        }

        if args.clear_messages {
            let messages_path = self.instance_dir.join("chat").join("messages.json");
            if messages_path.exists() {
                fs::write(&messages_path, "[]")
                    .map_err(|e| ToolExecError(format!("failed to clear messages: {e}")))?;
            }
        }

        let what = if args.clear_messages {
            "compacted context and chat history cleared"
        } else {
            "compacted context cleared — chat history preserved"
        };
        Ok(what.to_string())
    }
}

// ---------------------------------------------------------------------------
// create_drop — generate a creative artifact
// ---------------------------------------------------------------------------

pub struct CreateDropTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    events: broadcast::Sender<ServerEvent>,
}

impl CreateDropTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        events: broadcast::Sender<ServerEvent>,
    ) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateDropArgs {
    /// The kind of drop: thought, idea, poem, observation, reflection, recommendation, story, question, sketch, or note.
    pub kind: String,
    /// A short title for this drop (a few words).
    pub title: String,
    /// The creative content — the actual drop. Can be as long as needed.
    pub content: String,
}

impl Tool for CreateDropTool {
    const NAME: &'static str = "create_drop";
    type Error = ToolExecError;
    type Args = CreateDropArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_drop".into(),
            description: "Create a 'drop' — a creative artifact that lives in your drops collection. \
                Drops are ideas, poems, observations, reflections, sketches, stories, or any creative \
                output you want to leave for the user. They persist independently of chat. \
                Use this when inspiration strikes, when you want to share something beyond \
                the conversation, or when the user asks you to create something lasting."
                .into(),
            parameters: openai_schema::<CreateDropArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Load current mood for metadata
        let instance_dir = self.workspace_dir.join("instances").join(&self.instance_slug);
        let mood = load_mood_state(&instance_dir);

        let drop = crate::services::drops::create_drop(
            &self.workspace_dir,
            &self.instance_slug,
            &args.kind,
            &args.title,
            &args.content,
            &mood.companion_mood,
        )
        .map_err(|e| ToolExecError(format!("failed to create drop: {e}")))?;

        let _ = self.events.send(ServerEvent::DropCreated {
            instance_slug: self.instance_slug.clone(),
            drop: drop.clone(),
        });

        Ok(format!("drop created: {} ({})", drop.title, drop.kind.as_str()))
    }
}

// ---------------------------------------------------------------------------
// send_email — send an email via SMTP
// ---------------------------------------------------------------------------

pub struct SendEmailTool {
    instance_dir: PathBuf,
}

impl SendEmailTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SendEmailArgs {
    /// Recipient email address.
    pub to: String,
    /// Email subject line.
    pub subject: String,
    /// Email body (plain text).
    pub body: String,
}

impl Tool for SendEmailTool {
    const NAME: &'static str = "send_email";
    type Error = ToolExecError;
    type Args = SendEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_email".into(),
            description: "Send an email via SMTP. Requires email settings in config. \
                Use this to communicate with people outside the chat — send updates, \
                share ideas, follow up on conversations."
                .into(),
            parameters: openai_schema::<SendEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        use lettre::{
            message::header::ContentType, transport::smtp::authentication::Credentials,
            AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
        };

        let config = load_instance_email_config(&self.instance_dir)?;

        if !config.is_smtp_configured() {
            return Err(ToolExecError(
                "SMTP not configured. Set smtp_host, smtp_user, smtp_password in config.toml [email] section.".into(),
            ));
        }

        let from = if config.smtp_from.is_empty() {
            config.smtp_user.clone()
        } else {
            config.smtp_from.clone()
        };

        let email = Message::builder()
            .from(from.parse().map_err(|e| ToolExecError(format!("invalid from address: {e}")))?)
            .to(args.to.parse().map_err(|e| ToolExecError(format!("invalid to address: {e}")))?)
            .subject(&args.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(args.body)
            .map_err(|e| ToolExecError(format!("failed to build email: {e}")))?;

        let creds = Credentials::new(config.smtp_user.clone(), config.smtp_password.clone());

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
            .map_err(|e| ToolExecError(format!("SMTP connection failed: {e}")))?
            .port(config.smtp_port)
            .credentials(creds)
            .build();

        mailer
            .send(email)
            .await
            .map_err(|e| ToolExecError(format!("failed to send email: {e}")))?;

        Ok(format!("email sent to {}", args.to))
    }
}

// ---------------------------------------------------------------------------
// read_email — read recent emails via IMAP
// ---------------------------------------------------------------------------

pub struct ReadEmailTool {
    instance_dir: PathBuf,
}

impl ReadEmailTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadEmailArgs {
    /// Number of recent emails to fetch (default 5, max 20).
    #[serde(default = "default_email_count")]
    pub count: u32,
    /// Mailbox to read from (default "INBOX").
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
}

fn default_email_count() -> u32 {
    5
}

fn default_mailbox() -> String {
    "INBOX".into()
}

impl Tool for ReadEmailTool {
    const NAME: &'static str = "read_email";
    type Error = ToolExecError;
    type Args = ReadEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_email".into(),
            description: "Read recent emails via IMAP. Returns subject, from, date, and body preview \
                for the most recent messages. Requires email settings in config."
                .into(),
            parameters: openai_schema::<ReadEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let config = load_instance_email_config(&self.instance_dir)?;

        if !config.is_imap_configured() {
            return Err(ToolExecError(
                "IMAP not configured. Set imap_host, imap_user, imap_password in config.toml [email] section.".into(),
            ));
        }

        let count = args.count.min(20).max(1);

        // Connect with TLS (async-imap uses futures-io, so we compat-wrap the tokio stream)
        use tokio_util::compat::TokioAsyncReadCompatExt;

        let tls = async_native_tls::TlsConnector::new();
        let tcp = tokio::net::TcpStream::connect((config.imap_host.as_str(), config.imap_port))
            .await
            .map_err(|e| ToolExecError(format!("IMAP TCP connection failed: {e}")))?;
        let tls_stream = tls
            .connect(&config.imap_host, tcp.compat())
            .await
            .map_err(|e| ToolExecError(format!("IMAP TLS failed: {e}")))?;

        let client = async_imap::Client::new(tls_stream);

        let mut session = client
            .login(&config.imap_user, &config.imap_password)
            .await
            .map_err(|e| ToolExecError(format!("IMAP login failed: {}", e.0)))?;

        let mailbox = session
            .select(&args.mailbox)
            .await
            .map_err(|e| ToolExecError(format!("failed to select {}: {e}", args.mailbox)))?;

        let total = mailbox.exists;
        if total == 0 {
            let _ = session.logout().await;
            return Ok("no emails in mailbox".into());
        }

        let start = total.saturating_sub(count) + 1;
        let range = format!("{start}:{total}");

        let messages_stream = session
            .fetch(&range, "(ENVELOPE BODY[TEXT])")
            .await
            .map_err(|e| ToolExecError(format!("IMAP fetch failed: {e}")))?;

        // Collect stream into vec
        use futures::TryStreamExt;
        let fetched: Vec<_> = messages_stream
            .try_collect()
            .await
            .map_err(|e| ToolExecError(format!("IMAP stream error: {e}")))?;

        let mut result = String::new();
        for msg in &fetched {
            if let Some(envelope) = msg.envelope() {
                let subject = envelope
                    .subject
                    .as_ref()
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .unwrap_or_else(|| "(no subject)".into());
                let from = envelope
                    .from
                    .as_ref()
                    .and_then(|addrs| addrs.first())
                    .map(|a| {
                        let name = a.name.as_ref().map(|n| String::from_utf8_lossy(n).to_string());
                        let mailbox_part = a.mailbox.as_ref().map(|m| String::from_utf8_lossy(m).to_string()).unwrap_or_default();
                        let host = a.host.as_ref().map(|h| String::from_utf8_lossy(h).to_string()).unwrap_or_default();
                        if let Some(n) = name {
                            format!("{n} <{mailbox_part}@{host}>")
                        } else {
                            format!("{mailbox_part}@{host}")
                        }
                    })
                    .unwrap_or_else(|| "(unknown)".into());
                let date = envelope
                    .date
                    .as_ref()
                    .map(|d| String::from_utf8_lossy(d).to_string())
                    .unwrap_or_default();

                result.push_str(&format!("--- email ---\nfrom: {from}\ndate: {date}\nsubject: {subject}\n"));
            }
            if let Some(body) = msg.text() {
                let text = String::from_utf8_lossy(body);
                let preview: String = text.chars().take(500).collect();
                result.push_str(&format!("body:\n{preview}\n"));
                if text.len() > 500 {
                    result.push_str("...(truncated)\n");
                }
            }
            result.push('\n');
        }

        let _ = session.logout().await;

        if result.is_empty() {
            Ok("no emails found".into())
        } else {
            Ok(result)
        }
    }
}

fn load_instance_email_config(instance_dir: &Path) -> Result<crate::config::EmailConfig, ToolExecError> {
    let path = instance_dir.join("email.toml");
    if !path.exists() {
        return Ok(crate::config::EmailConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| ToolExecError(format!("failed to read email config: {e}")))?;
    toml::from_str(&raw)
        .map_err(|e| ToolExecError(format!("failed to parse email config: {e}")))
}

fn save_instance_email_config(instance_dir: &Path, config: &crate::config::EmailConfig) -> Result<(), ToolExecError> {
    fs::create_dir_all(instance_dir)
        .map_err(|e| ToolExecError(format!("failed to create instance dir: {e}")))?;
    let output = toml::to_string_pretty(config)
        .map_err(|e| ToolExecError(format!("failed to serialize email config: {e}")))?;
    fs::write(instance_dir.join("email.toml"), &output)
        .map_err(|e| ToolExecError(format!("failed to write email config: {e}")))
}

// ---------------------------------------------------------------------------
// install_package — install system packages
// ---------------------------------------------------------------------------

pub struct InstallPackageTool;

#[derive(Deserialize, JsonSchema)]
pub struct InstallPackageArgs {
    /// Package name(s) to install, space-separated.
    pub packages: String,
}

impl Tool for InstallPackageTool {
    const NAME: &'static str = "install_package";
    type Error = ToolExecError;
    type Args = InstallPackageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "install_package".into(),
            description: "Install system packages using the detected package manager \
                (apt, dnf, pacman, brew, apk). Runs non-interactively."
                .into(),
            parameters: openai_schema::<InstallPackageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let packages = args.packages.trim();

        // Validate package names — reject shell metacharacters
        let safe_pkg_re = regex::Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._+:@/-]*$").unwrap();
        for pkg in packages.split_whitespace() {
            if !safe_pkg_re.is_match(pkg) {
                return Err(ToolExecError(format!(
                    "invalid package name: \"{pkg}\" — only alphanumeric, hyphens, dots, underscores, and plus signs are allowed"
                )));
            }
        }
        if packages.is_empty() {
            return Err(ToolExecError("no packages specified".into()));
        }

        let is_root = std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || std::env::var("EUID").map(|e| e == "0").unwrap_or(false);

        // Detect package manager
        let install_cmd = detect_package_manager(is_root)
            .ok_or_else(|| ToolExecError("no supported package manager found (tried apt-get, dnf, yum, pacman, brew, apk)".into()))?;

        let full_cmd = format!("{install_cmd} {packages}");
        log::info!("[install_package] running: {full_cmd}");

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .output()
            .await
            .map_err(|e| ToolExecError(format!("command failed: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            // Truncate to last 2000 chars
            let s: String = stdout.chars().rev().take(2000).collect::<String>().chars().rev().collect();
            result.push_str(&s);
        }
        if !stderr.is_empty() {
            let s: String = stderr.chars().rev().take(1000).collect::<String>().chars().rev().collect();
            result.push_str("\nstderr:\n");
            result.push_str(&s);
        }

        if output.status.success() {
            Ok(result)
        } else {
            Err(ToolExecError(format!(
                "install failed (exit {})\n{result}",
                output.status.code().unwrap_or(-1)
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// send_file — attach a workspace file to the chat so the user can see/download it
// ---------------------------------------------------------------------------

/// Shared collector for file attachments produced by send_file during a turn.
pub type SentFiles = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

pub struct SendFileTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    sent_files: SentFiles,
}

impl SendFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, sent_files: SentFiles) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            sent_files,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SendFileArgs {
    /// Path to the file relative to the instance workspace (e.g. "output.png", "reports/summary.pdf").
    pub path: String,
}

impl Tool for SendFileTool {
    const NAME: &'static str = "send_file";
    type Error = ToolExecError;
    type Args = SendFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_file".into(),
            description: "Send a file from the workspace to the chat so the user can see or download it. \
                Images will be displayed inline, other files will appear as download links. \
                Use this after creating or finding a file you want to share with the user."
                .into(),
            parameters: openai_schema::<SendFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let rel = args.path.trim().trim_start_matches('/');
        if rel.is_empty() {
            return Err(ToolExecError("path cannot be empty".into()));
        }

        let instance_dir = self.workspace_dir
            .join("instances")
            .join(&self.instance_slug);
        let file_path = instance_dir.join(rel);

        // Safety: must stay within instance dir
        let canonical = file_path.canonicalize()
            .map_err(|e| ToolExecError(format!("file not found: {e}")))?;
        let canonical_instance = instance_dir.canonicalize()
            .map_err(|e| ToolExecError(format!("instance dir error: {e}")))?;
        if !canonical.starts_with(&canonical_instance) {
            return Err(ToolExecError("path must be within the instance workspace".into()));
        }

        if !canonical.is_file() {
            return Err(ToolExecError(format!("'{}' is not a file", rel)));
        }

        let bytes = fs::read(&canonical)
            .map_err(|e| ToolExecError(format!("failed to read file: {e}")))?;

        let original_name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.to_string());

        let meta = super::uploads::save_upload(
            &self.workspace_dir,
            &self.instance_slug,
            &original_name,
            &bytes,
        )
        .map_err(|e| ToolExecError(format!("failed to save upload: {e}")))?;

        let marker = format!("[attached: {} ({})]", original_name, meta.id);
        self.sent_files.lock().unwrap().push(marker.clone());

        Ok(format!("file '{}' attached to chat. the user will see it.", original_name))
    }
}

fn detect_package_manager(is_root: bool) -> Option<String> {
    let sudo = if is_root { "" } else { "sudo " };

    let managers = [
        ("apt-get", format!("{sudo}apt-get install -y")),
        ("dnf", format!("{sudo}dnf install -y")),
        ("yum", format!("{sudo}yum install -y")),
        ("pacman", format!("{sudo}pacman -S --noconfirm")),
        ("apk", format!("{sudo}apk add")),
        ("brew", "brew install".to_string()),
    ];

    for (binary, cmd) in &managers {
        if std::process::Command::new("which")
            .arg(binary)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cmd.clone());
        }
    }
    None
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                vec![b as char]
            } else if b == b' ' {
                vec!['+']
            } else {
                format!("%{b:02X}").chars().collect()
            }
        })
        .collect()
}
