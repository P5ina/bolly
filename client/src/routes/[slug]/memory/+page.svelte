<script lang="ts">
	import { page } from "$app/state";
	import MemoryMapView from "$lib/components/memory/MemoryMapView.svelte";
	import VectorMapView from "$lib/components/memory/VectorMapView.svelte";

	const slug = $derived(page.params.slug!);
	let view = $state<"map" | "vectors">("map");
</script>

<div class="memory-page">
	<div class="view-toggle">
		<button class:active={view === "map"} onclick={() => view = "map"}>map</button>
		<button class:active={view === "vectors"} onclick={() => view = "vectors"}>vectors</button>
	</div>

	{#key slug}
		{#if view === "map"}
			<MemoryMapView {slug} />
		{:else}
			<VectorMapView {slug} />
		{/if}
	{/key}
</div>

<style>
	.memory-page {
		position: relative;
		width: 100%;
		height: 100%;
	}
	.view-toggle {
		position: absolute;
		top: 0.75rem;
		left: 50%;
		transform: translateX(-50%);
		z-index: 10;
		display: flex;
		gap: 0;
		background: oklch(0.1 0.02 250 / 80%);
		backdrop-filter: blur(12px);
		border-radius: 0.5rem;
		border: 1px solid oklch(1 0 0 / 8%);
		overflow: hidden;
	}
	.view-toggle button {
		all: unset;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		padding: 0.35rem 0.75rem;
		color: oklch(0.55 0.08 240 / 40%);
		cursor: pointer;
		transition: all 0.2s;
	}
	.view-toggle button:hover {
		color: oklch(0.8 0.02 75 / 60%);
	}
	.view-toggle button.active {
		color: oklch(0.9 0.02 75 / 80%);
		background: oklch(1 0 0 / 6%);
	}
</style>
