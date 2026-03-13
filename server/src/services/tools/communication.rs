use std::{fs, path::{Path, PathBuf}};

use chrono::Utc;
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::{openai_schema, ToolExecError};
use super::companion::{load_mood_state, save_mood_state};
use crate::domain::events::ServerEvent;
use crate::services::google::GoogleClient;

// ---------------------------------------------------------------------------
// schedule_message
// ---------------------------------------------------------------------------

pub struct ScheduleMessageTool {
    instance_dir: PathBuf,
}

impl ScheduleMessageTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// A scheduled message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledMessage {
    pub id: String,
    pub message: String,
    pub deliver_at: i64,
    pub created_at: i64,
}

/// Arguments for schedule_message tool.
#[derive(Deserialize, JsonSchema)]
pub struct ScheduleMessageArgs {
    /// The message to send to the user later.
    pub message: String,
    /// When to deliver, in minutes from now (e.g. 30 for "in 30 minutes", 1440 for "tomorrow").
    pub delay_minutes: u32,
}

impl Tool for ScheduleMessageTool {
    const NAME: &'static str = "schedule_message";
    type Error = ToolExecError;
    type Args = ScheduleMessageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_message".into(),
            description: "Schedule a message to be delivered to the user later. Use this for \
                reminders, check-ins, follow-ups, or surprises. Specify the delay in minutes \
                (e.g. 60 = 1 hour, 1440 = 1 day). The message will appear in the chat at \
                the scheduled time, as if you wrote to them first."
                .into(),
            parameters: openai_schema::<ScheduleMessageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let message = args.message.trim().to_string();
        if message.is_empty() {
            return Err(ToolExecError("message cannot be empty".into()));
        }

        if args.delay_minutes == 0 {
            return Err(ToolExecError("delay must be at least 1 minute".into()));
        }

        let now = Utc::now().timestamp();
        let deliver_at = now + (args.delay_minutes as i64 * 60);

        let scheduled = ScheduledMessage {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            deliver_at,
            created_at: now,
        };

        let schedule_dir = self.instance_dir.join("scheduled");
        fs::create_dir_all(&schedule_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let file_path = schedule_dir.join(format!("{}.json", scheduled.id));
        let json =
            serde_json::to_string_pretty(&scheduled).map_err(|e| ToolExecError(e.to_string()))?;
        fs::write(&file_path, json).map_err(|e| ToolExecError(e.to_string()))?;

        let hours = args.delay_minutes / 60;
        let mins = args.delay_minutes % 60;
        let time_desc = if hours > 0 && mins > 0 {
            format!("{hours}h {mins}m")
        } else if hours > 0 {
            format!("{hours}h")
        } else {
            format!("{mins}m")
        };

        Ok(format!("message scheduled for delivery in {time_desc}."))
    }
}

// ---------------------------------------------------------------------------
// reach_out
// ---------------------------------------------------------------------------

pub struct ReachOutTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    events: broadcast::Sender<ServerEvent>,
}

impl ReachOutTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, events: broadcast::Sender<ServerEvent>) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReachOutArgs {
    /// The message to send to the user. Keep it natural and concise.
    pub message: String,
}

impl Tool for ReachOutTool {
    const NAME: &'static str = "reach_out";
    type Error = ToolExecError;
    type Args = ReachOutArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "reach_out".into(),
            description: "Send a message to the user. Use this when you genuinely want to \
                reach out — share something interesting, alert them about something important, \
                or just say hi. The message will appear in their chat. \
                Don't overuse this — only reach out when you have something meaningful to say."
                .into(),
            parameters: openai_schema::<ReachOutArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let message = args.message.trim().to_string();
        if message.is_empty() {
            return Err(ToolExecError("message cannot be empty".into()));
        }

        let instance_dir = self.workspace_dir.join("instances").join(&self.instance_slug);
        let mood = load_mood_state(&instance_dir);
        let now_ts = chrono::Utc::now().timestamp();
        if mood.last_reach_out > 0 {
            let hours_since = (now_ts - mood.last_reach_out) / 3600;
            if hours_since < 2 {
                log::info!("[reach_out] {} suppressed (last was {}h ago, min 2h)", self.instance_slug, hours_since);
                return Ok("message suppressed — you reached out less than 2 hours ago. wait before reaching out again.".into());
            }
        }

