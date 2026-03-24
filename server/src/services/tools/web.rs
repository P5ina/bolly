use std::path::{Path, PathBuf};

use base64::Engine;
use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_browse_result(result: &serde_json::Value) -> String {
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
                    // Read screenshot and return as image + file path so the agent
                    // can both see it and send it via send_file
                    if !path.is_empty() {
                        if let Ok(bytes) = std::fs::read(path) {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            return serde_json::to_string(&serde_json::json!([
                                {"type": "text", "text": format!("[screenshot saved to {path}]")},
                                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64}}
                            ])).unwrap();
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
