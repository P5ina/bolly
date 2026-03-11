<script lang="ts">
	let {
		kind = "tool",
		label,
		timestamp,
	}: {
		kind?: "tool" | "mood" | "state" | "output";
		label: string;
		timestamp?: string;
	} = $props();

	const accentMap: Record<string, string> = {
		tool: "act-tool",
		mood: "act-mood",
		state: "act-state",
		output: "act-output",
	};
</script>

<div class="act-row {accentMap[kind] ?? 'act-tool'}">
	<div class="act-border"></div>
	<div class="act-body">
		{#if kind === "output"}
			<pre class="act-output-text">{label}</pre>
		{:else}
			<span class="act-label">{label}</span>
		{/if}
		{#if timestamp}
			<span class="act-time">{timestamp}</span>
		{/if}
	</div>
</div>

<style>
	.act-row {
		display: flex;
		gap: 0;
		padding: 0.125rem 0;
		animation: act-in 0.35s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes act-in {
		from { opacity: 0; transform: translateX(-4px); }
		to { opacity: 1; transform: translateX(0); }
	}

	.act-border {
		width: 2px;
		flex-shrink: 0;
		border-radius: 1px;
		margin-right: 0.6rem;
		transition: background 0.3s ease;
	}

	.act-tool .act-border {
		background: oklch(0.65 0.12 200 / 50%);
	}
	.act-mood .act-border {
		background: oklch(0.72 0.14 75 / 50%);
	}
	.act-state .act-border {
		background: oklch(0.60 0.08 280 / 40%);
	}
	.act-output .act-border {
		background: oklch(0.55 0.06 200 / 30%);
	}

	.act-body {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		min-width: 0;
		padding: 0.2rem 0;
	}

	.act-label {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.02em;
		color: oklch(0.72 0.02 280 / 60%);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.act-tool .act-label {
		color: oklch(0.68 0.08 200 / 70%);
	}
	.act-mood .act-label {
		color: oklch(0.75 0.08 75 / 70%);
	}
	.act-state .act-label {
		color: oklch(0.62 0.04 280 / 55%);
	}

	.act-output-text {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		line-height: 1.5;
		color: oklch(0.60 0.03 200 / 55%);
		white-space: pre-wrap;
		word-break: break-all;
		margin: 0;
		max-height: 120px;
		overflow-y: auto;
	}

	.act-time {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.50 0.01 280 / 35%);
		white-space: nowrap;
		flex-shrink: 0;
	}
</style>
