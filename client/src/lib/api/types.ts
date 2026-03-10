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
}

export interface ChatResponse {
	instance_slug: string;
	messages: ChatMessage[];
}

export interface InstanceSummary {
	slug: string;
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
	  };
