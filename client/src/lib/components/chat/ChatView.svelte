<script lang="ts">
	import { untrack } from "svelte";
	import { fetchMessages, sendMessage } from "$lib/api/client.js";
	import type { ChatMessage, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import MessageBubble from "./MessageBubble.svelte";
	import ChatInput from "./ChatInput.svelte";
	import AsciiRenderer from "./AsciiRenderer.svelte";

	let { slug }: { slug: string } = $props();

	let messages = $state<ChatMessage[]>([]);
	let loading = $state(true);
	let sending = $state(false);
	let scrollContainer: HTMLDivElement | undefined = $state();

	const ws = getWebSocket();

	function scrollToBottom() {
		requestAnimationFrame(() => {
			if (scrollContainer) {
				scrollContainer.scrollTop = scrollContainer.scrollHeight;
			}
		});
	}

	function addMessage(msg: ChatMessage) {
		const exists = messages.some((m) => m.id === msg.id);
		console.log("[chat] addMessage", msg.id, exists ? "DUPLICATE" : "NEW", "total:", messages.length);
		if (!exists) {
			messages = [...messages, msg];
			scrollToBottom();
		}
	}

	// Track only `slug` — use untrack for everything else to prevent re-subscription loops
	$effect(() => {
		const currentSlug = slug;

		untrack(() => {
			messages = [];
			loading = true;

			fetchMessages(currentSlug)
				.then((res) => {
					messages = res.messages;
					scrollToBottom();
				})
				.catch(() => {
					messages = [];
				})
				.finally(() => {
					loading = false;
				});
		});

		const unsub = ws.subscribe((event: ServerEvent) => {
			if (
				event.type === "chat_message_created" &&
				event.instance_slug === currentSlug
			) {
				addMessage(event.message);
			}
		});

		return unsub;
	});

	async function handleSend(content: string) {
		sending = true;
		try {
			const res = await sendMessage(slug, content);
			for (const msg of res.messages) {
				addMessage(msg);
			}
		} finally {
			sending = false;
		}
	}
</script>

<div class="companion-space">
	<!-- living atmosphere -->
	<div class="companion-atmosphere" class:companion-thinking={sending}></div>
	<div class="companion-atmosphere-inner" class:companion-thinking={sending}></div>

	<!-- ambient particles -->
	<div class="companion-particles">
		{#each Array(10) as _, i}
			<div
				class="companion-particle"
				class:companion-particle-thinking={sending}
				style="--i:{i}; --x:{8 + (i * 13) % 84}; --y:{5 + (i * 19) % 85}"
			></div>
		{/each}
	</div>

	<!-- companion presence — ASCII creature -->
	<div class="companion-presence">
		<AsciiRenderer thinking={sending} />
	</div>

	<!-- messages — flowing from the companion -->
	<div class="companion-messages" bind:this={scrollContainer}>
		<div class="companion-messages-inner">
			{#if loading}
				<div class="companion-loading">
					<div class="companion-loading-dot"></div>
				</div>
			{:else if messages.length === 0}
				<div class="companion-empty">
					<p class="font-display text-sm italic text-muted-foreground/30">
						i'm here. say something.
					</p>
				</div>
			{:else}
				{#each messages as message, i (message.id)}
					<MessageBubble {message} index={i} />
				{/each}
			{/if}

			{#if sending}
				<div class="companion-thinking-indicator">
					<div class="thinking-dot" style="animation-delay: 0ms"></div>
					<div class="thinking-dot" style="animation-delay: 200ms"></div>
					<div class="thinking-dot" style="animation-delay: 400ms"></div>
				</div>
			{/if}
		</div>
	</div>

	<!-- whisper input -->
	<ChatInput onSend={handleSend} disabled={sending} />
</div>

<style>
	.companion-space {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	/* atmospheric glow — the companion's aura */
	.companion-atmosphere {
		position: absolute;
		top: -5%;
		left: 50%;
		width: 700px;
		height: 500px;
		transform: translate(-50%, 0);
		border-radius: 50%;
		background: radial-gradient(
			ellipse,
			oklch(0.78 0.12 75 / 5%) 0%,
			oklch(0.78 0.12 75 / 2%) 30%,
			transparent 65%
		);
		animation: breathe-slow 7s ease-in-out infinite;
		pointer-events: none;
		transition: all 1s ease;
	}
	.companion-atmosphere.companion-thinking {
		animation: breathe-intense 2.5s ease-in-out infinite;
		background: radial-gradient(
			ellipse,
			oklch(0.78 0.12 75 / 8%) 0%,
			oklch(0.78 0.12 75 / 3%) 30%,
			transparent 65%
		);
	}

	.companion-atmosphere-inner {
		position: absolute;
		top: 0;
		left: 48%;
		width: 400px;
		height: 350px;
		transform: translate(-50%, 0);
		border-radius: 50%;
		background: radial-gradient(
			ellipse,
			oklch(0.70 0.08 300 / 2%) 0%,
			transparent 60%
		);
		animation: breathe-offset 9s ease-in-out infinite;
		pointer-events: none;
		transition: all 1s ease;
	}
	.companion-atmosphere-inner.companion-thinking {
		background: radial-gradient(
			ellipse,
			oklch(0.70 0.08 300 / 4%) 0%,
			transparent 60%
		);
	}

	@keyframes breathe-slow {
		0%, 100% { transform: translate(-50%, 0) scale(1); opacity: 0.7; }
		50% { transform: translate(-50%, 0) scale(1.06); opacity: 1; }
	}
	@keyframes breathe-intense {
		0%, 100% { transform: translate(-50%, 0) scale(1); opacity: 0.8; }
		30% { transform: translate(-50%, 0) scale(1.12); opacity: 1; }
		60% { transform: translate(-50%, 0) scale(0.98); opacity: 0.6; }
	}
	@keyframes breathe-offset {
		0%, 100% { transform: translate(-50%, 0) scale(1); opacity: 0.5; }
		50% { transform: translate(-50%, 0) scale(1.1); opacity: 0.8; }
	}

	/* particles */
	.companion-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
		overflow: hidden;
		z-index: 1;
	}
	.companion-particle {
		position: absolute;
		width: 1.5px;
		height: 1.5px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 15%);
		left: calc(var(--x) * 1%);
		top: calc(var(--y) * 1%);
		animation: drift 18s ease-in-out infinite;
		animation-delay: calc(var(--i) * -1.8s);
		transition: background 1s ease;
	}
	.companion-particle-thinking {
		background: oklch(0.78 0.12 75 / 35%);
		animation-duration: 8s;
	}

	@keyframes drift {
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 0.15; }
		25% { transform: translate(10px, -15px) scale(1.2); opacity: 0.5; }
		50% { transform: translate(-6px, -25px) scale(0.8); opacity: 0.2; }
		75% { transform: translate(14px, -10px) scale(1.1); opacity: 0.4; }
	}

	/* companion presence — ASCII creature */
	.companion-presence {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		padding-top: 1rem;
		padding-bottom: 0.5rem;
		z-index: 2;
		flex-shrink: 0;
		animation: creature-enter 1s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes creature-enter {
		from { opacity: 0; transform: scale(0.8) translateY(10px); }
		to { opacity: 1; transform: scale(1) translateY(0); }
	}

	/* messages area */
	.companion-messages {
		flex: 1;
		overflow-y: auto;
		position: relative;
		z-index: 2;
	}

	.companion-messages-inner {
		max-width: 600px;
		margin: 0 auto;
		padding: 0 1.5rem 2rem;
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.companion-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 4rem 0;
		animation: page-fade-in 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 0.5s;
	}
	@keyframes page-fade-in {
		from { opacity: 0; transform: translateY(8px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.companion-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 3rem 0;
	}
	.companion-loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: loading-pulse 2s ease-in-out infinite;
	}
	@keyframes loading-pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(0.8); }
	}

	/* thinking dots */
	.companion-thinking-indicator {
		display: flex;
		gap: 0.375rem;
		padding: 0.75rem 0;
		justify-content: flex-start;
		animation: page-fade-in 0.4s ease both;
	}
	.thinking-dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 40%);
		animation: thinking-bounce 1.4s ease-in-out infinite;
	}
	@keyframes thinking-bounce {
		0%, 60%, 100% { transform: translateY(0); opacity: 0.3; }
		30% { transform: translateY(-6px); opacity: 1; }
	}
</style>
