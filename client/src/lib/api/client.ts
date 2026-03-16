import type {
	ChatMessage,
	ChatResponse,
	ChatSummary,
	ContextStats,
	Drop,
	InstanceSummary,
	RegistryEntry,
	ServerMeta,
	Skill,
	Soul,
	SoulTemplate,
	Thought,
	UpdateLlmRequest,
	MemoryEntry,
	UploadMeta,
	Usage,
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

export async function deleteInstance(slug: string): Promise<void> {
	const res = await authedFetch(`/api/instances/${encodeURIComponent(slug)}`, {
		method: "DELETE",
	});
	if (res.status === 401) throw new AuthError();
	if (!res.ok) throw new Error(await res.text().catch(() => res.statusText));
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

export function fetchConfigStatus(): Promise<{ llm_configured: boolean; provider?: string; model?: string }> {
	return json("/api/config/status");
}

export interface McpServerInfo {
	name: string;
	url?: string;
	connected: boolean;
}

export function fetchMcpServers(): Promise<McpServerInfo[]> {
	return json("/api/config/mcp");
}

export function addMcpServer(name: string, url: string): Promise<{ status: string; name: string; tool_count: number }> {
	return json("/api/config/mcp", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ name, url }),
	});
}

export function removeMcpServer(name: string): Promise<void> {
	return json(`/api/config/mcp/${encodeURIComponent(name)}`, {
		method: "DELETE",
	});
}

export function fetchTimezone(slug: string): Promise<{ timezone: string }> {
	return json(`/api/instances/${encodeURIComponent(slug)}/timezone`);
}

export function updateTimezone(slug: string, timezone: string): Promise<void> {
	return json(`/api/instances/${encodeURIComponent(slug)}/timezone`, {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ timezone }),
	});
}

export function fetchGithubConfig(): Promise<{ configured: boolean }> {
	return json("/api/config/github");
}

export function updateGithubToken(token: string): Promise<{ status: string; configured: boolean }> {
	return json("/api/config/github", {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ token }),
	});
}

// ---------------------------------------------------------------------------
// Email config (per-instance SMTP/IMAP)
// ---------------------------------------------------------------------------

export interface EmailConfig {
	smtp_host: string;
	smtp_port: number;
	smtp_user: string;
	smtp_password: string;
	smtp_from: string;
	imap_host: string;
	imap_port: number;
	imap_user: string;
	imap_password: string;
}

export function fetchEmailAccounts(slug: string): Promise<{ accounts: Partial<EmailConfig>[] }> {
	return json(`/api/instances/${encodeURIComponent(slug)}/email`);
}

export function saveEmailAccounts(slug: string, accounts: EmailConfig[]): Promise<void> {
	return json(`/api/instances/${encodeURIComponent(slug)}/email`, {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ accounts }),
	});
}

