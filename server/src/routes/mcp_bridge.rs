use std::path::Path;
use std::sync::Arc;

use axum::{Json, Router, extract::{Path as AxumPath, State}, routing::post};
use serde_json::json;

use crate::app::state::AppState;
use crate::services::tool::ToolDyn;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mcp/{instance_slug}/{chat_id}", post(mcp_handler))
}

#[derive(serde::Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

fn jsonrpc_ok(id: Option<serde_json::Value>, result: serde_json::Value) -> Json<serde_json::Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
}

fn jsonrpc_error(id: Option<serde_json::Value>, code: i64, message: &str) -> Json<serde_json::Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
}

async fn mcp_handler(
    State(state): State<AppState>,
    AxumPath((instance_slug, chat_id)): AxumPath<(String, String)>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<serde_json::Value> {
    let id = request.id;

    match request.method.as_str() {
        "initialize" => {
            jsonrpc_ok(id, json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "personality",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }))
        }

        "notifications/initialized" => {
            // Client ack — no response needed for notifications
            jsonrpc_ok(id, json!({}))
        }

        "tools/list" => {
            let config = state.config.read().await;
            let tools = build_mcp_tools(
                &state.workspace_dir,
                &instance_slug,
                &chat_id,
                &config,
                state.vector_store.clone(),
            );

            let mut tool_defs = Vec::new();
            for tool in &tools {
                let def = tool.definition(String::new()).await;
                tool_defs.push(json!({
                    "name": def.name,
                    "description": def.description,
                    "inputSchema": def.parameters,
                }));
            }

            jsonrpc_ok(id, json!({ "tools": tool_defs }))
        }

        "tools/call" => {
            let tool_name = request.params["name"].as_str().unwrap_or("");
            let arguments = &request.params["arguments"];

            let config = state.config.read().await;
            let tools = build_mcp_tools(
                &state.workspace_dir,
                &instance_slug,
                &chat_id,
                &config,
                state.vector_store.clone(),
            );

            let tool = tools.iter().find(|t| t.name() == tool_name);
            match tool {
                Some(tool) => {
                    let args_str = serde_json::to_string(arguments).unwrap_or_default();
                    log::info!("[mcp-bridge] calling tool: {tool_name}");
                    match tool.call(args_str).await {
                        Ok(result) => {
                            jsonrpc_ok(id, json!({
                                "content": [{ "type": "text", "text": result }]
                            }))
                        }
                        Err(e) => {
                            log::warn!("[mcp-bridge] tool {tool_name} error: {e}");
                            jsonrpc_ok(id, json!({
                                "content": [{ "type": "text", "text": format!("error: {e}") }],
                                "isError": true,
                            }))
                        }
                    }
                }
                None => {
                    jsonrpc_error(id, -32601, &format!("unknown tool: {tool_name}"))
                }
            }
        }

        other => {
            log::debug!("[mcp-bridge] unhandled method: {other}");
            jsonrpc_error(id, -32601, &format!("method not found: {other}"))
        }
    }
}

/// Build a curated set of tools for the MCP bridge.
/// Subset of build_tools() — only tools that work without events/streaming.
fn build_mcp_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    _chat_id: &str,
    config: &crate::config::Config,
    vector_store: Arc<crate::services::vector::VectorStore>,
) -> Vec<Box<dyn ToolDyn>> {
    use crate::services::tools::*;

    let google_ai_key = &config.llm.tokens.google_ai;

    let tools: Vec<Box<dyn ToolDyn>> = vec![
        // Memory
        Box::new(MemoryWriteTool::new(workspace_dir, instance_slug, vector_store.clone(), google_ai_key)),
        Box::new(MemoryReadTool::new(workspace_dir, instance_slug)),
        Box::new(MemoryListTool::new(workspace_dir, instance_slug)),
        Box::new(MemorySearchTool::new(workspace_dir, instance_slug, vector_store.clone(), google_ai_key)),
        Box::new(MemoryForgetTool::new(workspace_dir, instance_slug, vector_store.clone(), google_ai_key)),
        Box::new(MemoryConnectTool::new(workspace_dir, instance_slug)),
        // Soul
        Box::new(EditSoulTool::new(workspace_dir, instance_slug)),
        // Files
        Box::new(ReadFileTool::new(workspace_dir, instance_slug)),
        Box::new(WriteFileTool::new(workspace_dir, instance_slug)),
        Box::new(ListFilesTool::new(workspace_dir, instance_slug)),
        // System
        Box::new(GetTimeTool::new(workspace_dir, instance_slug)),
    ];

    tools
}