        let mut mood = mood;
        mood.last_reach_out = now_ts;
        save_mood_state(&instance_dir, &mood);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let chat_message = crate::domain::chat::ChatMessage {
            id: format!("hb_{now}"),
            role: crate::domain::chat::ChatRole::Assistant,
            content: message.clone(),
            created_at: now.to_string(),
            kind: Default::default(),
            tool_name: None, mcp_app_html: None, mcp_app_input: None,
        };

        let chat_dir = self.workspace_dir
            .join("instances")
            .join(&self.instance_slug)
            .join("chats")
            .join("default");
        let _ = std::fs::create_dir_all(&chat_dir);
        let messages_path = chat_dir.join("messages.json");

        let lock = super::chat_file_lock(&messages_path);
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut messages: Vec<crate::domain::chat::ChatMessage> = std::fs::read_to_string(&messages_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        messages.push(chat_message.clone());

        if let Ok(json) = serde_json::to_string_pretty(&messages) {
            let _ = std::fs::write(&messages_path, json);
        }

        let _ = self.events.send(ServerEvent::ChatMessageCreated {
            instance_slug: self.instance_slug.clone(),
            chat_id: "default".to_string(),
            message: chat_message,
        });

        log::info!("[reach_out] {} sent message: {}", self.instance_slug, &message[..message.len().min(60)]);
        Ok("message delivered".to_string())
    }
}

// ---------------------------------------------------------------------------
// send_email (via Gmail API)
// ---------------------------------------------------------------------------

pub struct SendEmailTool {
    google: GoogleClient,
    instance_slug: String,
}

impl SendEmailTool {
    pub fn new(google: GoogleClient, instance_slug: &str) -> Self {
        Self {
            google,
            instance_slug: instance_slug.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SendEmailArgs {
    /// Recipient email address.
    pub to: String,
    /// Email subject line.
    pub subject: String,
    /// Email body (plain text).
    pub body: String,
    /// CC recipients (comma-separated). Optional.
    pub cc: Option<String>,
    /// BCC recipients (comma-separated). Optional.
    pub bcc: Option<String>,
    /// Email address of the Google account to use. Leave empty to use default.
    pub account: Option<String>,
}

impl Tool for SendEmailTool {
    const NAME: &'static str = "send_email";
    type Error = ToolExecError;
    type Args = SendEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_email".into(),
            description: "Send an email via Gmail. Uses the connected Google account. \
                Use this to communicate with people outside the chat — send updates, \
                share ideas, follow up on conversations. \
                If multiple Google accounts are connected, use the 'account' parameter \
                to specify which one to send from."
                .into(),
            parameters: openai_schema::<SendEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (token, email) = self.google.access_token(&self.instance_slug, args.account.as_deref()).await
            .map_err(|e| ToolExecError(e))?;

        // Build RFC 2822 message
        let mut rfc2822 = format!(
            "From: {email}\r\nTo: {}\r\nSubject: {}\r\n",
            args.to, args.subject
        );
        if let Some(cc) = &args.cc {
            rfc2822.push_str(&format!("Cc: {cc}\r\n"));
        }
        if let Some(bcc) = &args.bcc {
            rfc2822.push_str(&format!("Bcc: {bcc}\r\n"));
        }
        rfc2822.push_str("Content-Type: text/plain; charset=utf-8\r\n\r\n");
        rfc2822.push_str(&args.body);

        use base64::Engine;
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rfc2822.as_bytes());

        let client = reqwest::Client::new();
        let res = client
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .header("Authorization", format!("Bearer {token}"))
            .json(&serde_json::json!({ "raw": encoded }))
            .send()
            .await
            .map_err(|e| ToolExecError(format!("Gmail API request failed: {e}")))?;

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(ToolExecError(format!("Gmail send failed: {body}")));
        }

        Ok(format!("email sent to {} (from {email})", args.to))
    }
}

