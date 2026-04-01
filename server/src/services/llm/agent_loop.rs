use std::path::Path;

use tokio::sync::broadcast;

use crate::domain::chat::{ChatMessage, ChatRole};
use crate::domain::events::ServerEvent;
use crate::services::tool::{ToolDefinition, ToolDyn};

use super::anthropic::{anthropic_complete, anthropic_stream};
use super::helpers::{estimate_history_tokens, strip_context_blocks};
use super::openai::{openai_complete, openai_stream};
use super::types::{ContentBlock, HistoryEntry, LlmBackend, Message, StreamOnceResult, ToolUseBlock};

// ═══════════════════════════════════════════════════════════════════════════
// Agent loops (tool call -> execute -> send back)
// ═══════════════════════════════════════════════════════════════════════════

pub(crate) async fn collect_tool_defs(tools: &[Box<dyn ToolDyn>]) -> Vec<ToolDefinition> {
    let mut defs = Vec::with_capacity(tools.len());
    for t in tools {
        defs.push(t.definition(String::new()).await);
    }
    defs
}

/// Non-streaming agent loop. Returns (final text, total tokens used).
pub(crate) async fn agent_loop(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    tools: &[Box<dyn ToolDyn>],
    messages: &mut Vec<Message>,
) -> anyhow::Result<(String, u64)> {
    let mut total_tokens: u64 = 0;
    loop {
        let (text, tool_uses, stop_reason, tokens) = complete_once(backend, system, tool_defs, messages).await?;
        total_tokens += tokens;

        // Build assistant message
        let mut assistant_content = Vec::new();
        if !text.is_empty() {
            assistant_content.push(ContentBlock::text(&text));
        }
        for tu in &tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: tu.id.clone(),
                name: tu.name.clone(),
                input: tu.input.clone(),
            });
        }
        messages.push(Message::Assistant {
            content: assistant_content,
        });

        if stop_reason == "max_tokens" {
            log::warn!("[llm] response truncated (max_tokens reached) — requesting continuation");
            messages.push(Message::User {
                content: vec![ContentBlock::text(
                    "[system: your previous response was cut off due to length. please continue exactly where you left off.]",
                )],
            });
            continue;
        }

        if stop_reason == "pause_turn" {
            log::info!("[llm] pause_turn — code execution in progress, continuing...");
            continue;
        }

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            return Ok((text, total_tokens));
        }

        // Execute tools — images stay inside tool_result content per Anthropic API spec
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        messages.push(Message::User { content: results });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Context compaction — provider-agnostic
// ═══════════════════════════════════════════════════════════════════════════

const COMPACT_TOKEN_THRESHOLD: usize = 100_000;
const COMPACT_KEEP_MESSAGES: usize = 10;
const COMPACT_KEEP_TOKENS: usize = 20_000;

/// Flatten messages to a plain-text transcript for the summarizer.
fn messages_to_transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for msg in messages {
        let (role, content) = match msg {
            Message::User { content } => ("User", content),
            Message::Assistant { content } => ("Assistant", content),
        };
        out.push_str(&format!("\n{role}:\n"));
        for block in content {
            match block {
                ContentBlock::Text { text } => {
                    if text.starts_with("[current time:") || text.starts_with("[system: auto-recalled") {
                        continue;
                    }
                    let truncated: String = text.chars().take(2000).collect();
                    out.push_str(&truncated);
                    out.push('\n');
                }
                ContentBlock::Compaction { content } => {
                    out.push_str(&format!("[Previous summary: {content}]\n"));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    let input_str: String = input.to_string().chars().take(300).collect();
                    out.push_str(&format!("[Called tool: {name}({input_str})]\n"));
                }
                ContentBlock::ToolResult { content, .. } => {
                    let s = content.as_str().unwrap_or("(non-text)");
                    let truncated: String = s.chars().take(500).collect();
                    out.push_str(&format!("[Tool result: {truncated}]\n"));
                }
                _ => {}
            }
        }
    }
    out
}

