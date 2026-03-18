<script lang="ts">
	import { page } from "$app/state";
	import {
		checkUpdate,
		applyUpdate,
		getUpdateChannel,
		setUpdateChannel,
		type UpdateCheck,
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
		fetchEmailAccounts,
		saveEmailAccounts,
		deleteAllEmailAccounts,
		fetchUsage,
		exportInstanceUrl,
		importInstance,
	} from "$lib/api/client.js";
	import type { McpServerInfo, EmailConfig } from "$lib/api/client.js";
	import type { Usage } from "$lib/api/types.js";

	const slug = $derived(page.params.slug!);

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
	let updateInfo = $state<UpdateCheck | null>(null);
	let updating = $state(false);
	let updateDone = $state(false);
	let channel = $state("stable");
	$effect(() => {
		checkUpdate().then(u => updateInfo = u).catch(() => {});
		getUpdateChannel().then(r => channel = r.channel).catch(() => {});
	});

	async function doUpdate() {
		updating = true;
		try {
			await applyUpdate();
		} catch {
			// Connection may reset when server exits — that's expected
		}
		// Server is restarting. Wait a bit then poll until it's back.
		await new Promise(r => setTimeout(r, 4000));
		for (let i = 0; i < 30; i++) {
			try {
				updateInfo = await checkUpdate();
				// Server responded — it's back
				updating = false;
				updateDone = true;
				setTimeout(() => { updateDone = false; }, 8000);
				return;
			} catch {
				// Still down, keep polling
				await new Promise(r => setTimeout(r, 2000));
			}
		}
		// Timeout
		updating = false;
	}

	// Usage state
	let usage = $state<Usage | null>(null);
	$effect(() => { fetchUsage().then(u => usage = u).catch(() => {}); });

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
	});
</script>

