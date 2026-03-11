<script lang="ts">
	import { play } from "$lib/sounds.js";
	import UsageBar from "$lib/components/layout/UsageBar.svelte";

	let {
		onSend,
		onStop,
		disabled = false,
		agentRunning = false,
		mood = "calm",
	}: {
		onSend: (content: string, files?: File[]) => void;
		onStop: () => void;
		disabled?: boolean;
		agentRunning?: boolean;
		mood?: string;
	} = $props();

	let value = $state("");
	let focused = $state(false);
	let usageTick = $state(0);
	let prevDisabled = $state(false);

	// Refresh usage when agent finishes (disabled goes from true to false)
	$effect(() => {
		if (prevDisabled && !disabled) {
			usageTick++;
		}
		prevDisabled = disabled;
	});
	let attachments = $state<File[]>([]);
	let fileInput: HTMLInputElement | undefined = $state();
	let textareaEl: HTMLTextAreaElement | undefined = $state();
	let dragging = $state(false);
	let dragCounter = $state(0);

	$effect(() => {
		// Refocus textarea when it re-enables after sending
		const isDisabled = disabled && !agentRunning;
		if (!isDisabled && textareaEl) {
			textareaEl.focus();
		}
	});

	function handleSubmit() {
		const trimmed = value.trim();
		if ((!trimmed && attachments.length === 0) || disabled) return;
		play("message_send");
		onSend(trimmed, attachments.length > 0 ? [...attachments] : undefined);
		value = "";
		attachments = [];
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}

	function openFilePicker() {
		fileInput?.click();
	}

	function handleFiles(e: Event) {
		const input = e.target as HTMLInputElement;
		if (input.files && input.files.length > 0) {
			play("attachment_added");
			attachments = [...attachments, ...Array.from(input.files)];
		}
		input.value = "";
	}

	function removeAttachment(index: number) {
		attachments = attachments.filter((_, i) => i !== index);
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes}B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
	}

	function isImage(file: File): boolean {
		return file.type.startsWith("image/");
	}

	function handleDragEnter(e: DragEvent) {
		e.preventDefault();
		dragCounter++;
		if (e.dataTransfer?.types.includes("Files")) {
			dragging = true;
		}
	}

	function handleDragLeave(e: DragEvent) {
		e.preventDefault();
		dragCounter--;
		if (dragCounter === 0) {
			dragging = false;
		}
	}

	function handleDragOver(e: DragEvent) {
		e.preventDefault();
		if (e.dataTransfer) {
			e.dataTransfer.dropEffect = "copy";
		}
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault();
		dragging = false;
		dragCounter = 0;
		if (disabled && !agentRunning) return;
		const files = e.dataTransfer?.files;
		if (files && files.length > 0) {
			play("attachment_added");
			attachments = [...attachments, ...Array.from(files)];
		}
	}
</script>

<input
	type="file"
	bind:this={fileInput}
	onchange={handleFiles}
	multiple
	hidden
/>

<div
	class="whisper-container"
	class:whisper-focused={focused}
	class:whisper-disabled={disabled}
	class:whisper-dragover={dragging}
	data-mood={mood}
	ondragenter={handleDragEnter}
	ondragleave={handleDragLeave}
	ondragover={handleDragOver}
	ondrop={handleDrop}
	role="region"