/// Compact history if it exceeds the token threshold.
/// Returns true if compaction was performed.
async fn maybe_compact_history(
    backend: &LlmBackend,
    messages: &mut Vec<Message>,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    workspace_dir: &Path,
) -> bool {
    let total_tokens = estimate_history_tokens(messages);
    if total_tokens < COMPACT_TOKEN_THRESHOLD {
        return false;
    }

    log::info!(
        "[compaction] history ~{total_tokens} tokens (threshold {COMPACT_TOKEN_THRESHOLD}) — compacting"
    );

    let msg_count = messages.len();

    // Determine split point: keep recent messages from the end
    let mut keep_from = msg_count.saturating_sub(COMPACT_KEEP_MESSAGES);

    // Ensure we keep at least COMPACT_KEEP_TOKENS worth of recent context
    let mut recent_tokens = 0usize;
    for i in (0..msg_count).rev() {
        recent_tokens += estimate_history_tokens(&messages[i..=i]);
        if recent_tokens >= COMPACT_KEEP_TOKENS {
            keep_from = keep_from.min(i);
            break;
        }
    }

    // Never split a tool_use/tool_result pair
    if keep_from > 0 {
        if let Message::User { content } = &messages[keep_from] {
            if content.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. })) {
                keep_from -= 1;
            }
        }
    }

    if keep_from <= 1 {
        log::info!("[compaction] too few messages to compact — skipping");
        return false;
    }

    let messages_to_compact = &messages[..keep_from];
    let compacted_count = messages_to_compact.len();

    // Broadcast UI event
    let _ = events.send(ServerEvent::ContextCompacting {
        instance_slug: instance_slug.to_string(),
        chat_id: chat_id.to_string(),
        messages_compacted: compacted_count,
    });

    // Build transcript and summarize with the cheap model
    let transcript = messages_to_transcript(messages_to_compact);
    let system_prompt = "\
        You are a conversation summarizer. Produce a concise summary of the conversation transcript below.\n\
        Capture:\n\
        1. Key facts, decisions, and user preferences\n\
        2. Current task state and goals\n\
        3. Important tool calls and their outcomes\n\
        4. Pending work or unresolved questions\n\
        Be factual and specific. Do not editorialize. Write in third person.\n\
        Target 1000-2000 words. If there is a previous summary, incorporate it.";

    let summary = match backend.chat(system_prompt, &transcript, vec![]).await {
        Ok((text, _tokens)) => {
            log::info!("[compaction] summary generated: {} chars", text.len());
            text
        }
        Err(e) => {
            log::error!("[compaction] summarization failed: {e} — skipping");
            return false;
        }
    };

    // Replace old messages with compaction block + recent messages
    let recent: Vec<Message> = messages[keep_from..].to_vec();
    messages.clear();
    messages.push(Message::Assistant {
        content: vec![ContentBlock::Compaction { content: summary }],
    });
    messages.extend(recent);

    // Persist compacted history to disk
    let rig_path = crate::services::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
    let ts = crate::services::tools::unix_millis().to_string();
    let entries: Vec<HistoryEntry> = messages.iter().enumerate().map(|(i, msg)| {
        HistoryEntry::new(msg.clone(), ts.clone(), format!("compact_{i}_{ts}"))
    }).collect();
    crate::services::chat::save_rig_history(&rig_path, &entries);

    // Broadcast snapshot so the client immediately reflects the compacted state
    if let Ok(resp) = crate::services::chat::load_messages(workspace_dir, instance_slug, chat_id) {
        let _ = events.send(ServerEvent::ChatSnapshot {
            instance_slug: instance_slug.to_string(),
            chat_id: chat_id.to_string(),
            messages: resp.messages,
            agent_running: true,
        });
    }

    log::info!(
        "[compaction] compacted {compacted_count} messages → {} remaining",
        messages.len(),
    );
    true
}

