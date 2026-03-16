use std::path::{Path, PathBuf};

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
    workspace_dir: PathBuf,
    instance_slug: String,
    landing_url: String,
    auth_token: String,
}

impl WatchVideoTool {
    pub fn new(
        openrouter_key: &str,
        workspace_dir: &Path,
        instance_slug: &str,
        landing_url: &str,
        auth_token: &str,
    ) -> Self {
        Self {
            openrouter_key: openrouter_key.to_string(),
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            landing_url: landing_url.to_string(),
            auth_token: auth_token.to_string(),
        }
    }

    /// Save bytes as an upload and return a public URL.
    fn save_and_get_url(&self, name: &str, bytes: &[u8]) -> Result<String, ToolExecError> {
        let meta = crate::services::uploads::save_upload(
            &self.workspace_dir, &self.instance_slug, name, bytes,
        ).map_err(|e| ToolExecError(format!("failed to save upload: {e}")))?;

        if self.landing_url.is_empty() {
            return Err(ToolExecError(
                "no public URL configured — cannot send video to Gemini. \
                 set a landing_url in config.toml".into()
            ));
        }

        Ok(format!(
            "{}/public/files/{}/{}?token={}",
            self.landing_url, self.instance_slug, meta.id, self.auth_token
        ))
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

        let local_path = Path::new(&input);
        let is_local = local_path.exists() && local_path.is_file();

        if is_local {
            // Local file — compress if needed, save as upload, get public URL
            let video_path = maybe_compress(&input).await?;
            let bytes = std::fs::read(&video_path)
                .map_err(|e| ToolExecError(format!("failed to read video: {e}")))?;
            if video_path != input { let _ = std::fs::remove_file(&video_path); }

            let url = self.save_and_get_url(
                local_path.file_name().and_then(|n| n.to_str()).unwrap_or("video.mp4"),
                &bytes,
            )?;
            log::info!("[watch_video] local file → upload URL");
            analyze_with_gemini(&self.openrouter_key, &url, prompt).await

        } else if is_youtube_url(&input) {
            // YouTube — download with yt-dlp, compress, save as upload
            log::info!("[watch_video] downloading YouTube video...");
            let downloaded = download_youtube(&input).await?;
            let video_path = maybe_compress(&downloaded).await?;
            let bytes = std::fs::read(&video_path)
                .map_err(|e| ToolExecError(format!("failed to read video: {e}")))?;
            let _ = std::fs::remove_file(&downloaded);
            if video_path != downloaded { let _ = std::fs::remove_file(&video_path); }

            let url = self.save_and_get_url("youtube_video.mp4", &bytes)?;
            log::info!("[watch_video] YouTube → upload URL ({:.1} MB)", bytes.len() as f64 / 1024.0 / 1024.0);
            analyze_with_gemini(&self.openrouter_key, &url, prompt).await

        } else {
            // Direct URL — pass through
            log::info!("[watch_video] direct URL: {input}");
            analyze_with_gemini(&self.openrouter_key, &input, prompt).await
        }
    }
}

fn is_youtube_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("youtube.com/") || lower.contains("youtu.be/")
}

/// Compress if over MAX_VIDEO_SIZE, return path (may be same or new temp file).
async fn maybe_compress(path: &str) -> Result<String, ToolExecError> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size <= MAX_VIDEO_SIZE {
        return Ok(path.to_string());
    }
    log::info!(
        "[watch_video] compressing {:.1} MB → target {:.1} MB",
        size as f64 / 1024.0 / 1024.0,
        MAX_VIDEO_SIZE as f64 / 1024.0 / 1024.0
    );
    compress_video(Path::new(path)).await
}

