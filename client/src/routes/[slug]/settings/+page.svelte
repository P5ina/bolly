<script lang="ts">
	import { page } from "$app/state";
	import {
		fetchGoogleAccounts,
		getGoogleConnectUrl,
		disconnectGoogleAccount,
	} from "$lib/api/client.js";

	const slug = $derived(page.params.slug!);

	let accounts = $state<{ email: string }[]>([]);
	let loading = $state(true);
	let disconnecting = $state<string | null>(null);
	let connecting = $state(false);
	let error = $state("");

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

	$effect(() => {
		slug;
		loadAccounts();
	});
</script>

<div class="settings-page">
	<h2 class="settings-title">settings</h2>

	<!-- Google Accounts -->
	<section class="settings-section">
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
			<div class="accounts-loading">
				<div class="loading-dot"></div>
			</div>
		{:else}
			{#if accounts.length > 0}
				<div class="accounts-list">
					{#each accounts as account}
						<div class="account-row">
							<span class="account-email">{account.email}</span>
							<button
								class="account-disconnect"
								disabled={disconnecting === account.email}
								onclick={() => disconnect(account.email)}
							>
								{disconnecting === account.email
									? "disconnecting..."
									: "disconnect"}
							</button>
						</div>
					{/each}
				</div>
			{:else}
				<p class="no-accounts">no google accounts connected</p>
			{/if}

			<button
				class="connect-btn"
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

	.account-disconnect {
		font-family: var(--font-body);
		font-size: 0.65rem;
		color: oklch(0.65 0.12 25 / 60%);
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		transition: all 0.2s ease;
	}
	.account-disconnect:hover:not(:disabled) {
		color: oklch(0.65 0.15 25 / 90%);
		background: oklch(0.65 0.15 25 / 8%);
	}
	.account-disconnect:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.no-accounts {
		font-family: var(--font-body);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 30%);
		font-style: italic;
		margin-bottom: 0.75rem;
	}

	.connect-btn {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		padding: 0.5rem 1rem;
		border-radius: 0.5rem;
		cursor: pointer;
		transition: all 0.2s ease;
		letter-spacing: 0.02em;
	}
	.connect-btn:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 25%);
	}
	.connect-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.accounts-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
	}

	.loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse 1.5s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.3;
			transform: scale(0.7);
		}
	}

	.error-msg {
		font-family: var(--font-body);
		font-size: 0.7rem;
		color: oklch(0.65 0.15 25 / 70%);
		font-style: italic;
		margin-top: 0.5rem;
	}
</style>
