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

	let expanded = $state(false);

	const accentMap: Record<string, string> = {
		tool: "act-tool",
		mood: "act-mood",
		state: "act-state",
		output: "act-output",
	};

	// Unescape literal \n and \t sequences that may come from JSON-escaped output
	const displayLabel = $derived(
		label.replace(/\\n/g, "\n").replace(/\\t/g, "\t")
	);

	const isCollapsible = $derived(
		(kind === "output" || kind === "tool") && displayLabel.includes("\n")
	);

	const firstLine = $derived(displayLabel.split("\n")[0]);
</script>

<div class="act-row {accentMap[kind] ?? 'act-tool'}">
	<div class="act-border"></div>
	<div class="act-body">
		{#if isCollapsible}
			<button class="act-toggle" onclick={() => expanded = !expanded}>
				<span class="act-chevron" class:act-chevron-open={expanded}>›</span>
				{#if kind === "output"}
					<pre class="act-output-text act-single-line">{firstLine}</pre>
				{:else}
					<span class="act-label">{firstLine}</span>
				{/if}
			</button>
			{#if expanded}
				<pre class="act-output-text act-expanded">{displayLabel}</pre>
			{/if}
		{:else if kind === "output"}
			<pre class="act-output-text">{displayLabel}</pre>
		{:else}
			<span class="act-label">{displayLabel}</span>
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
		flex-direction: column;
		gap: 0;
		min-width: 0;
		padding: 0.2rem 0;
		flex: 1;
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
	}

	.act-single-line {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.act-expanded {
		max-height: 400px;
		overflow-y: auto;
		padding: 0.3rem 0;
		animation: expand-in 0.2s ease both;
	}

	@keyframes expand-in {
		from { opacity: 0; max-height: 0; }
		to { opacity: 1; max-height: 400px; }
	}

	.act-toggle {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
		min-width: 0;
		width: 100%;
		text-align: left;
	}

	.act-toggle:hover .act-chevron {
		color: oklch(0.78 0.12 75 / 70%);
	}

	.act-chevron {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.55 0.04 280 / 40%);
		flex-shrink: 0;
		transition: transform 0.2s ease, color 0.2s ease;
		line-height: 1;
	}

	.act-chevron-open {
		transform: rotate(90deg);
	}

	.act-time {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.50 0.01 280 / 35%);
		white-space: nowrap;
		flex-shrink: 0;
		align-self: flex-start;
	}
</style>
