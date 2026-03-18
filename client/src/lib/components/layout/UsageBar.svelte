<script lang="ts">
	import { fetchUsage } from "$lib/api/client.js";
	import type { Usage } from "$lib/api/types.js";

	let { tick = 0 }: { tick?: number } = $props();

	let usage = $state<Usage | null>(null);
	let now = $state(Date.now());

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
		const interval = setInterval(() => { load(); now = Date.now(); }, 60_000);
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

	let resetLabel = $derived.by(() => {
		if (!usage?.resets_at) return "";
		const resetMs = new Date(usage.resets_at).getTime();
		const diff = resetMs - now;
		if (diff <= 0) return "resets soon";
		const mins = Math.floor(diff / 60_000);
		if (mins < 60) return `${mins}m`;
		const h = Math.floor(mins / 60);
		const m = mins % 60;
		return m > 0 ? `${h}h${m}m` : `${h}h`;
	});
</script>

{#if limit4h > 0}
	{@const p = pct(used4h, limit4h)}
	<div class="usage-bar">
		<div class="usage-item" title="{formatTokens(used4h)} / {formatTokens(limit4h)} tokens (4h) — resets {resetLabel}">
			<span class="usage-label">{formatTokens(used4h)}/{formatTokens(limit4h)}</span>
			<div class="usage-track">
				<div class="usage-fill" style="width: {p}%; background: {barColor(p)}"></div>
			</div>
			{#if resetLabel}
				<span class="usage-reset">{resetLabel}</span>
			{/if}
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
		font-size: 0.75rem;
		letter-spacing: 0.03em;
		color: oklch(0.78 0.12 75 / 35%);
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

	.usage-reset {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.78 0.12 75 / 35%);
		white-space: nowrap;
	}
</style>
