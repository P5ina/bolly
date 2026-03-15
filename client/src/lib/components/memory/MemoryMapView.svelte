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

	$effect(() => { load(); });

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

	// --- data ---

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

	// --- colors ---

	const folderColors: Record<string, string> = {
		about: "oklch(0.72 0.10 200)",
		facts: "oklch(0.75 0.12 140)",
		moments: "oklch(0.72 0.14 0)",
		preferences: "oklch(0.78 0.12 75)",
		projects: "oklch(0.70 0.10 260)",
		interests: "oklch(0.75 0.14 310)",
		people: "oklch(0.70 0.12 30)",
		emotions: "oklch(0.72 0.14 350)",
		knowledge: "oklch(0.70 0.10 170)",
		technical: "oklch(0.68 0.08 230)",
		"(root)": "oklch(0.65 0.06 240)",
	};

	function getColor(folder: string): string {
		if (folderColors[folder]) return folderColors[folder];
		let hash = 0;
		for (let i = 0; i < folder.length; i++) hash = folder.charCodeAt(i) + ((hash << 5) - hash);
		const hue = ((hash % 360) + 360) % 360;
		return `oklch(0.72 0.11 ${hue})`;
	}

	// --- circle packing (front-chain algorithm) ---

	interface Circle {
		x: number;
		y: number;
		r: number;
		id: string;
		label: string;
		color: string;
		size: number;
		fileCount?: number;
		entry?: MemoryEntry;
		folder?: FolderNode;
	}

	/**
	 * Place circles without overlap using a simple physics-based approach.
	 * Uses log scale for radii to prevent one huge circle from dominating.
	 */
	function packCircles(
		items: { id: string; weight: number }[],
		w: number,
		h: number,
		minR: number,
		maxR: number,
	): { id: string; x: number; y: number; r: number }[] {
		if (items.length === 0) return [];

		const maxWeight = Math.max(...items.map((i) => i.weight));
		const minWeight = Math.min(...items.map((i) => i.weight));
		const range = Math.max(maxWeight - minWeight, 1);

		// Map weights to radii using log scale — dampens extreme differences
		const circles = items.map((item) => {
			const norm = (item.weight - minWeight) / range; // 0..1
			const logNorm = Math.log1p(norm * 9) / Math.log(10); // log scale 0..1
			const r = minR + logNorm * (maxR - minR);
			return { id: item.id, x: w / 2, y: h / 2, r };
		});

		// Sort largest first for placement
		circles.sort((a, b) => b.r - a.r);

		// Place first at center
		circles[0].x = w / 2;
		circles[0].y = h / 2;

		const pad = 6;

		// Place each subsequent circle touching the closest placed circle
		for (let i = 1; i < circles.length; i++) {
			const c = circles[i];
			let bestX = w / 2;
			let bestY = h / 2;
			let bestScore = Infinity;

			// Try placing adjacent to each already-placed circle at various angles
			for (let j = 0; j < i; j++) {
				const ref = circles[j];
				const dist = ref.r + c.r + pad;

				for (let a = 0; a < Math.PI * 2; a += Math.PI / 12) {
					const tx = ref.x + Math.cos(a) * dist;
					const ty = ref.y + Math.sin(a) * dist;

					// Stay within bounds (with padding)
					if (tx - c.r < pad || tx + c.r > w - pad || ty - c.r < pad || ty + c.r > h - pad) continue;

					// Check overlaps
					let overlaps = false;
					for (let k = 0; k < i; k++) {
						const dx = tx - circles[k].x;
						const dy = ty - circles[k].y;
						const minD = c.r + circles[k].r + pad;
						if (dx * dx + dy * dy < minD * minD) {
							overlaps = true;
							break;
						}
					}

					if (!overlaps) {
						// Score: prefer positions closer to center
						const dcx = tx - w / 2;
						const dcy = ty - h / 2;
						const score = dcx * dcx + dcy * dcy;
						if (score < bestScore) {
							bestScore = score;
							bestX = tx;
							bestY = ty;
						}
					}
				}
			}

			c.x = bestX;
			c.y = bestY;
		}

		// Re-center the whole group
		let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
		for (const c of circles) {
			minX = Math.min(minX, c.x - c.r);
			maxX = Math.max(maxX, c.x + c.r);
			minY = Math.min(minY, c.y - c.r);
			maxY = Math.max(maxY, c.y + c.r);
		}
		const groupW = maxX - minX;
		const groupH = maxY - minY;
		const offsetX = (w - groupW) / 2 - minX;
		const offsetY = (h - groupH) / 2 - minY;
		for (const c of circles) {
			c.x += offsetX;
			c.y += offsetY;
		}

		return circles;
	}

	let svgH = $derived(containerHeight - 48);

	// Top-level folder bubbles
	let folderCircles = $derived.by((): Circle[] => {
		if (folders.length === 0) return [];
		const short = Math.min(containerWidth, svgH);
		const maxR = Math.min(short * 0.28, 140);
		const minR = Math.max(short * 0.06, 28);
		const items = folders.map((f) => ({ id: f.name, weight: f.totalSize }));
		const packed = packCircles(items, containerWidth, svgH, minR, maxR);
		return packed.map((p) => {
			const folder = folders.find((f) => f.name === p.id)!;
			return {
				...p,
				label: folder.name,
				color: getColor(folder.name),
				size: folder.totalSize,
				fileCount: folder.files.length,
				folder,
			};
		});
	});

	// File-level bubbles when drilled into a folder
	let fileCircles = $derived.by((): Circle[] => {
		if (!focusedFolder) return [];
		const folder = folders.find((f) => f.name === focusedFolder);
		if (!folder) return [];
		const short = Math.min(containerWidth, svgH);
		const maxR = Math.min(short * 0.18, 80);
		const minR = Math.max(short * 0.03, 16);
		const items = folder.files.map((f) => ({ id: f.path, weight: Math.max(f.size, 20) }));
		const packed = packCircles(items, containerWidth, svgH, minR, maxR);
		return packed.map((p) => {
			const entry = folder.files.find((f) => f.path === p.id)!;
			return {
				...p,
				label: entry.path.split("/").pop()?.replace(".md", "") ?? entry.path,
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
			hoveredNode = null;
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

	function truncLabel(label: string, r: number): string {
		const maxChars = Math.floor(r / 4.5);
		if (label.length <= maxChars) return label;
		return label.slice(0, Math.max(maxChars - 2, 3)) + "..";
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
			<svg
				width={containerWidth}
				height={svgH}
				viewBox="0 0 {containerWidth} {svgH}"
			>
				{#each activeCircles as circle (circle.id)}
					{@const isHovered = hoveredNode === circle.id}
					{@const isFolderView = !focusedFolder}
					{@const fontSize = Math.max(Math.min(circle.r * 0.3, 14), 9)}
					{@const subFontSize = Math.max(fontSize - 2, 8)}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<g
						class="memory-node"
						class:memory-node-clickable={isFolderView}
						onmouseenter={() => hoveredNode = circle.id}
						onmouseleave={() => hoveredNode = null}
						onclick={() => handleCircleClick(circle)}
					>
						<!-- Glow ring on hover -->
						{#if isHovered}
							<circle
								cx={circle.x}
								cy={circle.y}
								r={circle.r + 3}
								fill="none"
								stroke={circle.color}
								stroke-width="1"
								opacity="0.25"
							/>
						{/if}

						<!-- Main bubble -->
						<circle
							cx={circle.x}
							cy={circle.y}
							r={circle.r}
							fill={circle.color}
							fill-opacity={isHovered ? 0.16 : 0.08}
							stroke={circle.color}
							stroke-width={isHovered ? 1.5 : 1}
							stroke-opacity={isHovered ? 0.55 : 0.22}
						/>

						<!-- Label -->
						<text
							x={circle.x}
							y={circle.y - (circle.r > 40 ? fontSize * 0.45 : 0)}
							text-anchor="middle"
							dominant-baseline="central"
							fill={circle.color}
							fill-opacity={isHovered ? 0.95 : 0.75}
							font-size={fontSize}
							font-family="var(--font-mono)"
							letter-spacing="0.03em"
						>
							{truncLabel(circle.label, circle.r)}
						</text>

						<!-- Subtitle (file count / size) -->
						{#if circle.r > 40}
							<text
								x={circle.x}
								y={circle.y + fontSize * 0.7}
								text-anchor="middle"
								dominant-baseline="central"
								fill={circle.color}
								fill-opacity={isHovered ? 0.5 : 0.3}
								font-size={subFontSize}
								font-family="var(--font-mono)"
							>
								{isFolderView
									? `${circle.fileCount} files`
									: formatSize(circle.size)}
							</text>
						{/if}
					</g>
				{/each}
			</svg>
		</div>

		<!-- Tooltip -->
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
							{#each circle.folder.files.slice(0, 6) as file}
								<div class="memory-tooltip-file">{fileName(file.path)}</div>
							{/each}
							{#if circle.folder.files.length > 6}
								<div class="memory-tooltip-file memory-tooltip-more">+{circle.folder.files.length - 6} more</div>
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
		transition: opacity 0.15s ease;
	}

	.memory-node-clickable {
		cursor: pointer;
	}

	.memory-node circle {
		transition:
			fill-opacity 0.2s ease,
			stroke-opacity 0.2s ease,
			stroke-width 0.2s ease;
	}

	.memory-node text {
		transition: fill-opacity 0.2s ease;
		pointer-events: none;
		user-select: none;
	}

	/* Tooltip */

	.memory-tooltip {
		position: absolute;
		bottom: 1.25rem;
		left: 50%;
		transform: translateX(-50%);
		background: oklch(0.10 0.01 280 / 92%);
		backdrop-filter: blur(12px);
		border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 0.75rem;
		padding: 0.75rem 1rem;
		max-width: 320px;
		min-width: 180px;
		animation: tooltip-enter 0.12s ease;
		pointer-events: none;
		z-index: 10;
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
