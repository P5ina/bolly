<script lang="ts">
	import { fetchUsage } from "$lib/api/client.js";
	import type { Usage } from "$lib/api/types.js";

	let { tick = 0 }: { tick?: number } = $props();

	let usage = $state<Usage | null>(null);

	async function load() {
		try {
			usage = await fetchUsage();
		} catch {
			usage = null;
		}
	}

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

	let limit4h = $derived(usage?.tokens_4h_limit ?? 0);
	let used4h = $derived(usage?.tokens_last_4h ?? 0);
</script>

{#if limit4h > 0}
	{@const p = pct(used4h, limit4h)}
	<div class="usage-bar">
		<div class="usage-item" title="{formatTokens(used4h)} / {formatTokens(limit4h)} tokens (4h)">
			<span class="usage-label">{formatTokens(used4h)}/{formatTokens(limit4h)}</span>
			<div class="usage-track">
				<div class="usage-fill" style="width: {p}%; background: {barColor(p)}"></div>
			</div>
		</div>
	</div>
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