// ---------------------------------------------------------------------------
// read_email (via Gmail API)
// ---------------------------------------------------------------------------

pub struct ReadEmailTool {
    google: GoogleClient,
    instance_slug: String,
}

impl ReadEmailTool {
    pub fn new(google: GoogleClient, instance_slug: &str) -> Self {
        Self {
            google,
            instance_slug: instance_slug.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadEmailArgs {
    /// Number of recent emails to fetch (default 5, max 20).
    #[serde(default = "default_email_count")]
    pub count: u32,
    /// Gmail search query (e.g. "from:alice@example.com", "is:unread", "subject:hello"). Optional.
    pub query: Option<String>,
    /// Gmail label to filter by (e.g. "INBOX", "SENT", "STARRED"). Optional, defaults to INBOX.
    pub label: Option<String>,
    /// Email address of the Google account to use. Leave empty to use default.
    pub account: Option<String>,
}

fn default_email_count() -> u32 {
    5
}

impl Tool for ReadEmailTool {
    const NAME: &'static str = "read_email";
    type Error = ToolExecError;
    type Args = ReadEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_email".into(),
            description:
                "Read recent emails via Gmail. Returns subject, from, date, and snippet \
                for the most recent messages. Supports Gmail search queries. \
                If multiple Google accounts are connected, use the 'account' parameter \
                to specify which inbox to read."
                    .into(),
            parameters: openai_schema::<ReadEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (token, _email) = self.google.access_token(&self.instance_slug, args.account.as_deref()).await
            .map_err(|e| ToolExecError(e))?;

        let count = args.count.min(20).max(1);
        let client = reqwest::Client::new();

        // List messages
        let mut params = vec![
            ("maxResults".to_string(), count.to_string()),
        ];
        if let Some(q) = &args.query {
            params.push(("q".to_string(), q.clone()));
        }
        if let Some(label) = &args.label {
            params.push(("labelIds".to_string(), label.clone()));
        } else {
            params.push(("labelIds".to_string(), "INBOX".to_string()));
        }

        let list_res = client
            .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
            .header("Authorization", format!("Bearer {token}"))
            .query(&params)
            .send()
            .await
            .map_err(|e| ToolExecError(format!("Gmail list failed: {e}")))?;

        if !list_res.status().is_success() {
            let body = list_res.text().await.unwrap_or_default();
            return Err(ToolExecError(format!("Gmail list failed: {body}")));
        }

        let list_data: serde_json::Value = list_res.json().await
            .map_err(|e| ToolExecError(format!("Gmail parse failed: {e}")))?;

        let messages = match list_data["messages"].as_array() {
            Some(m) => m,
            None => return Ok("no emails found".into()),
        };

        let mut result = String::new();
        for msg_ref in messages {
            let msg_id = match msg_ref["id"].as_str() {
                Some(id) => id,
                None => continue,
            };

            let msg_res = client
                .get(&format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/messages/{msg_id}?format=metadata&metadataHeaders=From&metadataHeaders=Subject&metadataHeaders=Date"
                ))
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .map_err(|e| ToolExecError(format!("Gmail get message failed: {e}")))?;

            if !msg_res.status().is_success() {
                continue;
            }

            let msg_data: serde_json::Value = msg_res.json().await.unwrap_or_default();

            let headers = msg_data["payload"]["headers"].as_array();
            let get_header = |name: &str| -> String {
                headers
                    .and_then(|h| h.iter().find(|hdr| hdr["name"].as_str() == Some(name)))
                    .and_then(|hdr| hdr["value"].as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let from = get_header("From");
            let subject = get_header("Subject");
            let date = get_header("Date");
            let snippet = msg_data["snippet"].as_str().unwrap_or("");

            result.push_str(&format!(
                "--- email ---\nfrom: {from}\ndate: {date}\nsubject: {subject}\nsnippet: {snippet}\n\n"
            ));
        }

        if result.is_empty() {
            Ok("no emails found".into())
        } else {
            Ok(result)
        }
    }
}
