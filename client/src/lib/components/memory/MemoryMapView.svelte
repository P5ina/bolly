<script lang="ts">
	import { fetchMemory } from "$lib/api/client.js";
	import type { MemoryEntry } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();

	let { slug }: { slug: string } = $props();

	let entries = $state<MemoryEntry[]>([]);
	let loading = $state(true);
	let focusedFolder = $state<string | null>(null);
	let hoveredNode = $state<string | null>(null);
	let containerEl = $state<HTMLDivElement | null>(null);
	let containerWidth = $state(600);
	let containerHeight = $state(500);

	async function load() {
		loading = true;
		try {
			entries = await fetchMemory(slug);
		} catch {
			toast.error("failed to load memory");
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();
	});

	$effect(() => {
		if (!containerEl) return;
		const ro = new ResizeObserver((es) => {
			const e = es[0];
			if (e) {
				containerWidth = e.contentRect.width;
				containerHeight = e.contentRect.height;
			}
		});
		ro.observe(containerEl);
		return () => ro.disconnect();
	});

	// --- data transforms ---

	interface FolderNode {
		name: string;
		files: MemoryEntry[];
		totalSize: number;
	}

	let folders = $derived.by(() => {
		const map = new Map<string, MemoryEntry[]>();
		for (const e of entries) {
			const slash = e.path.indexOf("/");
			const folder = slash !== -1 ? e.path.substring(0, slash) : "(root)";
			if (!map.has(folder)) map.set(folder, []);
			map.get(folder)!.push(e);
		}
		const result: FolderNode[] = [];
		for (const [name, files] of map) {
			result.push({ name, files, totalSize: files.reduce((s, f) => s + f.size, 0) });
		}
		result.sort((a, b) => b.totalSize - a.totalSize);
		return result;
	});

	let totalSize = $derived(folders.reduce((s, f) => s + f.totalSize, 0));

	// --- folder colors ---

	const folderColors: Record<string, string> = {
		about: "oklch(0.72 0.10 200)",
		facts: "oklch(0.75 0.12 140)",
		moments: "oklch(0.72 0.14 0)",
		preferences: "oklch(0.78 0.12 75)",
		projects: "oklch(0.70 0.10 260)",
		interests: "oklch(0.75 0.14 310)",
		people: "oklch(0.70 0.12 30)",
		emotions: "oklch(0.72 0.14 350)",
		"(root)": "oklch(0.65 0.06 240)",
	};

	function getColor(folder: string): string {
		if (folderColors[folder]) return folderColors[folder];
		// Generate a deterministic color from folder name
		let hash = 0;
		for (let i = 0; i < folder.length; i++) {
			hash = folder.charCodeAt(i) + ((hash << 5) - hash);
		}
		const hue = ((hash % 360) + 360) % 360;
		return `oklch(0.72 0.11 ${hue})`;
	}

	// --- circle packing layout ---

	interface Circle {
		x: number;
		y: number;
		r: number;
		id: string;
		label: string;
		color: string;
		size: number;
		entry?: MemoryEntry;
		folder?: FolderNode;
	}

	function packCircles(
		items: { id: string; size: number }[],
		cx: number,
		cy: number,
		containerR: number,
	): { id: string; x: number; y: number; r: number }[] {
		if (items.length === 0) return [];
		const totalSize = items.reduce((s, i) => s + i.size, 0);
		if (totalSize === 0) return items.map((i) => ({ id: i.id, x: cx, y: cy, r: 8 }));

		// Compute radii proportional to sqrt of size (area-based)
		const minR = 18;
		const maxUsableArea = Math.PI * containerR * containerR * 0.72;
		const scale = maxUsableArea / totalSize;
		const circles = items.map((item) => {
			const area = item.size * scale;
			const r = Math.max(minR, Math.sqrt(area / Math.PI));
			return { id: item.id, x: cx, y: cy, r, placed: false };
		});

		// Sort largest first
		circles.sort((a, b) => b.r - a.r);

		// Place first circle at center
		circles[0].placed = true;

		// Greedy placement — spiral outward
		for (let i = 1; i < circles.length; i++) {
			const c = circles[i];
			let bestDist = Infinity;
			let bestX = cx;
			let bestY = cy;

			// Try positions in a spiral
			for (let angle = 0; angle < Math.PI * 8; angle += 0.15) {
				for (let dist = 0; dist < containerR; dist += 3) {
					const tx = cx + Math.cos(angle) * dist;
					const ty = cy + Math.sin(angle) * dist;

					// Check if fits within container
					if (Math.sqrt((tx - cx) ** 2 + (ty - cy) ** 2) + c.r > containerR - 2) continue;

					// Check overlap with placed circles
					let overlaps = false;
					for (let j = 0; j < i; j++) {
						if (!circles[j].placed) continue;
						const dx = tx - circles[j].x;
						const dy = ty - circles[j].y;
						const minDist = c.r + circles[j].r + 3;
						if (dx * dx + dy * dy < minDist * minDist) {
							overlaps = true;
							break;
						}
					}

					if (!overlaps) {
						const d = Math.sqrt((tx - cx) ** 2 + (ty - cy) ** 2);
						if (d < bestDist) {
							bestDist = d;
							bestX = tx;
							bestY = ty;
						}
						// Found a close enough spot, break inner loop
						if (d < c.r * 2) break;
					}
				}
				if (bestDist < circles[i].r * 2) break;
			}

			c.x = bestX;
			c.y = bestY;
			c.placed = true;
		}

		return circles.map(({ id, x, y, r }) => ({ id, x, y, r }));
	}

	// Top-level folder bubbles
	let folderCircles = $derived.by((): Circle[] => {
		if (folders.length === 0) return [];
		const r = Math.min(containerWidth, containerHeight) / 2 - 10;
		const cx = containerWidth / 2;
		const cy = containerHeight / 2;
		const items = folders.map((f) => ({ id: f.name, size: f.totalSize }));
		const packed = packCircles(items, cx, cy, r);
		return packed.map((p) => {
			const folder = folders.find((f) => f.name === p.id)!;
			return {
				...p,
				label: folder.name,
				color: getColor(folder.name),
				size: folder.totalSize,
				folder,
			};
		});
	});

	// When focused on a folder, show its files
	let fileCircles = $derived.by((): Circle[] => {
		if (!focusedFolder) return [];
		const folder = folders.find((f) => f.name === focusedFolder);
		if (!folder) return [];
		const r = Math.min(containerWidth, containerHeight) / 2 - 30;
		const cx = containerWidth / 2;
		const cy = containerHeight / 2;
		const items = folder.files.map((f) => ({ id: f.path, size: Math.max(f.size, 40) }));
		const packed = packCircles(items, cx, cy, r);
		return packed.map((p) => {
			const entry = folder.files.find((f) => f.path === p.id)!;
			const fileName = entry.path.split("/").pop()?.replace(".md", "") ?? entry.path;
			return {
				...p,
				label: fileName,
				color: getColor(folder.name),
				size: entry.size,
				entry,
			};
		});
	});

	let activeCircles = $derived(focusedFolder ? fileCircles : folderCircles);

	function handleCircleClick(circle: Circle) {
		if (!focusedFolder && circle.folder) {
			focusedFolder = circle.folder.name;
		}
	}

	function handleBack() {
		focusedFolder = null;
		hoveredNode = null;
	}

	function fileName(path: string): string {
		return path.split("/").pop()?.replace(".md", "") ?? path;
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		return `${(bytes / 1024).toFixed(1)} KB`;
	}
