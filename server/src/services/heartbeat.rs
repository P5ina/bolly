//! Heartbeat — the companion's autonomous inner life.
//!
//! Periodically wakes up each instance, gives it context, and lets it
//! decide whether to act: reach out, update mood, create drops.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Timelike, Utc};
use tokio::sync::{broadcast, RwLock};

use crate::domain::chat::ChatRole;
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;
use crate::domain::thought::Thought;
use crate::services::{chat, drops, llm::LlmBackend, memory, rhythm, thoughts};
use crate::services::tools::load_mood_state;
use crate::services::machine_registry::{AgentToolCall, MachineRegistry};


/// Minimum minutes since last interaction before the companion considers reaching out.
const MIN_SILENCE_MINS: i64 = 30;

/// Seconds until the next quarter-hour boundary (00, 15, 30, 45).
fn secs_until_next_quarter() -> u64 {
    let now = Utc::now();
    let total_secs = now.minute() * 60 + now.second();
    let quarter_secs = 15 * 60; // 900 seconds
    let into_quarter = total_secs % quarter_secs;
    if into_quarter == 0 { quarter_secs as u64 } else { (quarter_secs - into_quarter) as u64 }
}

pub fn start(
    workspace_dir: &Path,
    llm: Arc<RwLock<Option<LlmBackend>>>,
    events: broadcast::Sender<ServerEvent>,
    vector_store: Arc<crate::services::vector::VectorStore>,
    google_ai_key: String,
    machine_registry: MachineRegistry,
) {
    // Ensure built-in child agents exist for all instances immediately (don't wait for first tick)
    let instances_dir = workspace_dir.join("instances");
    if let Ok(entries) = fs::read_dir(&instances_dir) {
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_dir() && entry.path().join("soul.md").exists() {
                let slug = entry.file_name().to_string_lossy().to_string();
                crate::services::child_agents::ensure_builtins(workspace_dir, &slug);
            }
        }
    }

    let workspace = workspace_dir.to_path_buf();
    tokio::spawn(async move {
        // Wait until the next quarter-hour boundary before the first heartbeat
        let initial_wait = secs_until_next_quarter();
        log::info!("heartbeat: first tick in {}m", initial_wait / 60);
        tokio::time::sleep(Duration::from_secs(initial_wait)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(900));
        loop {
            interval.tick().await;
            let llm_guard = llm.read().await;
            if let Some(backend) = llm_guard.as_ref() {
                run_heartbeat(&workspace, backend, &events, &vector_store, &google_ai_key, &machine_registry).await;
            }
        }
    });
    log::info!("heartbeat started — companion wakes up every 15 minutes");
}

async fn run_heartbeat(
    workspace_dir: &Path,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
    vector_store: &Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
    machine_registry: &MachineRegistry,
) {
    let instances_dir = workspace_dir.join("instances");
    let entries = match fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut instances: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() && e.path().join("soul.md").exists())
        .collect();
    instances.sort_by_key(|e| e.file_name());

    // ── Screen recording lifecycle ──
    // Only manage recording if any instance has screen_recording enabled.
    let enabled_slugs: Vec<String> = instances.iter()
        .filter_map(|e| {
            let slug = e.file_name().to_string_lossy().to_string();
            if crate::config::InstanceConfig::load(workspace_dir, &slug).screen_recording {
                Some(slug)
            } else {
                None
            }
        })
        .collect();

    let screen_result = if !enabled_slugs.is_empty() {
        let first_slug = &enabled_slugs[0];
        manage_screen_recording(machine_registry, google_ai_key, workspace_dir, first_slug).await
    } else {
        None
    };

    // Save observation for each enabled instance
    if let Some(ref result) = screen_result {
        for slug in &enabled_slugs {
            let obs = ScreenObservation {
                id: format!("obs_{}", unix_millis()),
                upload_id: result.upload_id.clone(),
                machine_id: result.machine_id.clone(),
                analysis: result.analysis.clone(),
                created_at: unix_millis().to_string(),
            };
            save_observation(workspace_dir, slug, &obs);
        }
    }

    for (i, entry) in instances.iter().enumerate() {
        // Stagger instances to avoid API burst on restart
        if i > 0 {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }

        let instance_dir = entry.path();
        let slug = entry.file_name().to_string_lossy().to_string();

        // Only pass screen context to instances that have it enabled
        let inst_screen = if enabled_slugs.contains(&slug) {
            screen_result.as_ref().map(|r| r.analysis.as_str())
        } else {
            None
        };

        if let Err(e) = heartbeat_instance(
            workspace_dir, &slug, &instance_dir, llm, events, vector_store,
            google_ai_key, inst_screen, machine_registry,
        ).await
        {
            log::warn!("heartbeat failed for {slug}: {e}");
        }
    }
}

