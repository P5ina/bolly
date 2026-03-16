<script lang="ts">
import "../app.css";
import { dev } from "$app/environment";
import { injectAnalytics } from "@vercel/analytics/sveltekit";

injectAnalytics({ mode: dev ? "development" : "production" });

let { children } = $props();
</script>

<svelte:head>
	<title>Bolly — A companion that lives on your server</title>
	<link rel="icon" type="image/svg+xml" href="/favicon.svg" />
</svelte:head>

{@render children()}
