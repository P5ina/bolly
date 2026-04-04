<script lang="ts">
	import { fetchObservations, mediaUrl } from "$lib/api/client.js";
	import type { ScreenObservation } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();
	let { slug }: { slug: string } = $props();

	let observations = $state<ScreenObservation[]>([]);
	let loading = $state(true);
	let selectedId = $state<string | null>(null);

	async function load() {
		loading = true;
		try {
			observations = await fetchObservations(slug);
		} catch {
			toast.error("failed to load observations");
		} finally {
			loading = false;
		}
	}

	$effect(() => { load(); });

	function formatTime(ts: string): string {
		const ms = parseInt(ts);
		if (isNaN(ms)) return "";
		const d = new Date(ms);
		const now = new Date();
		const diff = now.getTime() - d.getTime();
		const mins = Math.floor(diff / 60000);
		const hours = Math.floor(diff / 3600000);
		const days = Math.floor(diff / 86400000);
		if (mins < 1) return "just now";
		if (mins < 60) return `${mins}m ago`;
		if (hours < 24) return `${hours}h ago`;
		if (days < 7) return `${days}d ago`;
		return d.toLocaleDateString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
	}

	function formatFullTime(ts: string): string {
		const ms = parseInt(ts);
		if (isNaN(ms)) return "";
		return new Date(ms).toLocaleString([], {
			month: "short", day: "numeric",
			hour: "2-digit", minute: "2-digit",
		});
	}

	const selected = $derived(observations.find(o => o.id === selectedId) ?? null);
</script>

