<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    auth, init, setSession, logout, connectUrl, connectSelfHosted,
    selfHostedConnectUrl, type Tenant, type SelfHostedConfig,
  } from "$lib/auth.svelte";
  import { updater, checkForUpdates, installUpdate, dismissUpdate } from "$lib/updater.svelte";

  const AUTH_URL = "https://bollyai.dev/desktop-auth";

  let splash = $state(true);
  let splashFading = $state(false);
  let mode = $state<"cloud" | "selfhosted">("cloud");

  // Self-hosted form
  let shUrl = $state("");
  let shToken = $state("");

  onMount(() => {
    init();
    checkForUpdates();

    const unlisten = listen<string>("deep-link", (event) => {
      try {
        const url = new URL(event.payload);
        if (url.protocol === "bolly:" && url.hostname === "callback") {
          const session = url.searchParams.get("session");
          if (session) setSession(session);
        }
      } catch {}
    });

    return () => { unlisten.then((fn) => fn()); };
  });

  function endSplash() {
    splashFading = true;
    setTimeout(() => { splash = false; }, 600);
  }

  let showPaste = $state(false);
  let pasteValue = $state("");

  function signIn() { openUrl(AUTH_URL); }

  function submitCode() {
    const v = pasteValue.trim();
    if (v) setSession(v);
  }

  function handleCodeKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); submitCode(); }
    if (e.key === "Escape") { showPaste = false; pasteValue = ""; }
  }

  async function connect(tenant: Tenant) {
    const instanceUrl = `https://${tenant.slug}.bollyai.dev`;
    await invoke("connect_computer_use", {
      instanceUrl,
      authToken: tenant.authToken ?? "",
    });
    invoke("navigate", { url: connectUrl(tenant) });
  }

  async function connectSH() {
    if (!shUrl.trim() || !shToken.trim()) return;
    await connectSelfHosted(shUrl.trim(), shToken.trim());
  }

  async function openSelfHosted(config: SelfHostedConfig) {
    await invoke("connect_computer_use", {
      instanceUrl: config.url,
      authToken: config.token,
    });
    invoke("navigate", { url: selfHostedConnectUrl(config) });
  }

  function handleSHKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); connectSH(); }
  }

  function statusColor(status: string): string {
    switch (status) {
      case "running": return "oklch(0.72 0.17 142)";
      case "provisioning": return "oklch(0.78 0.12 75)";
      case "error": return "oklch(0.65 0.20 25)";
      default: return "oklch(0.50 0.03 240)";
    }
  }

  function planLabel(plan: string): string {
    return plan.charAt(0).toUpperCase() + plan.slice(1);
  }
</script>

<!-- Audio lives outside splash so it's not destroyed on transition -->
<audio src="/splash.mp3" autoplay></audio>

