<script lang="ts">
	import { applyHeartbeatUpdate, dismissHeartbeatUpdate, fetchHeartbeatUpdates } from "$lib/api/client.js";
	import type { HeartbeatUpdate } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import CreatureBubble from "./CreatureBubble.svelte";

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
	<CreatureBubble ondismiss={() => dismiss(update.id)}>
		<div class="hb-inner">
			<span class="hb-label">heartbeat update</span>
			<span class="hb-desc">{update.description}</span>
			<div class="hb-actions">
				<button class="hb-toggle" onclick={() => (expanded = expanded === update.id ? null : update.id)}>
					{expanded === update.id ? "hide" : "preview"}
				</button>
				<button class="hb-apply" disabled={applying} onclick={() => apply(update.id)}>apply</button>
			</div>
			{#if expanded === update.id}
				<pre class="hb-preview">{update.preview}</pre>
			{/if}
		</div>
	</CreatureBubble>
{/each}

<style>
	.hb-inner {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.hb-label {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: oklch(0.78 0.12 75 / 45%);
	}

	.hb-desc {
		font-size: 0.72rem;
		color: oklch(0.85 0.02 280 / 75%);
		line-height: 1.35;
	}

	.hb-actions {
		display: flex;
		gap: 0.375rem;
		margin-top: 0.25rem;
	}

	.hb-toggle, .hb-apply {
		padding: 0.15rem 0.5rem;
		border-radius: 0.25rem;
		font-family: var(--font-mono);
		font-size: 0.55rem;
		letter-spacing: 0.04em;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.hb-toggle {
		background: none;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		color: oklch(0.78 0.12 75 / 50%);
	}
	.hb-toggle:hover {
		border-color: oklch(0.78 0.12 75 / 30%);
		color: oklch(0.78 0.12 75 / 75%);
	}

	.hb-apply {
		background: oklch(0.78 0.12 75 / 10%);
		border: 1px solid oklch(0.78 0.12 75 / 22%);
		color: oklch(0.78 0.12 75 / 80%);
	}
	.hb-apply:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 18%);
	}
	.hb-apply:disabled {
		opacity: 0.4;
		cursor: default;
	}

	.hb-preview {
		margin: 0.25rem 0 0;
		padding: 0.5rem;
		border-radius: 0.375rem;
		background: oklch(0.05 0.01 280 / 50%);
		font-family: var(--font-mono);
		font-size: 0.62rem;
		line-height: 1.5;
		color: oklch(0.75 0.02 280 / 65%);
		white-space: pre-wrap;
		word-break: break-word;
	}
</style>
