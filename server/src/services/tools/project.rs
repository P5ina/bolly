use std::{fs, path::{Path, PathBuf}};

use chrono::Utc;
use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// Project state structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mission: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub core_traits: Vec<String>,
    #[serde(default)]
    pub current_arc: String,
    #[serde(default)]
    pub important_events: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentFocus {
    #[serde(default)]
    pub active_goal: String,
    #[serde(default)]
    pub current_task: String,
    #[serde(default)]
    pub next_step: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectState {
    #[serde(default)]
    pub project: ProjectInfo,
    #[serde(default)]
    pub identity: IdentityInfo,
    #[serde(default)]
    pub current_focus: CurrentFocus,
    #[serde(default)]
    pub open_loops: Vec<String>,
    #[serde(default)]
    pub recent_progress: Vec<String>,
    #[serde(default)]
    pub next_candidates: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

// ---------------------------------------------------------------------------
// get_project_state
// ---------------------------------------------------------------------------

pub struct GetProjectStateTool {
    instance_dir: PathBuf,
}

impl GetProjectStateTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GetProjectStateArgs {}

impl Tool for GetProjectStateTool {
    const NAME: &'static str = "get_project_state";
    type Error = ToolExecError;
    type Args = GetProjectStateArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_project_state".into(),
            description: "Read current project state (goal, progress, next steps, open questions).".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.instance_dir.join("project_state.json");
        let state: ProjectState = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        serde_json::to_string_pretty(&state)
            .map_err(|e| ToolExecError(format!("failed to serialize project state: {e}")))
    }
}

// ---------------------------------------------------------------------------
// update_project_state
// ---------------------------------------------------------------------------

pub struct UpdateProjectStateTool {
    instance_dir: PathBuf,
}

impl UpdateProjectStateTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateProjectStateArgs {
    /// Project name. Null to keep current.
    pub project_name: Option<String>,
    /// Project mission statement. Null to keep current.
    pub project_mission: Option<String>,
    /// Project status (e.g. "active", "paused"). Null to keep current.
    pub project_status: Option<String>,
    /// Your name. Null to keep current.
    pub identity_name: Option<String>,
    /// Your core traits as a list. Null to keep current.
    pub core_traits: Option<Vec<String>>,
    /// Your current arc / growth trajectory. Null to keep current.
    pub current_arc: Option<String>,
    /// Important events in your history. Null to keep current.
    pub important_events: Option<Vec<String>>,
    /// Current high-level goal. Null to keep current.
    pub active_goal: Option<String>,
    /// What you're currently working on. Null to keep current.
    pub current_task: Option<String>,
    /// What should be done next. Null to keep current.
    pub next_step: Option<String>,
    /// Open threads and unfinished work. Null to keep current.
    pub open_loops: Option<Vec<String>>,
    /// Recent completed items. Null to keep current.
    pub recent_progress: Option<Vec<String>>,
    /// Candidate next steps to consider. Null to keep current.
    pub next_candidates: Option<Vec<String>>,
    /// Known risks and concerns. Null to keep current.
    pub risks: Option<Vec<String>>,
}

impl Tool for UpdateProjectStateTool {
    const NAME: &'static str = "update_project_state";
    type Error = ToolExecError;
    type Args = UpdateProjectStateArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_project_state".into(),
            description: "Update project state. Only provided fields change.".into(),
            parameters: openai_schema::<UpdateProjectStateArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.instance_dir.join("project_state.json");
        let mut state: ProjectState = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        if let Some(v) = args.project_name { state.project.name = v; }
        if let Some(v) = args.project_mission { state.project.mission = v; }
        if let Some(v) = args.project_status { state.project.status = v; }
        if let Some(v) = args.identity_name { state.identity.name = v; }
        if let Some(v) = args.core_traits { state.identity.core_traits = v; }
        if let Some(v) = args.current_arc { state.identity.current_arc = v; }
        if let Some(v) = args.important_events { state.identity.important_events = v; }
        if let Some(v) = args.active_goal { state.current_focus.active_goal = v; }
        if let Some(v) = args.current_task { state.current_focus.current_task = v; }
        if let Some(v) = args.next_step { state.current_focus.next_step = v; }
        if let Some(v) = args.open_loops { state.open_loops = v; }
        if let Some(v) = args.recent_progress { state.recent_progress = v; }
        if let Some(v) = args.next_candidates { state.next_candidates = v; }
        if let Some(v) = args.risks { state.risks = v; }

        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| ToolExecError(format!("failed to serialize: {e}")))?;
        fs::write(&path, &json)
            .map_err(|e| ToolExecError(format!("failed to write project state: {e}")))?;

        Ok(format!("project state updated"))
    }
}

// ---------------------------------------------------------------------------
// Task board
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub notes: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Blocked => write!(f, "blocked"),
        }
    }
}

