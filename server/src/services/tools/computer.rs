use schemars::JsonSchema;
use serde::Deserialize;

use crate::services::machine_registry::{AgentToolCall, MachineRegistry};
use crate::services::tool::ToolDefinition;
use crate::services::tools::{openai_schema, ToolExecError};

use crate::services::tool::Tool;

// ═══════════════════════════════════════════════════════════════════════════
// list_machines — returns connected Tauri agents
// ═══════════════════════════════════════════════════════════════════════════

pub struct ListMachinesTool {
    registry: MachineRegistry,
}

impl ListMachinesTool {
    pub fn new(registry: MachineRegistry) -> Self {
        Self { registry }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ListMachinesArgs {}

impl Tool for ListMachinesTool {
    const NAME: &'static str = "list_machines";
    type Error = ToolExecError;
    type Args = ListMachinesArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_machines".into(),
            description: "List all connected desktop machines that you can control via computer use. \
                Returns machine IDs, OS, hostname, and screen dimensions. \
                Use a machine_id from this list when calling computer_use."
                .into(),
            parameters: openai_schema::<ListMachinesArgs>(),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let machines = self.registry.list().await;
        if machines.is_empty() {
            return Ok("No machines connected. The user needs to open the Bolly desktop app first.".into());
        }
        let info: Vec<serde_json::Value> = machines
            .iter()
            .map(|m| {
                serde_json::json!({
                    "machine_id": m.machine_id,
                    "os": m.os,
                    "hostname": m.hostname,
                    "screen": format!("{}x{}", m.screen_width, m.screen_height),
                })
            })
            .collect();
        serde_json::to_string_pretty(&info).map_err(|e| ToolExecError(e.to_string()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// computer_use — route action to a specific machine agent
// ═══════════════════════════════════════════════════════════════════════════

pub struct ComputerUseTool {
    registry: MachineRegistry,
}

impl ComputerUseTool {
    pub fn new(registry: MachineRegistry) -> Self {
        Self { registry }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ComputerUseArgs {
    /// ID of the machine to control (from list_machines).
    pub machine_id: String,
    /// Action to perform: "screenshot", "left_click", "right_click", "middle_click",
    /// "double_click", "mouse_move", "type", "key", "scroll".
    pub action: String,
    /// [x, y] coordinates for click/move/scroll actions (in screen pixels).
    #[serde(default)]
    pub coordinate: Option<[i32; 2]>,
    /// Text to type (for "type" action).
    #[serde(default)]
    pub text: Option<String>,
    /// Key or key combination to press (for "key" action, e.g. "ctrl+c", "Return").
    #[serde(default)]
    pub key: Option<String>,
    /// Scroll direction: "up", "down", "left", "right".
    #[serde(default)]
    pub scroll_direction: Option<String>,
    /// Number of scroll clicks (default 3).
    #[serde(default)]
    pub scroll_amount: Option<i32>,
}

impl Tool for ComputerUseTool {
    const NAME: &'static str = "computer_use";
    type Error = ToolExecError;
    type Args = ComputerUseArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "computer_use".into(),
            description: "Control a connected desktop machine — take screenshots, click, type, press keys, scroll. \
                Always take a screenshot first to see the current state. \
                Coordinates are in the screenshot's pixel space. \
                Available actions: screenshot, left_click, right_click, middle_click, double_click, \
                mouse_move, type, key, scroll."
                .into(),
            parameters: openai_schema::<ComputerUseArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let request_id = uuid::Uuid::new_v4().to_string();

        let call = AgentToolCall {
            request_id: request_id.clone(),
            action: args.action.clone(),
            coordinate: args.coordinate,
            text: args.text,
            key: args.key,
            scroll_direction: args.scroll_direction,
            scroll_amount: args.scroll_amount,
        };

        log::info!(
            "[computer_use] {} on machine '{}' (req={})",
            args.action,
            args.machine_id,
            &request_id[..8]
        );

        let result = self
            .registry
            .execute(&args.machine_id, call)
            .await
            .map_err(|e| ToolExecError(e))?;

        match result.result_type.as_str() {
            "screenshot" => {
                // Return as JSON array of content blocks — tool_result handler will unwrap this
                let image = result.image.unwrap_or_default();
                let w = result.width.unwrap_or(0);
                let h = result.height.unwrap_or(0);
                let blocks = serde_json::json!([
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": image,
                        }
                    },
                    {
                        "type": "text",
                        "text": format!("Screenshot captured ({}x{})", w, h),
                    }
                ]);
                Ok(blocks.to_string())
            }
            "action" => {
                if result.success.unwrap_or(false) {
                    Ok(format!("Action '{}' executed successfully.", args.action))
                } else {
                    let err = result.error.unwrap_or_else(|| "unknown error".to_string());
                    Err(ToolExecError(format!("Action '{}' failed: {}", args.action, err)))
                }
            }
            other => Err(ToolExecError(format!("unexpected result type: {other}"))),
        }
    }
}
