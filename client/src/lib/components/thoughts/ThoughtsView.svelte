<script lang="ts">
	import { fetchThoughts } from "$lib/api/client.js";
	import type { Thought, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();

	let { slug }: { slug: string } = $props();

	let thoughts = $state<Thought[]>([]);
	let loading = $state(true);

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

	const actionMeta: Record<string, { icon: string; verb: string }> = {
		reach_out: { icon: "→", verb: "reached out" },
		drop: { icon: "✦", verb: "created" },
		mood: { icon: "◑", verb: "shifted to" },
		quiet: { icon: "·", verb: "resting" },
		wake: { icon: "⚡", verb: "woke up" },
		wake_failed: { icon: "✕", verb: "failed to wake" },
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

	function parseAction(action: string): { kind: string; label: string } {
		const colonIdx = action.indexOf(":");
		if (colonIdx === -1) return { kind: action.trim(), label: "" };
		return {
			kind: action.substring(0, colonIdx).trim(),
			label: action.substring(colonIdx + 1).trim(),
		};
	}

	function isQuiet(thought: Thought): boolean {
		return thought.actions.length === 1 && thought.actions[0].startsWith("quiet");
	}

	// Group consecutive quiet thoughts for a cleaner timeline
	interface ThoughtGroup {
		type: "single" | "quiet-cluster";
		thoughts: Thought[];
	}

	let grouped = $derived.by(() => {
		const groups: ThoughtGroup[] = [];
		let quietBuf: Thought[] = [];

		for (const t of thoughts) {
			if (isQuiet(t)) {
				quietBuf.push(t);
			} else {
				if (quietBuf.length > 0) {
					groups.push({ type: "quiet-cluster", thoughts: quietBuf });
					quietBuf = [];
				}
				groups.push({ type: "single", thoughts: [t] });
			}
		}
		if (quietBuf.length > 0) {
			groups.push({ type: "quiet-cluster", thoughts: quietBuf });
		}
		return groups;
	});
</script>

<div class="thoughts-container">
	{#if loading}
		<div class="thoughts-loading">
			<div class="thoughts-loading-dot"></div>
		</div>
	{:else if thoughts.length === 0}
		<div class="thoughts-empty">
			<div class="thoughts-empty-icon">·  ·  ·</div>
			<p class="thoughts-empty-text">no thoughts yet</p>
			<p class="thoughts-empty-hint">
				your companion thinks during heartbeats — every 45 minutes.
				their inner monologue appears here.
			</p>
		</div>
	{:else}
		<div class="thoughts-header">
			<span class="thoughts-count">{thoughts.length} heartbeat{thoughts.length !== 1 ? "s" : ""}</span>
		</div>
		<div class="thoughts-timeline">
			{#each grouped as group}
				{#if group.type === "quiet-cluster"}
					{@const count = group.thoughts.length}
					{@const first = group.thoughts[0]}
					{@const last = group.thoughts[group.thoughts.length - 1]}
					{@const accentColor = moodColors[first.mood] ?? "oklch(0.55 0.03 260)"}
					<div class="quiet-cluster" style="--accent: {accentColor}">
						<div class="quiet-line"></div>
						<span class="quiet-dot">·</span>
						<span class="quiet-label">
							{count === 1 ? "resting" : `resting · ${count} heartbeats`}
						</span>
						<span class="quiet-time">
							{formatTime(last.created_at)}{count > 1 ? ` – ${formatTime(first.created_at)}` : ""}
						</span>
					</div>
				{:else}
					{@const thought = group.thoughts[0]}
					{@const accentColor = moodColors[thought.mood] ?? "oklch(0.78 0.12 75)"}
					<div class="thought-node" style="--accent: {accentColor}">
						<div class="thought-line"></div>
						<div class="thought-dot-wrap">
							<span class="thought-dot"></span>
						</div>
						<div class="thought-body">
							<div class="thought-meta">
								<span class="thought-mood-badge" style="color: {accentColor}">
									{thought.mood}
								</span>
								<span class="thought-time">{formatTime(thought.created_at)}</span>
							</div>
							<div class="thought-actions">
								{#each thought.actions as action}
									{@const parsed = parseAction(action)}
									{@const meta = actionMeta[parsed.kind] ?? { icon: "·", verb: parsed.kind }}
									<div class="thought-action" class:thought-action-drop={parsed.kind === "drop"} class:thought-action-reach={parsed.kind === "reach_out"} class:thought-action-mood={parsed.kind === "mood"} class:thought-action-wake={parsed.kind === "wake"}>
										<span class="thought-action-icon">{meta.icon}</span>
										<span class="thought-action-text">
											<span class="thought-action-verb">{meta.verb}</span>
											{#if parsed.label}
												<span class="thought-action-label">{parsed.label}</span>
											{/if}
										</span>
									</div>
								{/each}
							</div>
						</div>
					</div>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.thoughts-container {
		height: 100%;
		overflow-y: auto;
		padding: 2rem 1.5rem;
		max-width: 480px;
		margin: 0 auto;
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
		font-size: 1.25rem;
		color: oklch(0.78 0.12 75 / 15%);
		animation: pulse-alive 3s ease-in-out infinite;
		letter-spacing: 0.3em;
	}

	.thoughts-empty-text {
		font-family: var(--font-display);
		font-size: 0.95rem;
		font-style: italic;
		color: oklch(0.78 0.12 75 / 45%);
	}

	.thoughts-empty-hint {
		font-size: 0.72rem;
		color: oklch(0.78 0.12 75 / 30%);
		max-width: 26ch;
		line-height: 1.5;
	}

	.thoughts-header {
		margin-bottom: 1.5rem;
	}

	.thoughts-count {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.06em;
	}

	/* ── timeline ── */

	.thoughts-timeline {
		display: flex;
		flex-direction: column;
		position: relative;
	}

	/* ── quiet cluster ── */

	.quiet-cluster {
		display: flex;
		align-items: center;
		gap: 0.625rem;
		padding: 0.625rem 0;
		position: relative;
	}

	.quiet-line {
		position: absolute;
		left: 3.5px;
		top: 0;
		bottom: 0;
		width: 1px;
		background: oklch(1 0 0 / 4%);
	}

	.quiet-dot {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 15%);
		width: 8px;
		text-align: center;
		flex-shrink: 0;
		position: relative;
		z-index: 1;
	}

	.quiet-label {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.04em;
	}

	.quiet-time {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 12%);
	}

	/* ── thought node ── */

	.thought-node {
		display: flex;
		gap: 0.75rem;
		padding: 0.75rem 0;
		position: relative;
		animation: thought-emerge 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes thought-emerge {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.thought-line {
		position: absolute;
		left: 3.5px;
		top: 0;
		bottom: 0;
		width: 1px;
		background: oklch(1 0 0 / 4%);
	}

	.thought-dot-wrap {
		flex-shrink: 0;
		width: 8px;
		display: flex;
		align-items: flex-start;
		padding-top: 0.3rem;
		position: relative;
		z-index: 1;
	}

	.thought-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--accent);
		opacity: 0.6;
		box-shadow: 0 0 8px color-mix(in oklch, var(--accent) 30%, transparent);
	}

	.thought-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.thought-meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.thought-mood-badge {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.04em;
		opacity: 0.7;
	}

	.thought-time {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 35%);
	}

	/* ── actions ── */

	.thought-actions {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}

	.thought-action {
		display: flex;
		align-items: baseline;
		gap: 0.4rem;
		padding: 0.35rem 0.625rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 2.5%);
		border: 1px solid oklch(1 0 0 / 4%);
	}

	.thought-action-drop {
		border-color: oklch(0.78 0.16 310 / 12%);
		background: oklch(0.78 0.16 310 / 3%);
	}

	.thought-action-reach {
		border-color: oklch(0.78 0.12 75 / 12%);
		background: oklch(0.78 0.12 75 / 3%);
	}

	.thought-action-mood {
		border-color: color-mix(in oklch, var(--accent) 15%, transparent);
		background: color-mix(in oklch, var(--accent) 4%, transparent);
	}

	.thought-action-wake {
		border-color: oklch(0.75 0.12 200 / 12%);
		background: oklch(0.75 0.12 200 / 3%);
	}

	.thought-action-icon {
		font-size: 0.7rem;
		color: var(--accent);
		opacity: 0.6;
		flex-shrink: 0;
	}

	.thought-action-text {
		font-size: 0.68rem;
		color: oklch(0.88 0.02 75 / 55%);
		line-height: 1.4;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.thought-action-verb {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.03em;
		margin-right: 0.25rem;
	}

	.thought-action-label {
		color: oklch(0.88 0.02 75 / 60%);
		font-family: var(--font-body);
	}

	@media (max-width: 640px) {
		.thoughts-container {
			padding: 1.5rem 1rem;
		}
	}
</style>
