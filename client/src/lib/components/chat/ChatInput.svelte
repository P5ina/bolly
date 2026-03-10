<script lang="ts">
	let {
		onSend,
		onStop,
		disabled = false,
		agentRunning = false,
		mood = "calm",
	}: {
		onSend: (content: string) => void;
		onStop: () => void;
		disabled?: boolean;
		agentRunning?: boolean;
		mood?: string;
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

<div class="whisper-container" class:whisper-focused={focused} class:whisper-disabled={disabled} data-mood={mood}>
	<div class="whisper-inner">
		<div class="whisper-glow" class:whisper-glow-active={focused || agentRunning}></div>
		<div class="whisper-sense-line">
			<span>{agentRunning ? "alive" : focused ? "listening" : "ready"}</span>
			<span>{mood}</span>
		</div>
		<textarea
			bind:value
			onkeydown={handleKeydown}
			onfocus={() => focused = true}
			onblur={() => focused = false}
			placeholder="..."
			rows={1}
			class="whisper-input"
			disabled={disabled && !agentRunning}
		></textarea>
		{#if agentRunning}
			<button onclick={onStop} class="whisper-stop" aria-label="Stop">
				<svg viewBox="0 0 24 24" fill="currentColor" class="w-3 h-3">
					<rect x="6" y="6" width="12" height="12" rx="2" />
				</svg>
			</button>
		{:else if value.trim() && !disabled}
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
		--input-accent: oklch(0.78 0.12 75 / 16%);
	}

	.whisper-container[data-mood="focused"] {
		--input-accent: oklch(0.76 0.12 170 / 16%);
	}

	.whisper-container[data-mood="playful"] {
		--input-accent: oklch(0.78 0.14 145 / 16%);
	}

	.whisper-container[data-mood="loving"] {
		--input-accent: oklch(0.8 0.12 20 / 16%);
	}

	.whisper-inner {
		position: relative;
		max-width: 600px;
		margin: 0 auto;
	}

	.whisper-sense-line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.45rem;
		padding: 0 0.3rem;
		font-family: var(--font-mono);
		font-size: 0.58rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.78 0.02 280 / 45%);
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
		background: radial-gradient(ellipse, var(--input-accent) 0%, transparent 70%);
	}

	.whisper-input {
		width: 100%;
		min-height: 44px;
		max-height: 120px;
		resize: none;
		padding: 0.75rem 1rem;
		font-family: var(--font-body);
		font-size: 0.875rem;
		line-height: 1.6;
		color: oklch(0.90 0.02 75 / 90%);
		background: oklch(0.10 0.01 280 / 40%);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		border-radius: 12px;
		outline: none;
		transition: all 0.3s ease;
		letter-spacing: 0.01em;
	}
	.whisper-input::placeholder {
		color: oklch(0.78 0.12 75 / 30%);
		font-style: italic;
		font-family: var(--font-display);
	}
	.whisper-input:focus {
		border-color: var(--input-accent);
		background: oklch(0.12 0.01 280 / 50%);
		box-shadow: 0 0 0 4px color-mix(in oklab, var(--input-accent) 55%, transparent);
	}
	.whisper-input:disabled {
		opacity: 0.3;
	}

	.whisper-send {
		position: absolute;
		right: 0.5rem;
		bottom: 0.45rem;
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

	.whisper-stop {
		position: absolute;
		right: 0.5rem;
		bottom: 0.45rem;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		color: oklch(0.70 0.15 25 / 70%);
		transition: all 0.3s ease;
		animation: send-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	.whisper-stop:hover {
		color: oklch(0.70 0.15 25 / 100%);
		background: oklch(0.70 0.15 25 / 12%);
	}

	@keyframes send-enter {
		from { opacity: 0; transform: scale(0.8); }
		to { opacity: 1; transform: scale(1); }
	}
</style>
