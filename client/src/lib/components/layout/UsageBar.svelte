<script lang="ts">
	import { fetchUsage } from "$lib/api/client.js";
	import type { Usage } from "$lib/api/types.js";

	let { tick = 0 }: { tick?: number } = $props();

	let usage: Usage | null = $state(null);

	async function load() {
		try {
			usage = await fetchUsage();
		} catch {
			usage = null;
		}
	}

	// Refresh on mount, every 60s, and whenever tick changes
	$effect(() => {
		void tick;
		load();
	});

	$effect(() => {
		const interval = setInterval(load, 60_000);
		return () => clearInterval(interval);
	});

	function pct(used: number, limit: number): number {
		if (limit <= 0) return 0;
		return Math.min(100, Math.round((used / limit) * 100));
	}

	function barColor(p: number): string {
		if (p >= 100) return "oklch(0.65 0.2 25)";
		if (p >= 80) return "oklch(0.75 0.15 85)";
		return "oklch(0.78 0.12 75 / 40%)";
	}

	function formatTokens(n: number): string {
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
		return String(n);
	}
</script>

{#if usage}
	{@const tokPct = pct(usage.tokens_this_month, usage.tokens_limit)}
	{@const tokUnlimited = usage.tokens_limit < 0}

	{#if !tokUnlimited}
		<div class="usage-bar">
			<div class="usage-item" title="Tokens this month: {formatTokens(usage.tokens_this_month)} / {formatTokens(usage.tokens_limit)}">
				<span class="usage-label">{formatTokens(usage.tokens_this_month)}/{formatTokens(usage.tokens_limit)} tokens</span>
				<div class="usage-track">
					<div
						class="usage-fill"
						style="width: {tokPct}%; background: {barColor(tokPct)}"
					></div>
				</div>
			</div>
		</div>
	{/if}
{/if}

<style>
	.usage-bar {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.75rem;
		padding: 0.5rem 0.3rem 0;
	}

	.usage-item {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		cursor: default;
	}

	.usage-label {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		letter-spacing: 0.03em;
		color: oklch(0.78 0.12 75 / 25%);
		white-space: nowrap;
	}

	.usage-track {
		width: 2.5rem;
		height: 2px;
		border-radius: 1px;
		background: oklch(1 0 0 / 4%);
		overflow: hidden;
	}

	.usage-fill {
		height: 100%;
		border-radius: 1px;
		transition: width 0.5s ease, background 0.5s ease;
	}
</style>
