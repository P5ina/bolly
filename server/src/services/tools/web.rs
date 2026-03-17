use std::{fs, path::{Path, PathBuf}};

use base64::Engine;
use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal HTML tag stripping — removes tags, decodes common entities.
pub(super) fn strip_html_tags(html: &str) -> String {
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let no_scripts = re_script.replace_all(html, " ");
    let no_scripts = re_style.replace_all(&no_scripts, " ");

    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&no_scripts, " ");

    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

pub(super) fn format_browse_result(result: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(error) = result["error"].as_str() {
        return format!("browser error: {error}");
    }
    if let Some(results) = result["results"].as_array() {
        for r in results {
            let action = r["action"].as_str().unwrap_or("?");
            let ok = r["ok"].as_bool().unwrap_or(false);
            if !ok {
                let err = r["error"].as_str().unwrap_or("unknown error");
                out.push_str(&format!("[{action}] error: {err}\n"));
                continue;
            }
            match action {
                "navigate" => {
                    let title = r["title"].as_str().unwrap_or("");
                    let url = r["url"].as_str().unwrap_or("");
                    out.push_str(&format!("[navigate] {title} ({url})\n"));
                }
                "content" => {
                    let text = r["text"].as_str().unwrap_or("");
                    out.push_str(&format!("[content]\n{text}\n"));
                }
                "screenshot" => {
                    let path = r["path"].as_str().unwrap_or("");
                    // Read screenshot and return as image so the LLM can see it
                    if !path.is_empty() {
                        if let Ok(bytes) = std::fs::read(path) {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            return format!("__IMAGE__:image/png:{b64}");
                        }
                    }
                    out.push_str(&format!("[screenshot] saved to {path}\n"));
                }
                "evaluate" => {
                    let value = r["value"].as_str().unwrap_or("");
                    out.push_str(&format!("[evaluate] {value}\n"));
                }
                _ => {
                    out.push_str(&format!("[{action}] ok\n"));
                }
            }
        }
    }
    if out.is_empty() {
        "browser completed with no results".into()
    } else {
        out
    }
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{ts:x}")
}

pub fn url_encode(s: &str) -> String {
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

// ---------------------------------------------------------------------------
// web_search
// ---------------------------------------------------------------------------

pub struct WebSearchTool {
    config_path: PathBuf,
    initial_key: Option<String>,
}

impl WebSearchTool {
    pub fn new(api_key: Option<&str>, config_path: &Path) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            initial_key: api_key.filter(|k| !k.is_empty()).map(|k| k.to_string()),
        }
    }

    fn resolve_api_key(&self) -> Option<String> {
        if self.initial_key.is_some() {
            return self.initial_key.clone();
        }
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
            description: "Search the web. Returns titles, snippets, and URLs.".into(),
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
        let url = format!("https://api.search.brave.com/res/v1/web/search?q={encoded}&count=8");

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
            output.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                title,
                description,
                url
            ));
        }
        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// web_fetch
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
            description: "Fetch a web page and return its text content (HTML stripped).".into(),
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
            .header(
                "Accept",
                "text/html,application/xhtml+xml,text/plain,application/json",
            )
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

        if content_type.contains("json") {
            let truncated: String = body.chars().take(12_000).collect();
            return Ok(truncated);
        }

        let text = if content_type.contains("html") {
            strip_html_tags(&body)
        } else {
            body
        };

        let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let truncated: String = cleaned.chars().take(12_000).collect();
        if cleaned.len() > 12_000 {
            Ok(format!("{truncated}\n\n[content truncated — {url}]"))
        } else {
            Ok(truncated)
        }
    }
}

// ---------------------------------------------------------------------------
// browse
// ---------------------------------------------------------------------------

pub struct BrowseTool {
    instance_dir: PathBuf,
}

impl BrowseTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }

    fn script_path() -> PathBuf {
        if let Ok(dir) = std::env::var("BOLLY_SCRIPTS_DIR") {
            return PathBuf::from(dir).join("browse.mjs");
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("browse.mjs")
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BrowseAction {
    /// Action type: "navigate", "content", "screenshot", "click", "type", "wait", "evaluate", "select"
    pub action: String,
    /// URL for "navigate" action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// CSS selector for "click", "type", and "select" actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Text for "type" action, or option value for "select" action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Milliseconds for "wait" action (max 10000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    /// JavaScript expression for "evaluate" action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Take full-page screenshot (default: false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_page: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct BrowseArgs {
    /// Sequence of browser actions. Always start with "navigate" to load a page.
    /// Then use "content" to read text, "screenshot" to capture the page,
    /// "click"/"type" to interact, "evaluate" to run JS.
    pub actions: Vec<BrowseAction>,
    /// Overall timeout in seconds. Default: 60, max: 120.
    pub timeout_secs: Option<u64>,
}

impl Tool for BrowseTool {
    const NAME: &'static str = "browse";
    type Error = ToolExecError;
    type Args = BrowseArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "browse".into(),
            description: "Headless Chromium browser. Actions: navigate, content, screenshot, click, type, scroll.".into(),
            parameters: openai_schema::<BrowseArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if args.actions.is_empty() {
            return Err(ToolExecError("at least one action is required".into()));
        }

        let script = Self::script_path();
        if !script.exists() {
            return Err(ToolExecError(
                "browse tool not available — Playwright script not found".into(),
            ));
        }

        let drops_dir = self.instance_dir.join("drops");
        let _ = std::fs::create_dir_all(&drops_dir);

        let mut actions_json: Vec<serde_json::Value> = Vec::new();
        for act in &args.actions {
            let mut obj = serde_json::to_value(act)
                .map_err(|e| ToolExecError(format!("invalid action: {e}")))?;
            if act.action == "screenshot" {
                if obj.get("path").and_then(|p| p.as_str()).is_none() {
                    let id = format!("screenshot-{}", uuid_short());
                    let path = drops_dir.join(format!("{id}.png"));
                    obj["path"] = serde_json::Value::String(path.to_string_lossy().to_string());
                }
            }
            actions_json.push(obj);
        }

        let timeout = args.timeout_secs.unwrap_or(60).min(120);
        let input = serde_json::json!({
            "actions": actions_json,
            "timeout": timeout * 1000,
        });

        log::info!("[browse] {} actions, timeout={}s", actions_json.len(), timeout);

        let pw_path = std::env::var("PLAYWRIGHT_BROWSERS_PATH")
            .unwrap_or_else(|_| "/data/.playwright".to_string());
        let mut child = tokio::process::Command::new("node")
            .arg("--max-old-space-size=384")
            .arg(&script)
            .env("PLAYWRIGHT_BROWSERS_PATH", &pw_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ToolExecError(format!("failed to start browser: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let payload = serde_json::to_string(&input).unwrap();
            let _ = stdin.write_all(payload.as_bytes()).await;
            drop(stdin);
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout + 10),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| ToolExecError(format!("browser timed out after {timeout}s")))?
        .map_err(|e| ToolExecError(format!("browser process error: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if let Ok(result) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
            return Ok(format_browse_result(&result));
        }

        let mut result = String::new();
        if !stdout.is_empty() {
            let truncated: String = stdout.chars().take(8000).collect();
            result.push_str(&truncated);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            let truncated: String = stderr.chars().take(2000).collect();
            result.push_str(&format!("stderr: {truncated}"));
        }
        if result.is_empty() {
            result = format!(
                "browser exited with code {}",
                output.status.code().unwrap_or(-1)
            );
        }
        Ok(result)
    }
}
