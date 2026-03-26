<script lang="ts">
	import { page } from "$app/state";
	import { getSceneStore } from "$lib/stores/scene.svelte.js";
	import {
		fetchGoogleAccounts,
		getGoogleConnectUrl,
		disconnectGoogleAccount,
		fetchMcpServers,
		addMcpServer,
		removeMcpServer,
		fetchGithubConfig,
		updateGithubToken,
		fetchTimezone,
		updateTimezone,
		fetchVoiceId,
		updateVoiceId,
		fetchMusicEnabled,
		updateMusicEnabled,
		fetchEmailAccounts,
		saveEmailAccounts,
		deleteAllEmailAccounts,
		fetchUsage,
		fetchConfigStatus,
		updateModelMode,
		exportInstanceUrl,
		importInstance,
		reindexMemory,
		importKnowledge,
		fetchScheduledTasks,
		cancelScheduledTask,
		type ScheduledTask,
	} from "$lib/api/client.js";
	import type { McpServerInfo, EmailConfig } from "$lib/api/client.js";
	import type { Usage, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { onDestroy } from "svelte";

	const slug = $derived(page.params.slug!);
	const scene = getSceneStore();

	// --- catalog of known extensions ---
	interface ExtensionEntry {
		name: string;
		label: string;
		description: string;
		url: string;
		icon: string;
	}

	const catalog: ExtensionEntry[] = [];

	// Export / Import
	let importing = $state(false);
	let importError = $state("");
	let importDone = $state(false);
	let importFileInput: HTMLInputElement | undefined = $state();

	function handleExport() {
		window.location.href = exportInstanceUrl(slug);
	}

	async function handleImport() {
		const file = importFileInput?.files?.[0];
		if (!file) return;
		importing = true;
		importError = "";
		importDone = false;
		try {
			await importInstance(slug, file);
			importDone = true;
			setTimeout(() => { importDone = false; }, 4000);
		} catch (e) {
			importError = e instanceof Error ? e.message : "import failed";
		} finally {
			importing = false;
			if (importFileInput) importFileInput.value = "";
		}
	}

	// Import knowledge
	let importingKnowledge = $state(false);
	let knowledgeError = $state("");
	let knowledgeStarted = $state(false);
	let knowledgeFileInput: HTMLInputElement | undefined = $state();

	// Import progress (via WebSocket)
	let importStage = $state<string | null>(null);
	let importDetail = $state("");
	const ws = getWebSocket();
	const unsub = ws.subscribe((event: ServerEvent) => {
		if (event.type === "import_progress" && event.instance_slug === slug) {
			importStage = event.stage;
			importDetail = event.detail;
			if (event.stage === "done" || event.stage === "error") {
				setTimeout(() => { importStage = null; importDetail = ""; }, 10000);
			}
		}
	});
	onDestroy(unsub);

	async function handleImportKnowledge() {
		const files = knowledgeFileInput?.files;
		if (!files || files.length === 0) return;
		importingKnowledge = true;
		knowledgeError = "";
		knowledgeStarted = false;
		try {
			await importKnowledge(slug, files);
			knowledgeStarted = true;
			setTimeout(() => { knowledgeStarted = false; }, 8000);
		} catch (e) {
			knowledgeError = e instanceof Error ? e.message : "import failed";
		} finally {
			importingKnowledge = false;
			if (knowledgeFileInput) knowledgeFileInput.value = "";
		}
	}

	// Reindex memory
	let reindexing = $state(false);

	async function handleReindex() {
		reindexing = true;
		try {
			await reindexMemory(slug);
		} catch (e) {
			console.error('reindex failed', e);
		} finally {
			reindexing = false;
		}
	}

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

	// GitHub state
	let ghConfigured = $state(false);
	let ghLoading = $state(true);
	let ghToken = $state("");
	let ghSaving = $state(false);
	let ghError = $state("");
	let ghEditing = $state(false);

	// Music state
	let musicEnabledVal = $state(true);
	let musicLoading = $state(true);
	let musicSaving = $state(false);

	async function loadMusic() {
		musicLoading = true;
		try {
			const res = await fetchMusicEnabled(slug);
			musicEnabledVal = res.music_enabled;
		} catch {
			// not critical
		} finally {
			musicLoading = false;
		}
	}

	async function toggleMusic() {
		musicSaving = true;
		try {
			const next = !musicEnabledVal;
			await updateMusicEnabled(slug, next);
			musicEnabledVal = next;
			scene.setMusicEnabled(next);
		} catch (e) {
			console.error("[music] toggle failed:", e);
		} finally {
			musicSaving = false;
		}
	}

	// Voice state
	let voiceId = $state("");
	let voiceLoading = $state(true);
	let voiceSaving = $state(false);
	let voiceInput = $state("");

	async function loadVoice() {
		voiceLoading = true;
		try {
			const res = await fetchVoiceId(slug);
			voiceId = res.voice_id || "";
			voiceInput = voiceId;
		} catch {
			// not critical
		} finally {
			voiceLoading = false;
		}
	}

	async function saveVoice() {
		voiceSaving = true;
		try {
			await updateVoiceId(slug, voiceInput.trim());
			voiceId = voiceInput.trim();
		} catch {
			// ignore
		} finally {
			voiceSaving = false;
		}
	}

	async function clearVoice() {
		voiceSaving = true;
		try {
			await updateVoiceId(slug, "");
			voiceId = "";
			voiceInput = "";
		} catch {
			// ignore
		} finally {
			voiceSaving = false;
		}
	}

	async function loadGithub() {
		ghLoading = true;
		try {
			const res = await fetchGithubConfig();
			ghConfigured = res.configured;
		} catch {
			// not critical
		} finally {
			ghLoading = false;
		}
	}

	async function saveGithubToken() {
		const token = ghToken.trim();
		ghSaving = true;
		ghError = "";
		try {
			const res = await updateGithubToken(token);
			ghConfigured = res.configured;
			ghToken = "";
			ghEditing = false;
		} catch (e: any) {
			ghError = e?.message || "failed to save token";
		} finally {
			ghSaving = false;
		}
	}

	async function disconnectGithub() {
		ghSaving = true;
		ghError = "";
		try {
			const res = await updateGithubToken("");
			ghConfigured = res.configured;
		} catch (e: any) {
			ghError = e?.message || "failed to disconnect";
		} finally {
			ghSaving = false;
		}
	}

	// Update state

	// Usage state
	let usage = $state<Usage | null>(null);
	$effect(() => { fetchUsage().then(u => usage = u).catch(() => {}); });

	// Model mode state
	let modelMode = $state("auto");
	let modelModeSaving = $state(false);
	$effect(() => {
		fetchConfigStatus().then(s => {
			if (s.model_mode) modelMode = s.model_mode;
		}).catch(() => {});
	});

	async function setModelMode(mode: string) {
		modelModeSaving = true;
		try {
			await updateModelMode(mode);
			modelMode = mode;
		} catch {
			// revert on failure
		} finally {
			modelModeSaving = false;
		}
	}

	function formatTokens(n: number): string {
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
		return String(n);
	}

	function usagePct(used: number, limit: number): number {
		if (limit <= 0) return 0;
		return Math.min(100, Math.round((used / limit) * 100));
	}

	// Timezone state
	let tzValue = $state("");
	let tzLoading = $state(true);
	let tzSaving = $state(false);

	const COMMON_TIMEZONES = [
		"Asia/Bishkek", "Asia/Almaty", "Asia/Tashkent",
		"Europe/Moscow", "Europe/London", "Europe/Berlin", "Europe/Paris",
		"America/New_York", "America/Chicago", "America/Denver", "America/Los_Angeles",
		"Asia/Tokyo", "Asia/Shanghai", "Asia/Kolkata", "Asia/Dubai",
		"Australia/Sydney", "Pacific/Auckland",
	];

	async function loadTimezone() {
		tzLoading = true;
		try {
			const res = await fetchTimezone(slug);
			tzValue = res.timezone || "";
		} catch {
			// not critical
		} finally {
			tzLoading = false;
		}
	}

	async function saveTimezone(tz: string) {
		tzSaving = true;
		try {
			await updateTimezone(slug, tz);
			tzValue = tz;
		} catch {
			// ignore
		} finally {
			tzSaving = false;
		}
	}

	// Email state
	let emailAccounts = $state<Partial<EmailConfig>[]>([]);
	let emailLoading = $state(true);
	let emailSaving = $state(false);
	let emailError = $state("");
	let emailAdding = $state(false);

	function emptyEmailForm(): EmailConfig {
		return {
			smtp_host: "", smtp_port: 587, smtp_user: "", smtp_password: "", smtp_from: "",
			imap_host: "", imap_port: 993, imap_user: "", imap_password: "",
		};
	}
	let emailForm = $state<EmailConfig>(emptyEmailForm());

	async function loadEmail() {
		emailLoading = true;
		try {
			const res = await fetchEmailAccounts(slug);
			emailAccounts = res.accounts || [];
		} catch {
			// not critical
		} finally {
			emailLoading = false;
		}
	}

	async function saveNewEmail() {
		emailSaving = true;
		emailError = "";
		try {
			// Merge existing accounts (fill in missing passwords) + new one
			const existing: EmailConfig[] = emailAccounts.map(a => ({
				...emptyEmailForm(),
				...a,
			}));
			existing.push(emailForm);
			await saveEmailAccounts(slug, existing);
			emailAdding = false;
			emailForm = emptyEmailForm();
			await loadEmail();
		} catch (e: any) {
			emailError = e?.message || "failed to save email account";
		} finally {
			emailSaving = false;
		}
	}

	async function removeEmailAccount(index: number) {
		emailSaving = true;
		emailError = "";
		try {
			const remaining = emailAccounts.filter((_, i) => i !== index).map(a => ({
				...emptyEmailForm(),
				...a,
			}));
			if (remaining.length > 0) {
				await saveEmailAccounts(slug, remaining);
			} else {
				await deleteAllEmailAccounts(slug);
			}
			await loadEmail();
		} catch (e: any) {
			emailError = e?.message || "failed to remove email account";
		} finally {
			emailSaving = false;
		}
	}

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
		loadGithub();
		loadTimezone();
		loadEmail();
		loadVoice();
		loadMusic();
		loadScheduled();
	});

	// Scheduled tasks
	let scheduledTasks = $state<ScheduledTask[]>([]);
	let scheduledLoading = $state(true);
	let cancellingId = $state<string | null>(null);

	async function loadScheduled() {
		scheduledLoading = true;
		try {
			scheduledTasks = await fetchScheduledTasks(slug);
		} catch {
			// not critical
		} finally {
			scheduledLoading = false;
		}
	}

	async function cancelTask(id: string) {
		cancellingId = id;
		try {
			await cancelScheduledTask(slug, id);
			scheduledTasks = scheduledTasks.filter((t) => t.id !== id);
		} catch {
			// ignore
		} finally {
			cancellingId = null;
		}
	}

	function formatDeliverAt(ts: number): string {
		const d = new Date(ts * 1000);
		const now = Date.now();
		const diff = ts * 1000 - now;
		if (diff <= 0) return "delivering...";
		if (diff < 60_000) return `in ${Math.ceil(diff / 1000)}s`;
		if (diff < 3600_000) return `in ${Math.ceil(diff / 60_000)}m`;
		if (diff < 86400_000) {
			const h = Math.floor(diff / 3600_000);
			const m = Math.ceil((diff % 3600_000) / 60_000);
			return m > 0 ? `in ${h}h ${m}m` : `in ${h}h`;
		}
		return d.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
	}