</script>

<div class="memory-container" bind:this={containerEl}>
	{#if loading}
		<div class="memory-loading">
			<div class="memory-loading-dot"></div>
		</div>
	{:else if entries.length === 0}
		<div class="memory-empty">
			<div class="memory-empty-icon">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" width="32" height="32">
					<circle cx="12" cy="12" r="10" opacity="0.3" />
					<circle cx="12" cy="12" r="4" opacity="0.5" />
					<circle cx="6" cy="8" r="2" opacity="0.2" />
					<circle cx="18" cy="16" r="2.5" opacity="0.2" />
				</svg>
			</div>
			<p class="memory-empty-text">no memories yet</p>
			<p class="memory-empty-hint">
				memories form as you talk — your companion learns and remembers.
			</p>
		</div>
	{:else}
		<div class="memory-header">
			{#if focusedFolder}
				<button class="memory-back" onclick={handleBack}>
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
						<path d="M19 12H5m0 0l7 7m-7-7l7-7" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					back
				</button>
				<span class="memory-breadcrumb">{focusedFolder}/</span>
				<span class="memory-count">{folders.find(f => f.name === focusedFolder)?.files.length ?? 0} memories</span>
			{:else}
				<span class="memory-count">{entries.length} memories in {folders.length} folders · {formatSize(totalSize)}</span>
			{/if}
		</div>

		<div class="memory-map">
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<svg
				width={containerWidth}
				height={containerHeight - 48}
				viewBox="0 0 {containerWidth} {containerHeight - 48}"
			>
				{#each activeCircles as circle (circle.id)}
					{@const isHovered = hoveredNode === circle.id}
					{@const isFolderView = !focusedFolder}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<g
						class="memory-node"
						class:memory-node-clickable={isFolderView}
						onmouseenter={() => hoveredNode = circle.id}
						onmouseleave={() => hoveredNode = null}
						onclick={() => handleCircleClick(circle)}
					>
						<!-- Glow -->
						{#if isHovered}
							<circle
								cx={circle.x}
								cy={circle.y}
								r={circle.r + 4}
								fill="none"
								stroke={circle.color}
								stroke-width="1"
								opacity="0.2"
							/>
						{/if}

						<!-- Main bubble -->
						<circle
							cx={circle.x}
							cy={circle.y}
							r={circle.r}
							fill={circle.color}
							fill-opacity={isHovered ? 0.18 : 0.10}
							stroke={circle.color}
							stroke-width={isHovered ? 1.5 : 1}
							stroke-opacity={isHovered ? 0.5 : 0.25}
						/>

						<!-- Inner glow dot -->
						<circle
							cx={circle.x}
							cy={circle.y - circle.r * 0.15}
							r={circle.r * 0.12}
							fill={circle.color}
							fill-opacity="0.25"
						/>

						<!-- Label -->
						{#if circle.r > 22}
							<text
								x={circle.x}
								y={circle.y - (circle.r > 45 ? 6 : 0)}
								text-anchor="middle"
								dominant-baseline="central"
								fill={circle.color}
								fill-opacity={isHovered ? 0.9 : 0.7}
								font-size={Math.min(circle.r * 0.35, 13)}
								font-family="var(--font-mono)"
								letter-spacing="0.03em"
							>
								{circle.label.length > 16 ? circle.label.slice(0, 14) + '..' : circle.label}
							</text>

							<!-- Size / count subtitle -->
							{#if circle.r > 45}
								<text
									x={circle.x}
									y={circle.y + 10}
									text-anchor="middle"
									dominant-baseline="central"
									fill={circle.color}
									fill-opacity="0.35"
									font-size={Math.min(circle.r * 0.22, 10)}
									font-family="var(--font-mono)"
								>
									{isFolderView
										? `${circle.folder?.files.length} files`
										: formatSize(circle.size)}
								</text>
							{/if}
						{/if}
					</g>
				{/each}
			</svg>
		</div>

		<!-- Tooltip / detail panel -->
		{#if hoveredNode}
			{@const circle = activeCircles.find(c => c.id === hoveredNode)}
			{#if circle}
				<div class="memory-tooltip">
					<div class="memory-tooltip-name" style="color: {circle.color}">
						{circle.label}
					</div>
					{#if circle.entry}
						<div class="memory-tooltip-summary">{circle.entry.summary}</div>
						<div class="memory-tooltip-size">{formatSize(circle.entry.size)}</div>
					{:else if circle.folder}
						<div class="memory-tooltip-summary">{circle.folder.files.length} memories · {formatSize(circle.folder.totalSize)}</div>
						<div class="memory-tooltip-files">
							{#each circle.folder.files.slice(0, 5) as file}
								<div class="memory-tooltip-file">{fileName(file.path)}</div>
							{/each}
							{#if circle.folder.files.length > 5}
								<div class="memory-tooltip-file memory-tooltip-more">+{circle.folder.files.length - 5} more</div>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		{/if}
	{/if}
</div>

<style>
	.memory-container {
		height: 100%;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		position: relative;
	}

	.memory-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.memory-loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	.memory-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 0.75rem;
		text-align: center;
	}

	.memory-empty-icon {
		color: oklch(0.78 0.12 75 / 20%);
		animation: pulse-alive 3s ease-in-out infinite;
	}

	.memory-empty-text {
		font-family: var(--font-display);
		font-size: 0.95rem;
		font-style: italic;
		color: oklch(0.78 0.12 75 / 45%);
	}

	.memory-empty-hint {
		font-size: 0.72rem;
		color: oklch(0.78 0.12 75 / 22%);
		max-width: 28ch;
		line-height: 1.5;
	}

	/* Header */

	.memory-header {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 1rem 1.5rem 0;
		flex-shrink: 0;
	}

	.memory-back {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.78 0.12 75 / 40%);
		background: none;
		border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 0.375rem;
		padding: 0.25rem 0.5rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.memory-back:hover {
		color: oklch(0.78 0.12 75 / 70%);
		border-color: oklch(1 0 0 / 12%);
	}

	.memory-breadcrumb {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 55%);
	}

	.memory-count {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.78 0.12 75 / 25%);
		letter-spacing: 0.04em;
		margin-left: auto;
	}

	/* Map */

	.memory-map {
		flex: 1;
		min-height: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.memory-map svg {
		display: block;
	}

	.memory-node {
		transition: opacity 0.2s ease;
	}

	.memory-node-clickable {
		cursor: pointer;
	}

	.memory-node circle {
		transition:
			fill-opacity 0.25s ease,
			stroke-opacity 0.25s ease,
			stroke-width 0.25s ease;
	}

	.memory-node text {
		transition: fill-opacity 0.25s ease;
		pointer-events: none;
		user-select: none;
	}

	/* Tooltip */

	.memory-tooltip {
		position: absolute;
		bottom: 1.5rem;
		left: 50%;
		transform: translateX(-50%);
		background: oklch(0.10 0.01 280 / 90%);
		backdrop-filter: blur(12px);
		border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 0.75rem;
		padding: 0.75rem 1rem;
		max-width: 320px;
		min-width: 180px;
		animation: tooltip-enter 0.15s ease;
		pointer-events: none;
	}

	@keyframes tooltip-enter {
		from { opacity: 0; transform: translateX(-50%) translateY(4px); }
		to { opacity: 1; transform: translateX(-50%) translateY(0); }
	}

	.memory-tooltip-name {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		font-weight: 500;
		letter-spacing: 0.03em;
		margin-bottom: 0.3rem;
	}

	.memory-tooltip-summary {
		font-family: var(--font-body);
		font-size: 0.68rem;
		color: oklch(0.88 0.02 75 / 55%);
		line-height: 1.4;
	}

	.memory-tooltip-size {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.78 0.12 75 / 25%);
		margin-top: 0.25rem;
	}

	.memory-tooltip-files {
		margin-top: 0.35rem;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	.memory-tooltip-file {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.88 0.02 75 / 35%);
	}

	.memory-tooltip-more {
		color: oklch(0.78 0.12 75 / 22%);
		font-style: italic;
	}

	@media (max-width: 640px) {
		.memory-header {
			padding: 0.75rem 1rem 0;
		}
		.memory-tooltip {
			left: 1rem;
			right: 1rem;
			transform: none;
			max-width: none;
		}
		@keyframes tooltip-enter {
			from { opacity: 0; transform: translateY(4px); }
			to { opacity: 1; transform: translateY(0); }
		}
	}
</style>
