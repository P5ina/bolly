<script lang="ts">
import "../app.css";
import favicon from '$lib/assets/favicon.svg';
import { dev } from "$app/environment";
import { injectAnalytics } from "@vercel/analytics/sveltekit";

injectAnalytics({ mode: dev ? "development" : "production" });

let { children } = $props();
</script>

<svelte:head>
	<title>Bolly — A companion that lives on your server</title>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
