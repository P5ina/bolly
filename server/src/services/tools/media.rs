use std::path::{Path, PathBuf};

use base64::Engine;
use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

const GEMINI_MODEL: &str = "google/gemini-2.5-flash";
const MAX_MEDIA_SIZE: u64 = 45 * 1024 * 1024; // 45 MB

// ---------------------------------------------------------------------------
// Shared media context (upload, compress, analyze)
// ---------------------------------------------------------------------------

pub struct MediaContext {
    openrouter_key: String,
    workspace_dir: PathBuf,
    instance_slug: String,
    public_url: String,
    auth_token: String,
}

impl MediaContext {
    pub fn new(
        openrouter_key: &str,
        workspace_dir: &Path,
        instance_slug: &str,
        public_url: &str,
        auth_token: &str,
    ) -> Self {
        Self {
            openrouter_key: openrouter_key.to_string(),
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            public_url: public_url.to_string(),
            auth_token: auth_token.to_string(),
        }
    }

    fn save_and_get_url(&self, name: &str, bytes: &[u8]) -> Result<String, ToolExecError> {
        let meta = crate::services::uploads::save_upload(
            &self.workspace_dir, &self.instance_slug, name, bytes,
        ).map_err(|e| ToolExecError(format!("failed to save upload: {e}")))?;

        if self.public_url.is_empty() {
            return Err(ToolExecError(
                "no public URL configured — set BOLLY_PUBLIC_URL env var".into()
            ));
        }

        Ok(format!(
            "{}/public/files/{}/{}?token={}",
            self.public_url, self.instance_slug, meta.id, self.auth_token
        ))
    }

    /// Process input → reference for Gemini.
    /// Video: returns a public URL (uploaded).
    /// Audio: returns a local file path (for base64 encoding).
    async fn resolve_media_ref(
        &self,
        input: &str,
        media_type: MediaType,
    ) -> Result<String, ToolExecError> {
        let local_path = Path::new(input);
        let is_local = local_path.exists() && local_path.is_file();

        if is_local {
            let processed = maybe_compress(input, media_type).await?;
            match media_type {
                MediaType::Audio => Ok(processed), // keep local for base64
                MediaType::Video => {
                    let bytes = std::fs::read(&processed)
                        .map_err(|e| ToolExecError(format!("failed to read file: {e}")))?;
                    if processed != input { let _ = std::fs::remove_file(&processed); }
                    let name = local_path.file_name().and_then(|n| n.to_str())
                        .unwrap_or(media_type.default_filename());
                    self.save_and_get_url(name, &bytes)
                }
            }
        } else if is_youtube_url(input) {
            log::info!("[media] downloading from YouTube...");
            let downloaded = download_youtube(input, media_type).await?;
            let processed = maybe_compress(&downloaded, media_type).await?;
            if processed != downloaded { let _ = std::fs::remove_file(&downloaded); }

            match media_type {
                MediaType::Audio => {
                    log::info!("[media] YouTube audio ready: {processed}");
                    Ok(processed) // keep local for base64
                }
                MediaType::Video => {
                    let bytes = std::fs::read(&processed)
                        .map_err(|e| ToolExecError(format!("failed to read file: {e}")))?;
                    let _ = std::fs::remove_file(&processed);
                    let name = format!("youtube_{}", media_type.default_filename());
                    let url = self.save_and_get_url(&name, &bytes)?;
                    log::info!("[media] YouTube video → upload ({:.1} MB)", bytes.len() as f64 / 1024.0 / 1024.0);
                    Ok(url)
                }
            }
        } else {
            Ok(input.to_string())
        }
    }
}

#[derive(Clone, Copy)]
pub enum MediaType {
    Video,
    Audio,
}

impl MediaType {
    fn default_filename(self) -> &'static str {
        match self { MediaType::Video => "video.mp4", MediaType::Audio => "audio.mp3" }
    }
    fn yt_dlp_format(self) -> &'static str {
        match self {
            MediaType::Video => "best[ext=mp4][filesize<100M]/best[ext=mp4]/best",
            MediaType::Audio => "bestaudio[ext=m4a]/bestaudio/best",
        }
    }
    fn yt_dlp_ext(self) -> &'static str {
        match self { MediaType::Video => "mp4", MediaType::Audio => "m4a" }
    }
}

