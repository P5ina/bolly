export type ChatRole = "user" | "assistant";

export interface ChatMessage {
	id: string;
	role: ChatRole;
	content: string;
	created_at: string;
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

export interface Skill {
	id: string;
	name: string;
	description: string;
	icon: string;
	builtin: boolean;
	enabled: boolean;
	instructions: string;
}

export interface UploadMeta {
	id: string;
	original_name: string;
	stored_name: string;
	mime_type: string;
	size: number;
	uploaded_at: string;
}

export type ServerEvent =
	| {
			type: "chat_message_created";
			instance_slug: string;
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
	  }
	| {
			type: "agent_stopped";
			instance_slug: string;
	  }
	| {
			type: "tool_activity";
			instance_slug: string;
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
			messages_compacted: number;
	  };