>
	{#if dragging}
		<div class="whisper-dropzone">
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-5 h-5">
				<path d="M12 16V4m0 0L8 8m4-4l4 4" stroke-linecap="round" stroke-linejoin="round"/>
				<path d="M20 16v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			<span>drop files</span>
		</div>
	{/if}
	<div class="whisper-inner">
		<div class="whisper-glow" class:whisper-glow-active={focused || agentRunning}></div>
		<div class="whisper-sense-line">
			<span>{agentRunning ? "alive" : focused ? "listening" : "ready"}</span>
			<span>{mood}</span>
		</div>

		{#if attachments.length > 0}
			<div class="whisper-attachments">
				{#each attachments as file, i}
					<div class="whisper-attachment">
						{#if isImage(file)}
							<img src={URL.createObjectURL(file)} alt={file.name} class="attachment-thumb" />
						{:else}
							<span class="attachment-icon">
								<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" class="w-3 h-3">
									<path d="M4 1h5.5L13 4.5V14a1 1 0 01-1 1H4a1 1 0 01-1-1V2a1 1 0 011-1z" stroke-linejoin="round"/>
									<path d="M9 1v4h4" stroke-linejoin="round"/>
								</svg>
							</span>
						{/if}
						<span class="attachment-name">{file.name}</span>
						<span class="attachment-size">{formatSize(file.size)}</span>
						<button class="attachment-remove" onclick={() => removeAttachment(i)} onmousedown={(e) => e.preventDefault()} aria-label="Remove">
							<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" class="w-2.5 h-2.5">
								<path d="M2 2l8 8M10 2l-8 8" stroke-linecap="round"/>
							</svg>
						</button>
					</div>
				{/each}
			</div>
		{/if}

		<div class="whisper-row">
			<button onclick={openFilePicker} onmousedown={(e) => e.preventDefault()} class="whisper-attach" aria-label="Attach file" disabled={disabled && !agentRunning}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-4 h-4">
					<path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
			<textarea
				bind:this={textareaEl}
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
				<button onclick={onStop} onmousedown={(e) => e.preventDefault()} class="whisper-stop" aria-label="Stop">
					<svg viewBox="0 0 24 24" fill="currentColor" class="w-3 h-3">
						<rect x="6" y="6" width="12" height="12" rx="2" />
					</svg>
				</button>
			{:else if (value.trim() || attachments.length > 0) && !disabled}
				<button onclick={handleSubmit} onmousedown={(e) => e.preventDefault()} class="whisper-send" aria-label="Send">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-4 h-4">
						<path d="M5 12h14" stroke-linecap="round"/><path d="m12 5 7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</button>
			{/if}
		</div>
		<UsageBar tick={usageTick} />
	</div>
</div>

<style>
	.whisper-container {
		position: relative;
		isolation: isolate;
		padding: 0.75rem 1.5rem calc(1.5rem + env(safe-area-inset-bottom, 0px));
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

	/* attachments */
	.whisper-attachments {
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
		margin-bottom: 0.5rem;
		padding: 0 0.25rem;
	}

	.whisper-attachment {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		padding: 0.25rem 0.5rem;
		border-radius: 8px;
		background: oklch(0.78 0.12 75 / 6%);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		animation: attach-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes attach-enter {
		from { opacity: 0; transform: scale(0.9); }
		to { opacity: 1; transform: scale(1); }
	}

	.attachment-thumb {
		width: 24px;
		height: 24px;
		border-radius: 4px;
		object-fit: cover;
	}

	.attachment-icon {
		display: flex;
		color: oklch(0.78 0.12 75 / 40%);
	}

	.attachment-name {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.85 0.03 75 / 70%);
		max-width: 100px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.attachment-size {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.55 0.02 280 / 40%);
	}

	.attachment-remove {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1rem;
		height: 1rem;
		border-radius: 50%;
		color: oklch(0.55 0.02 280 / 40%);
		transition: all 0.2s ease;
	}
	.attachment-remove:hover {
		color: oklch(0.70 0.15 25 / 80%);
		background: oklch(0.70 0.15 25 / 10%);
	}

	/* input row */
	.whisper-row {
		display: flex;
		align-items: flex-end;
		gap: 0.375rem;
	}

	.whisper-attach {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		flex-shrink: 0;
		border-radius: 50%;
		color: oklch(0.78 0.12 75 / 30%);
		transition: all 0.3s ease;
		margin-bottom: 0.375rem;
	}
	.whisper-attach:hover {
		color: oklch(0.78 0.12 75 / 60%);
		background: oklch(0.78 0.12 75 / 8%);
	}
	.whisper-attach:disabled {
		opacity: 0.2;
		pointer-events: none;
	}

	.whisper-input {
		flex: 1;
		min-width: 0;
		min-height: 44px;
		max-height: 120px;
		resize: none;
		padding: 0.75rem 1rem;
		font-family: var(--font-body);
		font-size: 1rem;
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
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		flex-shrink: 0;
		border-radius: 50%;
		color: oklch(0.78 0.12 75 / 50%);
		transition: all 0.3s ease;
		animation: send-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
		margin-bottom: 0.375rem;
	}
	.whisper-send:hover {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	.whisper-stop {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		flex-shrink: 0;
		border-radius: 50%;
		color: oklch(0.70 0.15 25 / 70%);
		transition: all 0.3s ease;
		animation: send-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
		margin-bottom: 0.375rem;
	}
	.whisper-stop:hover {
		color: oklch(0.70 0.15 25 / 100%);
		background: oklch(0.70 0.15 25 / 12%);
	}

	@keyframes send-enter {
		from { opacity: 0; transform: scale(0.8); }
		to { opacity: 1; transform: scale(1); }
	}

	/* drop zone */
	.whisper-dragover {
		outline: 2px dashed oklch(0.78 0.12 75 / 40%);
		outline-offset: -2px;
		border-radius: 16px;
	}

	.whisper-dropzone {
		position: absolute;
		inset: 0;
		z-index: 20;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		border-radius: 16px;
		background: oklch(0.10 0.02 75 / 85%);
		color: oklch(0.78 0.12 75 / 70%);
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		animation: dropzone-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1) both;
		pointer-events: none;
	}

	@keyframes dropzone-enter {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	@media (max-width: 720px) {
		.whisper-container {
			padding: 0.5rem 0.75rem calc(0.75rem + env(safe-area-inset-bottom, 0px));
		}
	}
</style>
