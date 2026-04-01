<script lang="ts">
	let {
		slug,
		mood = "calm",
		status = "waiting",
		focus = "listening for the next thread",
		messageCount = 0,
		connected = false,
		statusLine = "presence steady",
		roomTone = "the room is waiting",
	}: {
		slug: string;
		mood?: string;
		status?: string;
		focus?: string;
		messageCount?: number;
		connected?: boolean;
		statusLine?: string;
		roomTone?: string;
	} = $props();

	const statusTone = $derived.by(() => {
		if (status === "acting") return "status-acting";
		if (status === "thinking") return "status-thinking";
		return "status-waiting";
	});
</script>

<div class="presence-shell" data-status={status}>
	<div class="presence-row">
		<div class="presence-core">
			<div class="presence-led" class:presence-led-live={connected}></div>
			<div class="presence-identity">
				<span class="presence-name">{slug}</span>
				<span class="presence-sep">·</span>
				<span class="presence-mood">{mood}</span>
				<span class="presence-sep">·</span>
				<span class={statusTone}>{status}</span>
			</div>
		</div>

		<div class="presence-metrics">
			<span>{messageCount} msgs</span>
			<span class:presence-online={connected}>{connected ? "linked" : "offline"}</span>
		</div>
	</div>

	<div class="presence-grid">
		<div class="presence-focus">
			<span class="presence-focus-label">current focus</span>
			<p>{focus}</p>
		</div>

		<div class="presence-signal-card">
			<span class="presence-focus-label">signal</span>
			<strong>{statusLine}</strong>
			<p>{roomTone}</p>
		</div>
	</div>
</div>

<style>
	.presence-shell {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 0.9rem 1rem;
		border: 1px solid oklch(var(--ink) / 8%);
		border-radius: 16px;
		background: var(--surface-elevated);
		backdrop-filter: var(--glass-blur);
		-webkit-backdrop-filter: var(--glass-blur);
	}

	.presence-shell[data-status="thinking"] {
		border-color: oklch(0.82 0.08 75 / 18%);
	}

	.presence-shell[data-status="acting"] {
		border-color: oklch(0.76 0.12 170 / 18%);
	}

	.presence-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		flex-wrap: wrap;
	}

	.presence-core {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-width: 0;
	}

	.presence-led {
		width: 8px;
		height: 8px;
		border-radius: 999px;
		background: oklch(0.45 0.01 280 / 40%);
		box-shadow: 0 0 0 0 oklch(0.78 0.12 75 / 0%);
		transition: all 0.35s ease;
	}

	.presence-led-live {
		background: var(--warm);
		box-shadow: 0 0 0 6px oklch(0.78 0.12 75 / 8%), 0 0 14px oklch(0.78 0.12 75 / 24%);
	}

	.presence-identity {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		flex-wrap: wrap;
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.04em;
		text-transform: lowercase;
		color: var(--text-primary);
	}

	.presence-name {
		color: var(--text-primary);
	}

	.presence-sep {
		color: var(--text-muted);
	}

	.presence-metrics {
		display: flex;
		align-items: center;
		gap: 0.7rem;
		font-family: var(--font-mono);
		font-size: 0.67rem;
		color: var(--text-secondary);
	}

	.status-waiting,
	.status-thinking,
	.status-acting {
		transition: color 0.3s ease;
	}

	.status-waiting {
		color: var(--text-secondary);
	}

	.status-thinking {
		color: var(--text-primary);
	}

	.status-acting {
		color: oklch(0.76 0.12 170 / 88%);
	}

	.presence-online {
		color: var(--text-primary);
	}

	.presence-grid {
		display: grid;
		grid-template-columns: minmax(0, 1.3fr) minmax(220px, 0.8fr);
		gap: 0.75rem;
	}

	.presence-focus,
	.presence-signal-card {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0.75rem 0.85rem;
		border-radius: 14px;
		border: 1px solid oklch(var(--ink) / 6%);
		background: var(--surface-elevated);
	}

	.presence-focus-label {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--text-muted);
	}

	.presence-focus p,
	.presence-signal-card p {
		margin: 0;
		font-size: 0.84rem;
		line-height: 1.45;
		color: var(--text-primary);
	}

	.presence-signal-card strong {
		font-size: 0.82rem;
		font-weight: 500;
		color: var(--text-primary);
	}

	@media (max-width: 720px) {
		.presence-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