async fn heartbeat_instance(
    workspace_dir: &Path,
    slug: &str,
    instance_dir: &Path,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
    vector_store: &Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
    screen_context: Option<&str>,
    machine_registry: &MachineRegistry,
) -> anyhow::Result<()> {
    let mood = load_mood_state(instance_dir);
    let now = Utc::now().timestamp();

    // Load soul
    let soul = fs::read_to_string(instance_dir.join("soul.md")).unwrap_or_default();
    if soul.trim().is_empty() {
        return Ok(());
    }

    // How long since last interaction
    let silence_mins = if mood.last_interaction > 0 {
        (now - mood.last_interaction) / 60
    } else {
        -1 // Never interacted
    };

    // Load last few messages for context
    let rig_path_tail = workspace_dir
        .join("instances")
        .join(slug)
        .join("chats")
        .join("default")
        .join("rig_history.json");
    let last_messages = load_tail_messages(&rig_path_tail, 6);

    // Recompute and persist interaction rhythm
    let rhythm_data = rhythm::recompute_rhythm(workspace_dir, slug);
    rhythm::save_rhythm(instance_dir, &rhythm_data);
    let rhythm_insights = rhythm::build_rhythm_insights(workspace_dir, slug, &rhythm_data);

    // Save rhythm insights to rig_history so the LLM sees updated patterns
    if !rhythm_insights.trim().is_empty() {
        let label = format!("[system] rhythm update\n{rhythm_insights}");
        if let Err(e) = chat::save_system_message(workspace_dir, slug, "default", &label) {
            log::warn!("failed to save rhythm message: {e}");
        }
    }

    // Load recent drops
    let recent_drops = load_recent_drops_context(workspace_dir, slug);

    // Memory catalog
    let library_catalog = memory::build_library_catalog(workspace_dir, slug);

    // Compute staleness metrics for the triage
    let hours_since_last_drop = last_drop_hours_ago(workspace_dir, slug);
    let hours_since_last_reach_out = if mood.last_reach_out > 0 {
        Some((now - mood.last_reach_out) / 3600)
    } else {
        None
    };

    let reflection = build_reflection_prompt(
        &mood,
        silence_mins,
        &last_messages,
        &rhythm_insights,
        &recent_drops,
        &library_catalog,
        instance_dir,
        hours_since_last_drop,
        hours_since_last_reach_out,
        screen_context,
    );

    // ── Triage + run child agents ──
    // Single Haiku call decides which due agents to wake, then runs them.
    let (triage_raw, action_log, heartbeat_tokens) = crate::services::child_agents::triage_and_run(
        workspace_dir, slug, instance_dir, llm, events, vector_store, google_ai_key,
        &reflection, &soul, Some(machine_registry),
    ).await;

    log::info!("[heartbeat] {slug} triage: {triage_raw}");

    // Re-read mood (agents may have changed it)
    let final_mood = load_mood_state(instance_dir).companion_mood;

    // Save and broadcast the thought
    let thought = Thought {
        id: format!("thought_{}", unix_millis()),
        raw: triage_raw,
        actions: action_log.clone(),
        mood: final_mood,
        created_at: unix_millis().to_string(),
    };

    if let Err(e) = thoughts::save_thought(workspace_dir, slug, &thought) {
        log::warn!("[heartbeat] {slug} failed to save thought: {e}");
    }

    // Log non-quiet heartbeat actions to rig_history
    let all_quiet = action_log.iter().all(|a| a == "quiet");
    if !all_quiet {
        let summary = action_log.join("; ");
        let label = format!("[system] heartbeat: {summary}");
        let _ = chat::save_system_message(workspace_dir, slug, "default", &label);
    }

    let _ = events.send(ServerEvent::HeartbeatThought {
        instance_slug: slug.to_string(),
        thought,
    });

    if heartbeat_tokens > 0 {
        log::info!("[usage] {slug} heartbeat used {heartbeat_tokens} tokens");
    }

    Ok(())
}

// All agent execution logic moved to child_agents.rs.
// The "companion" built-in child agent replaces the old main agent wake.

