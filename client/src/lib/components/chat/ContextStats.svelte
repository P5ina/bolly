<script lang="ts">
	import { fetchContextStats } from "$lib/api/client.js";
	import type { ContextStats } from "$lib/api/types.js";

	interface Props {
		slug: string;
		chatId: string;
		onclose: () => void;
	}

	let { slug, chatId, onclose }: Props = $props();

	let stats = $state<ContextStats | null>(null);
	let error = $state("");
	let loading = $state(true);

	$effect(() => {
		loading = true;
		error = "";
		fetchContextStats(slug, chatId)
			.then((s) => (stats = s))
			.catch((e) => (error = e instanceof Error ? e.message : "failed to load"))
			.finally(() => (loading = false));
	});

	function fmt(n: number): string {
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
		return String(n);
	}

	function pct(part: number, total: number): number {
		return total > 0 ? (part / total) * 100 : 0;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Escape") onclose();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="panel" onclick={(e) => e.stopPropagation()}>
		<div class="header">
			<span class="title">context breakdown</span>
			<button class="close-btn" aria-label="Close" onclick={onclose}>
				<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" class="w-2.5 h-2.5">
					<path d="M2 2l8 8M10 2l-8 8" stroke-linecap="round" />
				</svg>
			</button>
		</div>

		{#if loading}
			<div class="loading">loading...</div>
		{:else if error}
			<div class="error-msg">{error}</div>
		{:else if stats}
			<!-- total bar -->
			<div class="total-row">
				<span class="total-label">total input estimate</span>
				<span class="total-value">{fmt(stats.total_input_tokens_estimate)} tokens</span>
			</div>

			<!-- system prompt breakdown -->
			<div class="section-header">
				<span>system prompt</span>
				<span class="section-total">{fmt(stats.system_prompt_total_tokens)} tokens</span>
			</div>
			<div class="bars">
				{#each stats.system_prompt as section}
					{@const width = pct(section.tokens, stats.system_prompt_total_tokens)}
					<div class="bar-row">
						<span class="bar-label">{section.name}</span>
						<div class="bar-track">
							<div
								class="bar-fill"
								style="width: {Math.max(width, 1)}%"
							></div>
						</div>
						<span class="bar-value">{fmt(section.tokens)}</span>
					</div>
				{/each}
			</div>

			<!-- history -->
			<div class="section-header">
				<span>conversation history</span>
				<span class="section-total">{fmt(stats.history_tokens_estimate)} tokens</span>
			</div>
			<div class="history-detail">
				{stats.history_messages} messages
			</div>

			<!-- composition bar -->
			{@const sysPct = pct(stats.system_prompt_total_tokens, stats.total_input_tokens_estimate)}
			{@const toolsPct = pct(stats.tools_tokens_estimate, stats.total_input_tokens_estimate)}
			{@const histPct = pct(stats.history_tokens_estimate, stats.total_input_tokens_estimate)}
			<div class="composition">
				<div class="comp-label">composition</div>
				<div class="comp-bar">
					<div class="comp-fill comp-sys" style="width: {sysPct}%"></div>
					<div class="comp-fill comp-tools" style="width: {toolsPct}%"></div>
					<div class="comp-fill comp-hist" style="width: {histPct}%"></div>
				</div>
				<div class="comp-legend">
					<span class="legend-item"><span class="dot dot-sys"></span>system</span>
					<span class="legend-item"><span class="dot dot-tools"></span>tools</span>
					<span class="legend-item"><span class="dot dot-hist"></span>history</span>
				</div>
			</div>

			<!-- tools -->
			<div class="section-header">
				<span>tools ({stats.tools_count})</span>
				<span class="section-total">{fmt(stats.tools_tokens_estimate)} tokens</span>
			</div>
			<div class="tools-grid">
				{#each stats.tools as name}
					<span class="tool-tag">{name}</span>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 150;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0 0 0 / 50%);
		backdrop-filter: blur(6px);
		animation: fade-in 0.15s ease;
	}
	@keyframes fade-in {
		from { opacity: 0; }
	}

	.panel {
		width: min(32rem, calc(100vw - 2rem));
		max-height: calc(100dvh - 4rem);
		overflow-y: auto;
		padding: 1.5rem;
		border-radius: 1rem;
		background: oklch(0.06 0.015 280);
		border: 1px solid oklch(0.18 0.02 280);
		animation: panel-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes panel-enter {
		from { opacity: 0; transform: scale(0.96) translateY(6px); }
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1.25rem;
	}
	.title {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.78 0.12 75 / 70%);
	}
	.close-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		border-radius: 50%;
		color: oklch(0.5 0.02 280);
		cursor: pointer;
		transition: all 0.15s;
	}
	.close-btn:hover {
		color: oklch(0.8 0.02 280);
		background: oklch(0.15 0.02 280);
	}

	.loading, .error-msg {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.5 0.02 280);
		text-align: center;
		padding: 2rem 0;
	}
	.error-msg { color: oklch(0.7 0.15 25); }

	.total-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.625rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(0.1 0.02 280);
		margin-bottom: 1.25rem;
	}
	.total-label {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.04em;
		color: oklch(0.6 0.02 280);
	}
	.total-value {
		font-family: var(--font-mono);
		font-size: 0.8rem;
		color: oklch(0.9 0.02 280);
		font-weight: 500;
	}

	.section-header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin-top: 1rem;
		margin-bottom: 0.5rem;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: oklch(0.55 0.02 280);
	}
	.section-total {
		color: oklch(0.7 0.02 280);
		font-size: 0.7rem;
	}

	.bars {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}
	.bar-row {
		display: grid;
		grid-template-columns: 5rem 1fr 3rem;
		align-items: center;
		gap: 0.5rem;
	}
	.bar-label {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.6 0.02 280);
		text-align: right;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.bar-track {
		height: 0.375rem;
		border-radius: 0.2rem;
		background: oklch(0.12 0.015 280);
		overflow: hidden;
	}
	.bar-fill {
		height: 100%;
		border-radius: 0.2rem;
		background: oklch(0.78 0.12 75 / 50%);
		transition: width 0.3s ease;
	}
	.bar-value {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(0.5 0.02 280);
		text-align: right;
	}

	.history-detail {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.5 0.02 280);
		padding-left: 0.25rem;
	}

	.composition {
		margin-top: 1rem;
	}
	.comp-label {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: oklch(0.45 0.02 280);
		margin-bottom: 0.375rem;
	}
	.comp-bar {
		display: flex;
		height: 0.5rem;
		border-radius: 0.25rem;
		overflow: hidden;
		background: oklch(0.12 0.015 280);
		gap: 1px;
	}
	.comp-fill {
		height: 100%;
		transition: width 0.3s ease;
	}
	.comp-sys { background: oklch(0.78 0.12 75 / 60%); }
	.comp-tools { background: oklch(0.65 0.12 150 / 60%); }
	.comp-hist { background: oklch(0.6 0.15 250 / 60%); }

	.comp-legend {
		display: flex;
		gap: 1rem;
		margin-top: 0.375rem;
	}
	.legend-item {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.5 0.02 280);
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}
	.dot-sys { background: oklch(0.78 0.12 75 / 60%); }
	.dot-tools { background: oklch(0.65 0.12 150 / 60%); }
	.dot-hist { background: oklch(0.6 0.15 250 / 60%); }

	.tools-grid {
		display: flex;
		flex-wrap: wrap;
		gap: 0.3rem;
		margin-top: 0.25rem;
	}
	.tool-tag {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.03em;
		padding: 0.15rem 0.4rem;
		border-radius: 0.25rem;
		white-space: nowrap;
		background: oklch(0.78 0.12 75 / 8%);
		color: oklch(0.78 0.12 75 / 70%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
	}
</style>
