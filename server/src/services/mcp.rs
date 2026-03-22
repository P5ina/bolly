use std::{collections::HashMap, fmt, future::Future, pin::Pin, sync::Arc};

use rmcp::{
    model::{ClientInfo, Implementation, ReadResourceRequestParams},
    service::ServerSink,
    ServiceExt,
};

use crate::config::McpServerConfig;
use crate::services::tool::{ToolDefinition, ToolDyn, ToolError};

// ---------------------------------------------------------------------------
// McpTool — our own wrapper replacing rig::tool::rmcp::McpTool
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct McpTool {
    definition: rmcp::model::Tool,
    client: ServerSink,
}

#[derive(Debug)]
struct McpToolError(String);

impl fmt::Display for McpToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MCP tool error: {}", self.0)
    }
}

impl std::error::Error for McpToolError {}

impl ToolDyn for McpTool {
    fn name(&self) -> String {
        self.definition.name.to_string()
    }

    fn definition<'a>(
        &'a self,
        _prompt: String,
    ) -> Pin<Box<dyn Future<Output = ToolDefinition> + Send + 'a>> {
        Box::pin(async move {
            ToolDefinition {
                name: self.definition.name.to_string(),
                description: self
                    .definition
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_string(),
                parameters: serde_json::to_value(self.definition.input_schema.as_ref())
                    .unwrap_or_default(),
            }
        })
    }

    fn call<'a>(
        &'a self,
        args: String,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + 'a>> {
        let name = self.definition.name.clone();
        let arguments: Option<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(&args).unwrap_or_default();

        Box::pin(async move {
            let mut params = rmcp::model::CallToolRequestParams::new(name);
            params.arguments = arguments;
            let result = self
                .client
                .call_tool(params)
                .await
                .map_err(|e| {
                    ToolError::ToolCallError(Box::new(McpToolError(format!(
                        "Tool returned an error: {e}"
                    ))))
                })?;

            if let Some(true) = result.is_error {
                let error_msg: String = result
                    .content
                    .iter()
                    .filter_map(|c| c.raw.as_text().map(|t| t.text.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");
                let msg = if error_msg.is_empty() {
                    "No message returned".to_string()
                } else {
                    error_msg
                };
                return Err(ToolError::ToolCallError(Box::new(McpToolError(msg))));
            }

            Ok(result
                .content
                .into_iter()
                .map(|c| match c.raw {
                    rmcp::model::RawContent::Text(raw) => raw.text,
                    rmcp::model::RawContent::Image(raw) => {
                        format!("data:{};base64,{}", raw.mime_type, raw.data)
                    }
                    rmcp::model::RawContent::Resource(raw) => match raw.resource {
                        rmcp::model::ResourceContents::TextResourceContents {
                            text, ..
                        } => text,
                        rmcp::model::ResourceContents::BlobResourceContents {
                            uri,
                            mime_type,
                            blob,
                            ..
                        } => format!(
                            "{mime_type}{uri}:{blob}",
                            mime_type = mime_type
                                .map(|m| format!("data:{m};"))
                                .unwrap_or_default(),
                        ),
                    },
                    other => format!("{other:?}"),
                })
                .collect::<String>())
        })
    }
}

// ---------------------------------------------------------------------------
// McpConnection
// ---------------------------------------------------------------------------

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
    /// Keep the RunningService alive — dropping this kills the transport.
    #[allow(dead_code)]
    _keepalive: tokio::task::JoinHandle<()>,
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
        let mut transport_config = rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(url.as_str());

        // Pass all headers as custom_headers (including Authorization)
        for (key, value) in &config.headers {
            if let (Ok(name), Ok(val)) = (key.parse::<reqwest::header::HeaderName>(), value.parse::<reqwest::header::HeaderValue>()) {
                transport_config.custom_headers.insert(name, val);
            }
        }

        let transport = rmcp::transport::StreamableHttpClientTransport::from_config(transport_config);
        let mut client_info = ClientInfo::default();
        client_info.client_info = Implementation::new("bolly", env!("CARGO_PKG_VERSION"));
        let running = client_info.serve(transport).await?;
        let sink: ServerSink = running.peer().clone();
        let conn_name = config.name.clone();
        let keepalive = tokio::spawn(async move {
            match running.waiting().await {
                Ok(reason) => log::warn!("MCP '{conn_name}': connection closed: {reason:?}"),
                Err(e) => log::warn!("MCP '{conn_name}': connection task failed: {e}"),
            }
        });

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
            match sink.read_resource(ReadResourceRequestParams::new(uri.clone())).await {
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
            .map(|t| McpTool {
                definition: t,
                client: sink.clone(),
            })
            .collect();

        Ok(McpConnection {
            name: config.name.clone(),
            tools,
            ui_tools,
            resources,
            sink,
            _keepalive: keepalive,
        })
    } else if let Some(_command) = &config.command {
        Err("stdio transport not yet supported".into())
    } else {
        Err("MCP server config must have either 'url' or 'command'".into())
    }
}

