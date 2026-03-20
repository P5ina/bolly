use std::{fs, path::{Path, PathBuf}};

use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::broadcast;

use super::{openai_schema, ToolExecError};
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;

// ---------------------------------------------------------------------------
// Mood state I/O
// ---------------------------------------------------------------------------

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

/// Allowed mood values that the client can visualize.
pub const ALLOWED_MOODS: &[&str] = &[
    "calm",
    "curious",
    "excited",
    "warm",
    "happy",
    "joyful",
    "reflective",
    "contemplative",
    "melancholy",
    "melancholic",
    "sad",
    "worried",
    "anxious",
    "playful",
    "mischievous",
    "focused",
    "tired",
    "peaceful",
    "loving",
    "tender",
    "creative",
    "energetic",
    "thoughtful",
    "grateful",
    "nostalgic",
];

// Mood tools removed — mood is now managed by background sentiment
// extraction (Haiku) and heartbeat triage, not by agent tool calls.

// ---------------------------------------------------------------------------
// play_music — control client-side music/ambient playback
// ---------------------------------------------------------------------------

pub struct PlayMusicTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    events: broadcast::Sender<ServerEvent>,
}

impl PlayMusicTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, events: broadcast::Sender<ServerEvent>) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct PlayMusicArgs {
    /// Action: "play", "pause", "set_volume".
    pub action: String,
    /// Track: "ambient", "intro", "loop", or a URL (YouTube, direct audio link).
    /// YouTube URLs are automatically downloaded. Required for "play".
    pub track: Option<String>,
    /// Volume 0.0–1.0. Used with "play" (sets initial volume) and "set_volume".
    pub volume: Option<f64>,
}

impl Tool for PlayMusicTool {
    const NAME: &'static str = "play_music";
    type Error = ToolExecError;
    type Args = PlayMusicArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "play_music".into(),
            description: "Control music/ambient playback in the user's browser. \
                Actions: \"play\" (start a track), \"pause\" (stop all music), \
                \"set_volume\" (change volume of a track). \
                Built-in tracks: \"ambient\", \"intro\", \"loop\". \
                You can also pass a YouTube URL or direct audio URL to play custom audio."
                .into(),
            parameters: openai_schema::<PlayMusicArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let action = args.action.to_lowercase();
        match action.as_str() {
            "play" | "pause" | "set_volume" => {}
            _ => return Err(ToolExecError(format!("unknown action '{action}', use play/pause/set_volume"))),
        }
        if action == "play" && args.track.is_none() {
            return Err(ToolExecError("track is required for play action".into()));
        }

        // Resolve track: download external URLs and save to uploads for same-origin playback
        let resolved_track = if action == "play" {
            if let Some(ref track) = args.track {
                let builtin = matches!(track.as_str(), "ambient" | "intro" | "loop");
                if builtin {
                    Some(track.clone())
                } else {
                    // External URL — download and proxy through uploads
                    let local_url = self.download_and_upload(track).await?;
                    Some(local_url)
                }
            } else {
                None
            }
        } else {
            args.track.clone()
        };

        let _ = self.events.send(ServerEvent::MusicControl {
            instance_slug: self.instance_slug.clone(),
            action: action.clone(),
            track: resolved_track.clone(),
            volume: args.volume,
        });
        let msg = match action.as_str() {
            "play" => format!("playing {}", args.track.as_deref().unwrap_or("?")),
            "pause" => "music paused".into(),
            "set_volume" => format!("volume set to {:.0}%", args.volume.unwrap_or(0.5) * 100.0),
            _ => "ok".into(),
        };
        Ok(msg)
    }
}

impl PlayMusicTool {
    /// Download audio from a URL (YouTube or direct) and save to instance uploads.
    /// Returns a local `/api/instances/{slug}/uploads/{id}/file` URL.
    async fn download_and_upload(&self, url: &str) -> Result<String, ToolExecError> {
        use super::media::{is_youtube_url, download_youtube, MediaType};

        let local_path = if is_youtube_url(url) {
            log::info!("[play_music] downloading YouTube audio: {url}");
            download_youtube(url, MediaType::Audio).await?
        } else {
            // Direct URL — download with reqwest
            log::info!("[play_music] downloading audio: {url}");
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|e| ToolExecError(format!("HTTP client error: {e}")))?;
            let response = client.get(url).send().await
                .map_err(|e| ToolExecError(format!("download failed: {e}")))?;
            if !response.status().is_success() {
                return Err(ToolExecError(format!("download failed: HTTP {}", response.status())));
            }
            let bytes = response.bytes().await
                .map_err(|e| ToolExecError(format!("failed to read audio: {e}")))?;

            let ext = url.rsplit('.').next()
                .filter(|e| ["mp3", "m4a", "ogg", "wav", "flac", "aac", "opus"].contains(e))
                .unwrap_or("mp3");
            let tmp = format!("/tmp/play_music_{}.{ext}", std::process::id());
            std::fs::write(&tmp, &bytes)
                .map_err(|e| ToolExecError(format!("failed to write temp file: {e}")))?;
            tmp
        };