/// Download YouTube video with yt-dlp to a temp file.
async fn download_youtube(url: &str) -> Result<String, ToolExecError> {
    ensure_ytdlp().await?;

    let output_path = format!("/tmp/yt_{}.mp4", std::process::id());

    let result = tokio::process::Command::new("yt-dlp")
        .args([
            "-f", "best[ext=mp4][filesize<100M]/best[ext=mp4]/best",
            "--no-playlist",
            "--no-warnings",
            "-o", &output_path,
            url,
        ])
        .output()
        .await
        .map_err(|e| ToolExecError(format!("yt-dlp failed: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(ToolExecError(format!("yt-dlp download failed: {}", stderr.chars().take(300).collect::<String>())));
    }

    if !Path::new(&output_path).exists() {
        return Err(ToolExecError("yt-dlp produced no output file".into()));
    }

    Ok(output_path)
}

async fn ensure_ytdlp() -> Result<(), ToolExecError> {
    let check = tokio::process::Command::new("yt-dlp")
        .arg("--version")
        .output()
        .await;

    if check.is_ok() && check.as_ref().unwrap().status.success() {
        return Ok(());
    }

    log::info!("[watch_video] installing yt-dlp...");
    let pipx = tokio::process::Command::new("pipx")
        .args(["install", "yt-dlp"])
        .output()
        .await;

    if pipx.as_ref().map(|o| o.status.success()).unwrap_or(false) {
        return Ok(());
    }

    let install = tokio::process::Command::new("sh")
        .args(["-c", "curl -sL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp && chmod +x /usr/local/bin/yt-dlp"])
        .output()
        .await
        .map_err(|e| ToolExecError(format!("failed to install yt-dlp: {e}")))?;

    if !install.status.success() {
        return Err(ToolExecError("yt-dlp not found and failed to install".into()));
    }
    Ok(())
}

/// Compress video with ffmpeg.
async fn compress_video(path: &Path) -> Result<String, ToolExecError> {
    let check = tokio::process::Command::new("ffmpeg").arg("-version").output().await;
    if check.is_err() || !check.as_ref().unwrap().status.success() {
        return Err(ToolExecError("ffmpeg is not installed — cannot compress video".into()));
    }

    let output_path = format!("/tmp/compressed_{}.mp4", std::process::id());

    let result = tokio::process::Command::new("ffmpeg")
        .args([
            "-i", &path.display().to_string(),
            "-vf", "scale=-2:480",
            "-c:v", "libx264", "-crf", "28", "-preset", "fast",
            "-c:a", "aac", "-b:a", "64k",
            "-movflags", "+faststart",
            "-y", &output_path,
        ])
        .output()
        .await
        .map_err(|e| ToolExecError(format!("ffmpeg failed: {e}")))?;

    if !result.status.success() {
        // Retry with simpler settings
        let result2 = tokio::process::Command::new("ffmpeg")
            .args([
                "-i", &path.display().to_string(),
                "-c:v", "libx264", "-crf", "32", "-preset", "ultrafast",
                "-c:a", "aac", "-b:a", "48k",
                "-y", &output_path,
            ])
            .output()
            .await
            .map_err(|e| ToolExecError(format!("ffmpeg retry failed: {e}")))?;

        if !result2.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(ToolExecError(format!("ffmpeg compression failed: {}", stderr.chars().take(300).collect::<String>())));
        }
    }

    let compressed_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
    log::info!("[watch_video] compressed to {:.1} MB", compressed_size as f64 / 1024.0 / 1024.0);
    Ok(output_path)
}

/// Send video URL to Gemini via OpenRouter.
async fn analyze_with_gemini(
    api_key: &str,
    video_url: &str,
    prompt: &str,
) -> Result<String, ToolExecError> {
    let body = serde_json::json!({
        "model": GEMINI_MODEL,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "video_url", "video_url": { "url": video_url } },
                { "type": "text", "text": prompt }
            ]
        }],
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
        return Err(ToolExecError(format!("OpenRouter HTTP {status}: {err_body}")));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ToolExecError(format!("failed to parse response: {e}")))?;

    Ok(result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no response from Gemini)")
        .to_string())
}
