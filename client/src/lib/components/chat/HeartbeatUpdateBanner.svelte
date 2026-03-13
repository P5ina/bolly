<script lang="ts">
	import { applyHeartbeatUpdate, dismissHeartbeatUpdate, fetchHeartbeatUpdates } from "$lib/api/client.js";
	import type { HeartbeatUpdate } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();

	let { slug }: { slug: string } = $props();

	let updates = $state<HeartbeatUpdate[]>([]);
	let expanded = $state<string | null>(null);
	let applying = $state(false);

	$effect(() => {
		fetchHeartbeatUpdates(slug)
			.then((u) => (updates = u))
			.catch(() => {});
	});

	async function apply(id: string) {
		applying = true;
		try {
			await applyHeartbeatUpdate(slug, id);
			updates = updates.filter((u) => u.id !== id);
			toast.success("heartbeat updated");
		} catch {
			toast.error("failed to apply update");
		}
		applying = false;
	}

	async function dismiss(id: string) {
		updates = updates.filter((u) => u.id !== id);
		try {
			await dismissHeartbeatUpdate(slug, id);
		} catch {}
	}
</script>

{#each updates as update (update.id)}
	<div class="hb-update">
		<div class="hb-update-header">
			<div class="hb-update-icon">~</div>
			<div class="hb-update-body">
				<span class="hb-update-label">heartbeat update</span>
				<span class="hb-update-desc">{update.description}</span>
			</div>
			<button class="hb-update-toggle" onclick={() => (expanded = expanded === update.id ? null : update.id)}>
				{expanded === update.id ? "hide" : "preview"}
			</button>
		</div>

		{#if expanded === update.id}
			<pre class="hb-update-preview">{update.preview}</pre>
		{/if}

		<div class="hb-update-actions">
			<button class="hb-update-apply" disabled={applying} onclick={() => apply(update.id)}>
				apply
			</button>
			<button class="hb-update-dismiss" onclick={() => dismiss(update.id)}>
				skip
			</button>
		</div>
	</div>
{/each}

<style>
	.hb-update {
		margin: 0.75rem 1rem;
		border-radius: 0.75rem;
		background: oklch(0.08 0.015 280 / 80%);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		overflow: hidden;
		animation: hb-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes hb-enter {
		from { opacity: 0; transform: translateY(-6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.hb-update-header {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		padding: 0.625rem 0.75rem;
	}

	.hb-update-icon {
		flex-shrink: 0;
		width: 1.5rem;
		height: 1.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 10%);
		color: oklch(0.78 0.12 75 / 70%);
		font-family: var(--font-mono);
		font-size: 0.75rem;
	}

	.hb-update-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.hb-update-label {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.78 0.12 75 / 50%);
	}

	.hb-update-desc {
		font-family: var(--font-body);
		font-size: 0.78rem;
		color: oklch(0.85 0.02 280 / 80%);
		line-height: 1.35;
	}

	.hb-update-toggle {
		flex-shrink: 0;
		padding: 0.15rem 0.5rem;
		border-radius: 1rem;
		background: transparent;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		color: oklch(0.78 0.12 75 / 60%);
		font-family: var(--font-mono);
		font-size: 0.58rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.hb-update-toggle:hover {
		border-color: oklch(0.78 0.12 75 / 30%);
		color: oklch(0.78 0.12 75 / 80%);
	}

	.hb-update-preview {
		margin: 0;
		padding: 0.625rem 0.75rem;
		background: oklch(0.05 0.01 280 / 60%);
		border-top: 1px solid oklch(0.78 0.12 75 / 6%);
		border-bottom: 1px solid oklch(0.78 0.12 75 / 6%);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		line-height: 1.5;
		color: oklch(0.75 0.02 280 / 70%);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.hb-update-actions {
		display: flex;
		gap: 0.375rem;
		padding: 0.5rem 0.75rem;
	}

	.hb-update-apply {
		padding: 0.25rem 0.75rem;
		border-radius: 1rem;
		background: oklch(0.78 0.12 75 / 12%);
		border: 1px solid oklch(0.78 0.12 75 / 25%);
		color: oklch(0.78 0.12 75 / 90%);
		font-family: var(--font-mono);
		font-size: 0.62rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.hb-update-apply:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 20%);
		border-color: oklch(0.78 0.12 75 / 40%);
	}
	.hb-update-apply:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.hb-update-dismiss {
		padding: 0.25rem 0.75rem;
		border-radius: 1rem;
		background: transparent;
		border: 1px solid oklch(0.5 0.02 280 / 15%);
		color: oklch(0.55 0.02 280 / 50%);
		font-family: var(--font-mono);
		font-size: 0.62rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.hb-update-dismiss:hover {
		border-color: oklch(0.5 0.02 280 / 30%);
		color: oklch(0.55 0.02 280 / 70%);
	}
</style>
