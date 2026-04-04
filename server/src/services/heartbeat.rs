//! Heartbeat — independent loops for each child agent.
//!
//! Each scheduled agent gets its own tokio task running on its own interval.
//! No triage — agents wake up on their own schedule and run directly.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use tokio::sync::{broadcast, RwLock};

use crate::domain::events::ServerEvent;
use crate::domain::thought::Thought;
use crate::services::{chat, llm::LlmBackend, rhythm, thoughts};
use crate::services::tools::load_mood_state;
use crate::services::machine_registry::{AgentToolCall, MachineRegistry};
use crate::domain::child_agent::ChildAgentConfig;


pub fn start(
    workspace_dir: &Path,
    llm: Arc<RwLock<Option<LlmBackend>>>,
    events: broadcast::Sender<ServerEvent>,
    vector_store: Arc<crate::services::vector::VectorStore>,
    google_ai_key: String,
    machine_registry: MachineRegistry,
) {
    let instances_dir = workspace_dir.join("instances");

    // Ensure built-in child agents exist for all instances
    if let Ok(entries) = fs::read_dir(&instances_dir) {
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_dir() && entry.path().join("soul.md").exists() {
                let slug = entry.file_name().to_string_lossy().to_string();
                crate::services::child_agents::ensure_builtins(workspace_dir, &slug);
            }
        }
    }

    // Spawn independent loops for each instance × agent
    let entries = match fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        if !entry.path().is_dir() || !entry.path().join("soul.md").exists() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        let agents = crate::services::child_agents::load_agents(workspace_dir, &slug);

        for agent in agents {
            if agent.interval_hours <= 0.0 || !agent.enabled {
                continue; // skip on-demand agents
            }

            let ws = workspace_dir.to_path_buf();
            let s = slug.clone();
            let l = llm.clone();
            let ev = events.clone();
            let vs = vector_store.clone();
            let gai = google_ai_key.clone();
            let mr = machine_registry.clone();
            let agent_name = agent.name.clone();
            let agent_hours = agent.interval_hours;
            let agent_clone = agent.clone();

            tokio::spawn(async move {
                run_agent_loop(&ws, &s, &agent_clone, l, ev, vs, &gai, mr).await;
            });

            log::info!(
                "heartbeat: spawned '{agent_name}' for instance '{slug}' (every {agent_hours}h)"
            );
        }
    }
}

