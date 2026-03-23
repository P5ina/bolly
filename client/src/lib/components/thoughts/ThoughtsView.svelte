<script lang="ts">
	import { fetchThoughts } from "$lib/api/client.js";
	import type { Thought, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();
	let { slug }: { slug: string } = $props();

	let thoughts = $state<Thought[]>([]);
	let loading = $state(true);
	let expandedIds = $state<Set<string>>(new Set());

	const ws = getWebSocket();

	const moodColors: Record<string, string> = {
		calm: "oklch(0.70 0.08 220)", curious: "oklch(0.75 0.12 200)",
		excited: "oklch(0.80 0.16 75)", warm: "oklch(0.78 0.12 75)",
		happy: "oklch(0.82 0.14 95)", joyful: "oklch(0.85 0.16 85)",
		reflective: "oklch(0.65 0.08 260)", contemplative: "oklch(0.60 0.06 270)",
		melancholy: "oklch(0.55 0.08 250)", sad: "oklch(0.50 0.06 260)",
		worried: "oklch(0.60 0.10 40)", anxious: "oklch(0.65 0.12 30)",
		playful: "oklch(0.78 0.15 140)", mischievous: "oklch(0.75 0.14 150)",
		focused: "oklch(0.70 0.10 230)", tired: "oklch(0.55 0.04 260)",
		peaceful: "oklch(0.72 0.06 180)", loving: "oklch(0.72 0.14 0)",
		tender: "oklch(0.70 0.10 350)", creative: "oklch(0.78 0.16 310)",
		energetic: "oklch(0.82 0.18 65)",
	};

	async function load() {
		loading = true;
		try { thoughts = await fetchThoughts(slug); }
		catch { toast.error("failed to load thoughts"); }
		finally { loading = false; }
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
		const diff = Date.now() - d.getTime();
		const mins = Math.floor(diff / 60000);
		const hours = Math.floor(diff / 3600000);
		const days = Math.floor(diff / 86400000);
		if (mins < 1) return "just now";
		if (mins < 60) return `${mins}m ago`;
		if (hours < 24) return `${hours}h ago`;
		if (days < 7) return `${days}d ago`;
		return d.toLocaleDateString([], { month: "short", day: "numeric" });
	}

	function parseAction(action: string): { kind: string; label: string } {
		const i = action.indexOf(":");
		if (i === -1) return { kind: action.trim(), label: "" };
		return { kind: action.substring(0, i).trim(), label: action.substring(i + 1).trim() };
	}

	function isQuiet(t: Thought): boolean {
		return t.actions.length === 1 && t.actions[0].startsWith("quiet");
	}

	function cleanRaw(raw: string): string {
		if (!raw) return "";
		// Strip JSON structured output
		try { const j = JSON.parse(raw); return ""; } catch {}
		return raw.trim();
	}

	function primaryKind(t: Thought): string {
		for (const a of t.actions) {
			const p = parseAction(a);
			if (p.kind !== "mood" && p.kind !== "quiet") return p.kind;
		}
		return "quiet";
	}

	function toggleExpand(id: string) {
		const next = new Set(expandedIds);
		if (next.has(id)) next.delete(id); else next.add(id);
		expandedIds = next;
	}

	let visibleThoughts = $derived(thoughts);
</script>

<div class="thoughts-page">
	{#if loading}
		<div class="thoughts-center">
			<div class="pulse-dot"></div>
		</div>
	{:else if thoughts.length === 0}
		<div class="thoughts-center">
			<p class="empty-text">no thoughts yet</p>
			<p class="empty-sub">thoughts appear when your companion reflects on their own</p>
		</div>
	{:else}
		<div class="thoughts-flow">
			{#each visibleThoughts as thought, i (thought.id)}
				{@const mood = thought.actions.find(a => a.startsWith("mood:"))?.substring(5).trim() ?? thought.mood}
				{@const color = moodColors[mood] ?? "oklch(0.78 0.12 75)"}
				{@const kind = primaryKind(thought)}
				{@const raw = cleanRaw(thought.raw)}
				{@const isExpanded = expandedIds.has(thought.id)}
				{@const isLong = raw.length > 180}

				<div
					class="thought"
					class:thought-reach={kind === "reach_out"}
					class:thought-drop={kind === "drop"}
					class:thought-wake={kind === "wake"}
					style="--c: {color}; --i: {Math.min(i, 10)};"
				>
					<!-- Mood glow -->
					<div class="thought-glow" style="background: {color};"></div>

					<!-- Mood + time -->
					<div class="thought-meta">
						<span class="thought-mood" style="color: {color}">{mood}</span>
						<span class="thought-time">{formatTime(thought.created_at)}</span>
					</div>

					<!-- Action label -->
					{#each thought.actions as action}
						{@const p = parseAction(action)}
						{#if p.kind === "reach_out"}
							<span class="thought-action thought-action-reach">reached out</span>
						{:else if p.kind === "drop"}
							<span class="thought-action thought-action-drop">created a drop</span>
						{:else if p.kind === "wake"}
							<span class="thought-action thought-action-wake">woke up</span>
						{/if}
					{/each}

					<!-- Content -->
					{#if raw}
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="thought-body"
							class:thought-body-collapsed={isLong && !isExpanded}
							onclick={() => isLong && toggleExpand(thought.id)}
						>
							{raw}
						</div>
						{#if isLong}
							<button class="thought-more" onclick={() => toggleExpand(thought.id)}>
								{isExpanded ? "less" : "more"}
							</button>
						{/if}
					{/if}
				</div>
			{/each}

		</div>
	{/if}
</div>

<style>
	.thoughts-page {
		height: 100%;
		overflow-y: auto;
		padding: 2rem 1.5rem;
	}

	.thoughts-center {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 0.75rem;
	}

	.pulse-dot {
		width: 6px; height: 6px; border-radius: 50%;
		background: oklch(0.78 0.12 75 / 40%);
		animation: pulse 2s ease-in-out infinite;
	}
	@keyframes pulse { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.3; transform: scale(0.7); } }

	.empty-text {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 0.9rem;
		color: oklch(1 0 0 / 30%);
	}
	.empty-sub {
		font-size: 0.72rem;
		color: oklch(1 0 0 / 18%);
		max-width: 26ch;
		text-align: center;
		line-height: 1.5;
	}

	/* ── Flow layout ── */
	.thoughts-flow {
		max-width: 480px;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}

	/* ── Individual thought ── */
	.thought {
		position: relative;
		padding: 0.875rem 1rem;
		opacity: 0;
		animation: thought-in 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards;
		animation-delay: calc(var(--i) * 50ms);
	}

	@keyframes thought-in {
		from { opacity: 0; transform: translateY(12px); filter: blur(2px); }
		to { opacity: 1; transform: translateY(0); filter: blur(0); }
	}

	/* Ambient glow from mood */
	.thought-glow {
		position: absolute;
		left: -8px;
		top: 0.5rem;
		width: 3px;
		height: 1.5rem;
		border-radius: 2px;
		opacity: 0.35;
		filter: blur(1px);
		transition: opacity 0.3s ease, height 0.3s ease;
	}

	.thought:hover .thought-glow {
		opacity: 0.6;
		height: 100%;
	}

	/* Meta line */
	.thought-meta {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		margin-bottom: 0.375rem;
	}

	.thought-mood {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 0.72rem;
		opacity: 0.7;
	}

	.thought-time {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(1 0 0 / 18%);
		letter-spacing: 0.04em;
	}

	/* Action labels */
	.thought-action {
		display: inline-block;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		letter-spacing: 0.04em;
		margin-bottom: 0.375rem;
		padding: 0.125rem 0.4rem;
		border-radius: 0.75rem;
		background: oklch(1 0 0 / 3%);
		color: oklch(1 0 0 / 35%);
	}
	.thought-action-reach { color: oklch(0.78 0.12 75 / 65%); background: oklch(0.78 0.12 75 / 5%); }
	.thought-action-drop { color: oklch(0.78 0.16 310 / 65%); background: oklch(0.78 0.16 310 / 5%); }
	.thought-action-wake { color: oklch(0.75 0.12 200 / 65%); background: oklch(0.75 0.12 200 / 5%); }

	/* Body text */
	.thought-body {
		font-family: var(--font-body);
		font-size: 0.78rem;
		line-height: 1.7;
		color: oklch(1 0 0 / 42%);
		white-space: pre-line;
		cursor: default;
	}

	.thought-body-collapsed {
		max-height: 4.5em;
		overflow: hidden;
		mask-image: linear-gradient(to bottom, black 50%, transparent 100%);
		-webkit-mask-image: linear-gradient(to bottom, black 50%, transparent 100%);
		cursor: pointer;
	}

	.thought-more {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(1 0 0 / 22%);
		background: none;
		border: none;
		padding: 0;
		margin-top: 0.25rem;
		cursor: pointer;
		letter-spacing: 0.05em;
		transition: color 0.2s ease;
	}
	.thought-more:hover { color: oklch(1 0 0 / 45%); }

	/* Quiet note */
	.quiet-note {
		text-align: center;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(1 0 0 / 15%);
		letter-spacing: 0.06em;
		padding: 1rem 0;
	}

	@media (max-width: 640px) {
		.thoughts-page { padding: 1.5rem 1rem; }
	}
</style>
