import { load, type Store } from "@tauri-apps/plugin-store";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

const CLOUD_API = "https://bollyai.dev";
const STORE_KEY = "session";
const STORE_SELF_HOSTED = "self_hosted";

let store: Store | null = null;

async function getStore(): Promise<Store> {
  if (!store) {
    store = await load("settings.json", { autoSave: true });
  }
  return store;
}

// ─── Types ───────────────────────────────────────────────────────────────────

export type Tenant = {
  id: string;
  slug: string;
  plan: string;
  status: string;
  authToken: string | null;
  shareToken: string | null;
  errorMessage: string | null;
  createdAt: string;
};

export type SelfHostedConfig = {
  url: string;
  token: string;
};

type AuthState = {
  session: string | null;
  tenants: Tenant[];
  loading: boolean;
  error: string | null;
  /** Self-hosted connection (bypasses cloud) */
  selfHosted: SelfHostedConfig | null;
};

const state: AuthState = $state({
  session: null,
  tenants: [],
  loading: true,
  error: null,
  selfHosted: null,
});

export const auth = state;

// ─── Init ────────────────────────────────────────────────────────────────────

export async function init() {
  try {
    const s = await getStore();

    // Check for self-hosted config first
    const sh = await s.get<SelfHostedConfig>(STORE_SELF_HOSTED);
    if (sh?.url && sh?.token) {
      state.selfHosted = sh;
      state.loading = false;
      return;
    }

    // Cloud mode
    const saved = await s.get<string>(STORE_KEY);
    if (saved) {
      state.session = saved;
      await fetchTenants();
    }
  } catch {
    // No saved session
  } finally {
    state.loading = false;
  }
}

// ─── Cloud mode ──────────────────────────────────────────────────────────────

export async function setSession(sessionId: string) {
  state.session = sessionId;
  state.selfHosted = null;
  state.error = null;
  const s = await getStore();
  await s.set(STORE_KEY, sessionId);
  await s.delete(STORE_SELF_HOSTED);
  await fetchTenants();
}

export async function fetchTenants() {
  if (!state.session) return;

  state.loading = true;
  state.error = null;

  try {
    const token = state.session!;
    const url = `${CLOUD_API}/api/tenants?session=${encodeURIComponent(token)}`;
    console.log("[auth] fetching tenants", "token:", token.slice(0, 6) + "...");

    const res = await tauriFetch(url);
    console.log("[auth] response", res.status, res.statusText);

    if (res.status === 401) {
      await logout();
      state.error = "Session expired. Please sign in again.";
      return;
    }

    if (!res.ok) {
      const body = await res.text();
      console.error("[auth] error body", body);
      throw new Error(`API error ${res.status}: ${body.slice(0, 200)}`);
    }

    const data = await res.json();
    console.log("[auth] tenants", data.length, data);
    state.tenants = data;
  } catch (err) {
    console.error("[auth] fetch failed", err);
    state.error = err instanceof Error ? err.message : "Failed to fetch instances";
  } finally {
    state.loading = false;
  }
}

export function instanceUrl(tenant: Tenant): string {
  return `https://${tenant.slug}.bollyai.dev`;
}

export function connectUrl(tenant: Tenant): string {
  return `${instanceUrl(tenant)}/auth?token=${encodeURIComponent(tenant.authToken!)}`;
}

// ─── Self-hosted mode ────────────────────────────────────────────────────────

export async function connectSelfHosted(url: string, token: string) {
  // Normalize URL
  let normalizedUrl = url.trim().replace(/\/+$/, "");
  if (!normalizedUrl.startsWith("http")) {
    normalizedUrl = `http://${normalizedUrl}`;
  }

  // Validate by fetching /api/meta
  state.loading = true;
  state.error = null;

  try {
    const res = await tauriFetch(`${normalizedUrl}/api/meta`, {
      headers: { Authorization: `Bearer ${token}` },
    });

    if (!res.ok) {
      throw new Error(res.status === 401 ? "Invalid auth token" : `Server error ${res.status}`);
    }

    const meta = await res.json();
    console.log("[auth] self-hosted connected:", meta);

    const config: SelfHostedConfig = { url: normalizedUrl, token };
    state.selfHosted = config;
    state.session = null;
    state.tenants = [];

    const s = await getStore();
    await s.set(STORE_SELF_HOSTED, config);
    await s.delete(STORE_KEY);
  } catch (err) {
    console.error("[auth] self-hosted connect failed:", err);
    state.error = err instanceof Error ? err.message : "Connection failed";
  } finally {
    state.loading = false;
  }
}

export function selfHostedConnectUrl(config: SelfHostedConfig): string {
  return `${config.url}/auth?token=${encodeURIComponent(config.token)}`;
}

// ─── Logout (both modes) ────────────────────────────────────────────────────

export async function logout() {
  state.session = null;
  state.selfHosted = null;
  state.tenants = [];
  state.error = null;
  const s = await getStore();
  await s.delete(STORE_KEY);
  await s.delete(STORE_SELF_HOSTED);
}
