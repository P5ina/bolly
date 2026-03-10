<script lang="ts">
	import "./layout.css";
	import favicon from "$lib/assets/favicon.svg";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { AuthError } from "$lib/api/client.js";
	import type { ServerEvent } from "$lib/api/types.js";
	import { page } from "$app/state";
	import AuthGate from "$lib/components/auth/AuthGate.svelte";

	let { children } = $props();

	const instances = getInstances();
	const ws = getWebSocket();

	let needsAuth = $state(false);

	function init() {
		needsAuth = false;
		instances.refresh().catch((e: unknown) => {
			if (e instanceof AuthError) needsAuth = true;
		});
		ws.connect();
	}

	$effect(() => {
		init();

		const unsub = ws.subscribe((event: ServerEvent) => {
			if (event.type === "instance_discovered") {
				instances.upsert(event.instance);
			}
		});

		return () => {
			unsub();
			ws.disconnect();
		};
	});

	function handleAuth() {
		// Re-init with new token
		ws.disconnect();
		init();
	}

	const isHome = $derived(page.url.pathname === "/");
	const currentSlug = $derived(page.params.slug);
	const showNav = $derived(!needsAuth && !instances.loading && instances.list.length > 0 && !isHome);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>personality</title>
</svelte:head>

<div class="relative h-dvh overflow-hidden">
	{#if needsAuth}
		<AuthGate onauth={handleAuth} />
	{:else}
		{@render children()}
	{/if}

	<!-- floating instance nav — organic dots -->
	{#if showNav}
		<nav class="companion-nav">
			<a
				href="/"
				class="companion-nav-home"
				title="all companions"
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3.5 h-3.5">
					<path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
			<div class="companion-nav-divider"></div>
			{#each instances.list as instance (instance.slug)}
				<a
					href="/{instance.slug}"
					class="companion-nav-dot"
					class:companion-nav-dot-active={currentSlug === instance.slug}
					title={instance.slug}
				>
					<span class="companion-nav-dot-letter">{instance.slug[0]?.toUpperCase()}</span>
					{#if currentSlug === instance.slug}
						<div class="companion-nav-dot-glow"></div>
					{/if}
				</a>
			{/each}
			<div
			class="companion-nav-pulse"
			class:companion-nav-pulse-on={ws.connected}
			class:companion-nav-pulse-reconnecting={ws.reconnecting}
			title={ws.connected ? "connected" : ws.reconnecting ? `reconnecting (attempt ${ws.retryCount})…` : "disconnected"}
		></div>
		</nav>
	{/if}

	<!-- connection lost banner -->
	{#if ws.reconnecting}
		<div class="connection-banner">
			<span class="connection-banner-dot"></span>
			reconnecting{ws.retryCount > 2 ? ` (attempt ${ws.retryCount})` : "…"}
		</div>
	{/if}
</div>

<style>
	.companion-nav {
		position: fixed;
		bottom: 1.5rem;
		right: 1.5rem;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.625rem;
		border-radius: 2rem;
		background: oklch(0.065 0.015 280 / 80%);
		backdrop-filter: blur(20px);
		border: 1px solid oklch(1 0 0 / 4%);
		z-index: 50;
		animation: nav-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 0.8s;
	}

	@keyframes nav-enter {
		from { opacity: 0; transform: translateY(12px) scale(0.95); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}

	.companion-nav-home {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		border-radius: 50%;
		color: oklch(0.78 0.12 75 / 40%);
		transition: all 0.3s ease;
	}
	.companion-nav-home:hover {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	.companion-nav-divider {
		width: 1px;
		height: 1rem;
		background: oklch(1 0 0 / 6%);
	}

	.companion-nav-dot {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		border-radius: 50%;
		transition: all 0.3s ease;
		text-decoration: none;
	}
	.companion-nav-dot:hover {
		background: oklch(0.78 0.12 75 / 10%);
	}

	.companion-nav-dot-letter {
		font-family: var(--font-display);
		font-size: 0.65rem;
		font-weight: 600;
		color: oklch(0.78 0.12 75 / 40%);
		transition: color 0.3s ease;
	}
	.companion-nav-dot-active .companion-nav-dot-letter {
		color: oklch(0.78 0.12 75 / 90%);
	}
	.companion-nav-dot:hover .companion-nav-dot-letter {
		color: oklch(0.78 0.12 75 / 70%);
	}

	.companion-nav-dot-glow {
		position: absolute;
		inset: -2px;
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 12px oklch(0.78 0.12 75 / 10%);
		animation: breathe-slow 4s ease-in-out infinite;
	}

	@keyframes breathe-slow {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}

	.companion-nav-pulse {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: oklch(0.50 0.03 75 / 20%);
		margin-left: 0.25rem;
		transition: background 0.5s ease;
	}
	.companion-nav-pulse-on {
		background: oklch(0.78 0.12 75 / 50%);
		animation: pulse-alive 2.5s ease-in-out infinite;
	}

	@keyframes pulse-alive {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.3; }
	}

	.companion-nav-pulse-reconnecting {
		background: oklch(0.75 0.15 55 / 70%);
		animation: pulse-warn 0.8s ease-in-out infinite;
	}

	@keyframes pulse-warn {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(1.4); }
	}

	.connection-banner {
		position: fixed;
		top: 1rem;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.4rem 1rem;
		border-radius: 2rem;
		background: oklch(0.065 0.015 280 / 85%);
		backdrop-filter: blur(20px);
		border: 1px solid oklch(0.75 0.15 55 / 15%);
		color: oklch(0.75 0.15 55 / 80%);
		font-family: var(--font-body);
		font-size: 0.75rem;
		letter-spacing: 0.03em;
		z-index: 100;
		animation: banner-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes banner-enter {
		from { opacity: 0; transform: translateX(-50%) translateY(-8px); }
		to { opacity: 1; transform: translateX(-50%) translateY(0); }
	}

	.connection-banner-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.75 0.15 55);
		animation: pulse-warn 0.8s ease-in-out infinite;
	}
</style>
