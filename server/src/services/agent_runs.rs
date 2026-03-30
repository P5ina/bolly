//! Agent run persistence — stores full execution traces to disk.

use std::fs;
use std::path::Path;

use crate::domain::agent_run::{AgentRun, AgentRunSummary};

fn runs_dir(workspace_dir: &Path, slug: &str) -> std::path::PathBuf {
    workspace_dir
        .join("instances")
        .join(slug)
        .join("agent_runs")
}

/// Save an agent run to `instances/{slug}/agent_runs/{id}.json`.
pub fn save_run(workspace_dir: &Path, slug: &str, run: &AgentRun) -> anyhow::Result<()> {
    let dir = runs_dir(workspace_dir, slug);
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", run.id));
    let json = serde_json::to_string(run)?;
    // Atomic write: tmp + rename
    let tmp = dir.join(format!(".{}.tmp", run.id));
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Load a single run by ID.
pub fn load_run(workspace_dir: &Path, slug: &str, run_id: &str) -> anyhow::Result<AgentRun> {
    let path = runs_dir(workspace_dir, slug).join(format!("{run_id}.json"));
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

/// List recent runs (newest first), without trace data.
pub fn list_runs(
    workspace_dir: &Path,
    slug: &str,
    limit: usize,
    agent_name: Option<&str>,
) -> anyhow::Result<Vec<AgentRunSummary>> {
    let dir = runs_dir(workspace_dir, slug);
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    let mut runs: Vec<AgentRunSummary> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .filter_map(|e| {
            let raw = fs::read_to_string(e.path()).ok()?;
            let run: AgentRun = serde_json::from_str(&raw).ok()?;
            Some(AgentRunSummary::from(run))
        })
        .collect();

    // Filter by agent name if specified
    if let Some(name) = agent_name {
        runs.retain(|r| r.agent_name == name);
    }

    // Newest first (started_at is millis timestamp)
    runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    runs.truncate(limit);

    Ok(runs)
}