/// Independent loop for a single child agent.
async fn run_agent_loop(
    workspace_dir: &Path,
    slug: &str,
    agent: &ChildAgentConfig,
    llm: Arc<RwLock<Option<LlmBackend>>>,
    events: broadcast::Sender<ServerEvent>,
    vector_store: Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
    machine_registry: MachineRegistry,
) {
    let interval_secs = (agent.interval_hours * 3600.0) as u64;
    let instance_dir = workspace_dir.join("instances").join(slug);

    // Initial delay: wait until the agent is due
    let marker_path = workspace_dir
        .join("instances").join(slug).join("agents")
        .join(format!(".last_run_{}", agent.name));
    let last_run: i64 = fs::read_to_string(&marker_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let now = Utc::now().timestamp();
    let elapsed = (now - last_run).max(0) as u64;
    if elapsed < interval_secs {
        let wait = interval_secs - elapsed;
        log::info!(
            "[heartbeat] {slug}/{}: next run in {}m",
            agent.name, wait / 60
        );
        tokio::time::sleep(Duration::from_secs(wait)).await;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    interval.tick().await; // first tick is immediate

    loop {
        // Check that soul.md still exists (instance might have been deleted)
        if !instance_dir.join("soul.md").exists() {
            log::info!("[heartbeat] {slug}/{}: soul.md gone, stopping", agent.name);
            break;
        }

        let llm_guard = llm.read().await;
        if let Some(backend) = llm_guard.as_ref() {
            run_agent_tick(
                workspace_dir, slug, &instance_dir, backend, &events,
                &vector_store, google_ai_key, agent, &machine_registry,
            ).await;
        }
        drop(llm_guard);

        interval.tick().await;
    }
}

/// Single tick of an agent's heartbeat.
async fn run_agent_tick(
    workspace_dir: &Path,
    slug: &str,
    instance_dir: &Path,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
    vector_store: &Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
    agent: &ChildAgentConfig,
    machine_registry: &MachineRegistry,
) {
    log::info!("[heartbeat] {slug}: running '{}'", agent.name);

    // ── Screen recording (observer agent) ──
    if agent.name == "observer" {
        let inst_cfg = crate::config::InstanceConfig::load(workspace_dir, slug);
        if inst_cfg.screen_recording {
            if let Some(r) = manage_screen_recording(machine_registry, google_ai_key, workspace_dir, slug).await {
                save_observation(workspace_dir, slug, &ScreenObservation {
                    id: format!("obs_{}", unix_millis()),
                    upload_id: r.upload_id.clone(),
                    machine_id: r.machine_id.clone(),
                    analysis: r.analysis.clone(),
                    created_at: unix_millis().to_string(),
                });
                // Save analysis as system message so the agent sees it
                let _ = chat::save_system_message(
                    workspace_dir, slug, "default",
                    &format!("[system] screen observation:\n{}", r.analysis),
                );
            }
        }
    }

    // ── Rhythm update (companion only) ──
    if agent.name == "companion" {
        let rhythm_data = rhythm::recompute_rhythm(workspace_dir, slug);
        rhythm::save_rhythm(instance_dir, &rhythm_data);
        let rhythm_insights = rhythm::build_rhythm_insights(workspace_dir, slug, &rhythm_data);
        if !rhythm_insights.trim().is_empty() {
            let label = format!("[system] rhythm update\n{rhythm_insights}");
            let _ = chat::save_system_message(workspace_dir, slug, "default", &label);
        }
    }

    // Run the agent
    match crate::services::child_agents::run_single_agent(
        workspace_dir, slug, instance_dir, llm, events, vector_store, google_ai_key,
        agent, None, "heartbeat", Some(machine_registry),
    ).await {
        Ok(r) => {
            // Mark as run
            let marker = workspace_dir
                .join("instances").join(slug).join("agents")
                .join(format!(".last_run_{}", agent.name));
            let _ = fs::write(&marker, Utc::now().timestamp().to_string());

            let _ = chat::save_system_message(
                workspace_dir, slug, "default",
                &format!("[system] child agent '{}' ran ({} tokens)", agent.name, r.tokens),
            );

            log::info!("[heartbeat] {slug}/{}: done ({} tokens)", agent.name, r.tokens);

            // Save thought with the agent's actual inner monologue
            let final_mood = load_mood_state(instance_dir).companion_mood;
            let thought = Thought {
                id: format!("thought_{}", unix_millis()),
                raw: r.response,
                actions: vec![format!("wake:{}", agent.name)],
                mood: final_mood,
                created_at: unix_millis().to_string(),
            };
            let _ = thoughts::save_thought(workspace_dir, slug, &thought);
            let _ = events.send(ServerEvent::HeartbeatThought {
                instance_slug: slug.to_string(),
                thought,
            });
        }
        Err(e) => {
            log::warn!("[heartbeat] {slug}/{}: failed: {e}", agent.name);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Screen recording management
// ═══════════════════════════════════════════════════════════════════════════

const SCREEN_RECORDING_PATH: &str = "/tmp/bolly_screen.mp4";

/// A saved screen observation — video + analysis.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ScreenObservation {
    pub id: String,
    pub upload_id: String,
    pub machine_id: String,
    pub analysis: String,
    pub created_at: String,
}

fn save_observation(workspace_dir: &Path, slug: &str, obs: &ScreenObservation) {
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

/// Result of processing a screen recording.
struct ScreenRecordingResult {
    analysis: String,
    upload_id: String,
    machine_id: String,
}

/// Stop previous recording, upload from desktop, analyze, start new one.
async fn manage_screen_recording(
    registry: &MachineRegistry,
    google_ai_key: &str,
    workspace_dir: &Path,
    slug: &str,
) -> Option<ScreenRecordingResult> {
    if google_ai_key.is_empty() {
        return None;
    }

    let machines = registry.list().await;
    let machine = match machines.iter().find(|m| m.screen_recording_allowed) {
        Some(m) => m,
        None => return None,
    };
    let machine_id = &machine.machine_id;

    // 1. Stop any existing recording
    let stop_cmd = AgentToolCall {
        request_id: uuid::Uuid::new_v4().to_string(),
        action: "bash".into(),
        params: serde_json::json!({
            "command": "pkill -f 'ffmpeg.*bolly_screen' 2>/dev/null; sleep 2; echo stopped"
        }),
    };
    if let Err(e) = registry.execute(machine_id, stop_cmd).await {
        log::warn!("[heartbeat] failed to stop screen recording: {e}");
    }

    // 2. Upload recording from desktop to server
    let result = {
        let cfg = crate::config::load_config().ok();
        let public_url = cfg.as_ref().map(|c| c.public_url.as_str()).unwrap_or("");
        let auth_token = cfg.as_ref().map(|c| c.auth_token.as_str()).unwrap_or("");
        let upload_url = format!("{}/api/instances/{}/uploads", public_url, slug);

        let upload_cmd = AgentToolCall {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: "upload_file".into(),
            params: serde_json::json!({
                "path": SCREEN_RECORDING_PATH,
                "upload_url": upload_url,
                "auth_token": auth_token,
            }),
        };

        let upload_result = registry.execute(machine_id, upload_cmd).await;

        // Clean up on desktop
        let cleanup = AgentToolCall {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: "bash".into(),
            params: serde_json::json!({ "command": format!("rm -f {}", SCREEN_RECORDING_PATH) }),
        };
        let _ = registry.execute(machine_id, cleanup).await;

        match upload_result {
            Ok(action_result) => {
                let upload_id = action_result.error.clone().unwrap_or_default();
                if upload_id.is_empty() || !upload_id.starts_with("upload_") {
                    log::warn!("[heartbeat] upload returned unexpected id: {upload_id:?}");
                    None
                } else {
                    log::info!("[heartbeat] recording uploaded from '{}': {}", machine_id, upload_id);

                    let file_path = workspace_dir
                        .join("instances").join(slug)
                        .join("uploads")
                        .join(format!("{}_blob.mp4", upload_id.trim_end_matches(".mp4")));

                    let analysis = if file_path.exists() {
                        match analyze_screen_recording(&file_path.display().to_string(), google_ai_key).await {
                            Ok(text) => text,
                            Err(e) => { log::warn!("[heartbeat] screen analysis failed: {e}"); "(analysis failed)".into() }
                        }
                    } else {
                        log::warn!("[heartbeat] uploaded file not found at {}", file_path.display());
                        "(uploaded but file not found for analysis)".into()
                    };

                    Some(ScreenRecordingResult {
                        analysis,
                        upload_id,
                        machine_id: machine_id.clone(),
                    })
                }
            }
            Err(e) => {
                log::warn!("[heartbeat] desktop upload failed: {e}");
                None
            }
        }
    };

    // 3. Start new recording
    start_recording_on_machine(registry, machine_id, &machine.os).await;

    result
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
        Ok(_) => log::info!("[heartbeat] started screen recording on '{}'", machine_id),
        Err(e) => log::warn!("[heartbeat] failed to start screen recording: {e}"),
    }
}

/// Analyze a screen recording with Gemini.
async fn analyze_screen_recording(path: &str, google_ai_key: &str) -> anyhow::Result<String> {
    use crate::services::tools::media::{analyze_with_gemini, MediaType};

    let prompt = "You are observing a screen recording of a user's computer over the last 15 minutes \
        (captured at 1 fps). Describe what the user was doing:\n\
        - What applications/websites were they using?\n\
        - What were they working on? (coding, writing, browsing, etc.)\n\
        - Any notable content visible on screen?\n\
        - What might they need help with?\n\n\
        Be concise but specific. Focus on actionable observations.";

    let result = analyze_with_gemini(google_ai_key, path, prompt, MediaType::Video).await
        .map_err(|e| anyhow::anyhow!("{}", e.0))?;

    Ok(result)
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