/// A shared handle holding all active MCP connections.
/// Cloneable and safe to pass around. Supports dynamic reconnection.
#[derive(Clone, Default)]
pub struct McpRegistry {
    connections: Arc<tokio::sync::RwLock<Vec<McpConnection>>>,
    configs: Arc<tokio::sync::RwLock<Vec<McpServerConfig>>>,
}

impl McpRegistry {
    pub fn new(connections: Vec<McpConnection>, configs: Vec<McpServerConfig>) -> Self {
        Self {
            connections: Arc::new(tokio::sync::RwLock::new(connections)),
            configs: Arc::new(tokio::sync::RwLock::new(configs)),
        }
    }

    /// Replace all connections with newly connected ones.
    pub async fn reconnect(&self, configs: &[McpServerConfig]) {
        let new_connections = connect_all(configs).await;
        *self.configs.write().await = configs.to_vec();
        *self.connections.write().await = new_connections;
    }

    /// Reconnect any MCP servers whose transport has died. Called before each chat turn.
    pub async fn ensure_connected(&self) {
        let configs = self.configs.read().await.clone();
        if configs.is_empty() {
            return;
        }

        let mut conns = self.connections.write().await;

        // Check which servers are still alive
        let alive_names: Vec<String> = conns.iter()
            .filter(|c| !c._keepalive.is_finished())
            .map(|c| c.name.clone())
            .collect();

        // Remove dead connections
        let before = conns.len();
        conns.retain(|c| !c._keepalive.is_finished());
        if conns.len() < before {
            log::info!("MCP: dropped {} dead connections", before - conns.len());
        }

        // Reconnect any configured servers that aren't alive
        for config in &configs {
            if !alive_names.contains(&config.name) {
                log::info!("MCP '{}': reconnecting...", config.name);
                match connect_one(config).await {
                    Ok(conn) => {
                        log::info!("MCP '{}': reconnected, {} tools", conn.name, conn.tools.len());
                        conns.push(conn);
                    }
                    Err(e) => log::error!("MCP '{}': reconnect failed: {e}", config.name),
                }
            }
        }
    }

    /// List connected server names.
    pub async fn server_names(&self) -> Vec<String> {
        self.connections.read().await.iter().map(|c| c.name.clone()).collect()
    }

    /// Get all MCP tools as boxed ToolDyn, ready to be wrapped in ObservableTool.
    pub async fn tools_as_dyn(&self) -> Vec<Box<dyn ToolDyn>> {
        self.connections
            .read()
            .await
            .iter()
            .flat_map(|conn| {
                conn.tools.iter().map(|t| {
                    let boxed: Box<dyn ToolDyn> = Box::new(t.clone());
                    boxed
                })
            })
            .collect()
    }

    /// Snapshot app tool info for sync access (used by ObservableTool at call time).
    pub async fn snapshot_app_tools(&self) -> McpAppSnapshot {
        let guard = self.connections.read().await;
        let mut tool_html = HashMap::new();
        for conn in guard.iter() {
            for (tool_name, uri) in &conn.ui_tools {
                if let Some(html) = conn.resources.get(uri) {
                    tool_html.insert(tool_name.clone(), html.clone());
                }
            }
        }
        McpAppSnapshot { tool_html }
    }

    pub async fn tool_count(&self) -> usize {
        self.connections.read().await.iter().map(|c| c.tools.len()).sum()
    }
}

/// A sync-safe snapshot of MCP app tool info, captured once per turn.
#[derive(Clone, Default)]
pub struct McpAppSnapshot {
    tool_html: HashMap<String, String>,
}

impl McpAppSnapshot {
    pub fn is_app_tool(&self, tool_name: &str) -> bool {
        self.tool_html.contains_key(tool_name)
    }

    pub fn get_app_html(&self, tool_name: &str) -> Option<String> {
        self.tool_html.get(tool_name).cloned()
    }
}
