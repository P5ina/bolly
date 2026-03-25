use base64::Engine;
use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024; // 5 MB

pub struct ViewImageTool;

#[derive(Deserialize, JsonSchema)]
pub struct ViewImageArgs {
    /// URL of the image to view. Supports jpg, png, gif, webp.
    pub url: String,
}

impl Tool for ViewImageTool {
    const NAME: &'static str = "view_image";
    type Error = ToolExecError;
    type Args = ViewImageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "view_image".into(),
            description: "Download an image from a URL. \
                Returns the image so you can see it, and it is \
                automatically sent to the user in chat. \
                Use this to show images to the user (e.g. after generating one)."
                .into(),
            parameters: openai_schema::<ViewImageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let url = args.url.trim();
        if url.is_empty() {
            return Err(ToolExecError("url cannot be empty".into()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolExecError(format!("http client error: {e}")))?;

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolExecError(format!("failed to fetch image: {e}")))?;

        if !resp.status().is_success() {
            return Err(ToolExecError(format!("HTTP {}", resp.status())));
        }

        // Detect mime type from content-type header or URL extension
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let mime = if content_type.contains("image/") {
            content_type.split(';').next().unwrap_or("image/jpeg").trim().to_string()
        } else {
            // Guess from URL extension
            let lower = url.to_lowercase();
            if lower.contains(".png") { "image/png".into() }
            else if lower.contains(".gif") { "image/gif".into() }
            else if lower.contains(".webp") { "image/webp".into() }
            else { "image/jpeg".into() }
        };

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ToolExecError(format!("failed to read image: {e}")))?;

        if bytes.len() > MAX_IMAGE_SIZE {
            return Err(ToolExecError(format!(
                "image too large: {:.1} MB (max 5 MB)",
                bytes.len() as f64 / 1024.0 / 1024.0
            )));
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        log::info!("[view_image] fetched {} ({}, {:.0} KB)", url, mime, bytes.len() as f64 / 1024.0);

        Ok(serde_json::to_string(&serde_json::json!([
            {"type": "image", "source": {"type": "base64", "media_type": mime, "data": b64}}
        ])).unwrap())
    }
}
