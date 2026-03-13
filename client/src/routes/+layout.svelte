<script lang="ts">
	import "./layout.css";
	import favicon from "$lib/assets/favicon.svg";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import { AuthError } from "$lib/api/client.js";
	import type { ServerEvent } from "$lib/api/types.js";
	import { onMount } from "svelte";
	import { pwaInfo } from "virtual:pwa-info";
	import AuthGate from "$lib/components/auth/AuthGate.svelte";
	import Toast from "$lib/components/layout/Toast.svelte";
	import SecretDialog from "$lib/components/layout/SecretDialog.svelte";

	let { children } = $props();

	onMount(async () => {
		if (pwaInfo) {
			const { registerSW } = await import("virtual:pwa-register");
			updateSW = registerSW({
				immediate: true,
				onNeedRefresh() {
					updateAvailable = true;
				},
			});
		}
	});

	const instances = getInstances();
	const ws = getWebSocket();

	let needsAuth = $state(false);
	let updateAvailable = $state(false);
	let updateSW: ((reloadPage?: boolean) => Promise<void>) | undefined;

	// Secret request state
	let secretRequest = $state<{
		instanceSlug: string;
		id: string;
		prompt: string;
		target: string;
	} | null>(null);

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
			} else if (event.type === "secret_request") {
				secretRequest = {
					instanceSlug: event.instance_slug,
					id: event.id,
					prompt: event.prompt,
					target: event.target,
				};
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


</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<link rel="manifest" href="/manifest.webmanifest" />
	<title>bolly</title>
</svelte:head>

<div class="relative h-dvh w-full overflow-hidden">
	{#if needsAuth}
		<AuthGate onauth={handleAuth} />
	{:else}
		{@render children()}
	{/if}

	<!-- update available banner -->
	{#if updateAvailable}
		<div class="update-banner">
			<span class="update-banner-text">update available</span>
			<button class="update-banner-btn" onclick={() => updateSW?.(true)}>refresh</button>
			<button class="update-banner-dismiss" onclick={() => updateAvailable = false} aria-label="Dismiss">
				<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" class="w-2.5 h-2.5">
					<path d="M2 2l8 8M10 2l-8 8" stroke-linecap="round"/>
				</svg>
			</button>
		</div>
	{/if}

	<Toast />

	{#if secretRequest}
		<SecretDialog
			instanceSlug={secretRequest.instanceSlug}
			requestId={secretRequest.id}
			prompt={secretRequest.prompt}
			target={secretRequest.target}
			onclose={() => (secretRequest = null)}
		/>
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
	@keyframes pulse-warn {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(1.4); }
	}

	.update-banner {
		position: fixed;
		top: calc(1rem + env(safe-area-inset-top, 0px));
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		align-items: center;
		gap: 0.625rem;
		padding: 0.4rem 0.5rem 0.4rem 1rem;
		border-radius: 2rem;
		background: oklch(0.065 0.015 280 / 85%);
		backdrop-filter: blur(20px);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		z-index: 100;
		animation: banner-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.update-banner-text {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: oklch(0.78 0.12 75 / 70%);
	}

	.update-banner-btn {
		padding: 0.2rem 0.625rem;
		border-radius: 1rem;
		background: oklch(0.78 0.12 75 / 12%);
		border: 1px solid oklch(0.78 0.12 75 / 20%);
		color: oklch(0.78 0.12 75 / 90%);
		font-family: var(--font-mono);
		font-size: 0.62rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		transition: all 0.2s ease;
		cursor: pointer;
	}
	.update-banner-btn:hover {
		background: oklch(0.78 0.12 75 / 20%);
		border-color: oklch(0.78 0.12 75 / 35%);
	}

	.update-banner-dismiss {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.25rem;
		height: 1.25rem;
		border-radius: 50%;
		color: oklch(0.55 0.02 280 / 40%);
		transition: all 0.2s ease;
		cursor: pointer;
	}
	.update-banner-dismiss:hover {
		color: oklch(0.78 0.12 75 / 60%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	.connection-banner {
		position: fixed;
		top: calc(1rem + env(safe-area-inset-top, 0px));
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