<div class="obs-page">
	{#if loading}
		<div class="obs-center">
			<div class="obs-pulse"></div>
		</div>
	{:else if observations.length === 0}
		<div class="obs-center">
			<div class="obs-empty-icon">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" opacity="0.3">
					<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>
				</svg>
			</div>
			<p class="obs-empty-text">no recordings yet</p>
			<p class="obs-empty-hint">
				enable screen recording in settings and connect your desktop app.
				the observer agent will record and analyze your screen every 15 minutes.
			</p>
		</div>
	{:else}
		<div class="obs-layout">
			<!-- Timeline sidebar -->
			<div class="obs-timeline">
				<div class="obs-timeline-header">
					<span class="obs-timeline-count">{observations.length}</span>
					<span class="obs-timeline-label">recordings</span>
				</div>
				<div class="obs-timeline-list">
					{#each observations as obs, i (obs.id)}
						<button
							class="obs-entry"
							class:obs-entry-active={selectedId === obs.id}
							onclick={() => selectedId = obs.id}
							style="animation-delay: {Math.min(i, 15) * 30}ms"
						>
							<div class="obs-entry-dot" class:obs-entry-dot-active={selectedId === obs.id}></div>
							<div class="obs-entry-content">
								<span class="obs-entry-time">{formatTime(obs.created_at)}</span>
								<span class="obs-entry-machine">{obs.machine_id}</span>
								<p class="obs-entry-preview">{obs.analysis.slice(0, 60)}{obs.analysis.length > 60 ? "..." : ""}</p>
							</div>
						</button>
					{/each}
				</div>
			</div>

			<!-- Detail panel -->
			<div class="obs-detail">
				{#if selected}
					<div class="obs-detail-inner">
						<div class="obs-detail-header">
							<span class="obs-detail-time">{formatFullTime(selected.created_at)}</span>
							<span class="obs-detail-machine">{selected.machine_id}</span>
						</div>

						<video
							class="obs-detail-video"
							src={mediaUrl(slug, selected.upload_id)}
							controls
							preload="metadata"
						></video>

						<div class="obs-detail-section">
							<span class="obs-detail-label">agent's analysis</span>
							<p class="obs-detail-analysis">{selected.analysis}</p>
						</div>
					</div>
				{:else}
					<div class="obs-detail-empty">
						<p class="obs-detail-empty-text">select a recording to view</p>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.obs-page {
		height: 100%;
		overflow: hidden;
	}

	.obs-center {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 0.75rem;
	}

	.obs-pulse {
		width: 6px; height: 6px; border-radius: 50%;
		background: var(--text-muted, oklch(var(--ink) / 30%));
		animation: obs-breathe 2s ease-in-out infinite;
	}
	@keyframes obs-breathe { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }

	.obs-empty-icon { margin-bottom: 0.25rem; color: var(--text-muted); }
	.obs-empty-text {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 0.9rem;
		color: oklch(var(--ink) / 30%);
		margin: 0;
	}
	.obs-empty-hint {
		font-size: 0.72rem;
		color: oklch(var(--ink) / 18%);
		max-width: 30ch;
		text-align: center;
		line-height: 1.5;
		margin: 0;
	}

	/* ─── Layout ─── */
	.obs-layout {
		display: grid;
		grid-template-columns: 280px 1fr;
		height: 100%;
	}

	/* ─── Timeline ─── */
	.obs-timeline {
		border-right: 1px solid oklch(var(--shade) / 6%);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.obs-timeline-header {
		padding: 1.25rem 1rem 0.75rem;
		display: flex;
		align-items: baseline;
		gap: 0.4rem;
	}

	.obs-timeline-count {
		font-family: var(--font-mono);
		font-size: 0.82rem;
		color: oklch(0.78 0.12 75 / 55%);
	}

	.obs-timeline-label {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(var(--ink) / 25%);
		letter-spacing: 0.04em;
	}

	.obs-timeline-list {
		flex: 1;
		overflow-y: auto;
		padding: 0 0.5rem 1rem;
		scrollbar-width: thin;
		scrollbar-color: oklch(var(--ink) / 8%) transparent;
	}

	/* ─── Timeline entry ─── */
	.obs-entry {
		width: 100%;
		display: flex;
		align-items: flex-start;
		gap: 0.625rem;
		padding: 0.625rem 0.5rem;
		border-radius: 0.5rem;
		background: none;
		border: 1px solid transparent;
		cursor: pointer;
		text-align: left;
		font: inherit;
		color: inherit;
		transition: all 0.2s ease;
		opacity: 0;
		animation: obs-entry-in 0.4s ease forwards;
	}
	@keyframes obs-entry-in {
		from { opacity: 0; transform: translateX(-8px); }
		to { opacity: 1; transform: translateX(0); }
	}

	.obs-entry:hover {
		background: oklch(var(--shade) / 4%);
	}

	.obs-entry-active {
		background: oklch(0.78 0.12 75 / 5%);
		border-color: oklch(0.78 0.12 75 / 12%);
	}

	.obs-entry-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		margin-top: 0.4rem;
		flex-shrink: 0;
		background: oklch(var(--ink) / 15%);
		transition: all 0.2s ease;
	}

	.obs-entry-dot-active {
		background: oklch(0.78 0.12 75);
		box-shadow: 0 0 6px oklch(0.78 0.12 75 / 40%);
	}

	.obs-entry-content {
		flex: 1;
		min-width: 0;
	}

	.obs-entry-time {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(var(--ink) / 35%);
		letter-spacing: 0.03em;
	}

	.obs-entry-machine {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(var(--ink) / 18%);
		margin-left: 0.4rem;
	}

	.obs-entry-preview {
		font-size: 0.68rem;
		color: oklch(var(--ink) / 28%);
		line-height: 1.4;
		margin: 0.2rem 0 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.obs-entry-active .obs-entry-preview {
		color: oklch(var(--ink) / 42%);
	}

	/* ─── Detail ─── */
	.obs-detail {
		overflow-y: auto;
		scrollbar-width: thin;
		scrollbar-color: oklch(var(--ink) / 8%) transparent;
	}

	.obs-detail-inner {
		padding: 1.25rem 1.5rem 2rem;
		max-width: 640px;
		animation: obs-detail-in 0.3s ease;
	}
	@keyframes obs-detail-in {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.obs-detail-header {
		display: flex;
		align-items: baseline;
		gap: 0.75rem;
		margin-bottom: 1rem;
	}

	.obs-detail-time {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(var(--ink) / 40%);
	}

	.obs-detail-machine {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(var(--ink) / 20%);
		letter-spacing: 0.04em;
	}

	.obs-detail-video {
		width: 100%;
		border-radius: 0.5rem;
		background: oklch(var(--shade) / 8%);
		margin-bottom: 1.25rem;
	}

	.obs-detail-section {
		margin-bottom: 1.25rem;
	}

	.obs-detail-label {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.78 0.12 75 / 40%);
		letter-spacing: 0.06em;
		text-transform: uppercase;
		display: block;
		margin-bottom: 0.5rem;
	}

	.obs-detail-analysis {
		font-size: 0.78rem;
		line-height: 1.7;
		color: var(--text-secondary, oklch(var(--ink) / 55%));
		white-space: pre-line;
		margin: 0;
	}

	.obs-detail-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.obs-detail-empty-text {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(var(--ink) / 18%);
	}

	@media (max-width: 768px) {
		.obs-layout {
			grid-template-columns: 1fr;
			grid-template-rows: auto 1fr;
		}
		.obs-timeline {
			border-right: none;
			border-bottom: 1px solid oklch(var(--shade) / 6%);
			max-height: 200px;
		}
		.obs-detail-inner {
			padding: 1rem;
		}
	}
</style>
