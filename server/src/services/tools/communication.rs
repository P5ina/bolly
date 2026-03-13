use std::{fs, path::{Path, PathBuf}};

use chrono::Utc;
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::{openai_schema, ToolExecError};
use super::companion::{load_mood_state, save_mood_state};
use crate::domain::events::ServerEvent;

// ---------------------------------------------------------------------------
// Email config helpers
// ---------------------------------------------------------------------------

pub fn load_instance_email_config(
    instance_dir: &Path,
) -> Result<crate::config::EmailConfig, ToolExecError> {
    let path = instance_dir.join("email.toml");
    if !path.exists() {
        return Ok(crate::config::EmailConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| ToolExecError(format!("failed to read email config: {e}")))?;
    toml::from_str(&raw).map_err(|e| ToolExecError(format!("failed to parse email config: {e}")))
}

pub fn save_instance_email_config(
    instance_dir: &Path,
    config: &crate::config::EmailConfig,
) -> Result<(), ToolExecError> {
    fs::create_dir_all(instance_dir)
        .map_err(|e| ToolExecError(format!("failed to create instance dir: {e}")))?;
    let output = toml::to_string_pretty(config)
        .map_err(|e| ToolExecError(format!("failed to serialize email config: {e}")))?;
    fs::write(instance_dir.join("email.toml"), &output)
        .map_err(|e| ToolExecError(format!("failed to write email config: {e}")))
}

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
            tool_name: None,
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
// send_email
// ---------------------------------------------------------------------------

pub struct SendEmailTool {
    instance_dir: PathBuf,
}

impl SendEmailTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
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
}

impl Tool for SendEmailTool {
    const NAME: &'static str = "send_email";
    type Error = ToolExecError;
    type Args = SendEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_email".into(),
            description: "Send an email via SMTP. Requires email settings in config. \
                Use this to communicate with people outside the chat — send updates, \
                share ideas, follow up on conversations."
                .into(),
            parameters: openai_schema::<SendEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        use lettre::{
            AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
            message::header::ContentType, transport::smtp::authentication::Credentials,
        };

        let config = load_instance_email_config(&self.instance_dir)?;

        if !config.is_smtp_configured() {
            return Err(ToolExecError(
                "SMTP not configured. Set smtp_host, smtp_user, smtp_password in config.toml [email] section.".into(),
            ));
        }

        let from = if config.smtp_from.is_empty() {
            config.smtp_user.clone()
        } else {
            config.smtp_from.clone()
        };

        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| ToolExecError(format!("invalid from address: {e}")))?,
            )
            .to(args
                .to
                .parse()
                .map_err(|e| ToolExecError(format!("invalid to address: {e}")))?)
            .subject(&args.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(args.body)
            .map_err(|e| ToolExecError(format!("failed to build email: {e}")))?;

        let creds = Credentials::new(config.smtp_user.clone(), config.smtp_password.clone());

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
            .map_err(|e| ToolExecError(format!("SMTP connection failed: {e}")))?
            .port(config.smtp_port)
            .credentials(creds)
            .build();

        mailer
            .send(email)
            .await
            .map_err(|e| ToolExecError(format!("failed to send email: {e}")))?;

        Ok(format!("email sent to {}", args.to))
    }
}

// ---------------------------------------------------------------------------
// read_email
// ---------------------------------------------------------------------------

pub struct ReadEmailTool {
    instance_dir: PathBuf,
}

impl ReadEmailTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadEmailArgs {
    /// Number of recent emails to fetch (default 5, max 20).
    #[serde(default = "default_email_count")]
    pub count: u32,
    /// Mailbox to read from (default "INBOX").
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
}

fn default_email_count() -> u32 {
    5
}

fn default_mailbox() -> String {
    "INBOX".into()
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
                "Read recent emails via IMAP. Returns subject, from, date, and body preview \
                for the most recent messages. Requires email settings in config."
                    .into(),
            parameters: openai_schema::<ReadEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let config = load_instance_email_config(&self.instance_dir)?;

        if !config.is_imap_configured() {
            return Err(ToolExecError(
                "IMAP not configured. Set imap_host, imap_user, imap_password in config.toml [email] section.".into(),
            ));
        }

        let count = args.count.min(20).max(1);

        use tokio_util::compat::TokioAsyncReadCompatExt;

        let tls = async_native_tls::TlsConnector::new();
        let tcp = tokio::net::TcpStream::connect((config.imap_host.as_str(), config.imap_port))
            .await
            .map_err(|e| ToolExecError(format!("IMAP TCP connection failed: {e}")))?;
        let tls_stream = tls
            .connect(&config.imap_host, tcp.compat())
            .await
            .map_err(|e| ToolExecError(format!("IMAP TLS failed: {e}")))?;

        let client = async_imap::Client::new(tls_stream);

        let mut session = client
            .login(&config.imap_user, &config.imap_password)
            .await
            .map_err(|e| ToolExecError(format!("IMAP login failed: {}", e.0)))?;

        let mailbox = session
            .select(&args.mailbox)
            .await
            .map_err(|e| ToolExecError(format!("failed to select {}: {e}", args.mailbox)))?;

        let total = mailbox.exists;
        if total == 0 {
            let _ = session.logout().await;
            return Ok("no emails in mailbox".into());
        }

        let start = total.saturating_sub(count) + 1;
        let range = format!("{start}:{total}");

        let messages_stream = session
            .fetch(&range, "(ENVELOPE BODY[TEXT])")
            .await
            .map_err(|e| ToolExecError(format!("IMAP fetch failed: {e}")))?;

        use futures::TryStreamExt;
        let fetched: Vec<_> = messages_stream
            .try_collect()
            .await
            .map_err(|e| ToolExecError(format!("IMAP stream error: {e}")))?;

        let mut result = String::new();
        for msg in &fetched {
            if let Some(envelope) = msg.envelope() {
                let subject = envelope
                    .subject
                    .as_ref()
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .unwrap_or_else(|| "(no subject)".into());
                let from = envelope
                    .from
                    .as_ref()
                    .and_then(|addrs| addrs.first())
                    .map(|a| {
                        let name = a
                            .name
                            .as_ref()
                            .map(|n| String::from_utf8_lossy(n).to_string());
                        let mailbox_part = a
                            .mailbox
                            .as_ref()
                            .map(|m| String::from_utf8_lossy(m).to_string())
                            .unwrap_or_default();
                        let host = a
                            .host
                            .as_ref()
                            .map(|h| String::from_utf8_lossy(h).to_string())
                            .unwrap_or_default();
                        if let Some(n) = name {
                            format!("{n} <{mailbox_part}@{host}>")
                        } else {
                            format!("{mailbox_part}@{host}")
                        }
                    })
                    .unwrap_or_else(|| "(unknown)".into());
                let date = envelope
                    .date
                    .as_ref()
                    .map(|d| String::from_utf8_lossy(d).to_string())
                    .unwrap_or_default();

                result.push_str(&format!(
                    "--- email ---\nfrom: {from}\ndate: {date}\nsubject: {subject}\n"
                ));
            }
            if let Some(body) = msg.text() {
                let text = String::from_utf8_lossy(body);
                let preview: String = text.chars().take(500).collect();
                result.push_str(&format!("body:\n{preview}\n"));
                if text.len() > 500 {
                    result.push_str("...(truncated)\n");
                }
            }
            result.push('\n');
        }

        let _ = session.logout().await;

        if result.is_empty() {
            Ok("no emails found".into())
        } else {
            Ok(result)
        }
    }
}