        // Save to uploads
        let bytes = std::fs::read(&local_path)
            .map_err(|e| ToolExecError(format!("failed to read downloaded file: {e}")))?;
        let _ = std::fs::remove_file(&local_path);

        let ext = std::path::Path::new(&local_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp3");
        let filename = format!("music.{ext}");

        let meta = crate::services::uploads::save_upload(
            &self.workspace_dir, &self.instance_slug, &filename, &bytes,
        ).map_err(|e| ToolExecError(format!("failed to save upload: {e}")))?;

        let local_url = format!(
            "/api/instances/{}/uploads/{}/file",
            self.instance_slug, meta.id
        );
        log::info!("[play_music] audio saved → {local_url} ({:.1} MB)", bytes.len() as f64 / 1024.0 / 1024.0);
        Ok(local_url)
    }
}

// ---------------------------------------------------------------------------
// speak — synthesize speech with a specific ElevenLabs voice ID
// ---------------------------------------------------------------------------

pub struct SpeakTool {
    instance_slug: String,
    chat_id: String,
    workspace_dir: PathBuf,
    events: broadcast::Sender<ServerEvent>,
}

impl SpeakTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        chat_id: &str,
        events: broadcast::Sender<ServerEvent>,
        _config_path: &Path,
    ) -> Self {
        Self {
            instance_slug: instance_slug.to_string(),
            chat_id: chat_id.to_string(),
            workspace_dir: workspace_dir.to_path_buf(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SpeakArgs {
    /// The text to speak aloud.
    pub text: String,
    /// ElevenLabs voice ID to use for this phrase. If omitted, uses the instance default.
    pub voice_id: Option<String>,
}

impl Tool for SpeakTool {
    const NAME: &'static str = "speak";
    type Error = ToolExecError;
    type Args = SpeakArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "speak".into(),
            description: "Speak a phrase aloud using text-to-speech. \
                Optionally specify a voice_id to use a different ElevenLabs voice \
                for this specific phrase (useful for character voices, impressions, etc.). \
                The audio plays immediately in the user's browser."
                .into(),
            parameters: openai_schema::<SpeakArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if args.text.trim().is_empty() {
            return Err(ToolExecError("text cannot be empty".into()));
        }

        let api_key = {
            let cfg = crate::config::load_config()
                .map_err(|e| ToolExecError(format!("failed to load config: {e}")))?;
            cfg.llm.tokens.elevenlabs.clone()
        };
        if api_key.is_empty() {
            return Err(ToolExecError("ElevenLabs API key not configured".into()));
        }

        let voice_id = match &args.voice_id {
            Some(vid) if !vid.is_empty() => vid.clone(),
            _ => crate::routes::tts::resolve_voice_id(&self.workspace_dir, &self.instance_slug),
        };

        let instance_dir = self.workspace_dir.join("instances").join(&self.instance_slug);
        let mood = load_mood_state(&instance_dir).companion_mood;

        let http = reqwest::Client::new();
        let audio_bytes = crate::routes::tts::synthesize_bytes(&http, &api_key, &voice_id, &args.text, &mood)
            .await
            .map_err(|e| ToolExecError(e))?;

        use base64::Engine;
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);

        // Generate a message ID for this spoken phrase
        let msg_id = format!("speak_{}_{}", super::tool_call_counter(), super::unix_millis());

        let _ = self.events.send(ServerEvent::ChatAudioReady {
            instance_slug: self.instance_slug.clone(),
            chat_id: self.chat_id.clone(),
            audio_base64,
            message_ids: vec![msg_id],
        });

        Ok(format!("spoke: \"{}\" (voice: {})", truncate_text(&args.text, 60), voice_id))
    }
}

fn truncate_text(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}

// ---------------------------------------------------------------------------
// edit_soul
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
            description: "Rewrite your soul.md (personality/voice definition). Full markdown content.".into(),
            parameters: openai_schema::<EditSoulArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = self.soul_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }
        fs::write(&self.soul_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(
            "soul.md updated. your personality will reflect these changes on the next message."
                .into(),
        )
    }
}

