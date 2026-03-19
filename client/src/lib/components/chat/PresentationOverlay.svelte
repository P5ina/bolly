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
	let inputFocused = $state(false);
	let inputVisible = $state(false);

	// Track visible bubbles — only show recent messages, auto-expire
	type VisibleBubble = {
		id: string;
		message: ChatMessage;
		side: "left" | "right";
		y: number;
	};

	let visible = $state<VisibleBubble[]>([]);
	// Seed with all existing message IDs so history doesn't fly in on mount
	let seenIds = new Set<string>(
		stream.filter(s => s.type === "message").map(s => (s as { type: "message"; data: ChatMessage }).data.id)
	);

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
				// Hide input after send with a short delay
				setTimeout(() => { inputVisible = false; }, 300);
			}
		}
		if (e.key === "Escape") {
			if (thinking) onStop?.();
			inputVisible = false;
			inputEl?.blur();
		}
	}

	function showInput() {
		inputVisible = true;
		requestAnimationFrame(() => inputEl?.focus({ preventScroll: true }));
	}

	// Any keypress on the page opens the input bar
	function handleGlobalKey(e: KeyboardEvent) {
		if (e.metaKey || e.ctrlKey || e.altKey) return;
		if (e.key === "Escape") return;
		if (e.key.length === 1 && !inputVisible) {
			showInput();
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="present-root" onclick={showInput} onkeydown={handleGlobalKey} role="application">
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

	<div class="present-bar" class:present-bar-visible={inputVisible}>
		<div class="present-bar-glass">
			<div class="present-bar-glow"></div>
			<textarea
				bind:this={inputEl}
				bind:value={inputValue}
				onkeydown={handleKeydown}
				onfocus={(e) => { inputFocused = true; e.currentTarget.scrollTop = 0; }}
				onblur={() => { inputFocused = false; if (!inputValue) inputVisible = false; }}
				class="present-textarea"
				placeholder="..."
				rows={1}
				autocomplete="off"
				aria-label="Chat input"
			></textarea>
		</div>
	</div>
</div>

<style>
	.present-root {
		position: fixed;
		inset: 0;
		z-index: 200;
		overflow: hidden;
		overscroll-behavior: none;
		touch-action: none;
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
		transition: bottom 0.5s cubic-bezier(0.16, 1, 0.3, 1);
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

	/* --- Animated input bar --- */

	.present-bar {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		padding: 2vh 8vw 4vh;
		transform: translateY(100%);
		opacity: 0;
		transition:
			transform 0.6s cubic-bezier(0.16, 1, 0.3, 1),
			opacity 0.4s ease;
		pointer-events: none;
	}

	.present-bar-visible {
		transform: translateY(0);
		opacity: 1;
		pointer-events: auto;
	}

	.present-bar-glass {
		position: relative;
		max-width: 800px;
		margin: 0 auto;
		overflow: hidden;
		border-radius: 20px;
		background: oklch(0.08 0.02 210 / 45%);
		backdrop-filter: blur(32px) saturate(150%);
		-webkit-backdrop-filter: blur(32px) saturate(150%);
		border: 1px solid oklch(0.5 0.06 200 / 12%);
		box-shadow:
			0 8px 40px oklch(0 0 0 / 30%),
			0 0 1px oklch(0.6 0.08 200 / 20%),
			inset 0 1px 0 oklch(1 0 0 / 4%);
	}

	.present-bar-glow {
		position: absolute;
		bottom: -20px;
		left: 50%;
		transform: translateX(-50%);
		width: 60%;
		height: 40px;
		border-radius: 50%;
		background: radial-gradient(ellipse, oklch(0.5 0.08 200 / 15%) 0%, transparent 70%);
		animation: bar-glow-pulse 3s ease-in-out infinite;
		pointer-events: none;
	}

	@keyframes bar-glow-pulse {
		0%, 100% { opacity: 0.5; width: 60%; }
		50% { opacity: 1; width: 75%; }
	}

	.present-textarea {
		display: block;
		width: 100%;
		padding: 1.2rem 1.6rem;
		font-family: var(--font-body);
		font-size: clamp(1.2rem, 2.2vw, 1.6rem);
		line-height: 1.5;
		color: oklch(0.95 0.02 75);
		background: transparent;
		border: none;
		outline: none;
		resize: none;
		caret-color: oklch(0.7 0.1 190);
	}

	.present-textarea::placeholder {
		color: oklch(0.45 0.04 200 / 30%);
		font-style: italic;
		font-family: var(--font-display);
	}
</style>
