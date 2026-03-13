use std::{collections::HashMap, sync::Arc};

use rmcp::{
    model::{ClientCapabilities, ClientInfo, Implementation, ReadResourceRequestParams},
    service::ServerSink,
    ServiceExt,
};
use rig::tool::rmcp::McpTool;

use crate::config::McpServerConfig;

/// A connected MCP server with its tools and client handle.
pub struct McpConnection {
    pub name: String,
    pub tools: Vec<McpTool>,
    /// Tool name → resource URI for tools with MCP Apps UI.
    pub ui_tools: HashMap<String, String>,
    /// Resource URI → cached HTML content.
    pub resources: HashMap<String, String>,
    #[allow(dead_code)]
    pub sink: ServerSink,
}

/// Connect to all configured MCP servers and return their tools.
pub async fn connect_all(configs: &[McpServerConfig]) -> Vec<McpConnection> {
    let mut connections = Vec::new();

    for config in configs {
        match connect_one(config).await {
            Ok(conn) => {
                log::info!(
                    "MCP '{}': connected, {} tools ({} with UI)",
                    conn.name,
                    conn.tools.len(),
                    conn.ui_tools.len(),
                );
                connections.push(conn);
            }
            Err(e) => {
                log::error!("MCP '{}': failed to connect: {e}", config.name);
            }
        }
    }

    connections
}

/// Extract `_meta.ui.resourceUri` from a tool's metadata.
fn extract_ui_resource_uri(tool: &rmcp::model::Tool) -> Option<String> {
    let meta = tool.meta.as_ref()?;
    // meta is Meta(JsonObject), access ui.resourceUri
    let ui = meta.0.get("ui")?.as_object()?;
    let uri = ui.get("resourceUri")?.as_str()?;
    Some(uri.to_string())
}

async fn connect_one(config: &McpServerConfig) -> Result<McpConnection, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(url) = &config.url {
        let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(url.as_str());
        let client_info = ClientInfo {
            meta: None,
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "bolly".into(),
                title: None,
                version: env!("CARGO_PKG_VERSION").into(),
                description: None,
                icons: None,
                website_url: None,
            },
        };
        let running = client_info.serve(transport).await?;
        let sink: ServerSink = running.peer().clone();

        let raw_tools = sink.list_all_tools().await?;

        // Detect tools with MCP Apps UI
        let mut ui_tools: HashMap<String, String> = HashMap::new();
        for t in &raw_tools {
            if let Some(uri) = extract_ui_resource_uri(t) {
                log::info!("MCP '{}': tool '{}' has UI resource: {}", config.name, t.name, uri);
                ui_tools.insert(t.name.to_string(), uri);
            }
        }

        // Fetch HTML resources for UI tools
        let mut resources: HashMap<String, String> = HashMap::new();
        let unique_uris: Vec<String> = ui_tools.values().cloned().collect::<std::collections::HashSet<_>>().into_iter().collect();
        for uri in unique_uris {
            match sink.read_resource(ReadResourceRequestParams {
                uri: uri.clone().into(),
                meta: None,
            }).await {
                Ok(result) => {
                    for content in &result.contents {
                        if let rmcp::model::ResourceContents::TextResourceContents { text, .. } = &content {
                            log::info!("MCP '{}': cached resource '{}' ({} bytes)", config.name, uri, text.len());
                            resources.insert(uri.clone(), text.clone());
                            break;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("MCP '{}': failed to fetch resource '{}': {e}", config.name, uri);
                }
            }
        }

        let tools: Vec<McpTool> = raw_tools
            .into_iter()
            .map(|t| McpTool::from_mcp_server(t, sink.clone()))
            .collect();

        Ok(McpConnection {
            name: config.name.clone(),
            tools,
            ui_tools,
            resources,
            sink,
        })
    } else if let Some(_command) = &config.command {
        Err("stdio transport not yet supported".into())
    } else {
        Err("MCP server config must have either 'url' or 'command'".into())
    }
}

/// A shared handle holding all active MCP connections.
/// Cloneable and safe to pass around.
#[derive(Clone, Default)]
pub struct McpRegistry {
    connections: Arc<Vec<McpConnection>>,
}

impl McpRegistry {
    pub fn new(connections: Vec<McpConnection>) -> Self {
        Self {
            connections: Arc::new(connections),
        }
    }

    /// Get all MCP tools as boxed ToolDyn, ready to be wrapped in ObservableTool.
    pub fn tools_as_dyn(&self) -> Vec<Box<dyn rig::tool::ToolDyn>> {
        self.connections
            .iter()
            .flat_map(|conn| {
                conn.tools.iter().map(|t| {
                    let boxed: Box<dyn rig::tool::ToolDyn> = Box::new(t.clone());
                    boxed
                })
            })
            .collect()
    }

    /// Get the cached HTML for a tool with MCP Apps UI, if any.
    pub fn get_app_html(&self, tool_name: &str) -> Option<String> {
        for conn in self.connections.iter() {
            if let Some(uri) = conn.ui_tools.get(tool_name) {
                if let Some(html) = conn.resources.get(uri) {
                    return Some(html.clone());
                }
            }
        }
        None
    }

    /// Check if a tool has MCP Apps UI.
    pub fn is_app_tool(&self, tool_name: &str) -> bool {
        self.connections.iter().any(|c| c.ui_tools.contains_key(tool_name))
    }

    pub fn tool_count(&self) -> usize {
        self.connections.iter().map(|c| c.tools.len()).sum()
    }
}
