use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

const GEMINI_MODEL: &str = "google/gemini-2.5-flash";

// ---------------------------------------------------------------------------
// watch_video
// ---------------------------------------------------------------------------

pub struct WatchVideoTool {
    openrouter_key: String,
}

impl WatchVideoTool {
    pub fn new(openrouter_key: &str) -> Self {
        Self {
            openrouter_key: openrouter_key.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct WatchVideoArgs {
    /// URL of the video to watch. Supports YouTube links and direct video URLs (.mp4, .webm, etc.).
    pub url: String,
    /// What to focus on when watching. E.g. "summarize this video", "what happens at 2:30?",
    /// "extract all code shown", "describe the UI demo". Defaults to a general summary.
    pub prompt: Option<String>,
}

impl Tool for WatchVideoTool {
    const NAME: &'static str = "watch_video";
    type Error = ToolExecError;
    type Args = WatchVideoArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "watch_video".into(),
            description: "Analyze a video via Gemini vision AI. YouTube URLs or direct links.".into(),
            parameters: openai_schema::<WatchVideoArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let url = args.url.trim();
        if url.is_empty() {
            return Err(ToolExecError("url cannot be empty".into()));
        }

        let prompt = args
            .prompt
            .as_deref()
            .unwrap_or("Watch this video carefully and provide a detailed summary. \
                Include key points, any text/code shown, and notable visual elements.");

        // For YouTube URLs, get a direct video URL via yt-dlp
        let video_url = if is_youtube_url(url) {
            get_direct_url(url).await?
        } else {
            url.to_string()
        };

        log::info!("[watch_video] sending video URL to Gemini: {video_url}");

        // Send to Gemini via OpenRouter
        analyze_with_gemini(&self.openrouter_key, &video_url, prompt).await
    }
}

fn is_youtube_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("youtube.com/") || lower.contains("youtu.be/")
}

/// Use yt-dlp to extract a direct video URL without downloading.
async fn get_direct_url(url: &str) -> Result<String, ToolExecError> {
    // Ensure yt-dlp is available
    let check = tokio::process::Command::new("yt-dlp")
        .arg("--version")
        .output()
        .await;

    if check.is_err() || !check.as_ref().unwrap().status.success() {
        log::info!("[watch_video] installing yt-dlp...");

        // Try pipx first (works on externally-managed Python environments)
        let pipx = tokio::process::Command::new("pipx")
            .args(["install", "yt-dlp"])
            .output()
            .await;

        let pipx_ok = pipx.as_ref().map(|o| o.status.success()).unwrap_or(false);

        if !pipx_ok {
            // Fallback: download standalone binary
            let install = tokio::process::Command::new("sh")
                .args([
                    "-c",
                    "curl -sL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp && chmod +x /usr/local/bin/yt-dlp",
                ])
                .output()
                .await
                .map_err(|e| ToolExecError(format!("failed to install yt-dlp: {e}")))?;

            if !install.status.success() {
                let stderr = String::from_utf8_lossy(&install.stderr);
                return Err(ToolExecError(format!(
                    "yt-dlp not found and failed to install: {stderr}"
                )));
            }
        }
    }

    // Get direct URL without downloading
    let result = tokio::process::Command::new("yt-dlp")
        .args([
            "-f", "best[ext=mp4][filesize<50M]/best[ext=mp4]/best",
            "--no-playlist",
            "--get-url",
            "--no-warnings",
            "--quiet",
            url,
        ])
        .output()
        .await
        .map_err(|e| ToolExecError(format!("yt-dlp failed: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(ToolExecError(format!("yt-dlp failed to get URL: {stderr}")));
    }

    let direct_url = String::from_utf8_lossy(&result.stdout).trim().to_string();
    if direct_url.is_empty() {
        return Err(ToolExecError("yt-dlp returned empty URL".into()));
    }

    // yt-dlp may return multiple URLs (video + audio), take the first
    let first_url = direct_url.lines().next().unwrap_or(&direct_url).to_string();

    Ok(first_url)
}

/// Send video URL to Gemini 2.5 Flash via OpenRouter API.
async fn analyze_with_gemini(
    api_key: &str,
    video_url: &str,
    prompt: &str,
) -> Result<String, ToolExecError> {
    let body = serde_json::json!({
        "model": GEMINI_MODEL,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "video_url",
                        "video_url": {
                            "url": video_url
                        }
                    },
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }
        ],
        "max_tokens": 4096
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| ToolExecError(format!("failed to build HTTP client: {e}")))?;

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ToolExecError(format!("OpenRouter request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_body = response.text().await.unwrap_or_default();
        return Err(ToolExecError(format!(
            "OpenRouter returned HTTP {status}: {err_body}"
        )));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ToolExecError(format!("failed to parse OpenRouter response: {e}")))?;

    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no response from Gemini)")
        .to_string();

    Ok(content)
}
