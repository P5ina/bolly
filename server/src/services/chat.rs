use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rig::providers::openai;

use rig::tool::ToolDyn;

use crate::{
    domain::chat::{ChatMessage, ChatRequest, ChatResponse, ChatRole},
    domain::instance::InstanceSummary,
    services::{
        llm::{self, LlmBackend},
        memory,
        tools::{EditSoulTool, ListFilesTool, ReadFileTool, WriteFileTool},
        workspace,
    },
};

static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);


pub async fn append_chat_turn(
    workspace_dir: &Path,
    request: ChatRequest,
    llm: Option<&LlmBackend>,
    embedding_model: Option<&openai::EmbeddingModel>,
) -> io::Result<ChatResponse> {
    let instance_slug = sanitize_slug(&request.instance_slug);
    if instance_slug.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "instance_slug cannot be empty",
        ));
    }

    let content = request.content.trim().to_string();
    if content.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "content cannot be empty",
        ));
    }

    ensure_instance_layout(workspace_dir, &instance_slug)?;

    let user_message = ChatMessage {
        id: next_id(),
        role: ChatRole::User,
        content: content.clone(),
        created_at: timestamp(),
    };

    // Generate reply via LLM or fall back to stub
    let reply = if let Some(backend) = llm {
        let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
        let memory_prompt = memory::retrieve_and_format(
            workspace_dir,
            &instance_slug,
            &content,
            embedding_model,
        )
        .await;
        let system_prompt = if memory_prompt.is_empty() {
            base_prompt
        } else {
            format!("{base_prompt}\n\n{memory_prompt}")
        };

        let existing = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;
        let history = llm::to_rig_messages(&existing);

        let tools = build_instance_tools(workspace_dir, &instance_slug);

        backend
            .chat_with_tools(&system_prompt, &content, history, tools)
            .await
            .unwrap_or_else(|e| {
                log::warn!("LLM call failed, using stub: {e}");
                stub_reply(&instance_slug, &content)
            })
    } else {
        stub_reply(&instance_slug, &content)
    };

    let assistant_message = ChatMessage {
        id: next_id(),
        role: ChatRole::Assistant,
        content: reply,
        created_at: timestamp(),
    };

    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;
    messages.push(user_message.clone());
    messages.push(assistant_message.clone());
    save_messages(workspace_dir, &instance_slug, &messages)?;

    // Spawn background memory extraction
    if let Some(backend) = llm {
        let backend = backend.clone();
        let emb = embedding_model.cloned();
        let ws = workspace_dir.to_path_buf();
        let slug = instance_slug.clone();
        let recent = vec![user_message.clone(), assistant_message.clone()];
        tokio::spawn(async move {
            if let Err(e) =
                memory::extract_and_store(&ws, &slug, &recent, &backend, emb.as_ref()).await
            {
                log::warn!("memory extraction failed: {e}");
            }
        });
    }

    Ok(ChatResponse {
        instance_slug,
        messages: vec![user_message, assistant_message],
    })
}

pub fn load_messages(workspace_dir: &Path, instance_slug: &str) -> io::Result<ChatResponse> {
    let instance_slug = sanitize_slug(instance_slug);
    let messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;

    Ok(ChatResponse {
        instance_slug,
        messages,
    })
}

pub fn discover_instance(
    workspace_dir: &Path,
    instance_slug: &str,
) -> io::Result<Option<InstanceSummary>> {
    let path = workspace_dir
        .join("instances")
        .join(sanitize_slug(instance_slug));
    Ok(workspace::summarize_instance(&path))
}

fn ensure_instance_layout(workspace_dir: &Path, instance_slug: &str) -> io::Result<()> {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    fs::create_dir_all(instance_dir.join("chat"))?;
    fs::create_dir_all(instance_dir.join("drops"))?;
    fs::create_dir_all(instance_dir.join("memory"))?;
    Ok(())
}

fn messages_path(workspace_dir: &Path, instance_slug: &str) -> PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chat")
        .join("messages.json")
}

fn load_messages_vec(path: &Path) -> io::Result<Vec<ChatMessage>> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn save_messages(
    workspace_dir: &Path,
    instance_slug: &str,
    messages: &[ChatMessage],
) -> io::Result<()> {
    let path = messages_path(workspace_dir, instance_slug);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let body = serde_json::to_string_pretty(messages)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
    fs::write(path, body)
}

fn stub_reply(instance_slug: &str, content: &str) -> String {
    format!(
        "i heard you, {instance_slug}. i just can't think yet — no language model is configured. \
         you said: \"{content}\""
    )
}

fn sanitize_slug(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect()
}

fn next_id() -> String {
    format!(
        "msg_{}_{}",
        unix_millis(),
        MESSAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn timestamp() -> String {
    unix_millis().to_string()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}

fn build_instance_tools(workspace_dir: &Path, instance_slug: &str) -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(EditSoulTool::new(workspace_dir, instance_slug)),
        Box::new(ReadFileTool::new(workspace_dir, instance_slug)),
        Box::new(WriteFileTool::new(workspace_dir, instance_slug)),
        Box::new(ListFilesTool::new(workspace_dir, instance_slug)),
    ]
}
