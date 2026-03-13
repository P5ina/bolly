<script lang="ts">
	import { page } from "$app/state";
	import {
		fetchGoogleAccounts,
		getGoogleConnectUrl,
		disconnectGoogleAccount,
		fetchMcpServers,
		addMcpServer,
		removeMcpServer,
	} from "$lib/api/client.js";
	import type { McpServerInfo } from "$lib/api/client.js";

	const slug = $derived(page.params.slug!);

	// --- catalog of known extensions ---
	interface ExtensionEntry {
		name: string;
		label: string;
		description: string;
		url: string;
		icon: string;
	}

	const catalog: ExtensionEntry[] = [
		{
			name: "excalidraw",
			label: "Excalidraw",
			description: "draw diagrams, flowcharts, and sketches right in chat",
			url: "https://mcp.excalidraw.com/mcp",
			icon: "pencil",
		},
	];

	// Google state
	let accounts = $state<{ email: string }[]>([]);
	let loading = $state(true);
	let disconnecting = $state<string | null>(null);
	let connecting = $state(false);
	let error = $state("");

	// MCP state
	let mcpServers = $state<McpServerInfo[]>([]);
	let mcpLoading = $state(true);
	let mcpBusy = $state<string | null>(null);
	let mcpError = $state("");
	let mcpNewName = $state("");
	let mcpNewUrl = $state("");
	let showCustomForm = $state(false);

	function isInstalled(name: string): boolean {
		return mcpServers.some((s) => s.name === name);
	}

	function isConnected(name: string): boolean {
		return mcpServers.some((s) => s.name === name && s.connected);
	}

	// Custom servers = installed servers that aren't in the catalog
	let customServers = $derived(
		mcpServers.filter((s) => !catalog.some((c) => c.name === s.name)),
	);

	async function loadAccounts() {
		loading = true;
		error = "";
		try {
			accounts = await fetchGoogleAccounts(slug);
		} catch (e) {
			console.error("Failed to load Google accounts:", e);
		} finally {
			loading = false;
		}
	}

	async function connectGoogle() {
		connecting = true;
		error = "";
		try {
			const url = await getGoogleConnectUrl(slug);
			window.location.href = url;
		} catch (e: any) {
			error = e?.message || "Failed to start Google connection";
			connecting = false;
		}
	}

	async function disconnect(email: string) {
		disconnecting = email;
		error = "";
		try {
			await disconnectGoogleAccount(slug, email);
			accounts = accounts.filter((a) => a.email !== email);
		} catch (e) {
			error = `Failed to disconnect ${email}`;
		} finally {
			disconnecting = null;
		}
	}

	async function loadMcpServers() {
		mcpLoading = true;
		mcpError = "";
		try {
			mcpServers = await fetchMcpServers();
		} catch (e) {
			console.error("Failed to load MCP servers:", e);
		} finally {
			mcpLoading = false;
		}
	}

	async function toggleExtension(entry: ExtensionEntry) {
		mcpBusy = entry.name;
		mcpError = "";
		try {
			if (isInstalled(entry.name)) {
				await removeMcpServer(entry.name);
			} else {
				await addMcpServer(entry.name, entry.url);
			}
			await loadMcpServers();
		} catch (e: any) {
			mcpError = e?.message || "something went wrong";
		} finally {
			mcpBusy = null;
		}
	}

	async function handleAddCustom() {
		const name = mcpNewName.trim();
		const url = mcpNewUrl.trim();
		if (!name || !url) {
			mcpError = "name and url are required";
			return;
		}
		mcpBusy = name;
		mcpError = "";
		try {
			await addMcpServer(name, url);
			mcpNewName = "";
			mcpNewUrl = "";
			showCustomForm = false;
			await loadMcpServers();
		} catch (e: any) {
			mcpError = e?.message || "failed to add server";
		} finally {
			mcpBusy = null;
		}
	}

	async function handleRemoveCustom(name: string) {
		mcpBusy = name;
		mcpError = "";
		try {
			await removeMcpServer(name);
			mcpServers = mcpServers.filter((s) => s.name !== name);
		} catch (e) {
			mcpError = `failed to remove ${name}`;
		} finally {
			mcpBusy = null;
		}
	}

	$effect(() => {
		slug;
		loadAccounts();
		loadMcpServers();
	});