{#if splash}
  <div class="splash" class:splash-fade={splashFading}>
    <video
      class="splash-video"
      src="/splash.mp4"
      autoplay
      muted
      playsinline
      onended={endSplash}
    ></video>
    <div class="splash-brand">
      <img src="/icon.png" alt="" class="splash-logo" />
      <span class="splash-name">bolly</span>
    </div>
  </div>
{/if}

<div class="dashboard" class:dashboard-enter={!splash}>
    <div class="dashboard-glow"></div>

    <header class="header">
      <div class="brand">
        <img src="/icon.png" alt="" class="logo" />
        <span class="brand-name">bolly</span>
      </div>
      {#if auth.session || auth.selfHosted}
        <button class="sign-out-btn" onclick={logout}>Disconnect</button>
      {/if}
    </header>

    {#if updater.available}
      <div class="update-banner">
        {#if updater.downloading}
          <div class="update-text">
            Updating to v{updater.version}...
          </div>
          <div class="update-progress-track">
            <div class="update-progress-bar" style:width="{Math.round(updater.progress * 100)}%"></div>
          </div>
        {:else if updater.error}
          <div class="update-text update-error-text">Update failed: {updater.error}</div>
          <div class="update-actions">
            <button class="update-btn" onclick={installUpdate}>Retry</button>
            <button class="update-dismiss" onclick={dismissUpdate}>Dismiss</button>
          </div>
        {:else}
          <div class="update-text">
            v{updater.version} is available
          </div>
          <div class="update-actions">
            <button class="update-btn" onclick={installUpdate}>Update & restart</button>
            <button class="update-dismiss" onclick={dismissUpdate}>Later</button>
          </div>
        {/if}
      </div>
    {/if}

    <main class="content">
      {#if auth.loading && !splash}
        <div class="center-message">
          <div class="spinner"></div>
        </div>
      {:else if auth.selfHosted && !splash}
        <!-- Self-hosted: connected -->
        <div class="sign-in-card">
          <h2 class="sign-in-title">self-hosted instance</h2>
          <p class="sign-in-desc">{auth.selfHosted.url}</p>
          <button class="sign-in-btn" onclick={() => openSelfHosted(auth.selfHosted!)}>
            Open
          </button>
        </div>
      {:else if !auth.session && !auth.selfHosted && !splash}
        <div class="sign-in-card">
          <h2 class="sign-in-title">connect to your companion</h2>

          <!-- Mode tabs -->
          <div class="mode-tabs">
            <button class="mode-tab" class:mode-tab-active={mode === "cloud"} onclick={() => mode = "cloud"}>Cloud</button>
            <button class="mode-tab" class:mode-tab-active={mode === "selfhosted"} onclick={() => mode = "selfhosted"}>Self-hosted</button>
          </div>

          {#if mode === "cloud"}
            <p class="sign-in-desc">Sign in with your bollyai.dev account.</p>
            <button class="sign-in-btn" onclick={signIn}>
              Sign in with bollyai.dev
            </button>
            {#if !showPaste}
              <button class="paste-toggle" onclick={() => showPaste = true}>
                or paste a code
              </button>
            {:else}
              <div class="paste-field">
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  class="paste-input"
                  bind:value={pasteValue}
                  onkeydown={handleCodeKey}
                  placeholder="Paste session code..."
                  autofocus
                />
                {#if pasteValue.trim()}
                  <button class="paste-go" onclick={submitCode}>Connect</button>
                {/if}
              </div>
            {/if}
          {:else}
            <p class="sign-in-desc">Connect to your own bolly server.</p>
            <div class="sh-form">
              <input
                class="paste-input"
                bind:value={shUrl}
                onkeydown={handleSHKey}
                placeholder="Server URL (e.g. http://localhost:3000)"
              />
              <input
                class="paste-input"
                bind:value={shToken}
                onkeydown={handleSHKey}
                placeholder="Auth token (from config.toml)"
                type="password"
              />
              <button class="sign-in-btn" onclick={connectSH} disabled={!shUrl.trim() || !shToken.trim()}>
                Connect
              </button>
            </div>
          {/if}
        </div>
      {:else if auth.error && !splash}
        <div class="center-message">
          <p class="error-text">{auth.error}</p>
          <button class="retry-btn" onclick={() => { import('$lib/auth.svelte').then(m => m.fetchTenants()); }}>Retry</button>
        </div>
      {:else if auth.tenants.length === 0 && !auth.loading && !splash}
        <div class="center-message">
          <p class="empty-text">No instances yet.</p>
          <p class="empty-sub">Create one at <button class="link-btn" onclick={() => openUrl("https://bollyai.dev/dashboard")}>bollyai.dev</button></p>
        </div>
      {:else if auth.tenants.length > 0 && !splash}
        <div class="instances">
          <h2 class="section-title">your instances</h2>
          <div class="instance-grid">
            {#each auth.tenants as tenant (tenant.id)}
              <button
                class="instance-card"
                disabled={tenant.status !== "running"}
                onclick={() => connect(tenant)}
              >
                <div class="instance-header">
                  <span class="instance-slug">{tenant.slug}</span>
                  <span class="instance-plan">{planLabel(tenant.plan)}</span>
                </div>
                <div class="instance-footer">
                  <span class="instance-status" style:color={statusColor(tenant.status)}>
                    <span class="status-dot" style:background={statusColor(tenant.status)}></span>
                    {tenant.status}
                  </span>
                  {#if tenant.status === "error" && tenant.errorMessage}
                    <span class="instance-error">{tenant.errorMessage}</span>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </main>
  </div>

<style>
  /* ─── Splash ───────────────────────────────────────────────── */
  .splash {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--background);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity 0.6s ease;
  }

  .splash-fade {
    opacity: 0;
  }

  .splash-video {
    position: absolute;
    width: 420px;
    height: 420px;
    object-fit: contain;
    pointer-events: none;
    opacity: 0.7;
  }

  .splash-brand {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    animation: splash-brand-in 1.2s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: 0.3s;
  }

  .splash-logo {
    width: 40px;
    height: 40px;
    object-fit: contain;
  }

  .splash-name {
    font-family: var(--font-display);
    font-style: italic;
    font-size: 1.8rem;
    color: var(--foreground);
    letter-spacing: -0.02em;
  }

  @keyframes splash-brand-in {
    0% { opacity: 0; transform: translateY(10px) scale(0.95); }
    100% { opacity: 1; transform: translateY(0) scale(1); }
  }

  /* ─── Dashboard ────────────────────────────────────────────── */
  .dashboard {
    display: flex;
    flex-direction: column;
    height: 100vh;
    position: relative;
    overflow: hidden;
    opacity: 0;
  }

  .dashboard-enter {
    animation: dash-in 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  @keyframes dash-in {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .dashboard-glow {
    position: absolute;
    top: 35%;
    left: 50%;
    width: 600px;
    height: 600px;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    background: radial-gradient(circle, oklch(0.55 0.08 240 / 3%) 0%, transparent 60%);
    pointer-events: none;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    position: relative;
    z-index: 1;
    -webkit-app-region: drag;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .logo {
    width: 28px;
    height: 28px;
    object-fit: contain;
  }

  .brand-name {
    font-family: var(--font-display);
    font-style: italic;
    font-size: 1.1rem;
    color: var(--foreground);
  }

  .sign-out-btn {
    -webkit-app-region: no-drag;
    padding: 5px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    font-family: var(--font-body);
    font-size: 0.72rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .sign-out-btn:hover {
    background: oklch(1 0 0 / 5%);
    color: var(--foreground);
    border-color: oklch(1 0 0 / 14%);
  }

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px;
    position: relative;
    z-index: 1;
  }

  /* ─── Update banner ─────────────────────────────────────────── */
  .update-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 0 24px;
    padding: 10px 16px;
    border-radius: 10px;
    background: oklch(0.78 0.12 75 / 8%);
    border: 1px solid oklch(0.78 0.12 75 / 14%);
    position: relative;
    z-index: 1;
    animation: dash-in 0.3s ease both;
  }

  .update-text {
    font-size: 0.78rem;
    color: var(--warm);
    white-space: nowrap;
  }

  .update-error-text {
    color: oklch(0.65 0.15 25 / 80%);
  }

  .update-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .update-btn {
    padding: 5px 14px;
    border-radius: 7px;
    border: 1px solid oklch(0.78 0.12 75 / 22%);
    background: oklch(0.78 0.12 75 / 12%);
    color: var(--warm);
    font-family: var(--font-body);
    font-size: 0.72rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }

  .update-btn:hover {
    background: oklch(0.78 0.12 75 / 20%);
    border-color: oklch(0.78 0.12 75 / 32%);
  }

  .update-dismiss {
    padding: 5px 10px;
    border-radius: 7px;
    border: none;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-body);
    font-size: 0.72rem;
    cursor: pointer;
    transition: color 0.2s;
  }

  .update-dismiss:hover {
    color: var(--foreground);
  }

  .update-progress-track {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: oklch(1 0 0 / 6%);
    overflow: hidden;
  }

  .update-progress-bar {
    height: 100%;
    border-radius: 2px;
    background: var(--warm);
    transition: width 0.3s ease;
  }

  /* ─── Sign in ──────────────────────────────────────────────── */
  .sign-in-card {
    text-align: center;
    max-width: 360px;
    animation: dash-in 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .sign-in-title {
    font-family: var(--font-display);
    font-style: italic;
    font-size: 1.5rem;
    font-weight: 400;
    color: var(--foreground);
    margin: 0 0 12px;
  }

  .sign-in-desc {
    font-size: 0.82rem;
    color: var(--muted);
    margin: 0 0 28px;
    line-height: 1.5;
  }

  .sign-in-btn {
    padding: 10px 28px;
    border-radius: 10px;
    font-size: 0.85rem;
    font-weight: 500;
    font-family: var(--font-body);
    color: var(--warm);
    background: oklch(0.78 0.12 75 / 10%);
    border: 1px solid oklch(0.78 0.12 75 / 18%);
    border-top-color: oklch(0.78 0.12 75 / 28%);
    cursor: pointer;
    transition: all 0.3s ease;
  }

  .sign-in-btn:hover:not(:disabled) {
    background: oklch(0.78 0.12 75 / 16%);
    border-color: oklch(0.78 0.12 75 / 30%);
    box-shadow: 0 0 40px oklch(0.78 0.12 75 / 8%);
  }

  .sign-in-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .mode-tabs {
    display: flex;
    gap: 2px;
    margin-bottom: 16px;
    background: oklch(1 0 0 / 4%);
    border-radius: 8px;
    padding: 2px;
  }

  .mode-tab {
    flex: 1;
    padding: 6px 12px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-body);
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .mode-tab-active {
    background: oklch(1 0 0 / 8%);
    color: var(--foreground);
  }

  .sh-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .paste-toggle {
    display: block;
    margin: 14px auto 0;
    background: none;
    border: none;
    color: oklch(0.50 0.03 240);
    font-family: var(--font-body);
    font-size: 0.72rem;
    cursor: pointer;
    transition: color 0.2s;
  }

  .paste-toggle:hover {
    color: var(--foreground);
  }

  .paste-field {
    display: flex;
    gap: 8px;
    margin-top: 14px;
    animation: dash-in 0.3s ease both;
  }

  .paste-input {
    flex: 1;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: oklch(1 0 0 / 3%);
    color: var(--foreground);
    font-family: monospace;
    font-size: 0.75rem;
    outline: none;
    transition: border-color 0.2s;
  }

  .paste-input:focus {
    border-color: oklch(1 0 0 / 16%);
  }

  .paste-input::placeholder {
    color: oklch(0.50 0.03 240);
  }

  .paste-go {
    padding: 8px 14px;
    border-radius: 8px;
    border: 1px solid oklch(0.78 0.12 75 / 18%);
    background: oklch(0.78 0.12 75 / 10%);
    color: var(--warm);
    font-family: var(--font-body);
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }

  .paste-go:hover {
    background: oklch(0.78 0.12 75 / 16%);
  }

  /* ─── Instances ────────────────────────────────────────────── */
  .instances {
    width: 100%;
    max-width: 560px;
    animation: dash-in 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .section-title {
    font-family: var(--font-display);
    font-style: italic;
    font-size: 1.1rem;
    font-weight: 400;
    color: var(--foreground);
    margin: 0 0 20px;
    text-align: center;
  }

  .instance-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .instance-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 20px;
    border-radius: 12px;
    background: var(--card);
    border: 1px solid var(--glass-border);
    border-top-color: var(--border-top);
    text-align: left;
    cursor: pointer;
    transition: all 0.25s ease;
    font-family: var(--font-body);
    position: relative;
    overflow: hidden;
  }

  .instance-card::before {
    content: "";
    position: absolute;
    top: 0;
    left: 12%;
    right: 12%;
    height: 1px;
    background: linear-gradient(90deg, transparent, oklch(1 0 0 / 14%), transparent);
    pointer-events: none;
  }

  .instance-card:not(:disabled):hover {
    background: oklch(1 0 0 / 6%);
    border-color: oklch(1 0 0 / 14%);
    box-shadow: 0 4px 20px oklch(0 0 0 / 30%);
  }

  .instance-card:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .instance-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .instance-slug {
    font-size: 0.95rem;
    font-weight: 500;
    color: var(--foreground);
  }

  .instance-plan {
    font-size: 0.68rem;
    color: var(--muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .instance-footer {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .instance-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.72rem;
    text-transform: capitalize;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .instance-error {
    font-size: 0.68rem;
    color: oklch(0.65 0.15 25 / 70%);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ─── Utility ──────────────────────────────────────────────── */
  .center-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
  }

  .error-text {
    font-size: 0.85rem;
    color: oklch(0.65 0.15 25 / 80%);
    margin: 0;
  }

  .empty-text {
    font-size: 0.95rem;
    color: var(--muted);
    margin: 0;
  }

  .empty-sub {
    font-size: 0.82rem;
    color: oklch(0.50 0.03 240);
    margin: 0;
  }

  .link-btn {
    background: none;
    border: none;
    color: var(--warm);
    font-family: var(--font-body);
    font-size: 0.82rem;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
    text-decoration-color: oklch(0.78 0.12 75 / 30%);
    text-underline-offset: 2px;
  }

  .link-btn:hover {
    text-decoration-color: var(--warm);
  }

  .retry-btn {
    padding: 8px 20px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-body);
    font-size: 0.82rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .retry-btn:hover {
    background: oklch(1 0 0 / 5%);
    border-color: oklch(1 0 0 / 14%);
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid oklch(1 0 0 / 10%);
    border-top-color: var(--warm);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
