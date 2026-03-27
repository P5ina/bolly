<script lang="ts">
	import { fetchAgents, triggerAgent, fetchAgentHistory } from "$lib/api/client.js";
	import type { ChildAgent, AgentHistoryEntry } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import { goto } from "$app/navigation";

	const toast = getToasts();
	let { slug }: { slug: string } = $props();

	let agents = $state<ChildAgent[]>([]);
	let loading = $state(true);
	let triggering = $state<string | null>(null);

	// History panel
	let selectedAgent = $state<string | null>(null);
	let history = $state<AgentHistoryEntry[]>([]);
	let historyLoading = $state(false);

	async function load() {
		loading = true;
		try {
			agents = await fetchAgents(slug);
		} catch {
			toast.error("failed to load agents");
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();
	});

	async function handleTrigger(name: string) {
		triggering = name;
		try {
			await triggerAgent(slug, name);
			toast.success(`${name} triggered`);
			// Refresh after a delay to pick up new last_run
			setTimeout(() => load(), 3000);
		} catch {
			toast.error(`failed to trigger ${name}`);
		} finally {
			triggering = null;
		}
	}

	async function showHistory(name: string) {
		if (selectedAgent === name) {
			selectedAgent = null;
			return;
		}
		selectedAgent = name;
		historyLoading = true;
		try {
			history = await fetchAgentHistory(slug, name);
		} catch {
			history = [];
		} finally {
			historyLoading = false;
		}
	}

	function addAgent() {
		// Navigate to chat with a pre-filled message asking the main agent to create a child agent
		const message = encodeURIComponent(
			"I want to create a new child agent. Help me define what it should do, how often it should run, and write the TOML config for it."
		);
		goto(`/${slug}/chat?draft=${message}`);
	}

	function formatInterval(hours: number): string {
		if (hours < 1) return `${Math.round(hours * 60)}m`;
		if (hours < 24) return `${hours}h`;
		if (hours === 24) return "daily";
		if (hours === 72) return "3 days";
		return `${Math.round(hours / 24)}d`;
	}

	function formatLastRun(ts: number): string {
		if (ts === 0) return "never";
		const diff = Date.now() / 1000 - ts;
		const mins = Math.floor(diff / 60);
		const hours = Math.floor(diff / 3600);
		const days = Math.floor(diff / 86400);
		if (mins < 1) return "just now";
		if (mins < 60) return `${mins}m ago`;
		if (hours < 24) return `${hours}h ago`;
		return `${days}d ago`;
	}

	function modelLabel(model: string): string {
		switch (model) {
			case "heavy": return "opus";
			case "fast": return "sonnet";
			case "cheap": return "haiku";
			default: return "default";
		}
	}

	const modelColors: Record<string, string> = {
		heavy: "oklch(0.75 0.14 310)",
		fast: "oklch(0.75 0.12 200)",
		cheap: "oklch(0.72 0.10 140)",
		default: "oklch(0.78 0.12 75)",
	};
</script>