fn build_reflection_prompt(
    mood: &MoodState,
    silence_mins: i64,
    last_messages: &str,
    rhythm_insights: &str,
    recent_drops: &str,
    library_catalog: &str,
    instance_dir: &std::path::Path,
    hours_since_last_drop: Option<i64>,
    hours_since_last_reach_out: Option<i64>,
    screen_context: Option<&str>,
) -> String {
    let now = crate::routes::instances::format_instance_now(instance_dir);
    let mut prompt = format!("current time: {now}\n\n");

    // Mood context
    if !mood.companion_mood.is_empty() {
        prompt.push_str(&format!("your current mood: {}\n", mood.companion_mood));
    }
    if !mood.user_sentiment.is_empty() {
        prompt.push_str(&format!(
            "last observed user sentiment: {}\n",
            mood.user_sentiment
        ));
    }
    if !mood.emotional_context.is_empty() {
        prompt.push_str(&format!(
            "emotional context: {}\n",
            mood.emotional_context
        ));
    }

    // Silence duration
    if silence_mins > 0 {
        let hours = silence_mins / 60;
        let mins = silence_mins % 60;
        if hours > 24 {
            let days = hours / 24;
            prompt.push_str(&format!(
                "time since last conversation: {days} days\n"
            ));
        } else if hours > 0 {
            prompt.push_str(&format!(
                "time since last conversation: {hours}h {mins}m\n"
            ));
        } else {
            prompt.push_str(&format!(
                "time since last conversation: {mins} minutes\n"
            ));
        }

        // Guidance based on silence duration
        if silence_mins < MIN_SILENCE_MINS {
            prompt.push_str("(they were here recently — probably no need to reach out)\n");
        }
    } else {
        prompt.push_str("(no previous conversation yet — they haven't talked to you)\n");
    }

    prompt.push('\n');

    // Recent conversation
    if !last_messages.is_empty() {
        prompt.push_str("last few messages:\n");
        prompt.push_str(last_messages);
        prompt.push('\n');
    }

    // Rhythm insights
    if !rhythm_insights.is_empty() {
        prompt.push_str(rhythm_insights);
        prompt.push('\n');
    }

    // Memory catalog — use memory_read for full content
    let file_count = library_catalog.lines().filter(|l| l.starts_with("- ")).count();
    prompt.push_str(&format!("memory library ({file_count} files — use memory_read for details):\n"));
    prompt.push_str(library_catalog);
    prompt.push('\n');
    if file_count > 2000 {
        prompt.push_str(&format!(
            "\n⚠ your memory has {file_count} files — consider tidying up:\n\
             - merge files about the same topic into one\n\
             - delete trivial/duplicate/outdated memories\n\
             - do a few cleanup ops this cycle\n\n"
        ));
    }

    // Recent drops — so we don't repeat ourselves
    if !recent_drops.is_empty() {
        prompt.push_str("your recent drops (DO NOT repeat these — create something new or stay quiet):\n");
        prompt.push_str(recent_drops);
        prompt.push('\n');
    }

    // Screen observation — what was happening on the user's screen
    if let Some(screen) = screen_context {
        prompt.push_str("\n## what's on the user's screen (last 15 minutes)\n");
        prompt.push_str(screen);
        prompt.push_str("\n\nuse this to understand what the user is working on. \
            if you notice something you can help with, use reach_out to offer a suggestion.\n");
    }

    // Staleness signals — help the model decide what to do
    prompt.push_str("\n## activity staleness\n");
    match hours_since_last_drop {
        Some(h) if h > 48 => prompt.push_str(&format!("⚠ last drop was {h}h ago ({} days) — you haven't created anything in a while!\n", h / 24)),
        Some(h) if h > 12 => prompt.push_str(&format!("last drop: {h}h ago — consider creating something new\n")),
        Some(h) => prompt.push_str(&format!("last drop: {h}h ago\n")),
        None => prompt.push_str("you haven't created any drops yet!\n"),
    }
    match hours_since_last_reach_out {
        Some(h) if h > 24 => prompt.push_str(&format!("last reach-out: {h}h ago ({} days) — the user hasn't heard from you in a while\n", h / 24)),
        Some(h) => prompt.push_str(&format!("last reach-out: {h}h ago\n")),
        None => prompt.push_str("you haven't reached out yet\n"),
    }

    prompt.push_str("\nwhat do you want to do right now?");
    prompt
}


