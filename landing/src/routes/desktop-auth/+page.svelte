<script lang="ts">
	import { onMount } from 'svelte';

	let { data } = $props();
	let opened = $state(false);
	let showCode = $state(false);
	let copied = $state(false);

	onMount(() => {
		window.location.href = `bolly://callback?session=${encodeURIComponent(data.sessionId)}`;
		opened = true;
		// Show manual code after a short delay in case deep link fails
		setTimeout(() => { showCode = true; }, 1500);
	});

	async function copyCode() {
		await navigator.clipboard.writeText(data.sessionId);
		copied = true;
		setTimeout(() => { copied = false; }, 2000);
	}
</script>

<div class="page">
	<div class="card">
		<h1 class="title">{opened ? 'opening bolly...' : 'redirecting...'}</h1>
		<p class="desc">If the app didn't open, make sure Bolly Desktop is running.</p>

		{#if showCode}
			<div class="fallback">
				<p class="fallback-label">Or paste this code in the app:</p>
				<div class="code-row">
					<code class="code">{data.sessionId}</code>
					<button class="copy-btn" onclick={copyCode}>
						{copied ? 'Copied!' : 'Copy'}
					</button>
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	.page {
		min-height: 100dvh;
		background: oklch(0.04 0.015 260);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.5rem;
	}

	.card {
		text-align: center;
		max-width: 26rem;
	}

	.title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 1.25rem;
		font-weight: 400;
		color: oklch(0.90 0.02 75);
		margin: 0 0 0.75rem;
	}

	.desc {
		font-size: 0.82rem;
		color: oklch(0.60 0.03 240);
		line-height: 1.5;
		margin: 0;
	}

	.fallback {
		margin-top: 1.5rem;
		animation: fade-in 0.4s ease both;
	}

	.fallback-label {
		font-size: 0.75rem;
		color: oklch(0.50 0.03 240);
		margin: 0 0 0.5rem;
	}

	.code-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		background: oklch(1 0 0 / 4%);
		border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 0.5rem;
		padding: 0.5rem 0.5rem 0.5rem 0.75rem;
	}

	.code {
		flex: 1;
		font-size: 0.72rem;
		color: oklch(0.78 0.12 75 / 70%);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: monospace;
		text-align: left;
	}

	.copy-btn {
		flex-shrink: 0;
		padding: 0.35rem 0.75rem;
		border-radius: 0.375rem;
		border: 1px solid oklch(1 0 0 / 10%);
		background: oklch(1 0 0 / 5%);
		color: oklch(0.90 0.02 75 / 60%);
		font-size: 0.72rem;
		cursor: pointer;
		transition: all 0.2s;
		font-family: inherit;
	}

	.copy-btn:hover {
		background: oklch(1 0 0 / 8%);
		color: oklch(0.90 0.02 75);
	}

	@keyframes fade-in {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>
