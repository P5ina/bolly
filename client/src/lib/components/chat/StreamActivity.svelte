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

	// Unescape literal \n and \t sequences that may come from JSON-escaped output
	const displayLabel = $derived(
		label.replace(/\\n/g, "\n").replace(/\\t/g, "\t")
	);

	const isCollapsible = $derived(
		(kind === "output" || kind === "tool") && displayLabel.includes("\n")
	);

	const firstLine = $derived(displayLabel.split("\n")[0]);
</script>

<div class="act act-{kind}">
	<div class="act-pip"></div>
	<div class="act-body">
		{#if isCollapsible}
			<button class="act-toggle" onclick={() => expanded = !expanded}>
				<span class="act-chevron" class:act-chevron-open={expanded}>›</span>
				{#if kind === "output"}
					<pre class="act-pre act-clamp">{firstLine}</pre>
				{:else}
					<span class="act-text">{firstLine}</span>
				{/if}
			</button>
			{#if expanded}
				<pre class="act-pre act-full">{displayLabel}</pre>
			{/if}
		{:else if kind === "output"}
			<pre class="act-pre">{displayLabel}</pre>
		{:else}
			<span class="act-text">{displayLabel}</span>
		{/if}
	</div>
	{#if timestamp}
		<span class="act-ts">{timestamp}</span>
	{/if}
</div>

<style>
	.act {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		padding: 0.2rem 0;
		animation: act-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
		max-width: 100%;
		overflow: hidden;
	}

	@keyframes act-in {
		from { opacity: 0; transform: translateX(-4px); }
		to { opacity: 1; transform: translateX(0); }
	}

	/* ── pip (left accent dot) ── */
	.act-pip {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		flex-shrink: 0;
		margin-top: 0.35rem;
		transition: background 0.3s ease;
	}

	.act-tool .act-pip { background: oklch(0.58 0.12 190 / 55%); box-shadow: 0 0 5px oklch(0.58 0.12 190 / 20%); }
	.act-mood .act-pip { background: oklch(0.68 0.12 75 / 55%); box-shadow: 0 0 5px oklch(0.68 0.12 75 / 20%); }
	.act-state .act-pip { background: oklch(0.50 0.06 240 / 40%); }
	.act-output .act-pip { background: oklch(0.45 0.04 240 / 30%); }

	/* ── body ── */
	.act-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}

	.act-text {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.01em;
		color: oklch(0.58 0.04 220 / 55%);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.act-tool .act-text { color: oklch(0.60 0.08 190 / 65%); }
	.act-mood .act-text { color: oklch(0.68 0.08 75 / 65%); }
	.act-state .act-text { color: oklch(0.52 0.04 240 / 45%); }

	.act-pre {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		line-height: 1.5;
		color: oklch(0.50 0.03 220 / 45%);
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
	}

	.act-clamp {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.act-full {
		max-height: 300px;
		overflow-y: auto;
		padding: 0.25rem 0;
		animation: expand 0.2s ease both;
	}

	@keyframes expand {
		from { opacity: 0; max-height: 0; }
		to { opacity: 1; max-height: 300px; }
	}

	/* ── toggle ── */
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
		color: oklch(0.65 0.1 190 / 70%);
	}

	.act-chevron {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.45 0.04 220 / 35%);
		flex-shrink: 0;
		transition: transform 0.2s ease, color 0.2s ease;
		line-height: 1;
	}

	.act-chevron-open {
		transform: rotate(90deg);
	}

	/* ── timestamp ── */
	.act-ts {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.42 0.03 220 / 25%);
		white-space: nowrap;
		flex-shrink: 0;
		margin-top: 0.25rem;
	}
</style>
