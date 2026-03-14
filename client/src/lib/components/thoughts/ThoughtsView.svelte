<script lang="ts">
	import { fetchThoughts } from "$lib/api/client.js";
	import type { Thought, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();

	let { slug }: { slug: string } = $props();

	let thoughts = $state<Thought[]>([]);
	let loading = $state(true);
	let expandedId = $state<string | null>(null);

	const ws = getWebSocket();

	const moodColors: Record<string, string> = {
		calm: "oklch(0.70 0.08 220)",
		curious: "oklch(0.75 0.12 200)",
		excited: "oklch(0.80 0.16 75)",
		warm: "oklch(0.78 0.12 75)",
		happy: "oklch(0.82 0.14 95)",
		joyful: "oklch(0.85 0.16 85)",
		reflective: "oklch(0.65 0.08 260)",
		contemplative: "oklch(0.60 0.06 270)",
		melancholy: "oklch(0.55 0.08 250)",
		sad: "oklch(0.50 0.06 260)",
		worried: "oklch(0.60 0.10 40)",
		anxious: "oklch(0.65 0.12 30)",
		playful: "oklch(0.78 0.15 140)",
		mischievous: "oklch(0.75 0.14 150)",
		focused: "oklch(0.70 0.10 230)",
		tired: "oklch(0.55 0.04 260)",
		peaceful: "oklch(0.72 0.06 180)",
		loving: "oklch(0.72 0.14 0)",
		tender: "oklch(0.70 0.10 350)",
		creative: "oklch(0.78 0.16 310)",
		energetic: "oklch(0.82 0.18 65)",
	};

	const actionIcons: Record<string, string> = {
		reach_out: ">",
		drop: "*",
		mood: "~",
		quiet: ".",
	};

	async function load() {
		loading = true;
		try {
			thoughts = await fetchThoughts(slug);
		} catch {
			toast.error("failed to load thoughts");
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();

		const unsub = ws.subscribe((event: ServerEvent) => {
			if (event.type === "heartbeat_thought" && event.instance_slug === slug) {
				thoughts = [event.thought, ...thoughts];
			}
		});

		return unsub;
	});

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
		return d.toLocaleDateString([], { month: "short", day: "numeric" });
	}

	function actionIcon(action: string): string {
		const key = action.split(":")[0];
		return actionIcons[key] ?? ".";
	}

	function actionLabel(action: string): string {
		const colonIdx = action.indexOf(":");
		if (colonIdx === -1) return action;
		return action.substring(colonIdx + 1).trim();
	}

	function actionKind(action: string): string {
		return action.split(":")[0];
	}
</script>