/// Streaming agent loop. Returns (final text, message_id, total tokens).
pub(crate) async fn streaming_agent_loop(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    tools: &[Box<dyn ToolDyn>],
    messages: &mut Vec<Message>,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    workspace_dir: &Path,
    mcp_snapshot: Option<&crate::services::mcp::McpAppSnapshot>,
    sent_files: &crate::services::tools::SentFiles,
) -> anyhow::Result<(String, Option<String>, u64)> {
    let mut all_text = String::new();
    let mut total_tokens: u64 = 0;
    let mut current_message_id = crate::services::chat::next_id();

    // Pre-call compaction: summarize old messages if context is too large
    maybe_compact_history(backend, messages, events, instance_slug, chat_id, workspace_dir).await;

    loop {
        let turn = stream_once(
            backend, system, tool_defs, messages, events,
            instance_slug, chat_id, &current_message_id, mcp_snapshot,
        ).await?;

        total_tokens += turn.tokens_used;
        let turn_text = turn.text;
        let tool_uses = turn.tool_uses;
        let stop_reason = turn.stop_reason;


        // Build assistant message — use ordered_content which preserves
        // the interleaving of text, server_tool_use, and server_tool_result.
        let mut assistant_content = Vec::new();
        // Ordered content: text and server tool blocks in their original order
        assistant_content.extend(turn.ordered_content.into_iter());
        for tu in &tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: tu.id.clone(),
                name: tu.name.clone(),
                input: tu.input.clone(),
            });
        }
        messages.push(Message::Assistant {
            content: assistant_content,
        });

        if stop_reason == "max_tokens" {
            log::warn!("[llm] response truncated (max_tokens reached) — requesting continuation");
            all_text.push_str(&turn_text);
            messages.push(Message::User {
                content: vec![ContentBlock::text(
                    "[system: your previous response was cut off due to length. please continue exactly where you left off.]",
                )],
            });
            continue;
        }

        // pause_turn: code execution skill is still running — continue with same messages
        if stop_reason == "pause_turn" {
            log::info!("[llm] pause_turn — code execution in progress, continuing...");
            all_text.push_str(&turn_text);
            continue;
        }

        // For the final turn (no more tool use), only keep this turn's text.
        all_text = turn_text.clone();

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            break;
        }

        // Save intermediate text before tool execution — reuse the streaming message_id
        if !turn_text.trim().is_empty() {
            let ts = crate::services::tools::unix_millis();
            let msg = ChatMessage {
                id: current_message_id.clone(),
                role: ChatRole::Assistant,
                content: turn_text.trim().to_string(),
                created_at: ts.to_string(),
                kind: Default::default(),
                tool_name: None,
                mcp_app_html: None,
                mcp_app_input: None, model: None,
            };
            let _ = events.send(ServerEvent::ChatMessageCreated {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                message: msg,
            });
            // Generate new ID for the next streaming turn
            current_message_id = crate::services::chat::next_id();
        }

        // Execute tools — images stay inside tool_result content per Anthropic API spec
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        let tool_result_msg = Message::User { content: results };
        messages.push(tool_result_msg.clone());

        // Append new messages to rig_history (append-only, no merge).
        let rig_path = crate::services::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
        let ts = crate::services::tools::unix_millis().to_string();
        // The assistant message (with tool_use) was pushed to messages a few lines above
        let assistant_msg = &messages[messages.len() - 2]; // assistant before tool_result
        crate::services::chat::append_to_rig_history(&rig_path, &HistoryEntry::new(
            strip_context_blocks(assistant_msg), ts.clone(), format!("tool_{}", crate::services::tools::unix_millis()),
        ));
        crate::services::chat::append_to_rig_history(&rig_path, &HistoryEntry::new(
            strip_context_blocks(&tool_result_msg), ts, format!("tool_{}", crate::services::tools::unix_millis()),
        ));

        // Snapshot after each tool cycle — all clients converge to ground truth
        if let Ok(resp) = crate::services::chat::load_messages(workspace_dir, instance_slug, chat_id) {
            let _ = events.send(ServerEvent::ChatSnapshot {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                messages: resp.messages,
                agent_running: true,
            });
        }

        // Re-check compaction after tool cycles (long chains grow fast)
        maybe_compact_history(backend, messages, events, instance_slug, chat_id, workspace_dir).await;
    }

    // -- Final assembly: file markers from send_file accumulated during the agent loop --
    let final_markers: Vec<String> = {
        let mut sf = sent_files.lock().unwrap_or_else(|e| e.into_inner());
        sf.drain(..).collect()
    };

    // Append all markers to the last assistant message in rig_history
    if !final_markers.is_empty() {
        if let Some(Message::Assistant { content }) = messages.last_mut() {
            for m in &final_markers {
                content.push(ContentBlock::text(m));
            }
        }
    }

    // Stamp model name on last assistant entry

    // Final save: append the last assistant message to rig_history.
    // Tool-cycle messages were already appended during the loop.
    // Only the final response (no more tool_use) needs to be saved here.
    let rig_path = crate::services::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
    if let Some(last_msg) = messages.last() {
        if matches!(last_msg, Message::Assistant { .. }) {
            let ts = crate::services::tools::unix_millis().to_string();
            let mut entry = HistoryEntry::new(
                strip_context_blocks(last_msg),
                ts,
                format!("msg_{}", crate::services::tools::unix_millis()),
            );
            entry.model = Some(backend.model.clone());
            crate::services::chat::append_to_rig_history(&rig_path, &entry);
        }
    }

    // Final snapshot so client converges to ground truth
    if let Ok(resp) = crate::services::chat::load_messages(workspace_dir, instance_slug, chat_id) {
        let _ = events.send(ServerEvent::ChatSnapshot {
            instance_slug: instance_slug.to_string(),
            chat_id: chat_id.to_string(),
            messages: resp.messages,
            agent_running: true,
        });
    }

    Ok((all_text, Some(current_message_id), total_tokens))
}