</script>

<div class="settings-page">
	<h2 class="settings-title">settings</h2>

	<!-- Extensions (MCP Servers) -->
	<section class="settings-section">
		<div class="section-header">
			<div class="section-icon ext-icon">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M12 2L2 7l10 5 10-5-10-5z" />
					<path d="M2 17l10 5 10-5" />
					<path d="M2 12l10 5 10-5" />
				</svg>
			</div>
			<div>
				<h3 class="section-label">extensions</h3>
				<p class="section-desc">
					Give your companion new abilities. Extensions add tools like drawing, data visualization, and more.
				</p>
			</div>
		</div>

		{#if mcpLoading}
			<div class="ext-loading">
				<div class="loading-dot"></div>
			</div>
		{:else}
			<!-- Catalog cards -->
			<div class="ext-catalog">
				{#each catalog as entry}
					{@const installed = isInstalled(entry.name)}
					{@const connected = isConnected(entry.name)}
					{@const busy = mcpBusy === entry.name}
					<div class="ext-card" class:ext-card-active={installed}>
						<div class="ext-card-icon">
							{#if entry.icon === "pencil"}
								<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
									<path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
									<path d="m15 5 4 4" />
								</svg>
							{/if}
						</div>
						<div class="ext-card-body">
							<span class="ext-card-name">{entry.label}</span>
							<span class="ext-card-desc">{entry.description}</span>
						</div>
						<button
							class="ext-toggle"
							class:ext-toggle-active={installed}
							disabled={busy}
							onclick={() => toggleExtension(entry)}
						>
							{#if busy}
								<span class="ext-spinner"></span>
							{:else if installed}
								{connected ? "connected" : "reconnecting..."}
							{:else}
								connect
							{/if}
						</button>
					</div>
				{/each}
			</div>

			<!-- Custom servers (installed but not in catalog) -->
			{#if customServers.length > 0}
				<div class="ext-custom-list">
					{#each customServers as server}
						<div class="ext-custom-row">
							<div class="ext-custom-info">
								<span class="ext-custom-name">{server.name}</span>
								<span class="ext-custom-status" class:ext-custom-connected={server.connected}>
									{server.connected ? "connected" : "disconnected"}
								</span>
							</div>
							<button
								class="ext-remove-btn"
								disabled={mcpBusy === server.name}
								onclick={() => handleRemoveCustom(server.name)}
							>
								{mcpBusy === server.name ? "..." : "remove"}
							</button>
						</div>
					{/each}
				</div>
			{/if}

			<!-- Advanced: custom server -->
			<div class="ext-advanced">
				{#if showCustomForm}
					<div class="ext-custom-form">
						<div class="ext-custom-form-title">add custom server</div>
						<input
							class="ext-input"
							type="text"
							placeholder="name"
							bind:value={mcpNewName}
						/>
						<input
							class="ext-input"
							type="url"
							placeholder="server url"
							bind:value={mcpNewUrl}
						/>
						<div class="ext-form-actions">
							<button class="ext-form-btn ext-form-add" disabled={mcpBusy !== null} onclick={handleAddCustom}>
								{mcpBusy ? "connecting..." : "add"}
							</button>
							<button class="ext-form-btn ext-form-cancel" onclick={() => { showCustomForm = false; mcpError = ""; }}>
								cancel
							</button>
						</div>
					</div>
				{:else}
					<button class="ext-advanced-toggle" onclick={() => showCustomForm = true}>
						+ add custom server
					</button>
				{/if}
			</div>
		{/if}

		{#if mcpError}
			<p class="error-msg">{mcpError}</p>
		{/if}
	</section>

	<!-- Google Accounts -->
	<section class="settings-section" style="margin-top: 1rem;">
		<div class="section-header">
			<div class="section-icon">
				<svg width="18" height="18" viewBox="0 0 24 24"
					><path
						d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z"
						fill="#4285F4"
					/><path
						d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
						fill="#34A853"
					/><path
						d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
						fill="#FBBC05"
					/><path
						d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
						fill="#EA4335"
					/></svg
				>
			</div>
			<div>
				<h3 class="section-label">google accounts</h3>
				<p class="section-desc">
					Connect Google to enable Gmail, Calendar, and Drive tools.
				</p>
			</div>
		</div>

		{#if loading}
			<div class="ext-loading">
				<div class="loading-dot"></div>
			</div>
		{:else}
			{#if accounts.length > 0}
				<div class="accounts-list">
					{#each accounts as account}
						<div class="account-row">
							<span class="account-email">{account.email}</span>
							<button
								class="ext-remove-btn"
								disabled={disconnecting === account.email}
								onclick={() => disconnect(account.email)}
							>
								{disconnecting === account.email
									? "..."
									: "disconnect"}
							</button>
						</div>
					{/each}
				</div>
			{:else}
				<p class="no-accounts">no google accounts connected</p>
			{/if}

			<button
				class="ext-form-btn ext-form-add"
				disabled={connecting}
				onclick={connectGoogle}
			>
				{connecting ? "connecting..." : "+ connect google account"}
			</button>
		{/if}

		{#if error}
			<p class="error-msg">{error}</p>
		{/if}
	</section>
</div>

<style>
	.settings-page {
		padding: 2rem 1.5rem;
		max-width: 560px;
		margin: 0 auto;
	}

	.settings-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 1.25rem;
		font-weight: 400;
		color: oklch(0.88 0.02 75 / 80%);
		margin-bottom: 2rem;
	}

	.settings-section {
		padding: 1.25rem;
		border-radius: 0.75rem;
		border: 1px solid oklch(1 0 0 / 6%);
		background: oklch(1 0 0 / 2%);
	}

	.section-header {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		margin-bottom: 1rem;
	}

	.section-icon {
		width: 2.25rem;
		height: 2.25rem;
		border-radius: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.5 0 0 / 8%);
		border: 1px solid oklch(1 0 0 / 6%);
		flex-shrink: 0;
	}

	.ext-icon {
		color: oklch(0.78 0.12 75 / 60%);
	}

	.section-label {
		font-family: var(--font-mono);
		font-size: 0.8rem;
		color: oklch(0.88 0.02 75 / 70%);
		letter-spacing: 0.02em;
		margin-bottom: 0.2rem;
	}

	.section-desc {
		font-family: var(--font-body);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75 / 35%);
	}

	/* --- extension catalog --- */

	.ext-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
	}

	.ext-catalog {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin-bottom: 0.75rem;
	}

	.ext-card {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		border-radius: 0.625rem;
		background: oklch(1 0 0 / 2.5%);
		border: 1px solid oklch(1 0 0 / 6%);
		transition: all 0.25s ease;
	}

	.ext-card-active {
		border-color: oklch(0.78 0.12 75 / 15%);
		background: oklch(0.78 0.12 75 / 4%);
	}

	.ext-card-icon {
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.78 0.12 75 / 6%);
		color: oklch(0.78 0.12 75 / 50%);
		flex-shrink: 0;
	}

	.ext-card-active .ext-card-icon {
		background: oklch(0.78 0.12 75 / 10%);
		color: oklch(0.78 0.12 75 / 70%);
	}

	.ext-card-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	.ext-card-name {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 75%);
		letter-spacing: 0.01em;
	}

	.ext-card-desc {
		font-family: var(--font-body);
		font-size: 0.65rem;
		color: oklch(0.88 0.02 75 / 35%);
		line-height: 1.4;
	}

	.ext-toggle {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		letter-spacing: 0.03em;
		padding: 0.375rem 0.75rem;
		border-radius: 0.375rem;
		cursor: pointer;
		transition: all 0.2s ease;
		flex-shrink: 0;
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
	}

	.ext-toggle:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 25%);
	}

	.ext-toggle-active {
		color: oklch(0.70 0.12 145 / 70%);
		background: oklch(0.70 0.12 145 / 6%);
		border-color: oklch(0.70 0.12 145 / 15%);
	}

	.ext-toggle-active:hover:not(:disabled) {
		color: oklch(0.65 0.12 25 / 70%);
		background: oklch(0.65 0.12 25 / 8%);
		border-color: oklch(0.65 0.12 25 / 20%);
	}

	.ext-toggle:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.ext-spinner {
		display: inline-block;
		width: 10px;
		height: 10px;
		border: 1.5px solid oklch(0.78 0.12 75 / 25%);
		border-top-color: oklch(0.78 0.12 75 / 70%);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	/* --- custom servers list --- */

	.ext-custom-list {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
		margin-bottom: 0.75rem;
	}

	.ext-custom-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 5%);
	}

	.ext-custom-info {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}

	.ext-custom-name {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(0.88 0.02 75 / 60%);
	}

	.ext-custom-status {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.55 0.02 280 / 40%);
	}

	.ext-custom-connected {
		color: oklch(0.70 0.12 145 / 70%);
	}

	.ext-remove-btn {
		font-family: var(--font-body);
		font-size: 0.62rem;
		color: oklch(0.65 0.12 25 / 50%);
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		transition: all 0.2s ease;
	}
	.ext-remove-btn:hover:not(:disabled) {
		color: oklch(0.65 0.15 25 / 90%);
		background: oklch(0.65 0.15 25 / 8%);
	}
	.ext-remove-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	/* --- advanced / custom form --- */

	.ext-advanced {
		margin-top: 0.25rem;
	}

	.ext-advanced-toggle {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.55 0.02 280 / 35%);
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.25rem 0;
		transition: color 0.2s ease;
		letter-spacing: 0.03em;
	}
	.ext-advanced-toggle:hover {
		color: oklch(0.78 0.12 75 / 55%);
	}

	.ext-custom-form {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0.75rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 2%);
		border: 1px solid oklch(1 0 0 / 6%);
	}

	.ext-custom-form-title {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.55 0.02 280 / 45%);
		letter-spacing: 0.04em;
		margin-bottom: 0.15rem;
	}

	.ext-input {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75 / 80%);
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 8%);
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		outline: none;
		transition: border-color 0.2s ease;
	}
	.ext-input:focus {
		border-color: oklch(0.78 0.12 75 / 30%);
	}
	.ext-input::placeholder {
		color: oklch(0.88 0.02 75 / 20%);
	}

	.ext-form-actions {
		display: flex;
		gap: 0.5rem;
	}

	.ext-form-btn {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		padding: 0.4rem 0.85rem;
		border-radius: 0.375rem;
		cursor: pointer;
		transition: all 0.2s ease;
		letter-spacing: 0.02em;
	}

	.ext-form-add {
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
	}
	.ext-form-add:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 25%);
	}
	.ext-form-add:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.ext-form-cancel {
		color: oklch(0.70 0.02 280 / 45%);
		background: none;
		border: 1px solid oklch(1 0 0 / 6%);
	}
	.ext-form-cancel:hover {
		color: oklch(0.80 0.02 280 / 60%);
		background: oklch(1 0 0 / 3%);
	}

	/* --- google / shared --- */

	.accounts-list {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
		margin-bottom: 0.75rem;
	}

	.account-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 5%);
	}

	.account-email {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 60%);
	}

	.no-accounts {
		font-family: var(--font-body);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 30%);
		font-style: italic;
		margin-bottom: 0.75rem;
	}

	.loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse 1.5s ease-in-out infinite;
	}
	@keyframes pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(0.7); }
	}

	.error-msg {
		font-family: var(--font-body);
		font-size: 0.7rem;
		color: oklch(0.65 0.15 25 / 70%);
		font-style: italic;
		margin-top: 0.5rem;
	}
</style>
