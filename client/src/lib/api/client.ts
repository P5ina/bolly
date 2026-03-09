import type {
	ChatMessage,
	ChatResponse,
	InstanceSummary,
	ServerMeta,
	UpdateLlmRequest,
} from "./types.js";

const BASE = "";

async function json<T>(url: string, init?: RequestInit): Promise<T> {
	const res = await fetch(`${BASE}${url}`, init);
	if (!res.ok) {
		const text = await res.text().catch(() => res.statusText);
		throw new Error(text);
	}
	return res.json();
}

export function fetchMeta(): Promise<ServerMeta> {
	return json("/api/meta");
}

export function fetchInstances(): Promise<InstanceSummary[]> {
	return json("/api/instances");
}

export function fetchMessages(slug: string): Promise<ChatResponse> {
	return json(`/api/chat/${encodeURIComponent(slug)}/messages`);
}

export function sendMessage(
	slug: string,
	content: string,
): Promise<ChatResponse> {
	return json("/api/chat", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ instance_slug: slug, content }),
	});
}

export function updateLlmConfig(req: UpdateLlmRequest): Promise<void> {
	return json("/api/config/llm", {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(req),
	});
}

export function createWebSocket(): WebSocket {
	const proto = location.protocol === "https:" ? "wss:" : "ws:";
	return new WebSocket(`${proto}//${location.host}/api/ws`);
}