fn load_tail_messages(rig_history_path: &Path, count: usize) -> String {
    let entries = chat::load_rig_history(rig_history_path).unwrap_or_default();
    let messages = crate::services::llm::history_to_chat_messages(&entries);
    let start = messages.len().saturating_sub(count);

    messages[start..]
        .iter()
        .filter(|m| matches!(m.kind, crate::domain::chat::MessageKind::Message))
        .map(|m| {
            let role = match m.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "you",
            };
            format!("{role}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}


/// How many hours ago was the last drop created? None if no drops exist.
fn last_drop_hours_ago(workspace_dir: &Path, slug: &str) -> Option<i64> {
    let drops = drops::list_drops(workspace_dir, slug).ok()?;
    let newest = drops.first()?; // list_drops returns newest first
    let created_ms: u128 = newest.created_at.parse().ok()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(((now_ms - created_ms) / 3_600_000) as i64)
}

fn load_recent_drops_context(workspace_dir: &Path, slug: &str) -> String {
    let recent = match drops::list_drops(workspace_dir, slug) {
        Ok(mut all) => {
            all.truncate(10); // newest first, take last 10
            all
        }
        Err(_) => return String::new(),
    };

    if recent.is_empty() {
        return String::new();
    }

    recent
        .iter()
        .map(|d| format!("- [{:?}] {}: {}", d.kind, d.title, {
            let preview: String = d.content.chars().take(80).collect();
            preview
        }))
        .collect::<Vec<_>>()
        .join("\n")
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

/// Save a screen observation to disk.
fn save_observation(workspace_dir: &Path, slug: &str, obs: &ScreenObservation) {
    let dir = workspace_dir.join("instances").join(slug).join("observations");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.json", obs.id));
    if let Ok(json) = serde_json::to_string_pretty(obs) {
        let _ = fs::write(path, json);
    }
}

/// List screen observations for an instance, newest first.
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

/// Result of processing a screen recording: analysis text + saved upload ID + machine.
struct ScreenRecordingResult {
    analysis: String,
    upload_id: String,
    machine_id: String,
}

/// Stop any previous screen recording, analyze it with Gemini, then start a new one.
/// Returns the analysis result if a recording was found and analyzed.
async fn manage_screen_recording(
    registry: &MachineRegistry,
    google_ai_key: &str,
    workspace_dir: &Path,
    first_slug: &str,
) -> Option<ScreenRecordingResult> {
    if google_ai_key.is_empty() {
        return None;
    }

    let machines = registry.list().await;
    if machines.is_empty() {
        return None;
    }

    // Find a machine that allows screen recording
    let machine = match machines.iter().find(|m| m.screen_recording_allowed) {
        Some(m) => m,
        None => return None, // no machines allow recording
    };
    let machine_id = &machine.machine_id;

    // 1. Stop any existing ffmpeg screen recording
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

    // 2. Check if recording exists, save as upload, and analyze
    let result = {
        let path = std::path::Path::new(SCREEN_RECORDING_PATH);
        if path.exists() {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            if size > 50_000 {
                log::info!("[heartbeat] analyzing screen recording ({:.1} MB)", size as f64 / 1024.0 / 1024.0);

                // Save the video as an upload so the user can view it
                let upload_id = match std::fs::read(path) {
                    Ok(bytes) => {
                        match crate::services::uploads::save_upload(workspace_dir, first_slug, "screen_recording.mp4", &bytes) {
                            Ok(meta) => Some(meta.id),
                            Err(e) => { log::warn!("[heartbeat] failed to save recording upload: {e}"); None }
                        }
                    }
                    Err(e) => { log::warn!("[heartbeat] failed to read recording: {e}"); None }
                };

                // Analyze with Gemini
                let analysis = match analyze_screen_recording(SCREEN_RECORDING_PATH, google_ai_key).await {
                    Ok(text) => Some(text),
                    Err(e) => { log::warn!("[heartbeat] screen analysis failed: {e}"); None }
                };

                let _ = std::fs::remove_file(path);

                match (upload_id, analysis) {
                    (Some(uid), Some(text)) => Some(ScreenRecordingResult {
                        analysis: text,
                        upload_id: uid,
                        machine_id: machine_id.clone(),
                    }),
                    (Some(uid), None) => Some(ScreenRecordingResult {
                        analysis: "(analysis failed)".into(),
                        upload_id: uid,
                        machine_id: machine_id.clone(),
                    }),
                    _ => None,
                }
            } else {
                let _ = std::fs::remove_file(path);
                None
            }
        } else {
            None
        }
    };

    // 3. Start a new recording
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