// ---------------------------------------------------------------------------
// watch_video tool
// ---------------------------------------------------------------------------

pub struct WatchVideoTool { ctx: MediaContext }

impl WatchVideoTool {
    pub fn new(openrouter_key: &str, workspace_dir: &Path, instance_slug: &str,
               public_url: &str, auth_token: &str) -> Self {
        Self { ctx: MediaContext::new(openrouter_key, workspace_dir, instance_slug, public_url, auth_token) }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct WatchVideoArgs {
    /// URL or local file path of the video. Supports YouTube, direct URLs, local paths.
    pub url: String,
    /// What to focus on. Defaults to a general summary.
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
            description: "Analyze a video via Gemini. YouTube URLs, direct links, or local files. Auto-compresses large files.".into(),
            parameters: openai_schema::<WatchVideoArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let input = args.url.trim();
        if input.is_empty() { return Err(ToolExecError("url/path cannot be empty".into())); }
        let prompt = args.prompt.as_deref()
            .unwrap_or("Watch this video carefully and provide a detailed summary. Include key points, any text/code shown, and notable visual elements.");
        let media_ref = self.ctx.resolve_media_ref(input, MediaType::Video).await?;
        analyze_with_gemini(&self.ctx.openrouter_key, &media_ref, prompt, MediaType::Video).await
    }
}

// ---------------------------------------------------------------------------
// listen_music tool
// ---------------------------------------------------------------------------

pub struct ListenMusicTool { ctx: MediaContext }

impl ListenMusicTool {
    pub fn new(openrouter_key: &str, workspace_dir: &Path, instance_slug: &str,
               public_url: &str, auth_token: &str) -> Self {
        Self { ctx: MediaContext::new(openrouter_key, workspace_dir, instance_slug, public_url, auth_token) }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ListenMusicArgs {
    /// URL or local file path of the audio. Supports YouTube, Spotify links (audio only), direct URLs, local paths.
    pub url: String,
    /// What to focus on. E.g. "what genre is this?", "transcribe the lyrics", "describe the mood".
    /// Defaults to a general analysis.
    pub prompt: Option<String>,
}

impl Tool for ListenMusicTool {
    const NAME: &'static str = "listen_music";
    type Error = ToolExecError;
    type Args = ListenMusicArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "listen_music".into(),
            description: "Analyze audio/music via Gemini. YouTube URLs, direct links, or local files. Can identify genre, transcribe lyrics, describe mood.".into(),
            parameters: openai_schema::<ListenMusicArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let input = args.url.trim();
        if input.is_empty() { return Err(ToolExecError("url/path cannot be empty".into())); }
        let prompt = args.prompt.as_deref()
            .unwrap_or("Listen to this audio carefully. Describe what you hear: genre, mood, instruments, lyrics (if any), and overall impression.");
        let media_ref = self.ctx.resolve_media_ref(input, MediaType::Audio).await?;
        let result = analyze_with_gemini(&self.ctx.openrouter_key, &media_ref, prompt, MediaType::Audio).await;
        // Clean up temp audio file if it was downloaded/compressed
        if media_ref != input && Path::new(&media_ref).exists() {
            let _ = std::fs::remove_file(&media_ref);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub fn is_youtube_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("youtube.com/") || lower.contains("youtu.be/") || lower.contains("music.youtube.com/")
}

async fn maybe_compress(path: &str, media_type: MediaType) -> Result<String, ToolExecError> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size <= MAX_MEDIA_SIZE {
        return Ok(path.to_string());
    }
    log::info!("[media] compressing {:.1} MB", size as f64 / 1024.0 / 1024.0);
    compress_media(Path::new(path), media_type).await
}

pub async fn download_youtube(url: &str, media_type: MediaType) -> Result<String, ToolExecError> {
    ensure_ytdlp().await?;
    let ext = media_type.yt_dlp_ext();
    let output_path = format!("/tmp/yt_{}.{ext}", std::process::id());

    let result = tokio::process::Command::new("yt-dlp")
        .args([
            "-f", media_type.yt_dlp_format(),
            "--no-playlist", "--no-warnings",
            "-o", &output_path,
            url,
        ])
        .output()
        .await
        .map_err(|e| ToolExecError(format!("yt-dlp failed: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(ToolExecError(format!("yt-dlp failed: {}", stderr.chars().take(300).collect::<String>())));
    }
    if !Path::new(&output_path).exists() {
        return Err(ToolExecError("yt-dlp produced no output file".into()));
    }
    Ok(output_path)
}

pub async fn ensure_ytdlp() -> Result<(), ToolExecError> {
    let check = tokio::process::Command::new("yt-dlp").arg("--version").output().await;
    if check.is_ok() && check.as_ref().unwrap().status.success() { return Ok(()); }

    log::info!("[media] installing yt-dlp...");
    let pipx = tokio::process::Command::new("pipx").args(["install", "yt-dlp"]).output().await;
    if pipx.as_ref().map(|o| o.status.success()).unwrap_or(false) { return Ok(()); }

    let install = tokio::process::Command::new("sh")
        .args(["-c", "curl -sL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp && chmod +x /usr/local/bin/yt-dlp"])
        .output().await
        .map_err(|e| ToolExecError(format!("failed to install yt-dlp: {e}")))?;
    if !install.status.success() {
        return Err(ToolExecError("yt-dlp not found and failed to install".into()));
    }
    Ok(())
}

async fn compress_media(path: &Path, media_type: MediaType) -> Result<String, ToolExecError> {
    let check = tokio::process::Command::new("ffmpeg").arg("-version").output().await;
    if check.is_err() || !check.as_ref().unwrap().status.success() {
        return Err(ToolExecError("ffmpeg is not installed".into()));
    }

    let ext = match media_type { MediaType::Video => "mp4", MediaType::Audio => "mp3" };
    let output_path = format!("/tmp/compressed_{}.{ext}", std::process::id());
    let input = path.display().to_string();

    let args: Vec<&str> = match media_type {
        MediaType::Video => vec![
            "-i", &input, "-vf", "scale=-2:480",
            "-c:v", "libx264", "-crf", "28", "-preset", "fast",
            "-c:a", "aac", "-b:a", "64k", "-movflags", "+faststart",
            "-y", &output_path,
        ],
        MediaType::Audio => vec![
            "-i", &input, "-c:a", "libmp3lame", "-b:a", "96k",
            "-y", &output_path,
        ],
    };

    let result = tokio::process::Command::new("ffmpeg").args(&args).output().await
        .map_err(|e| ToolExecError(format!("ffmpeg failed: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(ToolExecError(format!("ffmpeg failed: {}", stderr.chars().take(300).collect::<String>())));
    }

    let size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
    log::info!("[media] compressed to {:.1} MB", size as f64 / 1024.0 / 1024.0);
    Ok(output_path)
}

/// For audio: pass file path to read + base64 encode.
/// For video: pass public URL.
async fn analyze_with_gemini(
    api_key: &str,
    media_ref: &str,
    prompt: &str,
    media_type: MediaType,
) -> Result<String, ToolExecError> {
    let content_block = match media_type {
        MediaType::Video => {
            serde_json::json!({
                "type": "video_url",
                "video_url": { "url": media_ref }
            })
        }
        MediaType::Audio => {
            // Audio must be base64-encoded — URLs not supported
            let bytes = std::fs::read(media_ref)
                .map_err(|e| ToolExecError(format!("failed to read audio file: {e}")))?;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let ext = Path::new(media_ref)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("mp3");
            serde_json::json!({
                "type": "input_audio",
                "input_audio": { "data": b64, "format": ext }
            })
        }
    };

    let body = serde_json::json!({
        "model": GEMINI_MODEL,
        "messages": [{
            "role": "user",
            "content": [content_block, { "type": "text", "text": prompt }]
        }],
        "max_tokens": 4096
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| ToolExecError(format!("HTTP client error: {e}")))?;

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
        .map_err(|e| ToolExecError(format!("OpenRouter request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let err = response.text().await.unwrap_or_default();
        return Err(ToolExecError(format!("OpenRouter HTTP {status}: {err}")));
    }

    let result: serde_json::Value = response.json().await
        .map_err(|e| ToolExecError(format!("failed to parse response: {e}")))?;

    Ok(result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no response from Gemini)")
        .to_string())
}
