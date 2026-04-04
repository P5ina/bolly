use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::services::machine_registry::{AgentToolCall, MachineRegistry};
use crate::services::tool::{Tool, ToolDefinition};
use super::{openai_schema, ToolExecError};

pub const SCREEN_RECORDING_PATH: &str = "/tmp/bolly_screen.mp4";

// ═══════════════════════════════════════════════════════════════════════════
// Observations — saved screen recording + analysis
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Serialize, Deserialize, Clone)]
pub struct ScreenObservation {
    pub id: String,
    pub upload_id: String,
    pub machine_id: String,
    pub analysis: String,
    pub created_at: String,
}

pub fn save_observation(workspace_dir: &Path, slug: &str, obs: &ScreenObservation) {
    let dir = workspace_dir.join("instances").join(slug).join("observations");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.json", obs.id));
    if let Ok(json) = serde_json::to_string_pretty(obs) {
        let _ = fs::write(path, json);
    }
}

pub fn list_observations(workspace_dir: &Path, slug: &str) -> Vec<ScreenObservation> {
    let dir = workspace_dir.join("instances").join(slug).join("observations");
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut obs: Vec<ScreenObservation> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .filter_map(|e| {
            let content = fs::read_to_string(e.path()).ok()?;
            serde_json::from_str(&content).ok()
        })
        .collect();
    obs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    obs
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_millis()
}

/// Start screen recording on a specific machine.
pub async fn start_recording_on_machine(registry: &MachineRegistry, machine_id: &str, os: &str) {
    let record_cmd = if os.to_lowercase().contains("mac") || os.to_lowercase().contains("darwin") {
        format!(
            "nohup ffmpeg -f avfoundation -capture_cursor 1 -framerate 1 \
             -i \"Capture screen 0:none\" \
             -t 960 -c:v libx264 -preset ultrafast -crf 35 \
             -pix_fmt yuv420p -an -y {} > /dev/null 2>&1 &",
            SCREEN_RECORDING_PATH
        )
    } else {
        format!(
            "nohup ffmpeg -f x11grab -framerate 1 -i :0.0 \
             -t 960 -c:v libx264 -preset ultrafast -crf 35 \
             -pix_fmt yuv420p -an -y {} > /dev/null 2>&1 &",
            SCREEN_RECORDING_PATH
        )
    };

    let start_cmd = AgentToolCall {
        request_id: uuid::Uuid::new_v4().to_string(),
        action: "bash".into(),
        params: serde_json::json!({ "command": record_cmd }),
    };
    match registry.execute(machine_id, start_cmd).await {
        Ok(_) => log::info!("[screen] started recording on '{}'", machine_id),
        Err(e) => log::warn!("[screen] failed to start recording: {e}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// collect_screen_recording — stop, upload, restart
// ═══════════════════════════════════════════════════════════════════════════

pub struct CollectScreenRecordingTool {
    registry: MachineRegistry,
    workspace_dir: PathBuf,
    instance_slug: String,
    public_url: String,
    auth_token: String,
}

impl CollectScreenRecordingTool {
    pub fn new(
        registry: MachineRegistry,
        workspace_dir: &std::path::Path,
        instance_slug: &str,
        public_url: &str,
        auth_token: &str,
    ) -> Self {
        Self {
            registry,
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            public_url: public_url.to_string(),
            auth_token: auth_token.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CollectScreenRecordingArgs {}

impl Tool for CollectScreenRecordingTool {
    const NAME: &'static str = "collect_screen_recording";
    type Error = ToolExecError;
    type Args = CollectScreenRecordingArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "collect_screen_recording".into(),
            description: "Stop the current screen recording on the user's desktop, \
                upload the video to the server, and start a new recording. \
                Returns the upload_id and URL of the recorded video. \
                Use watch_video on the returned URL to analyze what the user was doing."
                .into(),
            parameters: openai_schema::<CollectScreenRecordingArgs>(),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let machines = self.registry.list().await;
        let machine = machines.iter().find(|m| m.screen_recording_allowed)
            .ok_or_else(|| ToolExecError("no desktop with screen recording enabled".into()))?;
        let machine_id = machine.machine_id.clone();
        let machine_os = machine.os.clone();

        // 1. Stop ffmpeg
        let stop_cmd = AgentToolCall {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: "bash".into(),
            params: serde_json::json!({
                "command": "pkill -f 'ffmpeg.*bolly_screen' 2>/dev/null; sleep 2; echo stopped"
            }),
        };
        self.registry.execute(&machine_id, stop_cmd).await
            .map_err(|e| ToolExecError(format!("failed to stop recording: {e}")))?;

        // 2. Desktop uploads the file to server
        let upload_url = format!("{}/api/instances/{}/uploads", self.public_url, self.instance_slug);
        let upload_cmd = AgentToolCall {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: "upload_file".into(),
            params: serde_json::json!({
                "path": SCREEN_RECORDING_PATH,
                "upload_url": upload_url,
                "auth_token": self.auth_token,
            }),
        };

        let upload_result = self.registry.execute(&machine_id, upload_cmd).await
            .map_err(|e| ToolExecError(format!("failed to upload recording: {e}")))?;

        let upload_id = upload_result.error.clone().unwrap_or_default();

        // 3. Clean up on desktop
        let cleanup = AgentToolCall {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: "bash".into(),
            params: serde_json::json!({ "command": format!("rm -f {SCREEN_RECORDING_PATH}") }),
        };
        let _ = self.registry.execute(&machine_id, cleanup).await;

        // 4. Start new recording
        start_recording_on_machine(&self.registry, &machine_id, &machine_os).await;

        if upload_id.is_empty() || !upload_id.starts_with("upload_") {
            return Err(ToolExecError(format!("upload failed or no recording found (got: {upload_id})")));
        }

        // Build the video URL for the agent to use with watch_video
        let video_url = format!(
            "/api/instances/{}/uploads/{}/file",
            self.instance_slug, upload_id
        );

        // Save observation stub (analysis will be added after watch_video)
        let obs = ScreenObservation {
            id: format!("obs_{}", unix_millis()),
            upload_id: upload_id.clone(),
            machine_id: machine_id.clone(),
            analysis: String::new(), // filled by agent after watch_video
            created_at: unix_millis().to_string(),
        };
        save_observation(&self.workspace_dir, &self.instance_slug, &obs);

        log::info!("[screen] collected recording from '{}': {}", machine_id, upload_id);

        Ok(format!(
            "Recording collected and uploaded.\n\
             upload_id: {upload_id}\n\
             video_url: {video_url}\n\
             observation_id: {}\n\n\
             Now use watch_video with the local file path to analyze what the user was doing.\n\
             File path: {}/instances/{}/uploads/{}_blob.mp4",
            obs.id,
            self.workspace_dir.display(), self.instance_slug,
            upload_id.trim_end_matches(".mp4")
        ))
    }
}
