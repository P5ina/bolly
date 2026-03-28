<script lang="ts">
	import { onMount } from 'svelte';

	let { data } = $props();
	let opened = $state(false);

	onMount(() => {
		// Open deep link via JS — Safari blocks 302 redirects to custom schemes
		window.location.href = `bolly://callback?session=${encodeURIComponent(data.sessionId)}`;
		opened = true;
	});
</script>

<div class="page">
	<div class="card">
		{#if opened}
			<h1 class="title">opening bolly...</h1>
			<p class="desc">If the app didn't open, make sure Bolly Desktop is installed.</p>
		{:else}
			<h1 class="title">redirecting...</h1>
		{/if}
	</div>
</div>

<style>
	.page {
		min-height: 100dvh;
		background: oklch(0.04 0.015 260);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.5rem;
	}

	.card {
		text-align: center;
		max-width: 20rem;
	}

	.title {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 1.25rem;
		font-weight: 400;
		color: oklch(0.90 0.02 75);
		margin: 0 0 0.75rem;
	}

	.desc {
		font-size: 0.82rem;
		color: oklch(0.60 0.03 240);
		line-height: 1.5;
		margin: 0;
	}
</style>
