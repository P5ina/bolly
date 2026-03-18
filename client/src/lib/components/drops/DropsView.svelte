<script lang="ts">
	import { deleteDrop, fetchDrops } from "$lib/api/client.js";
	import type { Drop, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import { play } from "$lib/sounds.js";
	import { hapticDouble } from "$lib/haptics.js";
	import DropCard from "./DropCard.svelte";

	const toast = getToasts();

	let { slug }: { slug: string } = $props();

	let drops = $state<Drop[]>([]);
	let loading = $state(true);
	let expandedId = $state<string | null>(null);

	const ws = getWebSocket();

	async function load() {
		loading = true;
		try {
			drops = await fetchDrops(slug);
		} catch {
			toast.error("failed to load drops");
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();

		const unsub = ws.subscribe((event: ServerEvent) => {
			if (event.type === "drop_created" && event.instance_slug === slug) {
				drops = [event.drop, ...drops];
				play("drop_received");
				hapticDouble();
			}
		});

		return unsub;
	});

	async function handleDelete(dropId: string) {
		try {
			await deleteDrop(slug, dropId);
			drops = drops.filter((d) => d.id !== dropId);
			if (expandedId === dropId) expandedId = null;
		} catch {
			toast.error("failed to delete drop");
		}
	}

	function toggleExpand(dropId: string) {
		expandedId = expandedId === dropId ? null : dropId;
	}

	const kindIcon: Record<string, string> = {
		thought: "~",
		idea: "*",
		poem: "\"",
		observation: "o",
		reflection: ".",
		recommendation: ">",
		story: "#",
		question: "?",
		note: "-",
	};

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
</script>

<div class="drops-container">
	{#if loading}
		<div class="drops-loading">
			<div class="drops-loading-dot"></div>
		</div>
	{:else if drops.length === 0}
		<div class="drops-empty">
			<div class="drops-empty-icon">~</div>
			<p class="drops-empty-text">no drops yet</p>
			<p class="drops-empty-hint">
				your companion creates drops autonomously — ideas, poems, observations, reflections.
				they appear here as they come.
			</p>
		</div>
	{:else}
		<div class="drops-header">
			<span class="drops-count">{drops.length} drop{drops.length !== 1 ? "s" : ""}</span>
		</div>
		<div class="drops-grid">
			{#each drops as drop (drop.id)}
				<DropCard
					{drop}
					icon={kindIcon[drop.kind] ?? "~"}
					time={formatTime(drop.created_at)}
					expanded={expandedId === drop.id}
					onexpand={() => toggleExpand(drop.id)}
					ondelete={() => handleDelete(drop.id)}
				/>
			{/each}
		</div>
	{/if}
</div>

<style>
	.drops-container {
		height: 100%;
		overflow-y: auto;
		padding: 2rem 1.5rem;
	}

	.drops-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.drops-loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	.drops-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 0.75rem;
		text-align: center;
	}

	.drops-empty-icon {
		font-family: var(--font-mono);
		font-size: 1.5rem;
		color: oklch(0.78 0.12 75 / 28%);
		animation: pulse-alive 3s ease-in-out infinite;
	}

	.drops-empty-text {
		font-family: var(--font-display);
		font-size: 0.95rem;
		color: oklch(0.78 0.12 75 / 50%);
	}

	.drops-empty-hint {
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 35%);
		max-width: 28ch;
		line-height: 1.5;
	}

	.drops-header {
		margin-bottom: 1.25rem;
	}

	.drops-count {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 30%);
		letter-spacing: 0.05em;
	}

	.drops-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: 0.75rem;
	}

	@media (max-width: 640px) {
		.drops-grid {
			grid-template-columns: 1fr;
		}
		.drops-container {
			padding: 1.5rem 1rem;
		}
	}
</style>