</script>

<div class="settings-page">
	<h2 class="settings-title">settings</h2>

	<div class="settings-grid">


	<!-- Usage -->
	{#if usage && (usage.tokens_4h_limit > 0 || usage.tokens_week_limit > 0 || usage.tokens_month_limit > 0)}
		<section class="settings-section">
			<div class="section-header">
				<img src="/icon-usage.png" alt="" class="section-icon-img" />
				<div>
					<h3 class="section-label">usage</h3>
					<p class="section-desc">Token usage across time windows.</p>
				</div>
			</div>
			<div class="usage-windows">
				{#if usage.tokens_4h_limit > 0}
					{@const p = usagePct(usage.tokens_last_4h, usage.tokens_4h_limit)}
					<div class="usage-window">
						<div class="usage-window-header">
							<span class="usage-window-label">4 hours</span>
							<span class="usage-window-value">{formatTokens(usage.tokens_last_4h)} / {formatTokens(usage.tokens_4h_limit)}</span>
						</div>
						<div class="usage-window-track">
							<div class="usage-window-fill" style="width: {p}%; background: {p >= 90 ? 'oklch(0.65 0.2 25)' : p >= 70 ? 'oklch(0.75 0.15 85)' : 'oklch(0.55 0.08 240 / 50%)'}"></div>
						</div>
					</div>
				{/if}
				{#if usage.tokens_week_limit > 0}
					{@const p = usagePct(usage.tokens_this_week, usage.tokens_week_limit)}
					<div class="usage-window">
						<div class="usage-window-header">
							<span class="usage-window-label">this week</span>
							<span class="usage-window-value">{formatTokens(usage.tokens_this_week)} / {formatTokens(usage.tokens_week_limit)}</span>
						</div>
						<div class="usage-window-track">
							<div class="usage-window-fill" style="width: {p}%; background: {p >= 90 ? 'oklch(0.65 0.2 25)' : p >= 70 ? 'oklch(0.75 0.15 85)' : 'oklch(0.55 0.08 240 / 50%)'}"></div>
						</div>
					</div>
				{/if}
				{#if usage.tokens_month_limit > 0}
					{@const p = usagePct(usage.tokens_this_month, usage.tokens_month_limit)}
					<div class="usage-window">
						<div class="usage-window-header">
							<span class="usage-window-label">this month</span>
							<span class="usage-window-value">{formatTokens(usage.tokens_this_month)} / {formatTokens(usage.tokens_month_limit)}</span>
						</div>
						<div class="usage-window-track">
							<div class="usage-window-fill" style="width: {p}%; background: {p >= 90 ? 'oklch(0.65 0.2 25)' : p >= 70 ? 'oklch(0.75 0.15 85)' : 'oklch(0.55 0.08 240 / 50%)'}"></div>
						</div>
					</div>
				{/if}
			</div>
		</section>
	{/if}

	<!-- Model Mode -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-model.png" alt="" class="section-icon-img" />
			<div>
				<h3 class="section-label">model mode</h3>
				<p class="section-desc">Choose how your companion picks the AI model for each message.</p>
			</div>
		</div>
		<div class="model-mode-options" class:disabled={modelModeSaving}>
			<button
				class="mode-option"
				class:mode-active={modelMode === "auto"}
				onclick={() => setModelMode("auto")}
				disabled={modelModeSaving}
			>
				<span class="mode-name">auto</span>
				<span class="mode-desc">smart routing — cheap for casual, powerful when needed</span>
			</button>
			<button
				class="mode-option"
				class:mode-active={modelMode === "fast"}
				onclick={() => setModelMode("fast")}
				disabled={modelModeSaving}
			>
				<span class="mode-name">fast</span>
				<span class="mode-desc">always use the lightweight model — saves budget</span>
			</button>
			<button
				class="mode-option"
				class:mode-active={modelMode === "heavy"}
				onclick={() => setModelMode("heavy")}
				disabled={modelModeSaving}
			>
				<span class="mode-name">heavy</span>
				<span class="mode-desc">always use the powerful model — uses 10x more budget</span>
			</button>
		</div>
	</section>

	<!-- Timezone -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-timezone.png" alt="" class="section-icon-img" />
			<div>
				<h3 class="section-label">timezone</h3>
				<p class="section-desc">
					Set your local timezone so your companion knows the right time of day.
				</p>
			</div>
		</div>

		{#if tzLoading}
			<div class="ext-loading">
				<div class="loading-dot"></div>
			</div>
		{:else}
			<div class="tz-picker">
				<select
					class="tz-select"
					value={tzValue}
					disabled={tzSaving}
					onchange={(e) => saveTimezone((e.target as HTMLSelectElement).value)}
				>
					<option value="">UTC (default)</option>
					{#each COMMON_TIMEZONES as tz}
						<option value={tz} selected={tzValue === tz}>{tz.replace(/_/g, " ")}</option>
					{/each}
				</select>
				{#if tzValue}
					<span class="tz-current">{tzValue.replace(/_/g, " ")}</span>
				{/if}
			</div>
		{/if}
	</section>

	<!-- Extensions (MCP Servers) — hidden for now to simplify UX -->
	<!--
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-extensions.png" alt="" class="section-icon-img" />
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

			<div class="ext-advanced">
				{#if showCustomForm}
					<div class="ext-custom-form">
						<div class="ext-custom-form-title">add custom server</div>
						<input class="ext-input" type="text" placeholder="name" bind:value={mcpNewName} />
						<input class="ext-input" type="url" placeholder="server url" bind:value={mcpNewUrl} />
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
	-->

	<!-- Google Accounts -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-google.png" alt="" class="section-icon-img" />
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

	<!-- Email (SMTP/IMAP) -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-email.png" alt="" class="section-icon-img" />
			<div>
				<h3 class="section-label">email</h3>
				<p class="section-desc">
					Connect any email via SMTP/IMAP (iCloud, Outlook, Yahoo, etc.)
					to enable send and read email tools.
				</p>
			</div>
		</div>

		{#if emailLoading}
			<div class="ext-loading">
				<div class="loading-dot"></div>
			</div>
		{:else}
			<!-- Existing accounts -->
			{#if emailAccounts.length > 0}
				<div class="accounts-list">
					{#each emailAccounts as acct, i}
						<div class="account-row">
							<span class="account-email">{acct.smtp_from || acct.smtp_user || acct.imap_user || "account"}</span>
							<button
								class="ext-remove-btn"
								disabled={emailSaving}
								onclick={() => removeEmailAccount(i)}
							>
								{emailSaving ? "..." : "remove"}
							</button>
						</div>
					{/each}
				</div>
			{/if}

			<!-- Add new account form -->
			{#if emailAdding}
				<div class="email-form">
					<div class="email-form-group">
						<span class="email-form-label">outgoing (smtp)</span>
						<input class="ext-input" type="text" placeholder="smtp host (e.g. smtp.mail.me.com)" bind:value={emailForm.smtp_host} />
						<div class="email-form-row">
							<input class="ext-input" type="number" placeholder="port" bind:value={emailForm.smtp_port} style="width: 5rem;" />
							<input class="ext-input" style="flex:1" type="text" placeholder="username / email" bind:value={emailForm.smtp_user} />
						</div>
						<input class="ext-input" type="password" placeholder="password / app-specific password" bind:value={emailForm.smtp_password} />
						<input class="ext-input" type="email" placeholder="from address (e.g. user@icloud.com)" bind:value={emailForm.smtp_from} />
					</div>

					<div class="email-form-group">
						<span class="email-form-label">incoming (imap)</span>
						<input class="ext-input" type="text" placeholder="imap host (e.g. imap.mail.me.com)" bind:value={emailForm.imap_host} />
						<div class="email-form-row">
							<input class="ext-input" type="number" placeholder="port" bind:value={emailForm.imap_port} style="width: 5rem;" />
							<input class="ext-input" style="flex:1" type="text" placeholder="username / email" bind:value={emailForm.imap_user} />
						</div>
						<input class="ext-input" type="password" placeholder="password / app-specific password" bind:value={emailForm.imap_password} />
					</div>

					<div class="ext-form-actions">
						<button
							class="ext-form-btn ext-form-add"
							disabled={emailSaving || (!emailForm.smtp_host && !emailForm.imap_host)}
							onclick={saveNewEmail}
						>
							{emailSaving ? "saving..." : "add account"}
						</button>
						<button class="ext-form-btn ext-form-cancel" onclick={() => { emailAdding = false; emailError = ""; emailForm = emptyEmailForm(); }}>
							cancel
						</button>
					</div>
				</div>
			{:else}
				<button
					class="ext-form-btn ext-form-add"
					onclick={() => emailAdding = true}
				>
					+ add email account
				</button>
			{/if}
		{/if}

		{#if emailError}
			<p class="error-msg">{emailError}</p>
		{/if}
	</section>

	<!-- GitHub -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-github.png" alt="" class="section-icon-img" />
			<div>
				<h3 class="section-label">github</h3>
				<p class="section-desc">
					Connect GitHub to enable cloning repos, creating branches, PRs, and managing issues.
				</p>
			</div>
		</div>

		{#if ghLoading}
			<div class="ext-loading">
				<div class="loading-dot"></div>
			</div>
		{:else if ghConfigured && !ghEditing}
			<div class="gh-status">
				<div class="gh-status-info">
					<span class="gh-status-dot"></span>
					<span class="gh-status-text">token configured</span>
				</div>
				<div class="gh-status-actions">
					<button class="ext-form-btn ext-form-cancel" onclick={() => ghEditing = true}>
						change
					</button>
					<button
						class="ext-remove-btn"
						disabled={ghSaving}
						onclick={disconnectGithub}
					>
						{ghSaving ? "..." : "remove"}
					</button>
				</div>
			</div>
		{:else}
			<div class="gh-token-form">
				<input
					class="ext-input"
					type="password"
					placeholder="ghp_... or github_pat_..."
					bind:value={ghToken}
					onkeydown={(e) => e.key === "Enter" && saveGithubToken()}
				/>
				<div class="ext-form-actions">
					<button
						class="ext-form-btn ext-form-add"
						disabled={ghSaving || !ghToken.trim()}
						onclick={saveGithubToken}
					>
						{ghSaving ? "saving..." : "save token"}
					</button>
					{#if ghEditing}
						<button class="ext-form-btn ext-form-cancel" onclick={() => { ghEditing = false; ghToken = ""; ghError = ""; }}>
							cancel
						</button>
					{/if}
				</div>
			</div>
		{/if}

		{#if ghError}
			<p class="error-msg">{ghError}</p>
		{/if}
	</section>

	<!-- Music -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-music.png" alt="" class="section-icon-img" />
			<div class="section-header-text">
				<h3 class="section-label">music</h3>
				<p class="section-desc">
					Background ambient and intro music when entering chat.
				</p>
			</div>
			{#if musicLoading}
				<div class="loading-dot" style="margin-left:auto"></div>
			{:else}
				<button
					class="switch"
					class:switch-on={musicEnabledVal}
					disabled={musicSaving}
					onclick={toggleMusic}
					role="switch"
					aria-label="Toggle music"
					aria-checked={musicEnabledVal}
				>
					<span class="switch-thumb"></span>
				</button>
			{/if}
		</div>
	</section>

	<!-- Voice -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-voice.png" alt="" class="section-icon-img" />
			<div>
				<h3 class="section-label">voice</h3>
				<p class="section-desc">
					ElevenLabs voice ID for text-to-speech. Leave empty to use the default voice.
				</p>
			</div>
		</div>

		{#if voiceLoading}
			<div class="ext-loading">
				<div class="loading-dot"></div>
			</div>
		{:else}
			<div class="gh-token-form">
				<input
					class="ext-input"
					type="text"
					placeholder="e.g. TWutjvRaJqAX89preB4e"
					bind:value={voiceInput}
					onkeydown={(e) => e.key === "Enter" && saveVoice()}
				/>
				<div class="ext-form-actions">
					<button
						class="ext-form-btn ext-form-add"
						disabled={voiceSaving || voiceInput.trim() === voiceId}
						onclick={saveVoice}
					>
						{voiceSaving ? "saving..." : "save"}
					</button>
					{#if voiceId}
						<button
							class="ext-form-btn ext-form-cancel"
							disabled={voiceSaving}
							onclick={clearVoice}
						>
							reset to default
						</button>
					{/if}
				</div>
			</div>
		{/if}
	</section>

	<!-- Scheduled tasks -->
	{#if !scheduledLoading && scheduledTasks.length > 0}
		<section class="settings-section">
			<div class="section-header">
				<img src="/icon-scheduled.png" alt="" class="section-icon-img" />
				<div>
					<h3 class="section-label">scheduled</h3>
					<p class="section-desc">{scheduledTasks.length} pending task{scheduledTasks.length === 1 ? "" : "s"}</p>
				</div>
			</div>
			<div class="sched-list">
				{#each scheduledTasks as task (task.id)}
					<div class="sched-item">
						<div class="sched-content">
							<span class="sched-text">{task.task.length > 80 ? task.task.slice(0, 80) + "…" : task.task}</span>
							<span class="sched-time">{formatDeliverAt(task.deliver_at)}</span>
						</div>
						<button
							class="sched-cancel"
							disabled={cancellingId === task.id}
							onclick={() => cancelTask(task.id)}
						>
							{cancellingId === task.id ? "…" : "cancel"}
						</button>
					</div>
				{/each}
			</div>
		</section>
	{/if}

	<!-- Export / Import -->
	<section class="settings-section">
		<div class="section-header">
			<img src="/icon-data.png" alt="" class="section-icon-img" />
			<div>
				<h3 class="section-label">data</h3>
			</div>
		</div>
		<div class="data-actions">
			<button class="data-btn" onclick={handleExport}>
				export
			</button>
			<label class="data-btn data-btn-import">
				{#if importing}
					importing...
				{:else if importDone}
					imported!
				{:else}
					import
				{/if}
				<input
					type="file"
					accept=".tar.gz,.tgz"
					bind:this={importFileInput}
					onchange={handleImport}
					hidden
					disabled={importing}
				/>
			</label>
		</div>
		<p class="data-hint">export downloads a .tar.gz of all instance data (soul, memory, drops, chat history). import merges into the current instance.</p>

		<div class="data-actions" style="margin-top: 1.25rem;">
			<label class="data-btn data-btn-knowledge">
				{#if importingKnowledge}
					uploading...
				{:else if knowledgeStarted}
					started!
				{:else}
					import knowledge
				{/if}
				<input
					type="file"
					accept=".json,.txt,.md,.csv"
					multiple
					bind:this={knowledgeFileInput}
					onchange={handleImportKnowledge}
					hidden
					disabled={importingKnowledge}
				/>
			</label>
			<span class="data-hint">drop your Claude export, notes, or any personal data — AI will extract facts and add them to memory</span>
		</div>
		{#if knowledgeError}
			<p class="error-msg">{knowledgeError}</p>
		{/if}
		{#if knowledgeStarted && !importStage}
			<p class="data-hint" style="color: oklch(0.72 0.15 155); margin-top: 0.5rem;">
				processing in background — check memory after a few minutes
			</p>
		{/if}

		{#if importStage}
			<div class="import-progress" class:import-done={importStage === 'done'} class:import-error={importStage === 'error'}>
				<div class="import-progress-header">
					{#if importStage === 'done'}
						<span class="import-progress-icon">&#10003;</span>
					{:else if importStage === 'error'}
						<span class="import-progress-icon">&#10007;</span>
					{:else}
						<span class="import-progress-spinner"></span>
					{/if}
					<span class="import-progress-stage">{importStage}</span>
				</div>
				<p class="import-progress-detail">{importDetail}</p>
			</div>
		{/if}

		<div class="data-actions" style="margin-top: 0.75rem;">
			<button
				class="ext-form-btn"
				disabled={reindexing}
				onclick={handleReindex}
			>
				{reindexing ? 'reindexing...' : 'reindex memory'}
			</button>
			<span class="data-hint">rebuild vector search index for all memories</span>
		</div>
		{#if importError}
			<p class="error-msg">{importError}</p>
		{/if}
	</section>

	</div><!-- settings-grid -->
</div>

<style>
	/* Usage */
	.usage-windows {
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
	}
	.usage-window-header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin-bottom: 0.25rem;
	}
	.usage-window-label {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75 / 50%);
	}
	.usage-window-value {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.55 0.08 240 / 30%);
	}
	.usage-window-track {
		height: 4px;
		border-radius: 2px;
		background: oklch(1 0 0 / 5%);
		overflow: hidden;
	}
	.usage-window-fill {
		height: 100%;
		border-radius: 2px;
		transition: width 0.5s ease;
	}

	.settings-page {
		padding: 2rem 2.5rem;
		padding-bottom: calc(2rem + env(safe-area-inset-bottom, 0px));
		max-width: 1200px;
		margin: 0 auto;
		height: 100%;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}

	.settings-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 1.25rem;
		font-weight: 400;
		color: oklch(0.88 0.02 240 / 80%);
		margin-bottom: 0.5rem;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 1rem;
		align-items: start;
	}

	@media (max-width: 900px) {
		.settings-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 768px) {
		.settings-page {
			padding: 1.5rem 1rem;
		}
	}

	.settings-section {
		position: relative;
		padding: 1.25rem;
		border-radius: 1rem;
		border: 1px solid oklch(1 0 0 / 10%);
		border-top-color: oklch(1 0 0 / 18%);
		background: linear-gradient(
			150deg,
			oklch(1 0 0 / 5%) 0%,
			oklch(0.5 0.02 250 / 8%) 40%,
			oklch(1 0 0 / 3%) 100%
		);
		backdrop-filter: blur(20px) saturate(150%) brightness(1.05);
		-webkit-backdrop-filter: blur(20px) saturate(150%) brightness(1.05);
		box-shadow:
			0 2px 12px oklch(0 0 0 / 12%),
			inset 0 1px 0 oklch(1 0 0 / 8%),
			inset 0 -1px 0 oklch(0 0 0 / 4%);
		overflow: hidden;
	}
	.settings-section::before {
		content: "";
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 20%), transparent);
		pointer-events: none;
	}

	.section-header {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		margin-bottom: 1rem;
	}

	.section-icon-img {
		width: 2.5rem;
		height: 2.5rem;
		object-fit: cover;
		flex-shrink: 0;
		border-radius: 0.625rem;
		border: 1px solid oklch(1 0 0 / 10%);
		border-top-color: oklch(1 0 0 / 18%);
		box-shadow:
			0 2px 8px oklch(0 0 0 / 20%),
			inset 0 1px 0 oklch(1 0 0 / 6%);
	}

	.tz-picker {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.tz-select {
		flex: 1;
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(0.88 0.02 75 / 80%);
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 8%);
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		outline: none;
		transition: border-color 0.2s ease;
		appearance: none;
		cursor: pointer;
	}
	.tz-select:focus {
		border-color: oklch(0.55 0.08 240 / 30%);
	}
	.tz-select option {
		background: oklch(0.10 0.015 280);
		color: oklch(0.88 0.02 75 / 80%);
	}

	.tz-current {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.75 0.10 200 / 50%);
		white-space: nowrap;
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

	.ext-remove-btn {
		font-family: var(--font-body);
		font-size: 0.72rem;
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
		border-color: oklch(0.55 0.08 240 / 30%);
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
		color: oklch(0.55 0.08 240 / 70%);
		background: oklch(0.55 0.08 240 / 8%);
		border: 1px solid oklch(0.55 0.08 240 / 15%);
	}
	.ext-form-add:hover:not(:disabled) {
		background: oklch(0.55 0.08 240 / 14%);
		border-color: oklch(0.55 0.08 240 / 35%);
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
		background: oklch(0.55 0.08 240 / 30%);
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

	/* --- github --- */

	.switch {
		margin-left: auto;
		position: relative;
		width: 2.5rem;
		height: 1.375rem;
		border-radius: 9999px;
		border: 1px solid oklch(1 0 0 / 10%);
		background: oklch(1 0 0 / 6%);
		cursor: pointer;
		transition: all 0.2s ease;
		padding: 0;
		flex-shrink: 0;
	}
	.switch-thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 1rem;
		height: 1rem;
		border-radius: 9999px;
		background: oklch(0.55 0.02 240);
		transition: all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
	}
	.switch-on {
		background: oklch(0.78 0.12 75 / 20%);
		border-color: oklch(0.78 0.12 75 / 30%);
	}
	.switch-on .switch-thumb {
		left: calc(100% - 1rem - 2px);
		background: oklch(0.78 0.12 75);
	}
	.switch:hover {
		border-color: oklch(1 0 0 / 18%);
	}
	.switch:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.gh-status {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 5%);
	}

	.gh-status-info {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.gh-status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.70 0.12 145 / 70%);
	}

	.gh-status-text {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(0.88 0.02 75 / 60%);
	}

	.gh-status-actions {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}

	.gh-token-form {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	/* --- email --- */

	.email-form {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.email-form-group {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.email-form-label {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.55 0.02 280 / 45%);
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.email-form-row {
		display: flex;
		gap: 0.4rem;
	}

	/* Data export/import */
	.data-actions {
		display: flex;
		gap: 0.5rem;
	}
	.data-btn {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		padding: 0.4rem 0.9rem;
		border-radius: 0.375rem;
		cursor: pointer;
		transition: all 0.2s ease;
		color: oklch(0.55 0.08 240 / 70%);
		background: oklch(0.55 0.08 240 / 8%);
		border: 1px solid oklch(0.55 0.08 240 / 15%);
	}
	.data-btn:hover {
		background: oklch(0.55 0.08 240 / 14%);
		border-color: oklch(0.55 0.08 240 / 25%);
	}
	.data-btn-knowledge {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
		border-color: oklch(0.78 0.12 75 / 18%);
	}
	.data-btn-knowledge:hover {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 28%);
	}
	.import-progress {
		margin-top: 0.75rem;
		padding: 0.625rem 0.875rem;
		border-radius: 0.5rem;
		background: oklch(0.55 0.08 240 / 6%);
		border: 1px solid oklch(0.55 0.08 240 / 12%);
	}
	.import-progress.import-done {
		background: oklch(0.72 0.15 155 / 6%);
		border-color: oklch(0.72 0.15 155 / 15%);
	}
	.import-progress.import-error {
		background: oklch(0.65 0.15 25 / 6%);
		border-color: oklch(0.65 0.15 25 / 15%);
	}
	.import-progress-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.import-progress-stage {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: oklch(0.55 0.08 240 / 60%);
	}
	.import-done .import-progress-stage {
		color: oklch(0.72 0.15 155 / 80%);
	}
	.import-error .import-progress-stage {
		color: oklch(0.65 0.15 25 / 80%);
	}
	.import-progress-icon {
		font-size: 0.75rem;
	}
	.import-done .import-progress-icon {
		color: oklch(0.72 0.15 155);
	}
	.import-error .import-progress-icon {
		color: oklch(0.65 0.15 25);
	}
	.import-progress-detail {
		font-size: 0.7rem;
		color: oklch(0.55 0.08 240 / 40%);
		margin-top: 0.25rem;
	}
	.import-progress-spinner {
		width: 12px;
		height: 12px;
		border: 1.5px solid oklch(0.55 0.08 240 / 20%);
		border-top-color: oklch(0.78 0.12 75 / 60%);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}
	/* Scheduled messages */
	.sched-list {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}
	.sched-item {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.625rem;
		border-radius: 0.5rem;
		background: oklch(0.4 0.04 220 / 6%);
		border: 1px solid oklch(0.5 0.06 220 / 6%);
	}
	.sched-content {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}
	.sched-text {
		font-family: var(--font-body);
		font-size: 0.72rem;
		color: oklch(0.85 0.02 220 / 65%);
		line-height: 1.35;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.sched-time {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.65 0.08 220 / 40%);
		letter-spacing: 0.04em;
	}
	.sched-cancel {
		flex-shrink: 0;
		font-family: var(--font-mono);
		font-size: 0.68rem;
		padding: 0.2rem 0.5rem;
		border-radius: 0.3rem;
		background: none;
		border: 1px solid oklch(0.65 0.10 25 / 20%);
		color: oklch(0.65 0.10 25 / 55%);
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.sched-cancel:hover:not(:disabled) {
		border-color: oklch(0.65 0.14 25 / 40%);
		color: oklch(0.65 0.14 25 / 80%);
		background: oklch(0.65 0.10 25 / 8%);
	}
	.sched-cancel:disabled {
		opacity: 0.4;
		cursor: default;
	}

	.data-hint {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75 / 30%);
		margin-top: 0.5rem;
		line-height: 1.5;
	}

	/* --- model mode --- */
	.model-mode-options {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}
	.model-mode-options.disabled {
		opacity: 0.5;
		pointer-events: none;
	}
	.mode-option {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 6%);
		cursor: pointer;
		text-align: left;
		transition: all 0.15s;
	}
	.mode-option:hover:not(:disabled) {
		background: oklch(1 0 0 / 5%);
		border-color: oklch(1 0 0 / 10%);
	}
	.mode-active {
		background: oklch(0.55 0.08 240 / 8%);
		border-color: oklch(0.55 0.08 240 / 25%);
	}
	.mode-active:hover:not(:disabled) {
		background: oklch(0.55 0.08 240 / 12%);
	}
	.mode-name {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		font-weight: 500;
		color: oklch(0.88 0.02 75 / 80%);
	}
	.mode-active .mode-name {
		color: oklch(0.55 0.08 240);
	}
	.mode-desc {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.88 0.02 75 / 35%);
	}
</style>