<div class="thoughts-container">
	{#if loading}
		<div class="thoughts-loading">
			<div class="thoughts-loading-dot"></div>
		</div>
	{:else if thoughts.length === 0}
		<div class="thoughts-empty">
			<div class="thoughts-empty-icon">...</div>
			<p class="thoughts-empty-text">no thoughts yet</p>
			<p class="thoughts-empty-hint">
				your companion thinks during heartbeats — every 45 minutes.
				their inner monologue appears here.
			</p>
		</div>
	{:else}
		<div class="thoughts-header">
			<span class="thoughts-count">{thoughts.length} thought{thoughts.length !== 1 ? "s" : ""}</span>
		</div>
		<div class="thoughts-list">
			{#each thoughts as thought (thought.id)}
				{@const accentColor = moodColors[thought.mood] ?? "oklch(0.78 0.12 75)"}
				{@const isExpanded = expandedId === thought.id}
				<button
					class="thought-card"
					class:thought-card-expanded={isExpanded}
					style="--accent: {accentColor}"
					onclick={() => (expandedId = isExpanded ? null : thought.id)}
				>
					<div class="thought-header">
						<span class="thought-pulse"></span>
						<span class="thought-label">heartbeat</span>
						<span class="thought-time">{formatTime(thought.created_at)}</span>
					</div>

					<div class="thought-raw" class:thought-raw-expanded={isExpanded}>
						{thought.raw}
					</div>

					{#if thought.actions.length > 0}
						<div class="thought-actions">
							{#each thought.actions as action}
								<div class="thought-action">
									<span class="thought-action-icon">{actionIcon(action)}</span>
									<span class="thought-action-kind">{actionKind(action)}</span>
									<span class="thought-action-detail">{actionLabel(action)}</span>
								</div>
							{/each}
						</div>
					{/if}

					{#if thought.mood}
						<div class="thought-mood">
							<span class="thought-mood-dot" style="background: {accentColor}"></span>
							{thought.mood}
						</div>
					{/if}
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.thoughts-container {
		height: 100%;
		overflow-y: auto;
		padding: 2rem 1.5rem;
	}

	.thoughts-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.thoughts-loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	.thoughts-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 0.75rem;
		text-align: center;
	}

	.thoughts-empty-icon {
		font-family: var(--font-mono);
		font-size: 1.5rem;
		color: oklch(0.78 0.12 75 / 20%);
		animation: pulse-alive 3s ease-in-out infinite;
		letter-spacing: 0.2em;
	}

	.thoughts-empty-text {
		font-family: var(--font-display);
		font-size: 0.95rem;
		color: oklch(0.78 0.12 75 / 50%);
	}

	.thoughts-empty-hint {
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 25%);
		max-width: 28ch;
		line-height: 1.5;
	}

	.thoughts-header {
		margin-bottom: 1.25rem;
	}

	.thoughts-count {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 30%);
		letter-spacing: 0.05em;
	}

	.thoughts-list {
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
	}

	.thought-card {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 1rem 1.125rem;
		border-radius: 0.75rem;
		background: oklch(0.09 0.018 278 / 60%);
		border: 1px solid oklch(1 0 0 / 4%);
		border-left: 2px solid color-mix(in oklch, var(--accent) 25%, transparent);
		cursor: pointer;
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		text-align: left;
		width: 100%;
		animation: thought-emerge 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes thought-emerge {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.thought-card:hover {
		background: oklch(0.10 0.020 278 / 70%);
		box-shadow: 0 0 20px color-mix(in oklch, var(--accent) 8%, transparent);
	}

	.thought-card-expanded {
		border-left-color: color-mix(in oklch, var(--accent) 45%, transparent);
		box-shadow: 0 0 30px color-mix(in oklch, var(--accent) 10%, transparent);
	}

	.thought-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.thought-pulse {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--accent, oklch(0.78 0.12 75));
		opacity: 0.5;
	}

	.thought-label {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.thought-time {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 20%);
	}

	.thought-raw {
		font-size: 0.78rem;
		color: oklch(0.88 0.02 75 / 70%);
		line-height: 1.6;
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		-webkit-box-orient: vertical;
		white-space: pre-wrap;
		font-family: var(--font-body);
	}

	.thought-raw-expanded {
		-webkit-line-clamp: unset;
		color: oklch(0.88 0.02 75 / 85%);
	}

	.thought-actions {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding-top: 0.25rem;
		border-top: 1px solid oklch(1 0 0 / 3%);
	}

	.thought-action {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.68rem;
	}

	.thought-action-icon {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: var(--accent, oklch(0.78 0.12 75));
		opacity: 0.5;
		width: 1em;
		text-align: center;
	}

	.thought-action-kind {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 40%);
		letter-spacing: 0.04em;
		text-transform: uppercase;
		min-width: 5ch;
	}

	.thought-action-detail {
		color: oklch(0.78 0.12 75 / 50%);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.thought-mood {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 25%);
		margin-top: 0.25rem;
	}

	.thought-mood-dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		opacity: 0.6;
	}

	@media (max-width: 640px) {
		.thoughts-container {
			padding: 1.5rem 1rem;
		}
	}
</style>
