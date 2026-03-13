export type ChatRole = "user" | "assistant";

export type MessageKind = "message" | "tool_call" | "tool_output" | "mcp_app";

export interface ChatMessage {
	id: string;
	role: ChatRole;
	content: string;
	created_at: string;
	kind?: MessageKind;
	tool_name?: string;
	mcp_app_html?: string;
	mcp_app_input?: string;
}

export interface ChatRequest {
	instance_slug: string;
	content: string;
	chat_id?: string;
}

export interface ChatResponse {
	instance_slug: string;
	chat_id: string;
	messages: ChatMessage[];
	agent_running: boolean;
}

export interface ChatSummary {
	id: string;
	title: string;
	message_count: number;
	last_message_at: string | null;
	created_at: string;
}

export interface InstanceSummary {
	slug: string;
	companion_name: string;
	soul_exists: boolean;
	drops_count: number;
	has_memory: boolean;
	has_skin: boolean;
}

export interface LlmSummary {
	provider: "anthropic" | "openai" | null;
	model: string | null;
	openai_configured: boolean;
	anthropic_configured: boolean;
}

export interface ServerMeta {
	app: string;
	version: string;
	commit: string;
	port: number;
	workspace_dir: string;
	instances_count: number;
	skills_count: number;
	llm: LlmSummary;
}

export interface UpdateLlmRequest {
	provider: "anthropic" | "openai";
	model?: string;
	api_key: string;
}

export interface Soul {
	content: string;
	exists: boolean;
}

export interface SoulTemplate {
	id: string;
	name: string;
	description: string;
	content: string;
}

export type DropKind =
	| "thought"
	| "idea"
	| "poem"
	| "observation"
	| "reflection"
	| "recommendation"
	| "story"
	| "question"
	| "sketch"
	| "note";

export interface Drop {
	id: string;
	kind: DropKind;
	title: string;
	content: string;
	mood: string;
	created_at: string;
}

export interface Thought {
	id: string;
	raw: string;
	actions: string[];
	mood: string;
	created_at: string;
}

export interface SkillSource {
	repo: string;
	version: string;
}

export interface Skill {
	id: string;
	name: string;
	description: string;
	icon: string;
	builtin: boolean;
	enabled: boolean;
	instructions: string;
	source?: SkillSource;
	resources?: string[];
}

export interface RegistryEntry {
	id: string;
	name: string;
	description: string;
	icon: string;
	repo: string;
	git_ref: string;
	author: string;
	path: string;
	installed: boolean;
}

export interface UploadMeta {
	id: string;
	original_name: string;
	stored_name: string;
	mime_type: string;
	size: number;
	uploaded_at: string;
}

export interface Usage {
	messages_today: number;
	messages_limit: number;
	tokens_this_month: number;
	tokens_limit: number;
}

export interface ContextSection {
	name: string;
	chars: number;
	tokens: number;
}

export interface ContextStats {
	system_prompt: ContextSection[];
	system_prompt_total_tokens: number;
	tools: string[];
	tools_count: number;
	history_messages: number;
	history_tokens_estimate: number;
	total_input_tokens_estimate: number;
}

export interface HeartbeatUpdate {
	id: string;
	description: string;
	preview: string;
}

export type ServerEvent =
	| {
			type: "chat_message_created";
			instance_slug: string;
			chat_id: string;
			message: ChatMessage;
	  }
	| {
			type: "instance_discovered";
			instance: InstanceSummary;
	  }
	| {
			type: "mood_updated";
			instance_slug: string;
			mood: string;
	  }
	| {
			type: "agent_running";
			instance_slug: string;
			chat_id: string;
	  }
	| {
			type: "agent_stopped";
			instance_slug: string;
			chat_id: string;
	  }
	| {
			type: "tool_activity";
			instance_slug: string;
			chat_id: string;
			tool_name: string;
			summary: string;
	  }
	| {
			type: "drop_created";
			instance_slug: string;
			drop: Drop;
	  }
	| {
			type: "heartbeat_thought";
			instance_slug: string;
			thought: Thought;
	  }
	| {
			type: "context_compacting";
			instance_slug: string;
			chat_id: string;
			messages_compacted: number;
	  }
	| {
			type: "chat_stream_delta";
			instance_slug: string;
			chat_id: string;
			delta: string;
	  }
	| {
			type: "secret_request";
			instance_slug: string;
			id: string;
			prompt: string;
			target: string;
	  }
	| {
			type: "tool_output_chunk";
			instance_slug: string;
			chat_id: string;
			chunk: string;
	  }
;
