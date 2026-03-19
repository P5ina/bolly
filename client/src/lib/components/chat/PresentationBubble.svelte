<script lang="ts">
	import { onMount } from "svelte";
	import type { ChatMessage } from "$lib/api/types.js";
	import { Marked } from "marked";

	let {
		message,
		side = "right",
		y = 40,
		streaming = false,
		onexpire,
	}: {
		message: ChatMessage;
		side?: "left" | "right";
		y?: number;
		streaming?: boolean;
		onexpire?: () => void;
	} = $props();

	const marked = new Marked({ breaks: true, gfm: true });

	let el: HTMLDivElement | undefined = $state();
	let fading = $state(false);
	let expired = false;

	const isUser = $derived(message.role === "user");
	const html = $derived(
		isUser ? message.content : (marked.parse(message.content) as string)
	);

	onMount(() => {
		// Start expiration timer only when not streaming
		let timer: ReturnType<typeof setTimeout>;

		function startExpiry() {
			timer = setTimeout(() => {
				fading = true;
				setTimeout(() => {
					if (!expired) { expired = true; onexpire?.(); }
				}, 1800);
			}, 14000);
		}

		if (!streaming) startExpiry();

		// Watch for streaming to finish
		const interval = setInterval(() => {
			if (!streaming && !timer) startExpiry();
		}, 500);

		return () => {
			clearTimeout(timer);
			clearInterval(interval);
		};
	});
</script>

<div
	bind:this={el}
	class="pbubble"
	class:pbubble-left={side === "left"}
	class:pbubble-right={side === "right"}
	class:pbubble-fading={fading}
	class:pbubble-streaming={streaming}
	style="--fly-y: {y}vh"
>
	<div class="pbubble-glass">
		{#if isUser}
			<span class="pbubble-text">{message.content}</span>
		{:else}
			<div class="pbubble-text pbubble-prose">{@html html}</div>
		{/if}
	</div>
</div>

<style>
	.pbubble {
		position: absolute;
		top: var(--fly-y);
		max-width: 48vw;
		pointer-events: none;
		will-change: transform, opacity;
	}

	.pbubble-left {
		left: 3vw;
		animation: fly-in-left 0.9s cubic-bezier(0.34, 1.56, 0.64, 1) both;
	}

	.pbubble-right {
		right: 3vw;
		animation: fly-in-right 0.9s cubic-bezier(0.34, 1.56, 0.64, 1) both;
	}

	.pbubble-fading {
		animation: bubble-fade-out 1.8s ease-out forwards !important;
	}

	@keyframes fly-in-left {
		from { transform: translateX(-110vw) scale(0.8) rotate(-2deg); opacity: 0; }
		to { transform: translateX(0) scale(1) rotate(0); opacity: 1; }
	}

	@keyframes fly-in-right {
		from { transform: translateX(110vw) scale(0.8) rotate(2deg); opacity: 0; }
		to { transform: translateX(0) scale(1) rotate(0); opacity: 1; }
	}

	@keyframes bubble-fade-out {
		0% { opacity: 1; transform: scale(1); filter: blur(0); }
		100% { opacity: 0; transform: scale(0.92); filter: blur(6px); }
	}

	.pbubble-glass {
		padding: 1.2rem 1.8rem;
		background: oklch(0.1 0.025 210 / 50%);
		backdrop-filter: blur(28px) saturate(140%);
		-webkit-backdrop-filter: blur(28px) saturate(140%);
		border: 1px solid oklch(0.5 0.06 200 / 12%);
		border-radius: 24px;
		box-shadow:
			0 4px 24px oklch(0 0 0 / 25%),
			0 0 1px oklch(0.6 0.08 200 / 20%),
			inset 0 1px 0 oklch(1 0 0 / 4%);
	}

	.pbubble-left .pbubble-glass {
		border-radius: 24px 24px 24px 6px;
		background: oklch(0.12 0.015 220 / 40%);
	}

	.pbubble-right .pbubble-glass {
		border-radius: 24px 24px 6px 24px;
	}

	.pbubble-streaming .pbubble-glass {
		box-shadow:
			0 4px 24px oklch(0 0 0 / 25%),
			0 0 20px oklch(0.55 0.08 200 / 10%),
			inset 0 1px 0 oklch(1 0 0 / 4%);
	}

	.pbubble-text {
		font-family: var(--font-body);
		font-size: clamp(1.4rem, 2.8vw, 2.2rem);
		line-height: 1.5;
		letter-spacing: 0.01em;
		color: oklch(0.95 0.02 75);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.pbubble-left .pbubble-text {
		color: oklch(0.78 0.02 220 / 70%);
		font-size: clamp(1.1rem, 2.2vw, 1.6rem);
	}

	/* Prose overrides for large text */
	.pbubble-prose :global(p) { margin: 0.3em 0; }
	.pbubble-prose :global(p:first-child) { margin-top: 0; }
	.pbubble-prose :global(p:last-child) { margin-bottom: 0; }
	.pbubble-prose :global(strong) { color: oklch(0.98 0.03 75); }
	.pbubble-prose :global(em) { font-style: italic; }
	.pbubble-prose :global(code) {
		font-family: var(--font-mono);
		font-size: 0.85em;
		background: oklch(0.1 0.015 200 / 50%);
		padding: 0.1em 0.3em;
		border-radius: 6px;
	}
	.pbubble-prose :global(a) { color: oklch(0.75 0.1 190); }
</style>
