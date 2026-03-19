<script lang="ts">
	import type { ChatMessage } from "$lib/api/types.js";
	import AsciiShader from "./AsciiShader.svelte";
	import BackgroundShader from "./BackgroundShader.svelte";
	import PresentationBubble from "./PresentationBubble.svelte";

	type StreamItem =
		| { type: "message"; data: ChatMessage }
		| { type: "activity"; id: string; kind: string; label: string; timestamp: string }
		| { type: "mcp_app"; id: string; toolName: string; toolInput: string; toolOutput: string; html: string }
		| { type: "compaction"; id: string; count: number; timestamp: string };

	let {
		stream = [],
		mood = "calm",
		thinking = false,
		voiceAmplitude = 0,
		streamingMessageId = "",
		onSend,
		onStop,
	}: {
		stream?: StreamItem[];
		mood?: string;
		thinking?: boolean;
		voiceAmplitude?: number;
		streamingMessageId?: string;
		onSend?: (content: string) => void;
		onStop?: () => void;
	} = $props();

	let inputValue = $state("");
	let inputEl: HTMLTextAreaElement | undefined = $state();

	// Track visible bubbles — only show recent messages, auto-expire
	type VisibleBubble = {
		id: string;
		message: ChatMessage;
		side: "left" | "right";
		y: number;
	};

	let visible = $state<VisibleBubble[]>([]);
	let seenIds = new Set<string>();

	// Vertical slot allocator: divide screen into zones, pick least-used
	const SLOTS = [18, 28, 38, 48, 58, 68, 78];
	let slotUsage = $state<Record<number, number>>({});

	function pickSlot(): number {
		let best = SLOTS[0];
		let bestCount = Infinity;
		for (const s of SLOTS) {
			const count = slotUsage[s] ?? 0;
			if (count < bestCount) {
				bestCount = count;
				best = s;
			}
		}
		slotUsage[best] = (slotUsage[best] ?? 0) + 1;
		return best;
	}

	function freeSlot(y: number) {
		if (slotUsage[y]) {
			slotUsage[y]--;
			if (slotUsage[y] <= 0) delete slotUsage[y];
		}
	}

	// Watch stream for new messages
	$effect(() => {
		const msgs = stream.filter(s => s.type === "message") as { type: "message"; data: ChatMessage }[];

		for (const item of msgs) {
			if (seenIds.has(item.data.id)) continue;
			seenIds.add(item.data.id);

			const y = pickSlot();
			visible = [...visible, {
				id: item.data.id,
				message: item.data,
				side: item.data.role === "user" ? "left" : "right",
				y,
			}];
		}
	});

	function removeBubble(id: string) {
		const bubble = visible.find(b => b.id === id);
		if (bubble) freeSlot(bubble.y);
		visible = visible.filter(b => b.id !== id);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			const trimmed = inputValue.trim();
			if (trimmed) {
				onSend?.(trimmed);
				inputValue = "";
			}
		}
		if (e.key === "Escape" && thinking) {
			onStop?.();
		}
	}

	// Auto-focus the hidden input
	$effect(() => {
		if (inputEl) inputEl.focus();
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="present-root" onclick={() => inputEl?.focus()}>
	<BackgroundShader {mood} {thinking} />

	<div class="present-creature">
		<AsciiShader {thinking} {mood} {voiceAmplitude} />
	</div>

	<div class="present-messages" aria-live="polite">
		{#each visible as bubble (bubble.id)}
			{@const msg = stream.find(s => s.type === "message" && s.data.id === bubble.id)}
			{#if msg && msg.type === "message"}
				<PresentationBubble
					message={msg.data}
					side={bubble.side}
					y={bubble.y}
					streaming={msg.data.id === streamingMessageId}
					onexpire={() => removeBubble(bubble.id)}
				/>
			{/if}
		{/each}
	</div>

	{#if thinking}
		<div class="present-thinking">
			<div class="pt-dot" style="animation-delay: 0ms"></div>
			<div class="pt-dot" style="animation-delay: 200ms"></div>
			<div class="pt-dot" style="animation-delay: 400ms"></div>
		</div>
	{/if}

	<textarea
		bind:this={inputEl}
		bind:value={inputValue}
		onkeydown={handleKeydown}
		class="present-input"
		aria-label="Chat input"
	></textarea>
</div>

<style>
	.present-root {
		position: fixed;
		inset: 0;
		z-index: 200;
		overflow: hidden;
		cursor: none;
	}

	.present-creature {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		pointer-events: none;
		opacity: 0.5;
	}

	.present-creature :global(.ascii-shader) {
		width: 65vh !important;
		height: 65vh !important;
	}

	.present-messages {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}

	.present-thinking {
		position: absolute;
		bottom: 6vh;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		gap: 0.6rem;
	}

	.pt-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(0.6 0.08 200 / 50%);
		box-shadow: 0 0 12px oklch(0.6 0.08 200 / 25%);
		animation: pt-bounce 1.4s ease-in-out infinite;
	}

	@keyframes pt-bounce {
		0%, 60%, 100% { transform: translateY(0); opacity: 0.3; }
		30% { transform: translateY(-10px); opacity: 1; }
	}

	.present-input {
		position: absolute;
		bottom: 0;
		left: 0;
		width: 1px;
		height: 1px;
		opacity: 0;
		pointer-events: auto;
	}
</style>
