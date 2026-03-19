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

	const moodEmoji: Record<string, string> = {
		calm: "◌",
		curious: "◎",
		excited: "✧",
		warm: "◉",
		happy: "○",
		joyful: "✦",
		reflective: "◈",
		contemplative: "◇",
		melancholy: "◆",
		sad: "●",
		worried: "◐",
		anxious: "◑",
		playful: "◍",
		mischievous: "◕",
		focused: "◎",
		tired: "◌",
		peaceful: "○",
		loving: "♡",
		tender: "♡",
		creative: "✧",
		energetic: "✦",
	};

	const actionKindLabels: Record<string, { icon: string; label: string }> = {
		reach_out: { icon: "→", label: "reached out" },
		drop: { icon: "✦", label: "created a drop" },
		mood: { icon: "◑", label: "mood shift" },
		quiet: { icon: "·", label: "resting" },
		wake: { icon: "⚡", label: "woke up" },
		wake_failed: { icon: "✕", label: "couldn't wake" },
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

	/** Extract the first meaningful sentence from raw thought text */
	function extractSummary(raw: string): string {
		if (!raw) return "";
		// Take first ~200 chars, break at sentence boundary
		const trimmed = raw.trim();
		if (trimmed.length <= 200) return trimmed;
		const cut = trimmed.substring(0, 200);
		const lastDot = cut.lastIndexOf(".");
		const lastExcl = cut.lastIndexOf("!");
		const lastQ = cut.lastIndexOf("?");
		const boundary = Math.max(lastDot, lastExcl, lastQ);
		if (boundary > 80) return trimmed.substring(0, boundary + 1);
		return cut + "…";
	}

	/** Clean raw text: strip action prefixes like "WAKE:foo bar" that duplicate action tags */
	function cleanRaw(raw: string, actions: string[]): string {
		if (!raw) return "";
		let cleaned = raw.trim();
		// Strip lines that are just action prefixes (e.g. "WAKE:memory-cleanup ...")
		for (const action of actions) {
			const parsed = parseAction(action);
			// Remove "KIND:label" prefix if raw starts with it
			const prefix = `${parsed.kind.toUpperCase()}:${parsed.label}`;
			if (prefix && cleaned.toUpperCase().startsWith(prefix.toUpperCase())) {
				cleaned = cleaned.substring(prefix.length).trim();
			}
			// Also try "kind (label)" format
			const altPrefix = `${parsed.kind} (${parsed.label})`;
			if (altPrefix && cleaned.toLowerCase().startsWith(altPrefix.toLowerCase())) {
				cleaned = cleaned.substring(altPrefix.length).trim();
			}
		}
		return cleaned;
	}

	/** Format the raw thought into readable paragraphs */
	function formatRaw(raw: string): string[] {
		if (!raw) return [];
		return raw
			.split(/\n{2,}/)
			.map((p) => p.trim())
			.filter(Boolean);
	}

	function toggleExpand(id: string) {
		const next = new Set(expandedIds);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expandedIds = next;
	}

	// Group consecutive quiet thoughts
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

	/** Get primary action kind for card accent */
	function primaryKind(thought: Thought): string {
		for (const a of thought.actions) {
			const p = parseAction(a);
			if (p.kind !== "mood" && p.kind !== "quiet") return p.kind;
		}
		return thought.actions[0]?.split(":")[0]?.trim() ?? "quiet";
	}
</script>

<div class="thoughts-container">
	{#if loading}
		<div class="thoughts-loading">
			<div class="loading-orb">
				<span class="loading-ring"></span>
				<span class="loading-dot"></span>
			</div>
		</div>
	{:else if thoughts.length === 0}
		<div class="thoughts-empty">
			<div class="empty-orb">
				<span class="empty-ring"></span>
				<span class="empty-dot"></span>
			</div>
			<p class="empty-title">no thoughts yet</p>
			<p class="empty-hint">
				thoughts appear during heartbeats — the quiet moments when your companion reflects on their own.
			</p>
		</div>
	{:else}
		<div class="thoughts-header">
			<span class="thoughts-count">{thoughts.length} heartbeat{thoughts.length !== 1 ? "s" : ""}</span>
		</div>

		<div class="thoughts-stream">
			{#each grouped as group, gi (group.type === "quiet-cluster" ? `q-${group.thoughts[0].id}` : group.thoughts[0].id)}
				{#if group.type === "quiet-cluster"}
					{@const count = group.thoughts.length}
					{@const first = group.thoughts[0]}
					{@const last = group.thoughts[group.thoughts.length - 1]}
					<div class="quiet-divider" style="animation-delay: {Math.min(gi * 60, 600)}ms">
						<div class="quiet-line"></div>
						<span class="quiet-badge">
							{count === 1 ? "resting" : `resting · ${count}`}
						</span>
						<div class="quiet-line"></div>
						<span class="quiet-time">
							{formatTime(last.created_at)}{count > 1 ? ` – ${formatTime(first.created_at)}` : ""}
						</span>
					</div>
				{:else}
					{@const thought = group.thoughts[0]}
					{@const moodAction = thought.actions.find(a => a.startsWith("mood:"))}
					{@const displayMood = moodAction ? moodAction.substring(5).trim() : thought.mood}
					{@const accentColor = moodColors[displayMood] ?? "oklch(0.78 0.12 75)"}
					{@const emoji = moodEmoji[displayMood] ?? "·"}
					{@const kind = primaryKind(thought)}
					{@const isExpanded = expandedIds.has(thought.id)}
					{@const cleaned = cleanRaw(thought.raw, thought.actions)}
					{@const rawParagraphs = formatRaw(cleaned)}
					{@const hasLongContent = cleaned.length > 200}
					{@const summary = extractSummary(cleaned)}

					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="thought-card"
						class:thought-card-wake={kind === "wake"}
						class:thought-card-reach={kind === "reach_out"}
						class:thought-card-drop={kind === "drop"}
						style="--accent: {accentColor}; animation-delay: {Math.min(gi * 60, 600)}ms"
						onclick={() => hasLongContent && toggleExpand(thought.id)}
					>
						<!-- Specular highlight -->
						<div class="card-highlight"></div>

						<!-- Header -->
						<div class="card-header">
							<div class="card-mood">
								<span class="mood-symbol" style="color: {accentColor}">{emoji}</span>
								<span class="mood-label" style="color: {accentColor}">{displayMood}</span>
							</div>
							<span class="card-time">{formatTime(thought.created_at)}</span>
						</div>

						<!-- Action tags -->
						{#if thought.actions.length > 0}
							<div class="card-actions">
								{#each thought.actions as action}
									{@const parsed = parseAction(action)}
									{@const meta = actionKindLabels[parsed.kind] ?? { icon: "·", label: parsed.kind }}
									{#if parsed.kind !== "quiet" && parsed.kind !== "mood"}
										<span
											class="action-tag"
											class:action-tag-wake={parsed.kind === "wake"}
											class:action-tag-reach={parsed.kind === "reach_out"}
											class:action-tag-drop={parsed.kind === "drop"}
											class:action-tag-mood={parsed.kind === "mood"}
										>
											<span class="action-icon">{meta.icon}</span>
											{meta.label}{#if parsed.kind === "mood" && parsed.label}: {parsed.label}{/if}
										</span>
									{/if}
								{/each}
							</div>
						{/if}

						<!-- Thought content -->
						{#if cleaned}
							<div class="card-content" class:card-content-expanded={isExpanded}>
								{#if isExpanded}
									{#each rawParagraphs as para}
										<p class="thought-para">{para}</p>
									{/each}
								{:else}
									<p class="thought-para">{summary}</p>
								{/if}
							</div>
							{#if hasLongContent}
								<button class="expand-toggle" onclick={(e: MouseEvent) => { e.stopPropagation(); toggleExpand(thought.id); }}>
									{isExpanded ? "collapse" : "read more"}
								</button>
							{/if}
						{/if}
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
		max-width: 560px;
		margin: 0 auto;
	}

	/* ── Loading ── */

	.thoughts-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.loading-orb {
		position: relative;
		width: 24px;
		height: 24px;
	}

	.loading-dot {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 40%);
		transform: translate(-50%, -50%);
		animation: orb-pulse 1.8s ease-in-out infinite;
	}

	.loading-ring {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		border: 1.5px solid oklch(0.78 0.12 75 / 20%);
		animation: ring-expand 1.8s ease-out infinite;
	}

	@keyframes orb-pulse {
		0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 0.5; }
		50% { transform: translate(-50%, -50%) scale(0.7); opacity: 0.2; }
	}

	@keyframes ring-expand {
		0% { transform: scale(0.6); opacity: 0.4; }
		100% { transform: scale(1.6); opacity: 0; }
	}

	/* ── Empty ── */

	.thoughts-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 1rem;
		text-align: center;
	}

	.empty-orb {
		position: relative;
		width: 40px;
		height: 40px;
		margin-bottom: 0.5rem;
	}

	.empty-dot {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 20%);
		transform: translate(-50%, -50%);
		animation: orb-pulse 3s ease-in-out infinite;
	}

	.empty-ring {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		animation: ring-expand 3s ease-out infinite;
	}

	.empty-title {
		font-family: var(--font-display);
		font-size: 1rem;
		font-style: italic;
		color: oklch(0.78 0.12 75 / 40%);
	}

	.empty-hint {
		font-size: 0.75rem;
		color: oklch(1 0 0 / 25%);
		max-width: 28ch;
		line-height: 1.6;
	}

	/* ── Header ── */

	.thoughts-header {
		margin-bottom: 1.5rem;
	}

	.thoughts-count {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(1 0 0 / 25%);
		letter-spacing: 0.06em;
	}

	/* ── Stream ── */

	.thoughts-stream {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	/* ── Quiet divider ── */

	.quiet-divider {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0;
		opacity: 0;
		animation: card-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
	}

	.quiet-line {
		flex: 1;
		height: 1px;
		background: oklch(1 0 0 / 5%);
	}

	.quiet-badge {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(1 0 0 / 20%);
		letter-spacing: 0.05em;
		white-space: nowrap;
	}

	.quiet-time {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(1 0 0 / 12%);
		white-space: nowrap;
	}

	/* ── Thought card (liquid glass) ── */

	.thought-card {
		position: relative;
		padding: 1rem 1.125rem;
		border-radius: 1rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		-webkit-backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		box-shadow:
			0 1px 3px oklch(0 0 0 / 10%),
			0 4px 16px oklch(0 0 0 / 6%),
			inset 0 1px 0 oklch(1 0 0 / 5%),
			inset 0 -1px 0 oklch(0 0 0 / 5%);
		overflow: hidden;
		cursor: default;
		opacity: 0;
		animation: card-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
		transition: border-color 0.3s ease, box-shadow 0.3s ease;
	}

	.thought-card:hover {
		border-color: oklch(1 0 0 / 14%);
		box-shadow:
			0 2px 6px oklch(0 0 0 / 14%),
			0 8px 24px oklch(0 0 0 / 8%),
			inset 0 1px 0 oklch(1 0 0 / 6%),
			inset 0 -1px 0 oklch(0 0 0 / 5%);
	}

	/* Accent border glow for special types */
	.thought-card-wake {
		border-left: 2px solid oklch(0.75 0.12 200 / 25%);
	}
	.thought-card-reach {
		border-left: 2px solid oklch(0.78 0.12 75 / 25%);
	}
	.thought-card-drop {
		border-left: 2px solid oklch(0.78 0.16 310 / 25%);
	}

	/* Specular highlight */
	.card-highlight {
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 12%), transparent);
		pointer-events: none;
	}

	@keyframes card-enter {
		from {
			opacity: 0;
			transform: translateY(8px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}

	/* ── Card header ── */

	.card-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.625rem;
	}

	.card-mood {
		display: flex;
		align-items: center;
		gap: 0.375rem;
	}

	.mood-symbol {
		font-size: 0.85rem;
		line-height: 1;
		opacity: 0.7;
	}

	.mood-label {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.05em;
		opacity: 0.65;
	}

	.card-time {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(1 0 0 / 25%);
		letter-spacing: 0.04em;
	}

	/* ── Action tags ── */

	.card-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
		margin-bottom: 0.625rem;
	}

	.action-tag {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.2rem 0.5rem;
		border-radius: 2rem;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		letter-spacing: 0.04em;
		color: oklch(1 0 0 / 40%);
		background: oklch(1 0 0 / 3%);
		border: 1px solid oklch(1 0 0 / 6%);
	}

	.action-tag-wake {
		color: oklch(0.75 0.12 200 / 70%);
		background: oklch(0.75 0.12 200 / 5%);
		border-color: oklch(0.75 0.12 200 / 12%);
	}

	.action-tag-reach {
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 5%);
		border-color: oklch(0.78 0.12 75 / 12%);
	}

	.action-tag-drop {
		color: oklch(0.78 0.16 310 / 70%);
		background: oklch(0.78 0.16 310 / 5%);
		border-color: oklch(0.78 0.16 310 / 12%);
	}

	.action-tag-mood {
		color: color-mix(in oklch, var(--accent) 65%, transparent);
		background: color-mix(in oklch, var(--accent) 5%, transparent);
		border-color: color-mix(in oklch, var(--accent) 12%, transparent);
	}

	.action-icon {
		font-size: 0.6rem;
		opacity: 0.7;
	}

	/* ── Card content ── */

	.card-content {
		position: relative;
		max-height: 5.5em;
		overflow: hidden;
		mask-image: linear-gradient(to bottom, black 60%, transparent 100%);
		-webkit-mask-image: linear-gradient(to bottom, black 60%, transparent 100%);
		transition: max-height 0.4s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.card-content-expanded {
		max-height: none;
		mask-image: none;
		-webkit-mask-image: none;
	}

	.thought-para {
		font-family: var(--font-body);
		font-size: 0.78rem;
		line-height: 1.65;
		color: oklch(1 0 0 / 50%);
		margin: 0;
	}

	.thought-para + .thought-para {
		margin-top: 0.625rem;
	}

	/* ── Expand toggle ── */

	.expand-toggle {
		display: block;
		margin-top: 0.5rem;
		padding: 0;
		background: none;
		border: none;
		font-family: var(--font-mono);
		font-size: 0.62rem;
		letter-spacing: 0.05em;
		color: oklch(1 0 0 / 25%);
		cursor: pointer;
		transition: color 0.2s ease;
	}

	.expand-toggle:hover {
		color: oklch(1 0 0 / 45%);
	}

	/* ── Mobile ── */

	@media (max-width: 640px) {
		.thoughts-container {
			padding: 1.5rem 1rem;
		}

		.thought-card {
			padding: 0.875rem 1rem;
		}
	}
</style>
