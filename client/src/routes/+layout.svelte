<script lang="ts">
	import "./layout.css";
	import favicon from "$lib/assets/favicon.svg";
	import Sidebar from "$lib/components/layout/Sidebar.svelte";
	import { getInstances } from "$lib/stores/instances.svelte.js";

	let { children } = $props();

	const instances = getInstances();
	const showSidebar = $derived(instances.loading || instances.list.length > 0);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>personality</title>
</svelte:head>

<div class="flex h-dvh overflow-hidden">
	{#if showSidebar}
		<Sidebar />
	{/if}
	<main class="flex-1 overflow-hidden">
		{@render children()}
	</main>
</div>