<div class="settings-page">
	<h2 class="settings-title">settings</h2>

	<div class="update-section">
		<div class="update-row">
			<span class="update-current">v{updateInfo?.current?.replace('v','') ?? '...'}</span>
			<select
				class="channel-select"
				value={channel}
				onchange={async (e) => {
					const val = (e.target as HTMLSelectElement).value;
					channel = val;
					await setUpdateChannel(val);
					updateInfo = await checkUpdate();
				}}
			>
				<option value="stable">stable</option>
				<option value="nightly">nightly</option>
			</select>
		</div>
		{#if updateDone}
			<div class="update-banner update-done">
				<span class="update-label">updated!</span>
				<button class="update-reload-btn" onclick={() => location.reload()}>reload page</button>
			</div>
		{:else if updating}
			<div class="update-banner">
				<span class="update-label">updating…</span>
				<span class="update-version">server restarting</span>
			</div>
		{:else if updateInfo?.update_available}
			<div class="update-banner">
				<div class="update-info">
					<span class="update-label">update available</span>
					<span class="update-version">{updateInfo.current} → {updateInfo.latest}</span>
				</div>
				<button
					class="update-btn"
					onclick={doUpdate}
				>
					update now
				</button>
			</div>
		{/if}
	</div>

	<!-- Usage -->
	{#if usage && (usage.tokens_4h_limit > 0 || usage.tokens_week_limit > 0 || usage.tokens_month_limit > 0)}
		<section class="settings-section">
			<div class="section-header">
				<div class="section-icon">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
						<path d="M12 20V10"/><path d="M18 20V4"/><path d="M6 20v-4"/>
					</svg>
				</div>
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
							<div class="usage-window-fill" style="width: {p}%; background: {p >= 90 ? 'oklch(0.65 0.2 25)' : p >= 70 ? 'oklch(0.75 0.15 85)' : 'oklch(0.78 0.12 75 / 50%)'}"></div>
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
							<div class="usage-window-fill" style="width: {p}%; background: {p >= 90 ? 'oklch(0.65 0.2 25)' : p >= 70 ? 'oklch(0.75 0.15 85)' : 'oklch(0.78 0.12 75 / 50%)'}"></div>
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
							<div class="usage-window-fill" style="width: {p}%; background: {p >= 90 ? 'oklch(0.65 0.2 25)' : p >= 70 ? 'oklch(0.75 0.15 85)' : 'oklch(0.78 0.12 75 / 50%)'}"></div>
						</div>
					</div>
				{/if}
			</div>
		</section>
	{/if}

	<!-- Timezone -->
	<section class="settings-section">
		<div class="section-header">
			<div class="section-icon tz-icon">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<circle cx="12" cy="12" r="10"/>
					<path d="M12 6v6l4 2"/>
				</svg>
			</div>
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

	<!-- Email (SMTP/IMAP) -->
	<section class="settings-section" style="margin-top: 1rem;">
		<div class="section-header">
			<div class="section-icon email-icon">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<rect x="2" y="4" width="20" height="16" rx="2"/>
					<path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/>
				</svg>
			</div>
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
	<section class="settings-section" style="margin-top: 1rem;">
		<div class="section-header">
			<div class="section-icon gh-icon">
				<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
					<path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z"/>
				</svg>
			</div>
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

	<!-- Export / Import -->
	<section class="settings-section">
		<div class="section-label">data</div>
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
		{#if importError}
			<p class="error-msg">{importError}</p>
		{/if}
	</section>
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
		color: oklch(0.78 0.12 75 / 30%);
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
		padding: 2rem 1.5rem;
		padding-bottom: calc(2rem + env(safe-area-inset-bottom, 0px));
		max-width: 560px;
		margin: 0 auto;
		height: 100%;
		overflow-y: auto;
	}

	.settings-title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 1.25rem;
		font-weight: 400;
		color: oklch(0.88 0.02 75 / 80%);
		margin-bottom: 2rem;
	}

	.update-section {
		margin-bottom: 1.5rem;
	}
	.update-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 0.5rem;
	}
	.update-current {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 30%);
	}
	.channel-select {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		padding: 0.2rem 0.4rem;
		border-radius: 0.25rem;
		background: oklch(1 0 0 / 4%);
		border: 1px solid oklch(1 0 0 / 8%);
		color: oklch(0.88 0.02 75 / 60%);
		cursor: pointer;
	}
	.update-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.625rem 0.75rem;
		margin-bottom: 1.5rem;
		border-radius: 0.5rem;
		background: oklch(0.72 0.15 155 / 6%);
		border: 1px solid oklch(0.72 0.15 155 / 18%);
	}
	.update-done {
		background: oklch(0.72 0.15 155 / 10%);
		border-color: oklch(0.72 0.15 155 / 30%);
	}
	.update-done .update-label {
		color: oklch(0.72 0.15 155);
	}
	.update-reload-btn {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		padding: 0.25rem 0.6rem;
		border-radius: 0.3rem;
		background: oklch(0.72 0.15 155 / 15%);
		color: oklch(0.72 0.15 155);
		border: 1px solid oklch(0.72 0.15 155 / 30%);
		cursor: pointer;
		transition: background 0.15s;
	}
	.update-reload-btn:hover {
		background: oklch(0.72 0.15 155 / 25%);
	}
	.update-info {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}
	.update-label {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.05em;
		color: oklch(0.72 0.15 155 / 75%);
	}
	.update-version {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.72 0.15 155 / 40%);
	}
	.update-btn {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		padding: 0.3rem 0.75rem;
		border-radius: 0.375rem;
		background: oklch(0.72 0.15 155 / 12%);
		border: 1px solid oklch(0.72 0.15 155 / 25%);
		color: oklch(0.72 0.15 155 / 85%);
		cursor: pointer;
		transition: all 0.2s ease;
		white-space: nowrap;
	}
	.update-btn:hover:not(:disabled) {
		background: oklch(0.72 0.15 155 / 20%);
	}
	.update-btn:disabled {
		opacity: 0.5;
		cursor: default;
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

	.tz-icon {
		color: oklch(0.75 0.10 200 / 60%);
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
		border-color: oklch(0.78 0.12 75 / 30%);
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
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 35%);
		line-height: 1.4;
	}

	.ext-toggle {
		font-family: var(--font-mono);
		font-size: 0.75rem;
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
		border-color: oklch(0.78 0.12 75 / 35%);
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
		border: 1.5px solid oklch(0.78 0.12 75 / 35%);
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
		font-size: 0.68rem;
		color: oklch(0.55 0.02 280 / 40%);
	}

	.ext-custom-connected {
		color: oklch(0.70 0.12 145 / 70%);
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

	/* --- advanced / custom form --- */

	.ext-advanced {
		margin-top: 0.25rem;
	}

	.ext-advanced-toggle {
		font-family: var(--font-mono);
		font-size: 0.72rem;
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
		font-size: 0.75rem;
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
		border-color: oklch(0.78 0.12 75 / 35%);
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

	.reconnect-banner {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		padding: 0.625rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(0.78 0.12 75 / 6%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		margin-bottom: 0.75rem;
	}
	.reconnect-icon {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		font-weight: 700;
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 15%);
		width: 16px;
		height: 16px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		margin-top: 1px;
	}
	.reconnect-text {
		font-family: var(--font-body);
		font-size: 0.68rem;
		line-height: 1.4;
		color: oklch(0.88 0.02 75 / 55%);
	}

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

	/* --- github --- */

	.gh-icon {
		color: oklch(0.88 0 0 / 60%);
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

	.email-icon {
		color: oklch(0.72 0.10 250 / 60%);
	}

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
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
	}
	.data-btn:hover {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 25%);
	}
	.data-hint {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75 / 30%);
		margin-top: 0.5rem;
		line-height: 1.5;
	}
</style>