<div class="agents-page">
	{#if loading}
		<div class="agents-center">
			<div class="pulse-dot"></div>
		</div>
	{:else}
		<div class="agents-header">
			<div class="agents-title">
				<span class="agents-count">{agents.length}</span>
				agents
			</div>
			<button class="agents-add" onclick={addAgent}>
				+ new agent
			</button>
		</div>

		<p class="agents-hint">
			built-in agents run on their own schedule — manual trigger is for custom agents that don't need a timer
		</p>

		{#if agents.length === 0}
			<div class="agents-center">
				<p class="empty-text">no agents yet</p>
				<p class="empty-sub">agents are autonomous helpers that wake up on their own schedule</p>
			</div>
		{:else}
			<div class="agents-list">
				{#each agents as agent (agent.name)}
					{@const color = modelColors[agent.model] ?? "oklch(0.78 0.12 75)"}
					{@const isRunning = triggering === agent.name}
					{@const isExpanded = selectedAgent === agent.name}

					<div class="agent-card" class:agent-disabled={!agent.enabled}>
						<!-- Status indicator -->
						<div class="agent-status" style="background: {color};" class:agent-status-due={agent.is_due}></div>

						<!-- Main info -->
						<div class="agent-main">
							<div class="agent-top">
								<span class="agent-name">{agent.name}</span>
								<span class="agent-model" style="color: {color};">{modelLabel(agent.model)}</span>
								<span class="agent-interval">every {formatInterval(agent.interval_hours)}</span>
							</div>

							<p class="agent-desc">{agent.description}</p>

							<div class="agent-meta">
								<span class="agent-last-run">
									{#if agent.last_run === 0}
										never ran
									{:else}
										last ran {formatLastRun(agent.last_run)}
									{/if}
								</span>
								{#if agent.is_due}
									<span class="agent-due-badge">due</span>
								{/if}
								{#if !agent.enabled}
									<span class="agent-paused-badge">paused</span>
								{/if}
							</div>
						</div>

						<!-- Actions -->
						<div class="agent-actions">
							<button
								class="agent-btn agent-btn-run"
								onclick={() => handleTrigger(agent.name)}
								disabled={isRunning || !agent.enabled}
								title="Run now"
							>
								{#if isRunning}
									<span class="agent-spinner"></span>
								{:else}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
								{/if}
							</button>
							<button
								class="agent-btn agent-btn-history"
								onclick={() => showHistory(agent.name)}
								class:agent-btn-active={isExpanded}
								title="View history"
							>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
							</button>
						</div>
					</div>

					<!-- History panel -->
					{#if isExpanded}
						<div class="agent-history">
							{#if historyLoading}
								<div class="history-loading"><div class="pulse-dot"></div></div>
							{:else if history.length === 0}
								<p class="history-empty">no history yet</p>
							{:else}
								{#each history as entry (entry.id)}
									<div class="history-entry">
										<span class="history-time">{formatLastRun(parseInt(entry.timestamp) / 1000)}</span>
										<p class="history-content">{entry.content.slice(0, 300)}{entry.content.length > 300 ? "..." : ""}</p>
									</div>
								{/each}
							{/if}
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	{/if}
</div>

<style>
	.agents-page {
		height: 100%;
		overflow-y: auto;
		padding: 2rem 1.5rem;
	}

	.agents-center {
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
		max-width: 30ch;
		text-align: center;
		line-height: 1.5;
	}

	/* Header */
	.agents-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 1.25rem;
		max-width: 600px;
		margin-left: auto;
		margin-right: auto;
	}

	.agents-title {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(1 0 0 / 35%);
		letter-spacing: 0.04em;
	}

	.agents-count {
		color: oklch(0.78 0.12 75 / 55%);
		margin-right: 0.25rem;
	}

	.agents-hint {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(1 0 0 / 18%);
		max-width: 600px;
		margin: -0.5rem auto 1rem;
		letter-spacing: 0.02em;
		line-height: 1.5;
	}

	.agents-add {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 40%);
		background: oklch(0.78 0.12 75 / 5%);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		padding: 0.35rem 0.75rem;
		border-radius: 0.5rem;
		cursor: pointer;
		letter-spacing: 0.04em;
		transition: all 0.2s ease;
	}
	.agents-add:hover {
		color: oklch(0.78 0.12 75 / 65%);
		background: oklch(0.78 0.12 75 / 10%);
		border-color: oklch(0.78 0.12 75 / 28%);
	}

	/* Agent list */
	.agents-list {
		max-width: 600px;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	/* Agent card */
	.agent-card {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		padding: 0.875rem 1rem;
		border-radius: 0.625rem;
		background: oklch(1 0 0 / 2%);
		border: 1px solid oklch(1 0 0 / 5%);
		transition: all 0.2s ease;
	}
	.agent-card:hover {
		background: oklch(1 0 0 / 3.5%);
		border-color: oklch(1 0 0 / 8%);
	}

	.agent-disabled {
		opacity: 0.45;
	}

	.agent-status {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		margin-top: 0.45rem;
		flex-shrink: 0;
		opacity: 0.4;
		transition: opacity 0.3s ease;
	}
	.agent-status-due {
		opacity: 1;
		box-shadow: 0 0 6px currentColor;
	}

	.agent-main {
		flex: 1;
		min-width: 0;
	}

	.agent-top {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		flex-wrap: wrap;
		margin-bottom: 0.25rem;
	}

	.agent-name {
		font-family: var(--font-mono);
		font-size: 0.82rem;
		color: oklch(1 0 0 / 70%);
		letter-spacing: 0.02em;
	}

	.agent-model {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		letter-spacing: 0.05em;
		opacity: 0.6;
		text-transform: uppercase;
	}

	.agent-interval {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(1 0 0 / 22%);
		letter-spacing: 0.04em;
	}

	.agent-desc {
		font-size: 0.72rem;
		color: oklch(1 0 0 / 32%);
		line-height: 1.5;
		margin: 0;
	}

	.agent-meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.375rem;
	}

	.agent-last-run {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(1 0 0 / 20%);
		letter-spacing: 0.04em;
	}

	.agent-due-badge {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		letter-spacing: 0.06em;
		padding: 0.1rem 0.35rem;
		border-radius: 0.5rem;
		background: oklch(0.78 0.12 75 / 8%);
		color: oklch(0.78 0.12 75 / 55%);
	}

	.agent-paused-badge {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		letter-spacing: 0.06em;
		padding: 0.1rem 0.35rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 5%);
		color: oklch(1 0 0 / 25%);
	}

	/* Actions */
	.agent-actions {
		display: flex;
		gap: 0.25rem;
		flex-shrink: 0;
		margin-top: 0.125rem;
	}

	.agent-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 0.375rem;
		background: none;
		color: oklch(1 0 0 / 28%);
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.agent-btn:hover:not(:disabled) {
		color: oklch(1 0 0 / 55%);
		border-color: oklch(1 0 0 / 15%);
		background: oklch(1 0 0 / 3%);
	}
	.agent-btn:disabled {
		opacity: 0.3;
		cursor: not-allowed;
	}
	.agent-btn-active {
		color: oklch(0.78 0.12 75 / 55%);
		border-color: oklch(0.78 0.12 75 / 20%);
		background: oklch(0.78 0.12 75 / 5%);
	}

	.agent-btn-run:hover:not(:disabled) {
		color: oklch(0.78 0.15 140 / 70%);
		border-color: oklch(0.78 0.15 140 / 20%);
	}

	.agent-spinner {
		width: 10px; height: 10px; border-radius: 50%;
		border: 1.5px solid oklch(0.78 0.12 75 / 20%);
		border-top-color: oklch(0.78 0.12 75 / 60%);
		animation: spin 0.6s linear infinite;
	}
	@keyframes spin { to { transform: rotate(360deg); } }

	/* History panel */
	.agent-history {
		margin-left: 1.5rem;
		padding: 0.75rem 1rem;
		border-left: 2px solid oklch(1 0 0 / 5%);
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
	}

	.history-loading {
		display: flex;
		justify-content: center;
		padding: 0.5rem;
	}

	.history-empty {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(1 0 0 / 20%);
		margin: 0;
	}

	.history-entry {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.history-time {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(1 0 0 / 18%);
		letter-spacing: 0.04em;
	}

	.history-content {
		font-size: 0.72rem;
		color: oklch(1 0 0 / 35%);
		line-height: 1.5;
		margin: 0;
		white-space: pre-line;
	}

	@media (max-width: 640px) {
		.agents-page { padding: 1.5rem 1rem; }
	}
</style>
