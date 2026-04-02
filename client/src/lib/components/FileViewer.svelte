<script lang="ts">
	import { getViewerFile, closeFile } from "$lib/stores/fileviewer.svelte.js";

	const file = $derived(getViewerFile());

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Escape" && file) closeFile();
	}

	async function download() {
		if (!file) return;
		try {
			const res = await fetch(file.url);
			const blob = await res.blob();
			const blobUrl = URL.createObjectURL(blob);
			const a = document.createElement("a");
			a.href = blobUrl;
			a.download = file.name;
			document.body.appendChild(a);
			a.click();
			document.body.removeChild(a);
			setTimeout(() => URL.revokeObjectURL(blobUrl), 1000);
		} catch {
			window.open(file.url, "_blank");
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if file}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div class="viewer-overlay" onclick={closeFile}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="viewer-content" onclick={(e) => e.stopPropagation()}>
			{#if file.type === "image"}
				<img src={file.url} alt={file.name} class="viewer-img" />
			{:else if file.type === "video"}
				<!-- svelte-ignore a11y_media_has_caption -->
				<video src={file.url} controls autoplay class="viewer-media"></video>
			{:else if file.type === "audio"}
				<div class="viewer-audio-wrap">
					<div class="viewer-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M9 18V5l12-2v13" stroke-linecap="round" stroke-linejoin="round"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
					</div>
					<span class="viewer-label">{file.name}</span>
					<!-- svelte-ignore a11y_media_has_caption -->
					<audio src={file.url} controls autoplay class="viewer-audio"></audio>
				</div>
			{:else if file.type === "pdf"}
				<iframe src={file.url} title={file.name} class="viewer-pdf"></iframe>
			{:else}
				<div class="viewer-file-wrap">
					<div class="viewer-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<path d="M6 2h9l5 5v13a2 2 0 01-2 2H6a2 2 0 01-2-2V4a2 2 0 012-2z" stroke-linejoin="round"/>
							<path d="M14 2v6h6" stroke-linejoin="round"/>
						</svg>
					</div>
					<span class="viewer-label">{file.name}</span>
				</div>
			{/if}
		</div>

		<div class="viewer-toolbar" onclick={(e) => e.stopPropagation()}>
			<span class="viewer-name">{file.name}</span>
			<div class="viewer-actions">
				<button class="viewer-btn" onclick={download} title="Download">
					<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M10 3v10m0 0l-3.5-3.5M10 13l3.5-3.5M4 15.5v1h12v-1"/></svg>
				</button>
				<button class="viewer-btn" onclick={closeFile} title="Close">
					<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M5 5l10 10M15 5L5 15"/></svg>
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.viewer-overlay {
		position: fixed;
		inset: 0;
		z-index: 500;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.05 0.01 220 / 88%);
		backdrop-filter: blur(20px);
		-webkit-backdrop-filter: blur(20px);
		animation: viewer-fade 0.15s ease;
	}

	@keyframes viewer-fade {
		from { opacity: 0; }
	}

	.viewer-content {
		max-width: 90vw;
		max-height: 80vh;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.viewer-img {
		max-width: 90vw;
		max-height: 80vh;
		object-fit: contain;
		border-radius: 8px;
		box-shadow: 0 8px 40px oklch(0 0 0 / 50%);
		animation: viewer-zoom 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}

	@keyframes viewer-zoom {
		from { transform: scale(0.92); opacity: 0; }
	}

	.viewer-media {
		max-width: 90vw;
		max-height: 80vh;
		border-radius: 8px;
		box-shadow: 0 8px 40px oklch(0 0 0 / 50%);
		outline: none;
	}

	.viewer-audio-wrap,
	.viewer-file-wrap {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		padding: 2rem;
	}

	.viewer-icon {
		width: 48px;
		height: 48px;
		color: oklch(0.65 0.06 200);
	}

	.viewer-icon svg {
		width: 100%;
		height: 100%;
	}

	.viewer-label {
		font-family: var(--font-mono, monospace);
		font-size: 0.85rem;
		color: oklch(0.8 0.02 220);
	}

	.viewer-audio {
		width: 320px;
		max-width: 80vw;
	}

	.viewer-pdf {
		width: 85vw;
		height: 78vh;
		border: none;
		border-radius: 8px;
		background: white;
		box-shadow: 0 8px 40px oklch(0 0 0 / 50%);
	}

	.viewer-toolbar {
		position: fixed;
		bottom: 0;
		left: 0;
		right: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1.25rem;
		padding-bottom: calc(0.75rem + env(safe-area-inset-bottom, 0px));
		background: oklch(0.1 0.01 220 / 80%);
		backdrop-filter: blur(20px);
		-webkit-backdrop-filter: blur(20px);
		border-top: 1px solid oklch(1 0 0 / 8%);
	}

	.viewer-name {
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		color: oklch(0.7 0.02 220);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.viewer-actions {
		display: flex;
		gap: 0.5rem;
		flex-shrink: 0;
	}

	.viewer-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border: 1px solid oklch(1 0 0 / 12%);
		border-radius: 8px;
		background: oklch(1 0 0 / 6%);
		color: oklch(0.85 0.02 220);
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.viewer-btn:hover {
		background: oklch(1 0 0 / 14%);
		border-color: oklch(1 0 0 / 20%);
	}

	.viewer-btn svg {
		width: 18px;
		height: 18px;
	}
</style>