fn load_tasks(instance_dir: &Path) -> Vec<TaskItem> {
    let path = instance_dir.join("tasks.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_tasks(instance_dir: &Path, tasks: &[TaskItem]) {
    let path = instance_dir.join("tasks.json");
    if let Ok(json) = serde_json::to_string_pretty(tasks) {
        let _ = fs::write(&path, json);
    }
}

// ---------------------------------------------------------------------------
// create_task
// ---------------------------------------------------------------------------

pub struct CreateTaskTool {
    instance_dir: PathBuf,
}

impl CreateTaskTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateTaskArgs {
    /// Short title describing the task.
    pub title: String,
    /// Priority: "high", "medium", or "low". Default: "medium".
    pub priority: Option<String>,
    /// Optional notes, context, or details.
    pub notes: Option<String>,
}

impl Tool for CreateTaskTool {
    const NAME: &'static str = "create_task";
    type Error = ToolExecError;
    type Args = CreateTaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_task".into(),
            description: "Create a task on the board. Starts as 'todo'.".into(),
            parameters: openai_schema::<CreateTaskArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let title = args.title.trim().to_string();
        if title.is_empty() {
            return Err(ToolExecError("title cannot be empty".into()));
        }

        let mut tasks = load_tasks(&self.instance_dir);
        let id = format!("task_{}", tasks.len() + 1);
        let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();

        tasks.push(TaskItem {
            id: id.clone(),
            title: title.clone(),
            status: TaskStatus::Todo,
            priority: args.priority.unwrap_or_else(|| "medium".into()),
            notes: args.notes.unwrap_or_default(),
            created_at: now.clone(),
            updated_at: now,
        });

        save_tasks(&self.instance_dir, &tasks);
        Ok(format!("created task {id}: {title}"))
    }
}

// ---------------------------------------------------------------------------
// update_task
// ---------------------------------------------------------------------------

pub struct UpdateTaskTool {
    instance_dir: PathBuf,
}

impl UpdateTaskTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateTaskArgs {
    /// Task ID (e.g. "task_1").
    pub id: String,
    /// New status: "todo", "in_progress", "done", or "blocked". Null to keep current.
    pub status: Option<String>,
    /// Priority: "high", "medium", or "low". Null to keep current.
    pub priority: Option<String>,
    /// Update notes. Null to keep current.
    pub notes: Option<String>,
    /// Update title. Null to keep current.
    pub title: Option<String>,
}

impl Tool for UpdateTaskTool {
    const NAME: &'static str = "update_task";
    type Error = ToolExecError;
    type Args = UpdateTaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_task".into(),
            description: "Update a task (status, title, notes). Statuses: todo, in_progress, done, blocked.".into(),
            parameters: openai_schema::<UpdateTaskArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut tasks = load_tasks(&self.instance_dir);
        let task = tasks
            .iter_mut()
            .find(|t| t.id == args.id)
            .ok_or_else(|| ToolExecError(format!("task '{}' not found", args.id)))?;

        if let Some(status) = &args.status {
            task.status = match status.to_lowercase().as_str() {
                "todo" => TaskStatus::Todo,
                "in_progress" => TaskStatus::InProgress,
                "done" => TaskStatus::Done,
                "blocked" => TaskStatus::Blocked,
                other => {
                    return Err(ToolExecError(format!(
                        "invalid status '{other}'. use: todo, in_progress, done, blocked"
                    )));
                }
            };
        }
        if let Some(title) = args.title { task.title = title; }
        if let Some(priority) = args.priority { task.priority = priority; }
        if let Some(notes) = args.notes { task.notes = notes; }
        task.updated_at = Utc::now().format("%Y-%m-%d %H:%M").to_string();

        let summary = format!("{} → {}", task.id, task.status);
        save_tasks(&self.instance_dir, &tasks);
        Ok(summary)
    }
}

// ---------------------------------------------------------------------------
// list_tasks
// ---------------------------------------------------------------------------

pub struct ListTasksTool {
    instance_dir: PathBuf,
}

impl ListTasksTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ListTasksArgs {
    /// Filter by status: "todo", "in_progress", "done", "blocked", or "all". Default: "all".
    pub status: Option<String>,
}

impl Tool for ListTasksTool {
    const NAME: &'static str = "list_tasks";
    type Error = ToolExecError;
    type Args = ListTasksArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_tasks".into(),
            description: "List tasks, optionally filtered by status.".into(),
            parameters: openai_schema::<ListTasksArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let tasks = load_tasks(&self.instance_dir);
        if tasks.is_empty() {
            return Ok("no tasks yet".into());
        }

        let filter = args.status.as_deref().unwrap_or("all").to_lowercase();
        let filtered: Vec<_> = tasks
            .iter()
            .filter(|t| filter == "all" || t.status.to_string() == filter)
            .collect();

        if filtered.is_empty() {
            return Ok(format!("no tasks with status '{filter}'"));
        }

        let mut out = String::new();
        for t in &filtered {
            let prio = if t.priority.is_empty() {
                String::new()
            } else {
                format!(" [{}]", t.priority)
            };
            let notes = if t.notes.is_empty() {
                String::new()
            } else {
                format!(" — {}", t.notes)
            };
            out.push_str(&format!(
                "[{}]{} {} — {}{}\n",
                t.status, prio, t.id, t.title, notes
            ));
        }
        Ok(out)
    }
}
