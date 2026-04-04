<script lang="ts">
	import { fetchAgents, triggerAgent, updateAgent, resetAgent, fetchAgentHistory, fetchAgentRuns, fetchAgentRun } from "$lib/api/client.js";
	import type { ChildAgent, AgentHistoryEntry, AgentRunSummary, AgentRun } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import { goto } from "$app/navigation";

	const toast = getToasts();
	let { slug }: { slug: string } = $props();

	let agents = $state<ChildAgent[]>([]);
	let loading = $state(true);
	let triggering = $state<string | null>(null);

	// Config editing
	let editingAgent = $state<string | null>(null);
	let saving = $state(false);

	// History panel
	let selectedAgent = $state<string | null>(null);
	let history = $state<AgentHistoryEntry[]>([]);
	let historyLoading = $state(false);

	// Activity panel
	let runs = $state<AgentRunSummary[]>([]);
	let runsLoading = $state(true);
	let selectedRun = $state<AgentRun | null>(null);
	let runLoading = $state(false);
	let expandedRunId = $state<string | null>(null);

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

	async function loadRuns() {
		runsLoading = true;
		try {
			runs = await fetchAgentRuns(slug, 30);
		} catch {
			runs = [];
		} finally {
			runsLoading = false;
		}
	}

	$effect(() => {
		load();
		loadRuns();
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

	const ALL_TOOL_GROUPS = ["memory", "creative", "communication", "files", "commands", "email", "computer", "media"];
	const BUILTIN_NAMES = ["companion", "reflection", "night-maintenance", "observer", "explore-code", "deep-research"];
	const MODEL_OPTIONS = [
		{ value: "default", label: "default" },
		{ value: "heavy", label: "opus" },
		{ value: "fast", label: "sonnet" },
		{ value: "cheap", label: "haiku" },
	];

	function isBuiltin(name: string): boolean {
		return BUILTIN_NAMES.includes(name);
	}

	async function toggleEnabled(agent: ChildAgent) {
		try {
			await updateAgent(slug, agent.name, { enabled: !agent.enabled });
			agent.enabled = !agent.enabled;
			agents = [...agents]; // trigger reactivity
			toast.success(`${agent.name} ${agent.enabled ? "enabled" : "disabled"}`);
		} catch {
			toast.error("failed to update agent");
		}
	}

	async function saveAgentField(agentName: string, field: string, value: unknown) {
		saving = true;
		try {
			const updated = await updateAgent(slug, agentName, { [field]: value });
			agents = agents.map(a => a.name === agentName ? { ...a, ...updated } : a);
			toast.success("saved");
		} catch {
			toast.error("failed to save");
		} finally {
			saving = false;
		}
	}

	async function handleReset(agentName: string) {
		try {
			const updated = await resetAgent(slug, agentName);
			agents = agents.map(a => a.name === agentName ? { ...a, ...updated } : a);
			toast.success(`${agentName} reset to defaults`);
		} catch (e) {
			toast.error("failed to reset");
		}
	}

	function toggleToolGroup(agent: ChildAgent, group: string) {
		const groups = agent.tool_groups ?? [];
		const next = groups.includes(group)
			? groups.filter(g => g !== group)
			: [...groups, group];
		saveAgentField(agent.name, "tool_groups", next);
	}

	function addAgent() {
		// Navigate to chat with a pre-filled message asking the main agent to create a child agent
		const message = encodeURIComponent(
			"I want to create a new child agent. Help me define what it should do, how often it should run, and write the TOML config for it."
		);
		goto(`/${slug}/chat?draft=${message}`);
	}

	function formatInterval(hours: number): string {
		if (hours === 0) return "on-demand";
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

	const kindColors: Record<string, string> = {
		scheduled: "oklch(0.78 0.12 75)",
		on_demand: "oklch(0.75 0.12 200)",
	};

	function formatDuration(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		const secs = ms / 1000;
		if (secs < 60) return `${secs.toFixed(1)}s`;
		const mins = Math.floor(secs / 60);
		const remainSecs = Math.round(secs % 60);
		return `${mins}m${remainSecs}s`;
	}

	function formatTokens(n: number): string {
		if (n < 1000) return String(n);
		if (n < 10000) return `${(n / 1000).toFixed(1)}k`;
		return `${Math.round(n / 1000)}k`;
	}

	function formatTimestamp(epochMs: number): string {
		if (epochMs === 0) return "never";
		const diff = (Date.now() - epochMs) / 1000;
		const mins = Math.floor(diff / 60);
		const hours = Math.floor(diff / 3600);
		const days = Math.floor(diff / 86400);
		if (mins < 1) return "just now";
		if (mins < 60) return `${mins}m ago`;
		if (hours < 24) return `${hours}h ago`;
		return `${days}d ago`;
	}

	function runStatusLabel(status: AgentRunSummary['status']): string {
		if (status === 'completed') return 'completed';
		return 'failed';
	}

	function runStatusFailed(status: AgentRunSummary['status']): boolean {
		return status !== 'completed';
	}

	function runErrorMessage(status: AgentRunSummary['status']): string {
		if (status === 'completed') return '';
		if (typeof status === 'object' && 'failed' in status) return status.failed.error;
		return 'unknown error';
	}

	async function toggleRunTrace(run: AgentRunSummary) {
		if (expandedRunId === run.id) {
			expandedRunId = null;
			selectedRun = null;
			return;
		}
		expandedRunId = run.id;
		selectedRun = null;
		runLoading = true;
		try {
			selectedRun = await fetchAgentRun(slug, run.id);
		} catch {
			toast.error("failed to load run trace");
			expandedRunId = null;
		} finally {
			runLoading = false;
		}
	}

	function traceRole(msg: any): string {
		return msg?.role ?? 'unknown';
	}

	function traceContent(msg: any): string {
		if (typeof msg?.content === 'string') return msg.content;
		if (Array.isArray(msg?.content)) {
			return msg.content
				.filter((b: any) => b.type === 'text')
				.map((b: any) => b.text)
				.join('\n');
		}
		return '';
	}

	function traceToolUses(msg: any): { name: string; input: string }[] {
		if (!Array.isArray(msg?.content)) return [];
		return msg.content
			.filter((b: any) => b.type === 'tool_use')
			.map((b: any) => ({
				name: b.name ?? 'tool',
				input: typeof b.input === 'string' ? b.input : JSON.stringify(b.input ?? {}).slice(0, 300),
			}));
	}

	function traceToolResults(msg: any): { content: string }[] {
		if (!Array.isArray(msg?.content)) return [];
		return msg.content
			.filter((b: any) => b.type === 'tool_result')
			.map((b: any) => {
				let text = '';
				if (typeof b.content === 'string') text = b.content;
				else if (Array.isArray(b.content)) {
					text = b.content
						.filter((c: any) => c.type === 'text')
						.map((c: any) => c.text)
						.join('\n');
				}
				return { content: text.slice(0, 500) + (text.length > 500 ? '...' : '') };
			});
	}
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
								<span class="agent-interval">{agent.interval_hours === 0 ? 'on-demand' : `every ${formatInterval(agent.interval_hours)}`}</span>
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
								class="agent-btn"
								onclick={() => toggleEnabled(agent)}
								title={agent.enabled ? "Disable" : "Enable"}
							>
								{#if agent.enabled}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
								{:else}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94"/><line x1="1" y1="1" x2="23" y2="23"/></svg>
								{/if}
							</button>
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
								class="agent-btn"
								onclick={() => editingAgent = editingAgent === agent.name ? null : agent.name}
								class:agent-btn-active={editingAgent === agent.name}
								title="Configure"
							>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
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

					<!-- Config panel -->
					{#if editingAgent === agent.name}
						<div class="agent-config">
							<div class="config-row">
								<label class="config-label">interval</label>
								<div class="config-field">
									<input class="config-input config-input-sm" type="number" min="0" step="0.25"
										value={agent.interval_hours}
										onchange={(e) => saveAgentField(agent.name, "interval_hours", parseFloat((e.target as HTMLInputElement).value))}
									/>
									<span class="config-unit">hours</span>
									<span class="config-hint">{agent.interval_hours === 0 ? "on-demand" : agent.interval_hours < 1 ? `= ${Math.round(agent.interval_hours * 60)}m` : `= ${agent.interval_hours}h`}</span>
								</div>
							</div>
							<div class="config-row">
								<label class="config-label">model</label>
								<div class="config-field">
									{#each MODEL_OPTIONS as opt (opt.value)}
										<button class="config-chip" class:config-chip-active={agent.model === opt.value}
											onclick={() => saveAgentField(agent.name, "model", opt.value)}
										>{opt.label}</button>
									{/each}
								</div>
							</div>
							<div class="config-row">
								<label class="config-label">tools</label>
								<div class="config-field config-field-wrap">
									{#each ALL_TOOL_GROUPS as group (group)}
										<button class="config-chip" class:config-chip-active={(agent.tool_groups ?? []).includes(group)}
											onclick={() => toggleToolGroup(agent, group)}
										>{group}</button>
									{/each}
								</div>
							</div>
							<div class="config-row config-row-full">
								<label class="config-label">prompt</label>
								<textarea class="config-textarea" value={agent.prompt} rows="4"
									onchange={(e) => saveAgentField(agent.name, "prompt", (e.target as HTMLTextAreaElement).value)}
								></textarea>
							</div>
							{#if isBuiltin(agent.name)}
								<div class="config-row config-row-actions">
									<button class="config-reset" onclick={() => handleReset(agent.name)}>reset to defaults</button>
								</div>
							{/if}
						</div>
					{/if}

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

		<!-- Recent activity section -->
		<div class="activity-section">
			<div class="activity-header">
				<div class="agents-title">
					<span class="agents-count">{runs.length}</span>
					recent activity
				</div>
			</div>

			{#if runsLoading}
				<div class="activity-loading"><div class="pulse-dot"></div></div>
			{:else if runs.length === 0}
				<div class="activity-empty">
					<p class="empty-text">no recent runs</p>
				</div>
			{:else}
				<div class="runs-list">
					{#each runs as run (run.id)}
						{@const color = kindColors[run.agent_kind] ?? kindColors.scheduled}
						{@const isFailed = runStatusFailed(run.status)}
						{@const isExpanded = expandedRunId === run.id}

						<button
							class="run-card"
							class:run-card-expanded={isExpanded}
							class:run-card-failed={isFailed}
							onclick={() => toggleRunTrace(run)}
						>
							<div class="run-dot" style="background: {isFailed ? 'oklch(0.65 0.15 25)' : color};"></div>

							<div class="run-main">
								<div class="run-top">
									<span class="run-agent" style="color: {color};">{run.agent_name}</span>
									<span class="run-trigger">{run.trigger}</span>
								</div>
								<div class="run-meta">
									<span class="run-time">{formatTimestamp(run.started_at)}</span>
									<span class="run-sep">&middot;</span>
									<span class="run-duration">{formatDuration(run.duration_ms)}</span>
									<span class="run-sep">&middot;</span>
									<span class="run-tokens">{formatTokens(run.tokens_used)} tok</span>
									<span class="run-sep">&middot;</span>
									<span class="run-status" class:run-status-fail={isFailed}>
										{runStatusLabel(run.status)}
									</span>
								</div>
								{#if run.summary}
									<p class="run-summary">{run.summary}</p>
								{/if}
							</div>

							<div class="run-chevron" class:run-chevron-open={isExpanded}>
								<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
							</div>
						</button>

						<!-- Expanded trace -->
						{#if isExpanded}
							<div class="run-trace">
								{#if runLoading}
									<div class="activity-loading"><div class="pulse-dot"></div></div>
								{:else if selectedRun && selectedRun.trace.length > 0}
									<div class="trace-scroll">
										{#each selectedRun.trace as msg, i (i)}
											{@const role = traceRole(msg)}
											{@const text = traceContent(msg)}
											{@const toolUses = traceToolUses(msg)}
											{@const toolResults = traceToolResults(msg)}

											<div class="trace-msg" class:trace-msg-user={role === 'user'} class:trace-msg-assistant={role === 'assistant'}>
												<span class="trace-role">{role}</span>

												{#if text}
													<p class="trace-text">{text.slice(0, 800)}{text.length > 800 ? '...' : ''}</p>
												{/if}

												{#each toolUses as tu (tu.name + tu.input.slice(0, 20))}
													<div class="trace-tool-use">
														<span class="trace-tool-name">{tu.name}</span>
														<pre class="trace-tool-input">{tu.input}</pre>
													</div>
												{/each}

												{#each toolResults as tr (tr.content.slice(0, 30))}
													<div class="trace-tool-result">
														<pre class="trace-tool-output">{tr.content}</pre>
													</div>
												{/each}
											</div>
										{/each}
									</div>
								{:else if selectedRun}
									<p class="history-empty">empty trace</p>
								{/if}

								{#if selectedRun && runStatusFailed(selectedRun.status)}
									<div class="trace-error">
										{runErrorMessage(selectedRun.status)}
									</div>
								{/if}
							</div>
						{/if}
					{/each}
				</div>
			{/if}
		</div>
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
		color: oklch(var(--ink) / 30%);
	}
	.empty-sub {
		font-size: 0.72rem;
		color: oklch(var(--ink) / 18%);
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
		color: oklch(var(--ink) / 35%);
		letter-spacing: 0.04em;
	}

	.agents-count {
		color: oklch(0.78 0.12 75 / 55%);
		margin-right: 0.25rem;
	}

	.agents-hint {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(var(--ink) / 18%);
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
		background: oklch(var(--ink) / 2%);
		border: 1px solid oklch(var(--ink) / 5%);
		transition: all 0.2s ease;
	}
	.agent-card:hover {
		background: oklch(var(--ink) / 3.5%);
		border-color: oklch(var(--ink) / 8%);
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
		color: oklch(var(--ink) / 70%);
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
		color: oklch(var(--ink) / 22%);
		letter-spacing: 0.04em;
	}

	.agent-desc {
		font-size: 0.72rem;
		color: oklch(var(--ink) / 32%);
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
		color: oklch(var(--ink) / 20%);
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
		background: oklch(var(--ink) / 5%);
		color: oklch(var(--ink) / 25%);
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
		border: 1px solid oklch(var(--ink) / 8%);
		border-radius: 0.375rem;
		background: none;
		color: oklch(var(--ink) / 28%);
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.agent-btn:hover:not(:disabled) {
		color: oklch(var(--ink) / 55%);
		border-color: oklch(var(--ink) / 15%);
		background: oklch(var(--ink) / 3%);
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
		border-left: 2px solid oklch(var(--ink) / 5%);
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
		color: oklch(var(--ink) / 20%);
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
		color: oklch(var(--ink) / 18%);
		letter-spacing: 0.04em;
	}

	.history-content {
		font-size: 0.72rem;
		color: oklch(var(--ink) / 35%);
		line-height: 1.5;
		margin: 0;
		white-space: pre-line;
	}

	/* Activity section */
	.activity-section {
		max-width: 600px;
		margin: 2rem auto 0;
	}

	.activity-header {
		margin-bottom: 1rem;
	}

	.activity-loading {
		display: flex;
		justify-content: center;
		padding: 1rem 0;
	}

	.activity-empty {
		text-align: center;
		padding: 1rem 0;
	}

	/* Run cards */
	.runs-list {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.run-card {
		display: flex;
		align-items: flex-start;
		gap: 0.625rem;
		padding: 0.625rem 0.875rem;
		border-radius: 0.5rem;
		background: oklch(var(--ink) / 1.5%);
		border: 1px solid oklch(var(--ink) / 4%);
		transition: all 0.2s ease;
		cursor: pointer;
		text-align: left;
		width: 100%;
		font: inherit;
		color: inherit;
	}
	.run-card:hover {
		background: oklch(var(--ink) / 3%);
		border-color: oklch(var(--ink) / 7%);
	}
	.run-card-expanded {
		background: oklch(var(--ink) / 3%);
		border-color: oklch(var(--ink) / 8%);
		border-bottom-left-radius: 0;
		border-bottom-right-radius: 0;
	}
	.run-card-failed {
		border-color: oklch(0.65 0.15 25 / 10%);
	}

	.run-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		margin-top: 0.4rem;
		flex-shrink: 0;
		opacity: 0.5;
	}

	.run-main {
		flex: 1;
		min-width: 0;
	}

	.run-top {
		display: flex;
		align-items: baseline;
		gap: 0.4rem;
		margin-bottom: 0.15rem;
	}

	.run-agent {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.02em;
	}

	.run-trigger {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(var(--ink) / 20%);
		letter-spacing: 0.04em;
	}

	.run-meta {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		flex-wrap: wrap;
	}

	.run-time,
	.run-duration,
	.run-tokens {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(var(--ink) / 22%);
		letter-spacing: 0.04em;
	}

	.run-sep {
		font-size: 0.5rem;
		color: oklch(var(--ink) / 12%);
	}

	.run-status {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.72 0.10 140 / 50%);
		letter-spacing: 0.04em;
	}
	.run-status-fail {
		color: oklch(0.65 0.15 25 / 60%);
	}

	.run-summary {
		font-size: 0.65rem;
		color: oklch(var(--ink) / 25%);
		line-height: 1.4;
		margin: 0.2rem 0 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.run-chevron {
		flex-shrink: 0;
		margin-top: 0.25rem;
		color: oklch(var(--ink) / 18%);
		transition: transform 0.2s ease;
	}
	.run-chevron-open {
		transform: rotate(180deg);
	}

	/* Trace view */
	.run-trace {
		border: 1px solid oklch(var(--ink) / 6%);
		border-top: none;
		border-bottom-left-radius: 0.5rem;
		border-bottom-right-radius: 0.5rem;
		background: oklch(var(--ink) / 1%);
		padding: 0.5rem;
		margin-bottom: 0.375rem;
	}

	.trace-scroll {
		max-height: 400px;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
		scrollbar-width: thin;
		scrollbar-color: oklch(var(--ink) / 8%) transparent;
	}

	.trace-msg {
		padding: 0.5rem 0.625rem;
		border-radius: 0.375rem;
		border: 1px solid oklch(var(--ink) / 3%);
	}
	.trace-msg-user {
		background: oklch(var(--ink) / 2.5%);
	}
	.trace-msg-assistant {
		background: oklch(var(--ink) / 1%);
	}

	.trace-role {
		font-family: var(--font-mono);
		font-size: 0.52rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: oklch(var(--ink) / 22%);
		display: block;
		margin-bottom: 0.25rem;
	}

	.trace-text {
		font-size: 0.68rem;
		color: oklch(var(--ink) / 55%);
		line-height: 1.5;
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.trace-msg-assistant .trace-text {
		color: oklch(var(--ink) / 75%);
	}

	.trace-tool-use {
		margin-top: 0.25rem;
		padding: 0.35rem 0.5rem;
		border-radius: 0.25rem;
		background: oklch(0.78 0.12 75 / 3%);
		border: 1px solid oklch(0.78 0.12 75 / 6%);
	}

	.trace-tool-name {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 55%);
		letter-spacing: 0.04em;
	}

	.trace-tool-input {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(var(--ink) / 28%);
		margin: 0.15rem 0 0;
		white-space: pre-wrap;
		word-break: break-all;
		line-height: 1.4;
	}

	.trace-tool-result {
		margin-top: 0.25rem;
		padding: 0.35rem 0.5rem;
		border-radius: 0.25rem;
		background: oklch(var(--ink) / 2%);
		border: 1px solid oklch(var(--ink) / 4%);
	}

	.trace-tool-output {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(var(--ink) / 30%);
		margin: 0;
		white-space: pre-wrap;
		word-break: break-all;
		line-height: 1.4;
		max-height: 200px;
		overflow-y: auto;
	}

	.trace-error {
		margin-top: 0.375rem;
		padding: 0.4rem 0.625rem;
		border-radius: 0.375rem;
		background: oklch(0.65 0.15 25 / 5%);
		border: 1px solid oklch(0.65 0.15 25 / 10%);
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.15 25 / 60%);
		line-height: 1.4;
	}

	/* Config panel */
	.agent-config {
		padding: 0.75rem 1rem;
		margin: -0.25rem 0 0;
		border-top: 1px solid oklch(var(--ink) / 5%);
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
	}

	.config-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.config-row-full {
		flex-direction: column;
		align-items: stretch;
	}

	.config-row-actions {
		justify-content: flex-end;
		padding-top: 0.25rem;
	}

	.config-label {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(var(--ink) / 25%);
		letter-spacing: 0.05em;
		min-width: 50px;
		flex-shrink: 0;
	}

	.config-field {
		display: flex;
		align-items: center;
		gap: 0.375rem;
	}

	.config-field-wrap {
		flex-wrap: wrap;
	}

	.config-input {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		padding: 0.3rem 0.5rem;
		border-radius: 0.375rem;
		border: 1px solid oklch(var(--ink) / 8%);
		background: oklch(var(--ink) / 3%);
		color: var(--foreground);
		outline: none;
	}
	.config-input:focus { border-color: oklch(0.78 0.12 75 / 30%); }
	.config-input-sm { width: 70px; }

	.config-unit {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(var(--ink) / 20%);
	}

	.config-hint {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.78 0.12 75 / 35%);
	}

	.config-chip {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		padding: 0.2rem 0.5rem;
		border-radius: 0.375rem;
		border: 1px solid oklch(var(--ink) / 8%);
		background: oklch(var(--ink) / 2%);
		color: oklch(var(--ink) / 30%);
		cursor: pointer;
		transition: all 0.15s ease;
		letter-spacing: 0.03em;
	}
	.config-chip:hover { border-color: oklch(var(--ink) / 15%); color: oklch(var(--ink) / 50%); }
	.config-chip-active {
		background: oklch(0.78 0.12 75 / 8%);
		border-color: oklch(0.78 0.12 75 / 20%);
		color: oklch(0.78 0.12 75 / 65%);
	}

	.config-textarea {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		line-height: 1.5;
		padding: 0.5rem;
		border-radius: 0.375rem;
		border: 1px solid oklch(var(--ink) / 8%);
		background: oklch(var(--ink) / 3%);
		color: var(--foreground);
		outline: none;
		resize: vertical;
		min-height: 80px;
	}
	.config-textarea:focus { border-color: oklch(0.78 0.12 75 / 30%); }

	.config-reset {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.60 0.12 25 / 50%);
		background: none;
		border: 1px solid oklch(0.60 0.12 25 / 12%);
		padding: 0.25rem 0.6rem;
		border-radius: 0.375rem;
		cursor: pointer;
		letter-spacing: 0.04em;
		transition: all 0.2s ease;
	}
	.config-reset:hover {
		color: oklch(0.60 0.12 25 / 75%);
		border-color: oklch(0.60 0.12 25 / 25%);
		background: oklch(0.60 0.12 25 / 5%);
	}

	@media (max-width: 640px) {
		.agents-page { padding: 1.5rem 1rem; }
		.config-row { flex-direction: column; align-items: stretch; }
		.config-label { min-width: unset; }
	}
</style>
