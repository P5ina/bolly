<script lang="ts">
	import { play } from "$lib/sounds.js";
	import { hapticLight, hapticMedium } from "$lib/haptics.js";
	import { fetchConfigStatus, updateModelMode } from "$lib/api/client.js";
	import UsageBar from "$lib/components/layout/UsageBar.svelte";

	let {
		onSend,
		onStop,
		disabled = false,
		agentRunning = false,
		mood = "calm",
		uploadProgress = null,
	}: {
		onSend: (content: string, files?: File[]) => void;
		onStop: () => void;
		disabled?: boolean;
		agentRunning?: boolean;
		mood?: string;
		uploadProgress?: { fileIndex: number; fileCount: number; loaded: number; total: number } | null;
	} = $props();

	// Model mode
	let modelMode = $state("auto");
	$effect(() => {
		fetchConfigStatus().then(s => { if (s.model_mode) modelMode = s.model_mode; }).catch(() => {});
	});
	async function cycleMode() {
		const next = modelMode === "auto" ? "fast" : modelMode === "fast" ? "heavy" : "auto";
		modelMode = next;
		hapticLight();
		await updateModelMode(next).catch(() => {});
	}

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

	// Auto-resize textarea to fit content, up to max-height
	$effect(() => {
		// Track value so this effect re-runs on every keystroke
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		value;
		if (!textareaEl) return;
		// Temporarily hide overflow to get accurate scrollHeight
		textareaEl.style.overflowY = "hidden";
		textareaEl.style.height = "auto";
		const h = textareaEl.scrollHeight;
		textareaEl.style.height = `${h}px`;
		// Only show scrollbar when content exceeds max-height
		textareaEl.style.overflowY = h >= 200 ? "auto" : "hidden";
	});

	function handleSubmit() {
		const trimmed = value.trim();
		if ((!trimmed && attachments.length === 0) || disabled) return;
		play("message_send");
		hapticLight();
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
			hapticMedium();
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
			hapticMedium();
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
			<button
				class="mode-toggle"
				class:mode-toggle-fast={modelMode === "fast"}
				class:mode-toggle-heavy={modelMode === "heavy"}
				onclick={cycleMode}
				onmousedown={(e) => e.preventDefault()}
				title="Model: {modelMode} (click to cycle)"
			>
				{modelMode === "auto" ? "A" : modelMode === "fast" ? "F" : "H"}
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
		{#if uploadProgress}
			{@const pct = uploadProgress.total > 0 ? (uploadProgress.loaded / uploadProgress.total) * 100 : 0}
			{@const mb = (s: number) => s < 1024 * 1024 ? `${(s / 1024).toFixed(0)} KB` : `${(s / 1024 / 1024).toFixed(1)} MB`}
			<div class="upload-progress">
				<div class="upload-progress-bar" style="width: {pct}%"></div>
				<span class="upload-progress-label">
					{#if uploadProgress.fileCount > 1}
						file {uploadProgress.fileIndex + 1}/{uploadProgress.fileCount} ·
					{/if}
					{mb(uploadProgress.loaded)} / {mb(uploadProgress.total)} ({pct.toFixed(0)}%)
				</span>
			</div>
		{/if}
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
		min-width: 0;
		box-sizing: border-box;
		--input-accent: oklch(0.6 0.1 190 / 20%);
	}

	.whisper-container[data-mood="focused"] {
		--input-accent: oklch(0.65 0.1 180 / 20%);
	}

	.whisper-container[data-mood="playful"] {
		--input-accent: oklch(0.7 0.12 160 / 20%);
	}

	.whisper-container[data-mood="loving"] {
		--input-accent: oklch(0.7 0.1 20 / 20%);
	}

	.whisper-inner {
		position: relative;
		max-width: 600px;
		margin: 0 auto;
	}

	.whisper-glow {
		position: absolute;
		bottom: -12px;
		left: 50%;
		transform: translateX(-50%);
		width: 250px;
		height: 50px;
		border-radius: 50%;
		background: radial-gradient(ellipse, oklch(0.55 0.08 200 / 0%) 0%, transparent 70%);
		transition: all 0.6s cubic-bezier(0.16, 1, 0.3, 1);
		pointer-events: none;
	}
	.whisper-glow-active {
		width: 350px;
		height: 60px;
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
		border-radius: 10px;
		background: oklch(0.14 0.02 200 / 30%);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border: 1px solid oklch(0.5 0.06 200 / 10%);
		animation: attach-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes attach-enter {
		from { opacity: 0; transform: scale(0.9); }
		to { opacity: 1; transform: scale(1); }
	}

	.attachment-thumb {
		width: 24px;
		height: 24px;
		border-radius: 5px;
		object-fit: cover;
	}

	.attachment-icon {
		display: flex;
		color: oklch(0.6 0.08 200 / 65%);
	}

	.attachment-name {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.80 0.03 200 / 82%);
		max-width: 100px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.attachment-size {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.55 0.03 200 / 55%);
	}

	.attachment-remove {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1rem;
		height: 1rem;
		border-radius: 50%;
		color: oklch(0.5 0.03 200 / 40%);
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
		min-width: 0;
	}

	.whisper-attach {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		flex-shrink: 0;
		border-radius: 50%;
		color: oklch(0.58 0.05 200 / 58%);
		transition: all 0.3s ease;
		margin-bottom: 0.375rem;
	}
	.whisper-attach:hover {
		color: oklch(0.70 0.08 200 / 80%);
		background: oklch(0.55 0.06 200 / 12%);
	}
	.whisper-attach:disabled {
		opacity: 0.2;
		pointer-events: none;
	}

	.mode-toggle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		flex-shrink: 0;
		border-radius: 5px;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		color: oklch(0.58 0.05 200 / 62%);
		background: oklch(0.14 0.02 200 / 35%);
		backdrop-filter: blur(8px);
		-webkit-backdrop-filter: blur(8px);
		border: 1px solid oklch(0.5 0.06 200 / 16%);
		cursor: pointer;
		transition: all 0.2s;
		margin-bottom: 0.75rem;
	}
	.mode-toggle:hover {
		color: oklch(0.70 0.08 200 / 80%);
		background: oklch(0.14 0.02 200 / 50%);
	}
	.mode-toggle-fast {
		color: oklch(0.68 0.1 170 / 72%);
		background: oklch(0.12 0.02 170 / 35%);
		border-color: oklch(0.5 0.08 170 / 22%);
	}
	.mode-toggle-fast:hover {
		color: oklch(0.75 0.12 170 / 90%);
		background: oklch(0.12 0.02 170 / 50%);
	}
	.mode-toggle-heavy {
		color: oklch(0.63 0.08 250 / 72%);
		background: oklch(0.12 0.02 250 / 35%);
		border-color: oklch(0.5 0.06 250 / 22%);
	}
	.mode-toggle-heavy:hover {
		color: oklch(0.72 0.1 250 / 90%);
		background: oklch(0.12 0.02 250 / 50%);
	}

	.whisper-input {
		flex: 1;
		min-width: 0;
		min-height: 44px;
		max-height: 200px;
		overflow-y: hidden;
		resize: none;
		padding: 0.75rem 1rem;
		font-family: var(--font-body);
		font-size: 1rem;
		line-height: 1.6;
		color: oklch(0.94 0.01 75);
		background: oklch(0.14 0.01 230 / 90%);
		border: 1px solid oklch(0.45 0.03 220 / 45%);
		border-top-color: oklch(0.50 0.03 220 / 50%);
		border-radius: 14px;
		outline: none;
		transition: all 0.35s cubic-bezier(0.16, 1, 0.3, 1);
		letter-spacing: 0.01em;
		box-shadow:
			0 2px 12px oklch(0 0 0 / 30%),
			inset 0 1px 0 oklch(1 0 0 / 6%);
	}
	.whisper-input::placeholder {
		color: oklch(0.50 0.03 220 / 65%);
		font-style: italic;
		font-family: var(--font-display);
	}
	.whisper-input:focus {
		border-color: oklch(0.55 0.06 200 / 55%);
		background: oklch(0.16 0.015 220 / 95%);
		box-shadow:
			0 0 0 3px oklch(0.55 0.08 200 / 14%),
			0 4px 20px oklch(0 0 0 / 35%),
			inset 0 1px 0 oklch(1 0 0 / 6%);
	}
	.whisper-input:disabled {
		opacity: 0.35;
	}

	.whisper-send {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		flex-shrink: 0;
		border-radius: 50%;
		color: oklch(0.65 0.1 190 / 75%);
		transition: all 0.3s ease;
		animation: send-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
		margin-bottom: 0.375rem;
	}
	.whisper-send:hover {
		color: oklch(0.75 0.12 190 / 95%);
		background: oklch(0.55 0.08 200 / 14%);
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
		outline: 2px dashed oklch(0.6 0.1 200 / 35%);
		outline-offset: -2px;
		border-radius: 18px;
	}

	.whisper-dropzone {
		position: absolute;
		inset: 0;
		z-index: 20;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		border-radius: 18px;
		background: oklch(0.08 0.02 200 / 85%);
		backdrop-filter: blur(16px);
		-webkit-backdrop-filter: blur(16px);
		color: oklch(0.65 0.1 200 / 70%);
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

	/* upload progress */
	.upload-progress {
		position: relative;
		height: 18px;
		margin-top: 0.375rem;
		border-radius: 6px;
		background: oklch(0.12 0.02 200 / 20%);
		overflow: hidden;
	}
	.upload-progress-bar {
		position: absolute;
		inset: 0;
		background: oklch(0.55 0.08 200 / 18%);
		border-radius: 6px;
		transition: width 0.3s ease;
	}
	.upload-progress-label {
		position: relative;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.63 0.06 200 / 65%);
		letter-spacing: 0.04em;
	}

	@media (max-width: 720px) {
		.whisper-container {
			padding: 0.5rem 0.75rem calc(0.75rem + env(safe-area-inset-bottom, 0px));
		}
	}
</style>
