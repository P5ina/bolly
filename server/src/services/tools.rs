use std::{
    fmt,
    fs,
    path::{Path, PathBuf},
};

use chrono::{Local, Utc};
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
            parameters: serde_json::to_value(schemars::schema_for!(EditSoulArgs)).unwrap(),
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
    /// Relative path within the instance directory (e.g. "soul.md", "drops/idea.md", "memory/facts.md").
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
            description: "Read a file from your instance workspace. The path is relative to \
                your instance directory."
                .into(),
            parameters: serde_json::to_value(schemars::schema_for!(ReadFileArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = self.instance_dir.join(&args.path);

        // prevent path traversal
        if !target.starts_with(&self.instance_dir) {
            return Err(ToolExecError(
                "path must stay within instance directory".into(),
            ));
        }

        fs::read_to_string(&target).map_err(|e| ToolExecError(format!("{}: {e}", args.path)))
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
    /// Relative path within the instance directory (e.g. "drops/new-idea.md"). Parent directories are created automatically.
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
            description: "Write or overwrite a file in your instance workspace. The path is \
                relative to your instance directory. Parent directories will be created \
                automatically."
                .into(),
            parameters: serde_json::to_value(schemars::schema_for!(WriteFileArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = self.instance_dir.join(&args.path);

        // prevent path traversal
        if !target.starts_with(&self.instance_dir) {
            return Err(ToolExecError(
                "path must stay within instance directory".into(),
            ));
        }

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
    /// Optional relative subdirectory path (e.g. "drops"). Omit to list the root of your instance directory.
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
            description: "List files and directories in your instance workspace.".into(),
            parameters: serde_json::to_value(schemars::schema_for!(ListFilesArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = match &args.path {
            Some(p) if !p.is_empty() => self.instance_dir.join(p),
            _ => self.instance_dir.clone(),
        };

        if !target.starts_with(&self.instance_dir) {
            return Err(ToolExecError(
                "path must stay within instance directory".into(),
            ));
        }

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
            parameters: serde_json::to_value(schemars::schema_for!(CurrentTimeArgs)).unwrap(),
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
            parameters: serde_json::to_value(schemars::schema_for!(WebSearchArgs)).unwrap(),
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
// set_api_key — save an API key to the global config
// ---------------------------------------------------------------------------

pub struct SetApiKeyTool {
    config_path: PathBuf,
}

impl SetApiKeyTool {
    pub fn new(config_path: &Path) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
        }
    }
}

/// Arguments for set_api_key tool.
#[derive(Deserialize, JsonSchema)]
pub struct SetApiKeyArgs {
    /// Which provider to set the key for: "anthropic", "openai", or "brave_search".
    pub provider: String,
    /// The API key value.
    pub key: String,
}

impl Tool for SetApiKeyTool {
    const NAME: &'static str = "set_api_key";
    type Error = ToolExecError;
    type Args = SetApiKeyArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "set_api_key".into(),
            description: "Save an API key to the global config file. Supported providers: \
                \"anthropic\", \"openai\", \"brave_search\". Use this when the user gives you \
                an API key to configure. The key will be saved and available on the next message."
                .into(),
            parameters: serde_json::to_value(schemars::schema_for!(SetApiKeyArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let provider = args.provider.trim().to_lowercase();
        let key = args.key.trim().to_string();

        if key.is_empty() {
            return Err(ToolExecError("key cannot be empty".into()));
        }

        let toml_field = match provider.as_str() {
            "anthropic" => "ANTHROPIC",
            "openai" | "open_ai" => "OPEN_AI",
            "brave_search" | "brave" => "BRAVE_SEARCH",
            other => {
                return Err(ToolExecError(format!(
                    "unknown provider \"{other}\". supported: anthropic, openai, brave_search"
                )));
            }
        };

        // Read existing config
        let raw = fs::read_to_string(&self.config_path)
            .map_err(|e| ToolExecError(format!("failed to read config: {e}")))?;

        let mut doc: toml::Table = raw
            .parse()
            .map_err(|e| ToolExecError(format!("failed to parse config: {e}")))?;

        // Ensure [llm.tokens] section exists
        let llm = doc
            .entry("llm")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| ToolExecError("llm config is not a table".into()))?;

        let tokens = llm
            .entry("tokens")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| ToolExecError("llm.tokens is not a table".into()))?;

        tokens.insert(
            toml_field.to_string(),
            toml::Value::String(key),
        );

        let output = doc.to_string();
        fs::write(&self.config_path, &output)
            .map_err(|e| ToolExecError(format!("failed to write config: {e}")))?;

        Ok(format!(
            "{provider} API key saved to config. you can now use tools that require this key immediately."
        ))
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
            parameters: serde_json::to_value(schemars::schema_for!(RememberArgs)).unwrap(),
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
                or anything they've told you before."
                .into(),
            parameters: serde_json::to_value(schemars::schema_for!(RecallArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let facts_path = self.instance_dir.join("memory").join("facts.md");
        let content = fs::read_to_string(&facts_path).unwrap_or_default();

        if content.is_empty() {
            return Ok("no memories yet.".into());
        }

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

        if matches.is_empty() {
            // Return all facts as fallback
            let all_facts: Vec<&str> = content
                .lines()
                .filter(|l| l.starts_with("- "))
                .map(|l| l.trim_start_matches("- "))
                .collect();
            if all_facts.is_empty() {
                return Ok("no memories yet.".into());
            }
            return Ok(format!(
                "no exact matches for \"{query}\", but here's everything I remember:\n{}",
                all_facts.join("\n")
            ));
        }

        Ok(format!(
            "memories matching \"{query}\":\n{}",
            categorized.join("\n")
        ))
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
            parameters: serde_json::to_value(schemars::schema_for!(JournalArgs)).unwrap(),
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
            parameters: serde_json::to_value(schemars::schema_for!(ReadJournalArgs)).unwrap(),
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
            parameters: serde_json::to_value(schemars::schema_for!(ScheduleMessageArgs)).unwrap(),
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
            parameters: serde_json::to_value(schemars::schema_for!(SetMoodArgs)).unwrap(),
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
            parameters: serde_json::to_value(schemars::schema_for!(GetMoodArgs)).unwrap(),
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
