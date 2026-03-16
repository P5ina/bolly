use std::path::Path;

use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

const GEMINI_MODEL: &str = "google/gemini-2.5-flash";
/// Max video size to send to Gemini (45 MB). Larger files get compressed.
const MAX_VIDEO_SIZE: u64 = 45 * 1024 * 1024;

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
    /// URL or local file path of the video to watch.
    /// Supports: YouTube links, direct video URLs (.mp4, .webm),
    /// and local file paths (from uploads or filesystem).
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
            description: "Analyze a video via Gemini vision AI. \
                Accepts YouTube URLs, direct video links, or local file paths. \
                Large videos are automatically compressed before analysis."
                .into(),
            parameters: openai_schema::<WatchVideoArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let input = args.url.trim().to_string();
        if input.is_empty() {
            return Err(ToolExecError("url/path cannot be empty".into()));
        }

        let prompt = args
            .prompt
            .as_deref()
            .unwrap_or("Watch this video carefully and provide a detailed summary. \
                Include key points, any text/code shown, and notable visual elements.");

        // Determine if input is a local file or URL
        let local_path = Path::new(&input);
        let is_local = local_path.exists() && local_path.is_file();

        if is_local {
            // Local file — upload to a temporary host or convert to data URL
            let file_size = std::fs::metadata(local_path)
                .map(|m| m.len())
                .unwrap_or(0);

            let video_path = if file_size > MAX_VIDEO_SIZE {
                // Compress with ffmpeg
                log::info!(
                    "[watch_video] compressing local video ({:.1} MB > {:.1} MB limit)",
                    file_size as f64 / 1024.0 / 1024.0,
                    MAX_VIDEO_SIZE as f64 / 1024.0 / 1024.0
                );
                compress_video(local_path).await?
            } else {
                input.clone()
            };

            // Upload to tmpfiles.org for a temporary public URL (1 hour)
            let public_url = upload_tmp_file(&video_path).await?;

            // Clean up compressed file if we created one
            if video_path != input {
                let _ = std::fs::remove_file(&video_path);
            }

            log::info!("[watch_video] local file uploaded to temporary URL");
            analyze_with_gemini(&self.openrouter_key, &public_url, prompt).await
        } else if is_youtube_url(&input) {
            let video_url = get_direct_url(&input).await?;
            log::info!("[watch_video] YouTube → direct URL");
            analyze_with_gemini(&self.openrouter_key, &video_url, prompt).await
        } else {
            // Direct URL
            log::info!("[watch_video] sending URL to Gemini: {input}");
            analyze_with_gemini(&self.openrouter_key, &input, prompt).await
        }
    }
}

fn is_youtube_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("youtube.com/") || lower.contains("youtu.be/")
}

/// Compress video with ffmpeg to fit within size limits.
async fn compress_video(path: &Path) -> Result<String, ToolExecError> {
    // Check ffmpeg is available
    let check = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .await;
    if check.is_err() || !check.as_ref().unwrap().status.success() {
        return Err(ToolExecError("ffmpeg is not installed — cannot compress video".into()));
    }

    let output_path = format!("/tmp/compressed_{}.mp4", std::process::id());

    let result = tokio::process::Command::new("ffmpeg")
        .args([
            "-i", &path.display().to_string(),
            "-vf", "scale=-2:480",           // Scale to 480p
            "-c:v", "libx264",
            "-crf", "28",                     // Quality (higher = smaller)
            "-preset", "fast",
            "-c:a", "aac",
            "-b:a", "64k",                    // Audio bitrate
            "-movflags", "+faststart",
            "-y",                              // Overwrite
            &output_path,
        ])
        .output()
        .await
        .map_err(|e| ToolExecError(format!("ffmpeg failed to start: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        // Try simpler compression without scaling
        let result2 = tokio::process::Command::new("ffmpeg")
            .args([
                "-i", &path.display().to_string(),
                "-c:v", "libx264",
                "-crf", "32",
                "-preset", "ultrafast",
                "-c:a", "aac",
                "-b:a", "48k",
                "-y",
                &output_path,
            ])
            .output()
            .await
            .map_err(|e| ToolExecError(format!("ffmpeg retry failed: {e}")))?;

        if !result2.status.success() {
            return Err(ToolExecError(format!(
                "ffmpeg compression failed: {}",
                stderr.chars().take(300).collect::<String>()
            )));
        }
    }

    let compressed_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    log::info!(
        "[watch_video] compressed to {:.1} MB",
        compressed_size as f64 / 1024.0 / 1024.0
    );

    Ok(output_path)
}

/// Upload a file to tmpfiles.org for a temporary public URL.
async fn upload_tmp_file(path: &str) -> Result<String, ToolExecError> {
    let file_bytes = std::fs::read(path)
        .map_err(|e| ToolExecError(format!("failed to read file: {e}")))?;

    let file_name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video.mp4");

    let part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(file_name.to_string())
        .mime_str("video/mp4")
        .map_err(|e| ToolExecError(format!("mime error: {e}")))?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| ToolExecError(format!("failed to build HTTP client: {e}")))?;

    let response = client
        .post("https://tmpfiles.org/api/v1/upload")
        .multipart(form)
        .send()
        .await
        .map_err(|e| ToolExecError(format!("upload failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ToolExecError(format!("tmpfiles.org returned HTTP {status}: {body}")));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ToolExecError(format!("failed to parse upload response: {e}")))?;

    // tmpfiles.org returns {"status":"ok","data":{"url":"https://tmpfiles.org/12345/video.mp4"}}
    // Direct download URL replaces /12345/ with /dl/12345/
    let page_url = result["data"]["url"]
        .as_str()
        .ok_or_else(|| ToolExecError("no URL in upload response".into()))?;

    let direct_url = page_url.replacen("tmpfiles.org/", "tmpfiles.org/dl/", 1);
    Ok(direct_url)
}

/// Use yt-dlp to extract a direct video URL without downloading.
async fn get_direct_url(url: &str) -> Result<String, ToolExecError> {
    let check = tokio::process::Command::new("yt-dlp")
        .arg("--version")
        .output()
        .await;

    if check.is_err() || !check.as_ref().unwrap().status.success() {
        log::info!("[watch_video] installing yt-dlp...");

        let pipx = tokio::process::Command::new("pipx")
            .args(["install", "yt-dlp"])
            .output()
            .await;

        let pipx_ok = pipx.as_ref().map(|o| o.status.success()).unwrap_or(false);

        if !pipx_ok {
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
