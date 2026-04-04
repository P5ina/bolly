<script lang="ts">
	import { page } from "$app/stores";
	import { onMount } from "svelte";

	const slug = $derived($page.params.slug!);

	let imgSrc = $state("");
	let connected = $state(false);
	let lastUpdate = $state("");

	onMount(() => {
		let active = true;

		async function poll() {
			while (active) {
				try {
					const res = await fetch(`/api/instances/${slug}/live-frame`);
					if (res.ok && res.headers.get("content-type")?.includes("image/jpeg")) {
						const blob = await res.blob();
						const url = URL.createObjectURL(blob);
						if (imgSrc) URL.revokeObjectURL(imgSrc);
						imgSrc = url;
						connected = true;
						lastUpdate = new Date().toLocaleTimeString();
					} else if (res.status === 204) {
						connected = true; // connected but no frame yet
					} else {
						connected = false;
					}
				} catch {
					connected = false;
				}
				await new Promise(r => setTimeout(r, 1000));
			}
		}

		poll();
		return () => { active = false; };
	});
</script>

<div class="live-page">
	<div class="live-header">
		<div class="live-indicator" class:live-active={connected && imgSrc}>
			<div class="live-dot"></div>
			<span class="live-label">{connected ? "LIVE" : "OFFLINE"}</span>
		</div>
		{#if lastUpdate}
			<span class="live-time">{lastUpdate}</span>
		{/if}
	</div>

	<div class="live-feed">
		{#if imgSrc}
			<img class="live-img" src={imgSrc} alt="Live screen" />
		{:else if connected}
			<div class="live-waiting">
				<div class="live-pulse"></div>
				<p>waiting for first frame...</p>
			</div>
		{:else}
			<div class="live-offline">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" opacity="0.25">
					<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>
				</svg>
				<p>no desktop connected</p>
				<p class="live-hint">open the desktop app and enable screen recording</p>
			</div>
		{/if}
	</div>
</div>

<style>
	.live-page {
		height: 100%;
		display: flex;
		flex-direction: column;
		padding: 1.5rem;
	}

	.live-header {
		display: flex;
		align-items: center;
		gap: 1rem;
		margin-bottom: 1rem;
	}

	.live-indicator {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.25rem 0.6rem;
		border-radius: 1rem;
		background: oklch(var(--shade) / 6%);
		border: 1px solid oklch(var(--shade) / 8%);
	}

	.live-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(var(--ink) / 20%);
	}

	.live-active .live-dot {
		background: oklch(0.65 0.22 25);
		box-shadow: 0 0 6px oklch(0.65 0.22 25 / 60%);
		animation: pulse 1.5s ease-in-out infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.4; }
	}

	.live-label {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		letter-spacing: 0.1em;
		font-weight: 600;
		color: oklch(var(--ink) / 40%);
	}

	.live-active .live-label {
		color: oklch(0.65 0.22 25);
	}

	.live-time {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(var(--ink) / 20%);
	}

	.live-feed {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 0.75rem;
		overflow: hidden;
		background: oklch(var(--shade) / 4%);
		border: 1px solid oklch(var(--shade) / 6%);
	}

	.live-img {
		width: 100%;
		height: 100%;
		object-fit: contain;
	}

	.live-waiting, .live-offline {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
		color: oklch(var(--ink) / 25%);
		font-size: 0.75rem;
	}

	.live-pulse {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(var(--ink) / 15%);
		animation: pulse 2s ease-in-out infinite;
	}

	.live-hint {
		font-size: 0.65rem;
		color: oklch(var(--ink) / 15%);
		margin: 0;
	}

	.live-offline p {
		margin: 0;
	}
</style>
