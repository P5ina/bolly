import { load, type Store } from "@tauri-apps/plugin-store";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

const API_BASE = "https://bollyai.dev";
const STORE_KEY = "session";

let store: Store | null = null;

async function getStore(): Promise<Store> {
  if (!store) {
    store = await load("settings.json", { autoSave: true });
  }
  return store;
}

// ─── Reactive state ──────────────────────────────────────────────────────────

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

type AuthState = {
  session: string | null;
  tenants: Tenant[];
  loading: boolean;
  error: string | null;
};

const state: AuthState = $state({
  session: null,
  tenants: [],
  loading: true,
  error: null,
});

export const auth = state;

// ─── Session persistence ─────────────────────────────────────────────────────

export async function init() {
  try {
    const s = await getStore();
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

export async function setSession(sessionId: string) {
  state.session = sessionId;
  state.error = null;
  const s = await getStore();
  await s.set(STORE_KEY, sessionId);
  await fetchTenants();
}

export async function logout() {
  state.session = null;
  state.tenants = [];
  state.error = null;
  const s = await getStore();
  await s.delete(STORE_KEY);
}

// ─── API calls ───────────────────────────────────────────────────────────────

export async function fetchTenants() {
  if (!state.session) return;

  state.loading = true;
  state.error = null;

  try {
    const token = state.session!;
    const url = `${API_BASE}/api/tenants?session=${encodeURIComponent(token)}`;
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
