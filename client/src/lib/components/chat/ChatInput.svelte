<script lang="ts">
	let {
		onSend,
		disabled = false,
	}: {
		onSend: (content: string) => void;
		disabled?: boolean;
	} = $props();

	let value = $state("");
	let focused = $state(false);

	function handleSubmit() {
		const trimmed = value.trim();
		if (!trimmed || disabled) return;
		onSend(trimmed);
		value = "";
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}
</script>

<div class="whisper-container" class:whisper-focused={focused} class:whisper-disabled={disabled}>
	<div class="whisper-inner">
		<div class="whisper-glow" class:whisper-glow-active={focused}></div>
		<textarea
			bind:value
			onkeydown={handleKeydown}
			onfocus={() => focused = true}
			onblur={() => focused = false}
			placeholder="..."
			rows={1}
			class="whisper-input"
			{disabled}
		></textarea>
		{#if value.trim() && !disabled}
			<button onclick={handleSubmit} class="whisper-send" aria-label="Send">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-4 h-4">
					<path d="M5 12h14" stroke-linecap="round"/><path d="m12 5 7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
		{/if}
	</div>
</div>

<style>
	.whisper-container {
		position: relative;
		padding: 0.75rem 1.5rem 1.5rem;
		z-index: 10;
		flex-shrink: 0;
	}

	.whisper-inner {
		position: relative;
		max-width: 600px;
		margin: 0 auto;
	}

	.whisper-glow {
		position: absolute;
		bottom: -8px;
		left: 50%;
		transform: translateX(-50%);
		width: 200px;
		height: 40px;
		border-radius: 50%;
		background: radial-gradient(ellipse, oklch(0.78 0.12 75 / 0%) 0%, transparent 70%);
		transition: all 0.5s ease;
		pointer-events: none;
	}
	.whisper-glow-active {
		width: 300px;
		background: radial-gradient(ellipse, oklch(0.78 0.12 75 / 6%) 0%, transparent 70%);
	}

	.whisper-input {
		width: 100%;
		min-height: 44px;
		max-height: 120px;
		resize: none;
		padding: 0.75rem 0;
		font-family: var(--font-body);
		font-size: 0.875rem;
		line-height: 1.6;
		color: oklch(0.88 0.02 75 / 80%);
		background: transparent;
		border: none;
		border-bottom: 1px solid oklch(0.78 0.12 75 / 8%);
		outline: none;
		transition: all 0.4s ease;
		letter-spacing: 0.01em;
	}
	.whisper-input::placeholder {
		color: oklch(0.78 0.12 75 / 15%);
		font-style: italic;
		font-family: var(--font-display);
	}
	.whisper-input:focus {
		border-bottom-color: oklch(0.78 0.12 75 / 25%);
	}
	.whisper-input:disabled {
		opacity: 0.3;
	}

	.whisper-send {
		position: absolute;
		right: 0;
		bottom: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		color: oklch(0.78 0.12 75 / 50%);
		transition: all 0.3s ease;
		animation: send-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	.whisper-send:hover {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	@keyframes send-enter {
		from { opacity: 0; transform: scale(0.8); }
		to { opacity: 1; transform: scale(1); }
	}
</style>
