import type {
	ChatMessage,
	ChatResponse,
	ChatSummary,
	Drop,
	InstanceSummary,
	ServerMeta,
	Soul,
	SoulTemplate,
	Thought,
	UpdateLlmRequest,
	UploadMeta,
} from "./types.js";

const BASE = "";

// ---------------------------------------------------------------------------
// Auth token management
// ---------------------------------------------------------------------------

const TOKEN_KEY = "bolly_auth_token";
const TOKEN_COOKIE = "bolly_token";

function getCookie(name: string): string | null {
	if (typeof document === "undefined") return null;
	const match = document.cookie.match(new RegExp(`(?:^|; )${name}=([^;]*)`));
	return match ? decodeURIComponent(match[1]) : null;
}

function setCookie(name: string, value: string) {
	if (typeof document === "undefined") return;
	// 1 year expiry, same-site strict, secure on https
	const secure = location.protocol === "https:" ? "; Secure" : "";
	document.cookie = `${name}=${encodeURIComponent(value)}; path=/; max-age=31536000; SameSite=Strict${secure}`;
}

function deleteCookie(name: string) {
	if (typeof document === "undefined") return;
	document.cookie = `${name}=; path=/; max-age=0`;
}

export function getAuthToken(): string | null {
	if (typeof localStorage === "undefined") return getCookie(TOKEN_COOKIE);
	// Try localStorage first, fall back to cookie (for PWA isolated storage)
	return localStorage.getItem(TOKEN_KEY) ?? getCookie(TOKEN_COOKIE);
}

export function setAuthToken(token: string) {
	if (typeof localStorage !== "undefined") {
		localStorage.setItem(TOKEN_KEY, token);
	}
	setCookie(TOKEN_COOKIE, token);
}

export function clearAuthToken() {
	if (typeof localStorage !== "undefined") {
		localStorage.removeItem(TOKEN_KEY);
	}
	deleteCookie(TOKEN_COOKIE);
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

export function fetchThoughts(slug: string): Promise<Thought[]> {
	return json(`/api/instances/${encodeURIComponent(slug)}/thoughts`);
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

export async function uploadFile(slug: string, file: File): Promise<UploadMeta> {
	const form = new FormData();
	form.append("file", file);
	const res = await authedFetch(
		`/api/instances/${encodeURIComponent(slug)}/uploads`,
		{ method: "POST", body: form },
	);
	if (res.status === 401) throw new AuthError();
	if (!res.ok) throw new Error(await res.text().catch(() => res.statusText));
	return res.json();
}

export function fetchUploads(slug: string): Promise<UploadMeta[]> {
	return json(`/api/instances/${encodeURIComponent(slug)}/uploads`);
}

export async function deleteUpload(slug: string, uploadId: string): Promise<void> {
	await authedFetch(
		`/api/instances/${encodeURIComponent(slug)}/uploads/${encodeURIComponent(uploadId)}`,
		{ method: "DELETE" },
	);
}

export function uploadFileUrl(slug: string, uploadId: string): string {
	const base = `/api/instances/${encodeURIComponent(slug)}/uploads/${encodeURIComponent(uploadId)}/file`;
	const token = getAuthToken();
	return token ? `${base}?token=${encodeURIComponent(token)}` : base;
}

export function createWebSocket(): WebSocket {
	const proto = location.protocol === "https:" ? "wss:" : "ws:";
	const token = getAuthToken();
	const query = token ? `?token=${encodeURIComponent(token)}` : "";
	return new WebSocket(`${proto}//${location.host}/api/ws${query}`);
}
