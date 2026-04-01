use futures::StreamExt;
use tokio::sync::broadcast;

use crate::domain::events::ServerEvent;
use crate::services::tool::ToolDefinition;

use super::types::{ContentBlock, Message, ToolUseBlock, StreamOnceResult};

// ═══════════════════════════════════════════════════════════════════════════
// OpenAI Chat Completions format
// ═══════════════════════════════════════════════════════════════════════════

/// Convert our internal Message format to OpenAI chat messages.
pub(crate) fn messages_to_openai(system: &[&str], messages: &[Message]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();

    // System message
    let sys_text = system.join("\n\n");
    if !sys_text.is_empty() {
        out.push(serde_json::json!({"role": "system", "content": sys_text}));
    }

    for msg in messages {
        match msg {
            Message::User { content } => {
                // Check for tool results
                let mut tool_results = Vec::new();
                let mut text_parts = Vec::new();

                for block in content {
                    match block {
                        ContentBlock::ToolResult { tool_use_id, content, .. } => {
                            tool_results.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                        ContentBlock::Text { text } => text_parts.push(text.clone()),
                        _ => {}
                    }
                }

                let has_tools = !tool_results.is_empty();
                if has_tools {
                    out.extend(tool_results);
                }
                if !text_parts.is_empty() {
                    out.push(serde_json::json!({"role": "user", "content": text_parts.join("\n")}));
                }
                if !has_tools && text_parts.is_empty() {
                    out.push(serde_json::json!({"role": "user", "content": ""}));
                }
            }
            Message::Assistant { content } => {
                let mut text = String::new();
                let mut tool_calls = Vec::new();

                for block in content {
                    match block {
                        ContentBlock::Text { text: t } => text.push_str(t),
                        ContentBlock::ToolUse { id, name, input } => {
                            tool_calls.push(serde_json::json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": serde_json::to_string(input).unwrap_or_default(),
                                }
                            }));
                        }
                        _ => {}
                    }
                }

                let mut msg = serde_json::json!({"role": "assistant"});
                if !text.is_empty() {
                    msg["content"] = serde_json::Value::String(text);
                }
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = serde_json::Value::Array(tool_calls);
                }
                out.push(msg);
            }
        }
    }

    out
}

/// Convert tool definitions to OpenAI format.
pub(crate) fn tools_to_openai(tool_defs: &[ToolDefinition]) -> Vec<serde_json::Value> {
    tool_defs.iter().map(|t| {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            }
        })
    }).collect()
}

/// Non-streaming OpenAI chat completion.
pub(crate) async fn openai_complete(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
    base_url: &str,
) -> anyhow::Result<(String, Vec<ToolUseBlock>, String, u64)> {
    let oai_messages = messages_to_openai(system, messages);
    let tools = tools_to_openai(tool_defs);

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": oai_messages,
        "stream": false,
    });
    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(tools);
    }

    let resp = http
        .post(&format!("{base_url}/v1/chat/completions"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!("OpenAI API error {status}: {resp_text}"));
    }

    let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;

    let choice = &resp_json["choices"][0];
    let finish_reason = choice["finish_reason"].as_str().unwrap_or("stop");
    let stop_reason = match finish_reason {
        "tool_calls" => "tool_use",
        "length" => "max_tokens",
        _ => "end_turn",
    }.to_string();

    let message = &choice["message"];
    let text = message["content"].as_str().unwrap_or("").to_string();

    let mut tool_uses = Vec::new();
    if let Some(calls) = message["tool_calls"].as_array() {
        for tc in calls {
            let id = tc["id"].as_str().unwrap_or("").to_string();
            let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
            let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
            let input = serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
            tool_uses.push(ToolUseBlock { id, name, input });
        }
    }

    let input_tokens = resp_json["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let output_tokens = resp_json["usage"]["completion_tokens"].as_u64().unwrap_or(0);
    let tokens_used = (output_tokens as f64 + input_tokens as f64 * 0.2) as u64;

    Ok((text, tool_uses, stop_reason, tokens_used))
}

/// Streaming OpenAI chat completion.
pub(crate) async fn openai_stream(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    message_id: &str,
    base_url: &str,
) -> anyhow::Result<StreamOnceResult> {
    let oai_messages = messages_to_openai(system, messages);
    let tools = tools_to_openai(tool_defs);

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": oai_messages,
        "stream": true,
    });
    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(tools);
    }

    let resp = http
        .post(&format!("{base_url}/v1/chat/completions"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("OpenAI stream error {status}: {text}"));
    }

    let mut text = String::new();
    let mut tool_calls: std::collections::HashMap<usize, (String, String, String)> = std::collections::HashMap::new(); // idx -> (id, name, args)
    let mut stop_reason = String::new();
    let mut ordered_content = Vec::new();

    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    const STREAM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(480);

    loop {
        let chunk = tokio::time::timeout(STREAM_TIMEOUT, stream.next()).await;
        let chunk = match chunk {
            Ok(Some(Ok(c))) => c,
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => break,
            Err(_) => break,
        };

        buf.extend_from_slice(&chunk);

        while let Some(newline_pos) = buf.iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&buf[..newline_pos]).to_string();
            buf = buf[newline_pos + 1..].to_vec();

            let line = line.trim();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }

            let Some(data) = line.strip_prefix("data: ") else { continue };
            let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else { continue };

            let choice = &ev["choices"][0];

            // Finish reason
            if let Some(reason) = choice["finish_reason"].as_str() {
                stop_reason = match reason {
                    "tool_calls" => "tool_use",
                    "length" => "max_tokens",
                    _ => "end_turn",
                }.to_string();
            }

            let delta = &choice["delta"];

            // Text delta
            if let Some(content) = delta["content"].as_str() {
                text.push_str(content);
                let _ = events.send(ServerEvent::ChatStreamDelta {
                    instance_slug: instance_slug.to_string(),
                    chat_id: chat_id.to_string(),
                    message_id: message_id.to_string(),
                    delta: content.to_string(),
                });
            }

            // Tool call deltas
            if let Some(calls) = delta["tool_calls"].as_array() {
                for tc in calls {
                    let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                    let entry = tool_calls.entry(idx).or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = tc["id"].as_str() {
                        entry.0 = id.to_string();
                    }
                    if let Some(name) = tc["function"]["name"].as_str() {
                        entry.1.push_str(name);
                    }
                    if let Some(args) = tc["function"]["arguments"].as_str() {
                        entry.2.push_str(args);
                    }
                }
            }
        }
    }

    // Build tool use blocks
    let mut tool_uses: Vec<ToolUseBlock> = Vec::new();
    let mut indices: Vec<usize> = tool_calls.keys().copied().collect();
    indices.sort();
    for idx in indices {
        let (id, name, args_str) = &tool_calls[&idx];
        let input = serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
        tool_uses.push(ToolUseBlock { id: id.clone(), name: name.clone(), input });
    }

    if !text.is_empty() {
        ordered_content.push(ContentBlock::Text { text: text.clone() });
    }

    if stop_reason.is_empty() {
        stop_reason = "end_turn".to_string();
    }

    Ok(StreamOnceResult { text, tool_uses, stop_reason, tokens_used: 0, ordered_content })
}
