import type {
	ChatMessage,
	ChatResponse,
	ChatSummary,
	Drop,
	InstanceSummary,
	ServerMeta,
	Soul,
	SoulTemplate,
	UpdateLlmRequest,
} from "./types.js";

const BASE = "";

// ---------------------------------------------------------------------------
// Auth token management
// ---------------------------------------------------------------------------

const TOKEN_KEY = "bolly_auth_token";

export function getAuthToken(): string | null {
	if (typeof localStorage === "undefined") return null;
	return localStorage.getItem(TOKEN_KEY);
}

export function setAuthToken(token: string) {
	localStorage.setItem(TOKEN_KEY, token);
}

export function clearAuthToken() {
	localStorage.removeItem(TOKEN_KEY);
}

function authHeaders(): Record<string, string> {
	const token = getAuthToken();
	return token ? { Authorization: `Bearer ${token}` } : {};
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

export class AuthError extends Error {
	constructor() {
		super("unauthorized");
		this.name = "AuthError";
	}
}

async function json<T>(url: string, init?: RequestInit): Promise<T> {
	const headers = {
		...authHeaders(),
		...(init?.headers as Record<string, string> | undefined),
	};
	const res = await fetch(`${BASE}${url}`, { ...init, headers });
	if (res.status === 401) {
		throw new AuthError();
	}
	if (!res.ok) {
		const text = await res.text().catch(() => res.statusText);
		throw new Error(text);
	}
	return res.json();
}

async function authedFetch(url: string, init?: RequestInit): Promise<Response> {
	const headers = {
		...authHeaders(),
		...(init?.headers as Record<string, string> | undefined),
	};
	return fetch(`${BASE}${url}`, { ...init, headers });
}

// ---------------------------------------------------------------------------
// API functions
// ---------------------------------------------------------------------------

export function fetchMeta(): Promise<ServerMeta> {
	return json("/api/meta");
}

export function fetchInstances(): Promise<InstanceSummary[]> {
	return json("/api/instances");
}

export function fetchChats(slug: string): Promise<ChatSummary[]> {
	return json(`/api/chat/${encodeURIComponent(slug)}/chats`);
}

export function fetchMessages(slug: string, chatId = "default"): Promise<ChatResponse> {
	return json(`/api/chat/${encodeURIComponent(slug)}/${encodeURIComponent(chatId)}/messages`);
}

export function sendMessage(
	slug: string,
	content: string,
	chatId = "default",
): Promise<ChatResponse> {
	return json("/api/chat", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ instance_slug: slug, content, chat_id: chatId }),
	});
}

export function updateLlmConfig(req: UpdateLlmRequest): Promise<void> {
	return json("/api/config/llm", {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(req),
	});
}

export function fetchSoul(slug: string): Promise<Soul> {
	return json(`/api/instances/${encodeURIComponent(slug)}/soul`);
}

export function updateSoul(slug: string, content: string): Promise<Soul> {
	return json(`/api/instances/${encodeURIComponent(slug)}/soul`, {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ content }),
	});
}

export function applySoulTemplate(
	slug: string,
	templateId: string,
): Promise<Soul> {
	return json(`/api/instances/${encodeURIComponent(slug)}/soul/apply-template`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ template_id: templateId }),
	});
}

export function fetchSoulTemplates(): Promise<SoulTemplate[]> {
	return json("/api/soul/templates");
}

export async function stopAgent(slug: string, chatId = "default"): Promise<void> {
	await authedFetch(`/api/chat/${encodeURIComponent(slug)}/${encodeURIComponent(chatId)}/stop`, { method: "POST" });
}

export async function clearContext(slug: string, chatId = "default"): Promise<void> {
	await authedFetch(`/api/chat/${encodeURIComponent(slug)}/${encodeURIComponent(chatId)}/context`, { method: "DELETE" });
}

export function fetchMood(slug: string): Promise<{ mood: string }> {
	return json(`/api/instances/${encodeURIComponent(slug)}/mood`);
}

export function fetchCompanionName(slug: string): Promise<{ name: string }> {
	return json(`/api/instances/${encodeURIComponent(slug)}/companion-name`);
}

export function setCompanionName(slug: string, name: string): Promise<void> {
	return json(`/api/instances/${encodeURIComponent(slug)}/companion-name`, {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ name }),
	});
}

export function fetchDrops(slug: string): Promise<Drop[]> {
	return json(`/api/instances/${encodeURIComponent(slug)}/drops`);
}

export function fetchDrop(slug: string, dropId: string): Promise<Drop> {
	return json(
		`/api/instances/${encodeURIComponent(slug)}/drops/${encodeURIComponent(dropId)}`,
	);
}

export async function deleteDrop(slug: string, dropId: string): Promise<void> {
	await authedFetch(
		`/api/instances/${encodeURIComponent(slug)}/drops/${encodeURIComponent(dropId)}`,
		{ method: "DELETE" },
	);
}

export function createWebSocket(): WebSocket {
	const proto = location.protocol === "https:" ? "wss:" : "ws:";
	const token = getAuthToken();
	const query = token ? `?token=${encodeURIComponent(token)}` : "";
	return new WebSocket(`${proto}//${location.host}/api/ws${query}`);
}