export function deleteAllEmailAccounts(slug: string): Promise<void> {
	return json(`/api/instances/${encodeURIComponent(slug)}/email`, {
		method: "DELETE",
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

export function fetchMemory(slug: string): Promise<MemoryEntry[]> {
	return json(`/api/instances/${encodeURIComponent(slug)}/memory`);
}

export interface MemorySearchResult {
	path: string;
	text: string;
	score: number;
}

export function searchMemory(slug: string, query: string, limit = 10): Promise<MemorySearchResult[]> {
	return json(`/api/instances/${encodeURIComponent(slug)}/memory/search?q=${encodeURIComponent(query)}&limit=${limit}`);
}

export async function fetchMemoryContent(slug: string, path: string): Promise<string> {
	const res = await authedFetch(`/api/instances/${encodeURIComponent(slug)}/memory/${path}`);
	if (res.status === 401) throw new AuthError();
	if (!res.ok) throw new Error(await res.text().catch(() => res.statusText));
	return res.text();
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

export async function uploadFile(
	slug: string,
	file: File,
	onProgress?: (loaded: number, total: number) => void,
): Promise<UploadMeta> {
	return new Promise((resolve, reject) => {
		const xhr = new XMLHttpRequest();
		xhr.open("POST", `${BASE}/api/instances/${encodeURIComponent(slug)}/uploads`);

		const token = getAuthToken();
		if (token) xhr.setRequestHeader("Authorization", `Bearer ${token}`);

		if (onProgress) {
			xhr.upload.onprogress = (e) => {
				if (e.lengthComputable) onProgress(e.loaded, e.total);
			};
		}

		xhr.onload = () => {
			if (xhr.status === 401) return reject(new AuthError());
			if (xhr.status < 200 || xhr.status >= 300) return reject(new Error(xhr.responseText || xhr.statusText));
			try {
				resolve(JSON.parse(xhr.responseText));
			} catch {
				reject(new Error("invalid response"));
			}
		};

		xhr.onerror = () => reject(new Error("upload failed"));

		const form = new FormData();
		form.append("file", file);
		xhr.send(form);
	});
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

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

export function fetchSkills(): Promise<Skill[]> {
	return json("/api/skills");
}

export function fetchSkill(skillId: string): Promise<Skill> {
	return json(`/api/skills/${encodeURIComponent(skillId)}`);
}

export function createSkill(skill: Skill): Promise<Skill> {
	return json("/api/skills", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(skill),
	});
}

export async function deleteSkill(skillId: string): Promise<void> {
	await authedFetch(`/api/skills/${encodeURIComponent(skillId)}`, {
		method: "DELETE",
	});
}

export function fetchRegistry(): Promise<RegistryEntry[]> {
	return json("/api/skills/registry");
}

export function installRegistrySkill(id: string): Promise<Skill> {
	return json("/api/skills/registry/install", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ id }),
	});
}

export function fetchUsage(): Promise<Usage> {
	return json("/api/usage");
}

export function fetchContextStats(slug: string, chatId = "default"): Promise<ContextStats> {
	return json(`/api/instances/${encodeURIComponent(slug)}/${encodeURIComponent(chatId)}/context-stats`);
}

export async function submitSecret(slug: string, id: string, value: string): Promise<void> {
	await authedFetch(`/api/instances/${encodeURIComponent(slug)}/secret`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ id, value }),
	});
}

export async function cancelSecret(slug: string, id: string): Promise<void> {
	await authedFetch(`/api/instances/${encodeURIComponent(slug)}/secret/${encodeURIComponent(id)}`, {
		method: "DELETE",
	});
}

// ---------------------------------------------------------------------------
// Heartbeat updates
// ---------------------------------------------------------------------------

export function fetchHeartbeatUpdates(slug: string): Promise<import("./types.js").HeartbeatUpdate[]> {
	return json(`/api/instances/${encodeURIComponent(slug)}/heartbeat/updates`);
}

export async function applyHeartbeatUpdate(slug: string, updateId: string): Promise<void> {
	const res = await authedFetch(
		`/api/instances/${encodeURIComponent(slug)}/heartbeat/updates/${encodeURIComponent(updateId)}/apply`,
		{ method: "POST" },
	);
	if (res.status === 401) throw new AuthError();
	if (!res.ok) throw new Error(await res.text().catch(() => res.statusText));
}

export async function dismissHeartbeatUpdate(slug: string, updateId: string): Promise<void> {
	const res = await authedFetch(
		`/api/instances/${encodeURIComponent(slug)}/heartbeat/updates/${encodeURIComponent(updateId)}/dismiss`,
		{ method: "POST" },
	);
	if (res.status === 401) throw new AuthError();
	if (!res.ok) throw new Error(await res.text().catch(() => res.statusText));
}

// ---------------------------------------------------------------------------
// Google Accounts
// ---------------------------------------------------------------------------

export async function fetchGoogleAccounts(slug: string): Promise<{ email: string }[]> {
	const data = await json<{ accounts: { email: string }[] }>(
		`/api/instances/${encodeURIComponent(slug)}/google/accounts`,
	);
	return data.accounts;
}

export async function getGoogleConnectUrl(slug: string): Promise<string> {
	const data = await json<{ url: string }>(
		`/api/instances/${encodeURIComponent(slug)}/google/connect`,
	);
	return data.url;
}

export async function disconnectGoogleAccount(slug: string, email: string): Promise<void> {
	const res = await authedFetch(
		`/api/instances/${encodeURIComponent(slug)}/google/accounts/${encodeURIComponent(email)}`,
		{ method: "DELETE" },
	);
	if (res.status === 401) throw new AuthError();
	if (!res.ok) throw new Error(await res.text().catch(() => res.statusText));
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

export function createWebSocket(): WebSocket {
	const proto = location.protocol === "https:" ? "wss:" : "ws:";
	const token = getAuthToken();
	const query = token ? `?token=${encodeURIComponent(token)}` : "";
	return new WebSocket(`${proto}//${location.host}/api/ws${query}`);
}
