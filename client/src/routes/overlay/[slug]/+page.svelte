<script lang="ts">
	import { page } from "$app/stores";
	import { onMount } from "svelte";
	import { getSkinStore, clipSrc } from "$lib/stores/skin.svelte.js";
	import { getSceneStore } from "$lib/stores/scene.svelte.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import type { ServerEvent } from "$lib/api/types.js";

	const slug = $derived($page.params.slug!);
	const skinStore = getSkinStore();
	const store = getSceneStore();
	const ws = getWebSocket();

	let recording = $state(false);
	let actionQueue = $state<{ id: number; text: string }[]>([]);
	let idCounter = 0;

	onMount(() => {
		skinStore.loadForInstance(slug);
		store.enterChat(slug);

		const unsub = ws.subscribe((event: ServerEvent) => {
			if (event.type === "tool_started" && event.instance_slug === slug) {
				const id = ++idCounter;
				actionQueue = [...actionQueue, { id, text: event.tool_name ?? "action" }];
				setTimeout(() => {
					actionQueue = actionQueue.filter(a => a.id !== id);
				}, 3000);
			}
		});

		return unsub;
	});

	let videoPhase = $derived(store.thinking ? 'thinking' : 'idle');
	let thinkingIdx = $state(0);

	let videoSrc = $derived.by(() => {
		const clips = skinStore.skin.clips;
		if (videoPhase === 'thinking') {
			return clips.thinking[thinkingIdx] ?? clips.idle;
		}
		return clips.idle;
	});

	let isLooping = $derived(videoPhase === 'idle');

	$effect(() => {
		if (store.thinking) {
			const clips = skinStore.skin.clips.thinking;
			thinkingIdx = Math.floor(Math.random() * clips.length);
		}
	});

	let videoEl: HTMLVideoElement | undefined = $state();

	$effect(() => {
		if (videoEl) {
			const src = clipSrc(videoSrc);
			if (videoEl.src !== src) {
				videoEl.src = src;
				videoEl.loop = isLooping;
				videoEl.play().catch(() => {});
			}
		}
	});

	function handleVideoEnded() {
		if (videoPhase === 'thinking') {
			const clips = skinStore.skin.clips.thinking;
			thinkingIdx = Math.floor(Math.random() * clips.length);
		}
	}
</script>

<div class="overlay">
	<div class="character">
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={videoEl}
			class="character-video"
			muted
			playsinline
			autoplay
			onended={handleVideoEnded}
		></video>
		{#if recording}
			<div class="rec-dot"></div>
		{/if}
	</div>

	{#if actionQueue.length > 0}
		<div class="flash-stack">
			{#each actionQueue as flash (flash.id)}
				<div class="flash">{flash.text}</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	:global(html), :global(body) {
		background: transparent !important;
		margin: 0;
		padding: 0;
		overflow: hidden;
	}

	.overlay {
		position: fixed;
		inset: 0;
		pointer-events: none;
	}

	.character {
		position: absolute;
		bottom: 12px;
		right: 12px;
		width: 64px;
		height: 64px;
		border-radius: 50%;
		overflow: hidden;
		background: oklch(0.08 0.02 160 / 80%);
		border: 2px solid oklch(0.75 0.12 160 / 40%);
		box-shadow: 0 2px 16px oklch(0 0 0 / 50%);
		animation: breathe 4s ease-in-out infinite;
	}

	@keyframes breathe {
		0%, 100% { transform: scale(1); }
		50% { transform: scale(1.02); }
	}

	.character-video {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.rec-dot {
		position: absolute;
		top: 0;
		right: 0;
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: oklch(0.62 0.25 25);
		box-shadow: 0 0 8px oklch(0.62 0.25 25 / 80%);
		animation: rec-pulse 1.5s ease-in-out infinite;
	}

	@keyframes rec-pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.4; }
	}

	.flash-stack {
		position: absolute;
		bottom: 84px;
		right: 12px;
		display: flex;
		flex-direction: column-reverse;
		gap: 4px;
		align-items: flex-end;
	}

	.flash {
		font-family: "SF Mono", ui-monospace, monospace;
		font-size: 11px;
		padding: 4px 10px;
		border-radius: 8px;
		background: oklch(0.10 0.02 260 / 85%);
		color: oklch(0.85 0.03 75);
		border: 1px solid oklch(1 0 0 / 8%);
		animation: flash-in 3s ease both;
	}

	@keyframes flash-in {
		0% { opacity: 0; transform: translateX(12px); }
		8% { opacity: 1; transform: translateX(0); }
		80% { opacity: 1; }
		100% { opacity: 0; transform: translateX(6px); }
	}
</style>