pub(crate) async fn execute_tool(tools: &[Box<dyn ToolDyn>], name: &str, input: &serde_json::Value) -> String {
    if let Some(tool) = tools.iter().find(|t| t.name() == name) {
        let args = serde_json::to_string(input).unwrap_or_default();
        match tool.call(args).await {
            Ok(s) => s,
            Err(e) => format!("error: {e}"),
        }
    } else {
        format!("error: unknown tool '{name}'")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Provider dispatch — route to Anthropic or OpenAI
// ═══════════════════════════════════════════════════════════════════════════

/// Non-streaming completion. Returns (text, tool_uses, stop_reason, tokens).
pub(crate) async fn complete_once(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
) -> anyhow::Result<(String, Vec<ToolUseBlock>, String, u64)> {
    if backend.provider.is_openai_format() {
        openai_complete(&backend.http, &backend.api_key, &backend.model, system, tool_defs, messages, 16384, &backend.base_url).await
    } else {
        anthropic_complete(&backend.http, &backend.api_key, &backend.model, system, tool_defs, messages, 16384, &backend.base_url).await
    }
}

/// Streaming dispatch: route to provider-specific streaming.
pub(crate) async fn stream_once(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    message_id: &str,
    mcp_snapshot: Option<&crate::services::mcp::McpAppSnapshot>,
) -> anyhow::Result<StreamOnceResult> {
    if backend.provider.is_openai_format() {
        openai_stream(
            &backend.http, &backend.api_key, &backend.model, system, tool_defs, messages,
            16384, events, instance_slug, chat_id, message_id, &backend.base_url,
        ).await
    } else {
        anthropic_stream(
            &backend.http, &backend.api_key, &backend.model, system, tool_defs, messages,
            16384, events, instance_slug, chat_id, message_id, mcp_snapshot, &backend.base_url,
        ).await
    }
}
