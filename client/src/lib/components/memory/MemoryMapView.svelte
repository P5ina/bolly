<script lang="ts">
	import { fetchMemory, fetchMemoryContent, searchMemory, type MemorySearchResult } from "$lib/api/client.js";
	import type { MemoryEntry } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();

	let { slug }: { slug: string } = $props();

	let entries = $state<MemoryEntry[]>([]);
	let loading = $state(true);
	let focusedFolder = $state<string | null>(null);
	let hoveredNode = $state<string | null>(null);
	let containerEl = $state<HTMLDivElement | null>(null);
	let canvasEl = $state<HTMLCanvasElement | null>(null);
	let containerWidth = $state(600);
	let containerHeight = $state(500);
	let viewKey = $state(0);

	// Search state
	let searchQuery = $state("");
	let searchOpen = $state(false);

	// Document viewer state
	let viewingEntry = $state<MemoryEntry | null>(null);
	let viewingContent = $state<string>("");
	let viewingLoading = $state(false);

	// Pan state
	let panX = $state(0);
	let panY = $state(0);
	let isPanning = $state(false);
	let panStartX = 0;
	let panStartY = 0;
	let panStartPanX = 0;
	let panStartPanY = 0;

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

	// --- particle canvas ---
	$effect(() => {
		if (!canvasEl || loading) return;
		const ctx = canvasEl.getContext("2d");
		if (!ctx) return;

		const dpr = window.devicePixelRatio || 1;
		let w = containerWidth;
		let h = containerHeight;
		canvasEl.width = w * dpr;
		canvasEl.height = h * dpr;
		ctx.scale(dpr, dpr);

		const PARTICLE_COUNT = 70;
		const CONNECTION_DIST = 120;

		interface Particle {
			x: number; y: number;
			vx: number; vy: number;
			r: number; opacity: number;
			baseOpacity: number;
		}

		const particles: Particle[] = Array.from({ length: PARTICLE_COUNT }, () => {
			const baseOp = Math.random() * 0.25 + 0.08;
			return {
				x: Math.random() * w, y: Math.random() * h,
				vx: (Math.random() - 0.5) * 0.25,
				vy: (Math.random() - 0.5) * 0.25,
				r: Math.random() * 1.2 + 0.4,
				opacity: baseOp, baseOpacity: baseOp,
			};
		});

		let raf: number;
		let time = 0;

		function animate() {
			time += 0.016;
			ctx!.clearRect(0, 0, w, h);

			for (const p of particles) {
				p.x += p.vx;
				p.y += p.vy;
				p.x += Math.sin(time * 0.5 + p.y * 0.01) * 0.08;
				p.y += Math.cos(time * 0.4 + p.x * 0.01) * 0.06;
				if (p.x < -10) p.x = w + 10;
				if (p.x > w + 10) p.x = -10;
				if (p.y < -10) p.y = h + 10;
				if (p.y > h + 10) p.y = -10;
				p.opacity = p.baseOpacity + Math.sin(time * 2 + p.x * 0.05) * 0.08;
			}

			for (let i = 0; i < particles.length; i++) {
				for (let j = i + 1; j < particles.length; j++) {
					const a = particles[i], b = particles[j];
					const dx = a.x - b.x, dy = a.y - b.y;
					const dist = Math.sqrt(dx * dx + dy * dy);
					if (dist < CONNECTION_DIST) {
						const alpha = (1 - dist / CONNECTION_DIST) * 0.06;
						ctx!.beginPath();
						ctx!.moveTo(a.x, a.y);
						ctx!.lineTo(b.x, b.y);
						ctx!.strokeStyle = `rgba(210, 180, 120, ${alpha})`;
						ctx!.lineWidth = 0.5;
						ctx!.stroke();
					}
				}
			}

			for (const p of particles) {
				ctx!.beginPath();
				ctx!.arc(p.x, p.y, p.r, 0, Math.PI * 2);
				ctx!.fillStyle = `rgba(210, 180, 120, ${Math.max(0, p.opacity)})`;
				ctx!.fill();
			}

			raf = requestAnimationFrame(animate);
		}

		animate();
		return () => cancelAnimationFrame(raf);
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

	// --- search (server-side BM25) ---

	let searchResults = $state<MemorySearchResult[]>([]);
	let searchLoading = $state(false);
	let searchDebounce: ReturnType<typeof setTimeout> | null = null;

	$effect(() => {
		const q = searchQuery.trim();
		if (q.length < 2) {
			searchResults = [];
			return;
		}
		searchLoading = true;
		if (searchDebounce) clearTimeout(searchDebounce);
		searchDebounce = setTimeout(async () => {
			try {
				searchResults = await searchMemory(slug, q, 20);
			} catch {
				searchResults = [];
			} finally {
				searchLoading = false;
			}
		}, 250);
	});

	function toggleSearch() {
		searchOpen = !searchOpen;
		searchQuery = "";
		searchResults = [];
	}

	// --- colors ---

	const folderHex: Record<string, string> = {
		about: "#5ba8d4", facts: "#6bc47a", moments: "#d46b6b",
		preferences: "#d4a55a", projects: "#7b7bd4", interests: "#c475d4",
		people: "#d49060", emotions: "#d46ba0", knowledge: "#5ab8a0",
		technical: "#7090c0", "(root)": "#8888a0",
	};

	function getHex(folder: string): string {
		if (folderHex[folder]) return folderHex[folder];
		let hash = 0;
		for (let i = 0; i < folder.length; i++) hash = folder.charCodeAt(i) + ((hash << 5) - hash);
		return `hsl(${((hash % 360) + 360) % 360}, 50%, 62%)`;
	}

	// --- circle packing ---

	interface Circle {
		x: number; y: number; r: number;
		id: string; label: string; hex: string;
		size: number; fileCount?: number;
		entry?: MemoryEntry; folder?: FolderNode;
		floatSeed: number;
	}

	function packCircles(
		items: { id: string; weight: number }[],
		w: number, h: number,
	): { id: string; x: number; y: number; r: number }[] {
		if (items.length === 0) return [];
		if (items.length === 1) {
			const r = Math.min(w, h) * 0.25;
			return [{ id: items[0].id, x: w / 2, y: h / 2, r }];
		}

		const maxWeight = Math.max(...items.map((i) => i.weight));
		const minWeight = Math.min(...items.map((i) => i.weight));
		const range = Math.max(maxWeight - minWeight, 1);

		const normed = items.map((item) => {
			const norm = (item.weight - minWeight) / range;
			return { id: item.id, logW: Math.log1p(norm * 9) / Math.log(10) };
		});

		const targetArea = w * h * 0.40;
		const sumLogSq = normed.reduce((s, n) => s + (0.35 + 0.65 * n.logW) ** 2, 0);
		const scale = Math.sqrt(targetArea / (Math.PI * sumLogSq));
		const minR = 28;
		const maxR = Math.min(w, h) * 0.22;

		const circles = normed.map((n) => {
			const r = Math.max(minR, Math.min(maxR, (0.35 + 0.65 * n.logW) * scale));
			return { id: n.id, x: 0, y: 0, r };
		});

		circles.sort((a, b) => b.r - a.r);

		const pad = 14;
		circles[0].x = w / 2;
		circles[0].y = h / 2;

		for (let i = 1; i < circles.length; i++) {
			const c = circles[i];
			let bestX = w / 2;
			let bestY = h / 2;
			let bestDist = Infinity;

			for (let j = 0; j < i; j++) {
				const ref = circles[j];
				const touchDist = ref.r + c.r + pad;

				for (let ai = 0; ai < 36; ai++) {
					const a = (ai / 36) * Math.PI * 2;
					const tx = ref.x + Math.cos(a) * touchDist;
					const ty = ref.y + Math.sin(a) * touchDist;

					// Clamp within viewport with margin
					const margin = c.r + 16;
					if (tx < margin || tx > w - margin || ty < margin || ty > h - margin) continue;

					let ok = true;
					for (let k = 0; k < i; k++) {
						if (k === j) continue;
						const dx = tx - circles[k].x;
						const dy = ty - circles[k].y;
						const need = c.r + circles[k].r + pad;
						if (dx * dx + dy * dy < need * need) { ok = false; break; }
					}
					if (!ok) continue;

					const dx = tx - w / 2;
					const dy = ty - h / 2;
					const d = dx * dx + dy * dy;
					if (d < bestDist) { bestDist = d; bestX = tx; bestY = ty; }
				}
			}

			c.x = bestX;
			c.y = bestY;
		}

		// Re-center within viewport
		let bx0 = Infinity, bx1 = -Infinity, by0 = Infinity, by1 = -Infinity;
		for (const c of circles) {
			bx0 = Math.min(bx0, c.x - c.r);
			bx1 = Math.max(bx1, c.x + c.r);
			by0 = Math.min(by0, c.y - c.r);
			by1 = Math.max(by1, c.y + c.r);
		}
		const ox = (w - (bx1 - bx0)) / 2 - bx0;
		const oy = (h - (by1 - by0)) / 2 - by0;
		for (const c of circles) { c.x += ox; c.y += oy; }

		return circles;
	}

	let mapH = $derived(containerHeight - 44);

	let folderCircles = $derived.by((): Circle[] => {
		if (folders.length === 0) return [];
		const items = folders.map((f) => ({ id: f.name, weight: f.totalSize }));
		const packed = packCircles(items, containerWidth, mapH);
		return packed.map((p, i) => {
			const folder = folders.find((f) => f.name === p.id)!;
			return {
				...p, label: folder.name, hex: getHex(folder.name),
				size: folder.totalSize, fileCount: folder.files.length,
				folder, floatSeed: i * 1.7,
			};
		});
	});

	let fileCircles = $derived.by((): Circle[] => {
		if (!focusedFolder) return [];
		const folder = folders.find((f) => f.name === focusedFolder);
		if (!folder) return [];
		const items = folder.files.map((f) => ({ id: f.path, weight: Math.max(f.size, 20) }));
		const packed = packCircles(items, containerWidth, mapH);
		return packed.map((p, i) => {
			const entry = folder.files.find((f) => f.path === p.id)!;
			return {
				...p,
				label: entry.path.split("/").pop()?.replace(".md", "") ?? entry.path,
				hex: getHex(folder.name), size: entry.size, entry,
				floatSeed: i * 1.3,
			};
		});
	});

	let activeCircles = $derived(focusedFolder ? fileCircles : folderCircles);

	function resetView() {
		panX = 0; panY = 0; zoom = 1;
	}

	function handleCircleClick(circle: Circle) {
		if (!focusedFolder && circle.folder) {
			focusedFolder = circle.folder.name;
			hoveredNode = null;
			resetView();
			viewKey++;
		} else if (focusedFolder && circle.entry) {
			openDocument(circle.entry);
		}
	}

	function handleBack() {
		if (viewingEntry) {
			viewingEntry = null;
			viewingContent = "";
		} else {
			focusedFolder = null;
			hoveredNode = null;
			resetView();
			viewKey++;
		}
	}

	async function openDocument(entry: MemoryEntry) {
		viewingEntry = entry;
		viewingLoading = true;
		try {
			viewingContent = await fetchMemoryContent(slug, entry.path);
		} catch {
			viewingContent = "(failed to load)";
		} finally {
			viewingLoading = false;
		}
	}

	function fileName(path: string): string {
		return path.split("/").pop()?.replace(".md", "") ?? path;
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		return `${(bytes / 1024).toFixed(1)} KB`;
	}

	function truncLabel(label: string, r: number): string {
		const maxChars = Math.floor(r / 4.2);
		if (maxChars < 4) return "";
		if (label.length <= maxChars) return label;
		return label.slice(0, Math.max(maxChars - 2, 3)) + "..";
	}

	// Zoom
	let zoom = $state(1);
	const ZOOM_MIN = 0.3;
	const ZOOM_MAX = 3;

	function onWheel(e: WheelEvent) {
		e.preventDefault();
		const delta = e.deltaY > 0 ? 0.9 : 1.1;
		const newZoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom * delta));

		// Zoom toward cursor position
		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const cx = e.clientX - rect.left;
		const cy = e.clientY - rect.top;
		panX = cx - (cx - panX) * (newZoom / zoom);
		panY = cy - (cy - panY) * (newZoom / zoom);
		zoom = newZoom;
	}

	// Pan: mousedown on empty area starts drag
	function onMapMouseDown(e: MouseEvent) {
		if ((e.target as HTMLElement).closest(".bubble")) return;
		if (e.button !== 0) return;
		isPanning = true;
		panStartX = e.clientX;
		panStartY = e.clientY;
		panStartPanX = panX;
		panStartPanY = panY;

		const onMove = (me: MouseEvent) => {
			panX = panStartPanX + (me.clientX - panStartX);
			panY = panStartPanY + (me.clientY - panStartY);
		};
		const onUp = () => {
			isPanning = false;
			window.removeEventListener("mousemove", onMove);
			window.removeEventListener("mouseup", onUp);
		};
		window.addEventListener("mousemove", onMove);
		window.addEventListener("mouseup", onUp);
	}
</script>

<div class="memory-container" bind:this={containerEl}>
	<canvas
		class="particle-canvas"
		bind:this={canvasEl}
		style="width: {containerWidth}px; height: {containerHeight}px"
	></canvas>

	{#if loading}
		<div class="memory-loading">
			<div class="memory-loading-orb">
				<div class="memory-loading-ring"></div>
				<div class="memory-loading-ring memory-loading-ring-2"></div>
				<div class="memory-loading-dot"></div>
			</div>
		</div>
	{:else if entries.length === 0}
		<div class="memory-empty">
			<div class="memory-empty-orbs">
				<div class="empty-orb empty-orb-1"></div>
				<div class="empty-orb empty-orb-2"></div>
				<div class="empty-orb empty-orb-3"></div>
			</div>
			<p class="memory-empty-text">no memories yet</p>
			<p class="memory-empty-hint">memories form as you talk — your companion learns and remembers.</p>
		</div>
	{:else if viewingEntry}
		<!-- Document viewer -->
		<div class="memory-header">
			<button class="memory-back" onclick={handleBack}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
					<path d="M19 12H5m0 0l7 7m-7-7l7-7" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
			<span class="memory-breadcrumb">{viewingEntry.path}</span>
			<span class="memory-count">{formatSize(viewingEntry.size)}</span>
		</div>
		<div class="doc-viewer">
			{#if viewingLoading}
				<div class="doc-loading">loading...</div>
			{:else}
				<pre class="doc-content">{viewingContent}</pre>
			{/if}
		</div>
	{:else}
		<!-- Map view -->
		<div class="memory-header">
			{#if focusedFolder}
				<button class="memory-back" onclick={handleBack}>
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
						<path d="M19 12H5m0 0l7 7m-7-7l7-7" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</button>
				<span class="memory-breadcrumb">{focusedFolder}/</span>
				<span class="memory-count">{folders.find(f => f.name === focusedFolder)?.files.length ?? 0} memories</span>
			{:else}
				<span class="memory-count">{entries.length} memories · {folders.length} folders · {formatSize(totalSize)}</span>
			{/if}
			<button class="search-toggle" class:search-toggle-active={searchOpen} onclick={toggleSearch} title="search memories">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
					<circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" stroke-linecap="round"/>
				</svg>
			</button>
		</div>

		{#if searchOpen}
			<div class="search-bar">
				<input
					class="search-input"
					type="text"
					placeholder="search memories..."
					bind:value={searchQuery}
					autofocus
					onkeydown={(e) => e.key === "Escape" && toggleSearch()}
				/>
				{#if searchQuery.length >= 2}
					<span class="search-count">{searchLoading ? "searching..." : `${searchResults.length} results`}</span>
				{/if}
			</div>

			{#if searchResults.length > 0}
				<div class="search-results">
					{#each searchResults as result}
						{@const basePath = result.path.split("#")[0]}
						{@const folder = basePath.split("/")[0] ?? "(root)"}
						{@const preview = result.text.trim().slice(0, 200)}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="search-result"
							style="--c: {getHex(folder)}"
							onclick={() => {
								const entry = entries.find(e => e.path === basePath);
								if (entry) openDocument(entry);
							}}
						>
							<div class="search-result-path">
								<span class="search-result-folder" style="color: {getHex(folder)}">{folder}/</span>{fileName(basePath)}
							</div>
							<div class="search-result-summary">{preview}</div>
							<div class="search-result-meta">score: {result.score.toFixed(1)}</div>
						</div>
					{/each}
				</div>
			{:else if searchQuery.length >= 2 && !searchLoading}
				<div class="search-empty">no matches</div>
			{/if}
		{/if}

		{#if !searchOpen}
		{#key viewKey}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="memory-map"
				style="height: {mapH}px"
				onmousedown={onMapMouseDown}
				onwheel={onWheel}
			>
				<div class="memory-map-inner" style="transform: translate({panX}px, {panY}px) scale({zoom}); transform-origin: 0 0">
					{#each activeCircles as circle, i (circle.id)}
						{@const isHovered = hoveredNode === circle.id}
						{@const isFolderView = !focusedFolder}
						{@const diameter = circle.r * 2}
						{@const showLabel = circle.r > 20}
						{@const showSub = circle.r > 42}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="bubble-anchor"
							style="
								left: {circle.x}px;
								top: {circle.y}px;
								animation-delay: {i * 60 + 50}ms;
							"
						>
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="bubble bubble-clickable"
								class:bubble-hovered={isHovered}
								style="
									width: {diameter}px;
									height: {diameter}px;
									--c: {circle.hex};
									--float-x: {Math.sin(circle.floatSeed) * 5}px;
									--float-y: {Math.cos(circle.floatSeed * 0.7) * 6}px;
									--float-dur: {5 + circle.floatSeed % 3}s;
									--float-delay: {circle.floatSeed * -0.4}s;
								"
								onmouseenter={() => hoveredNode = circle.id}
								onmouseleave={() => hoveredNode = null}
								onclick={() => handleCircleClick(circle)}
							>
								<div class="bubble-core"></div>
								<div class="bubble-shine"></div>
								{#if isHovered}
									<div class="bubble-ring"></div>
								{/if}
								{#if showLabel}
									<span class="bubble-label">{truncLabel(circle.label, circle.r)}</span>
									{#if showSub}
										<span class="bubble-sub">
											{isFolderView ? `${circle.fileCount} files` : formatSize(circle.size)}
										</span>
									{/if}
								{/if}
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/key}

		<!-- Tooltip -->
		{#if hoveredNode && !isPanning}
			{@const circle = activeCircles.find(c => c.id === hoveredNode)}
			{#if circle}
				<div class="memory-tooltip" style="--c: {circle.hex}">
					<div class="memory-tooltip-dot" style="background: {circle.hex}"></div>
					<div class="memory-tooltip-body">
						<div class="memory-tooltip-name">{circle.label}</div>
						{#if circle.entry}
							<div class="memory-tooltip-summary">{circle.entry.summary}</div>
							<div class="memory-tooltip-meta">{formatSize(circle.entry.size)} · click to read</div>
						{:else if circle.folder}
							<div class="memory-tooltip-summary">{circle.folder.files.length} memories · {formatSize(circle.folder.totalSize)}</div>
							<div class="memory-tooltip-files">
								{#each circle.folder.files.slice(0, 5) as file}
									<span class="memory-tooltip-file">{fileName(file.path)}</span>
								{/each}
								{#if circle.folder.files.length > 5}
									<span class="memory-tooltip-file memory-tooltip-more">+{circle.folder.files.length - 5}</span>
								{/if}
							</div>
						{/if}
					</div>
				</div>
			{/if}
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
		background: radial-gradient(ellipse 80% 70% at 50% 45%, oklch(0.09 0.02 75 / 40%) 0%, transparent 70%);
	}

	.particle-canvas {
		position: absolute;
		inset: 0;
		pointer-events: none;
		z-index: 0;
	}

	/* ═══════ Loading ═══════ */

	.memory-loading { display: flex; align-items: center; justify-content: center; height: 100%; z-index: 1; }
	.memory-loading-orb { position: relative; width: 48px; height: 48px; }
	.memory-loading-ring {
		position: absolute; inset: 0; border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		animation: loading-spin 3s linear infinite;
	}
	.memory-loading-ring::after {
		content: ""; position: absolute; top: -1px; left: 50%;
		width: 6px; height: 2px; background: oklch(0.78 0.12 75 / 60%);
		border-radius: 1px; transform: translateX(-50%);
	}
	.memory-loading-ring-2 { inset: 8px; animation-direction: reverse; animation-duration: 2s; border-color: oklch(0.78 0.12 75 / 10%); }
	.memory-loading-dot {
		position: absolute; top: 50%; left: 50%; width: 4px; height: 4px;
		border-radius: 50%; background: oklch(0.78 0.12 75 / 40%);
		transform: translate(-50%, -50%); animation: pulse-alive 1.5s ease-in-out infinite;
	}
	@keyframes loading-spin { to { transform: rotate(360deg); } }

	/* ═══════ Empty ═══════ */

	.memory-empty {
		display: flex; flex-direction: column; align-items: center; justify-content: center;
		height: 100%; gap: 1rem; text-align: center; z-index: 1;
	}
	.memory-empty-orbs { position: relative; width: 80px; height: 60px; }
	.empty-orb {
		position: absolute; border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		background: radial-gradient(circle at 35% 35%, oklch(0.78 0.12 75 / 8%), transparent 70%);
	}
	.empty-orb-1 { width: 40px; height: 40px; left: 20px; top: 0; animation: breathe-slow 4s ease-in-out infinite; }
	.empty-orb-2 { width: 24px; height: 24px; left: 0; top: 28px; animation: breathe-slow 5s ease-in-out infinite 0.5s; }
	.empty-orb-3 { width: 18px; height: 18px; right: 4px; top: 34px; animation: breathe-slow 3.5s ease-in-out infinite 1s; }
	@keyframes breathe-slow { 0%, 100% { opacity: 0.4; transform: scale(1); } 50% { opacity: 0.8; transform: scale(1.08); } }
	.memory-empty-text { font-family: var(--font-display); font-size: 0.95rem; font-style: italic; color: oklch(0.78 0.12 75 / 40%); }
	.memory-empty-hint { font-size: 0.7rem; color: oklch(0.78 0.12 75 / 35%); max-width: 26ch; line-height: 1.5; }

	/* ═══════ Header ═══════ */

	.memory-header {
		display: flex; align-items: center; gap: 0.625rem;
		padding: 0.75rem 1.5rem 0; flex-shrink: 0; z-index: 2;
	}
	.memory-back {
		display: flex; align-items: center; justify-content: center;
		width: 28px; height: 28px; color: oklch(0.78 0.12 75 / 35%);
		background: oklch(1 0 0 / 3%); border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 50%; cursor: pointer; transition: all 0.25s ease;
	}
	.memory-back:hover {
		color: oklch(0.78 0.12 75 / 70%);
		border-color: oklch(0.78 0.12 75 / 28%);
		background: oklch(0.78 0.12 75 / 6%);
	}
	.memory-breadcrumb { font-family: var(--font-mono); font-size: 0.68rem; color: oklch(0.78 0.12 75 / 50%); }
	.memory-count { font-family: var(--font-mono); font-size: 0.7rem; color: oklch(0.78 0.12 75 / 28%); letter-spacing: 0.04em; margin-left: auto; }

	/* ═══════ Search ═══════ */

	.search-toggle {
		display: flex; align-items: center; justify-content: center;
		width: 28px; height: 28px; color: oklch(0.78 0.12 75 / 30%);
		background: none; border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 50%; cursor: pointer; transition: all 0.25s ease;
		flex-shrink: 0;
	}
	.search-toggle:hover, .search-toggle-active {
		color: oklch(0.78 0.12 75 / 65%);
		border-color: oklch(0.78 0.12 75 / 28%);
		background: oklch(0.78 0.12 75 / 6%);
	}

	.search-bar {
		display: flex; align-items: center; gap: 0.5rem;
		padding: 0.5rem 1.5rem; flex-shrink: 0; z-index: 2;
	}
	.search-input {
		flex: 1; font-family: var(--font-mono); font-size: 0.7rem;
		background: oklch(1 0 0 / 3%); border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 0.5rem; padding: 0.4rem 0.75rem;
		color: oklch(0.90 0.02 75 / 80%); outline: none;
		transition: border-color 0.2s ease;
	}
	.search-input:focus { border-color: oklch(0.78 0.12 75 / 35%); }
	.search-input::placeholder { color: oklch(0.78 0.12 75 / 35%); }
	.search-count {
		font-family: var(--font-mono); font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 30%); white-space: nowrap;
	}

	.search-results {
		flex: 1; min-height: 0; overflow-y: auto; z-index: 2;
		padding: 0.25rem 1.5rem 1.5rem; display: flex; flex-direction: column; gap: 0.35rem;
	}
	.search-result {
		padding: 0.6rem 0.75rem; border-radius: 0.5rem;
		border: 1px solid oklch(1 0 0 / 5%); background: oklch(1 0 0 / 2%);
		cursor: pointer; transition: all 0.2s ease;
		display: flex; flex-direction: column; gap: 0.15rem;
	}
	.search-result:hover {
		border-color: color-mix(in srgb, var(--c) 20%, transparent);
		background: color-mix(in srgb, var(--c) 4%, transparent);
	}
	.search-result-path {
		font-family: var(--font-mono); font-size: 0.75rem;
		color: oklch(0.90 0.02 75 / 65%);
	}
	.search-result-folder {
		font-size: 0.7rem; opacity: 0.7;
	}
	.search-result-summary {
		font-family: var(--font-body); font-size: 0.72rem;
		color: oklch(0.88 0.02 75 / 38%); line-height: 1.4;
		overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
	}
	.search-result-meta {
		font-family: var(--font-mono); font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 15%);
	}
	.search-empty {
		font-family: var(--font-mono); font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 28%); text-align: center;
		padding: 2rem; z-index: 2;
	}

	/* ═══════ Document Viewer ═══════ */

	.doc-viewer {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding: 1.25rem 1.5rem 2rem;
		z-index: 1;
	}

	.doc-loading {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 35%);
		padding: 2rem;
		text-align: center;
	}

	.doc-content {
		font-family: var(--font-body);
		font-size: 0.78rem;
		line-height: 1.7;
		color: oklch(0.90 0.02 75 / 70%);
		white-space: pre-wrap;
		word-wrap: break-word;
		margin: 0;
		max-width: 560px;
	}

	/* ═══════ Bubble Map ═══════ */

	.memory-map {
		position: relative;
		flex: 1;
		min-height: 0;
		z-index: 1;
		overflow: hidden;
		cursor: grab;
		touch-action: none;
	}

	.memory-map:active {
		cursor: grabbing;
	}

	.memory-map-inner {
		position: absolute;
		inset: 0;
		will-change: transform;
	}

	.bubble-anchor {
		position: absolute;
		transform: translate(-50%, -50%);
		animation: bubble-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
		z-index: 1;
	}

	@keyframes bubble-enter {
		0% { opacity: 0; transform: translate(-50%, -50%) scale(0); filter: blur(12px); }
		60% { opacity: 1; transform: translate(-50%, -50%) scale(1.06); filter: blur(1px); }
		100% { opacity: 1; transform: translate(-50%, -50%) scale(1); filter: blur(0); }
	}

	.bubble {
		position: relative;
		border-radius: 50%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 2px;
		background: radial-gradient(
			circle at 38% 32%,
			color-mix(in srgb, var(--c) 14%, transparent) 0%,
			color-mix(in srgb, var(--c) 6%, transparent) 50%,
			transparent 80%
		);
		border: 1px solid color-mix(in srgb, var(--c) 18%, transparent);
		box-shadow:
			0 0 40px color-mix(in srgb, var(--c) 6%, transparent),
			inset 0 0 30px color-mix(in srgb, var(--c) 4%, transparent);
		animation: bubble-float var(--float-dur) ease-in-out infinite var(--float-delay);
		transition: border-color 0.3s ease, box-shadow 0.3s ease;
	}

	.bubble-clickable { cursor: pointer; }

	.bubble-hovered {
		border-color: color-mix(in srgb, var(--c) 35%, transparent);
		box-shadow:
			0 0 60px color-mix(in srgb, var(--c) 12%, transparent),
			0 0 120px color-mix(in srgb, var(--c) 5%, transparent),
			inset 0 0 40px color-mix(in srgb, var(--c) 8%, transparent);
		z-index: 5;
	}

	@keyframes bubble-float {
		0%, 100% { transform: translate(0, 0); }
		33% { transform: translate(var(--float-x), var(--float-y)); }
		66% { transform: translate(calc(var(--float-x) * -0.6), calc(var(--float-y) * -0.4)); }
	}

	.bubble-core {
		position: absolute; top: 50%; left: 50%; width: 30%; height: 30%;
		border-radius: 50%;
		background: radial-gradient(circle, color-mix(in srgb, var(--c) 20%, transparent), transparent 70%);
		transform: translate(-50%, -50%);
		animation: core-pulse 3s ease-in-out infinite;
	}

	@keyframes core-pulse {
		0%, 100% { opacity: 0.5; transform: translate(-50%, -50%) scale(1); }
		50% { opacity: 1; transform: translate(-50%, -50%) scale(1.3); }
	}

	.bubble-shine {
		position: absolute; top: 12%; left: 22%; width: 32%; height: 18%;
		border-radius: 50%;
		background: radial-gradient(ellipse at center, rgba(255,255,255,0.1) 0%, transparent 70%);
		transform: rotate(-20deg); pointer-events: none;
	}

	.bubble-ring {
		position: absolute; inset: -6px; border-radius: 50%;
		border: 1px solid color-mix(in srgb, var(--c) 30%, transparent);
		animation: ring-expand 1.2s ease-out infinite;
		pointer-events: none;
	}

	@keyframes ring-expand {
		0% { opacity: 0.6; inset: -4px; }
		100% { opacity: 0; inset: -20px; }
	}

	.bubble-label {
		font-family: var(--font-mono); font-size: 0.72rem; letter-spacing: 0.05em;
		color: color-mix(in srgb, var(--c) 85%, white);
		text-shadow: 0 1px 8px color-mix(in srgb, var(--c) 30%, transparent);
		pointer-events: none; user-select: none; z-index: 2;
		text-align: center; line-height: 1; max-width: 85%;
		overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
	}

	.bubble-sub {
		font-family: var(--font-mono); font-size: 0.7rem;
		color: color-mix(in srgb, var(--c) 45%, transparent);
		pointer-events: none; user-select: none; z-index: 2;
	}

	/* ═══════ Tooltip ═══════ */

	.memory-tooltip {
		position: absolute; bottom: 1.25rem; left: 50%;
		transform: translateX(-50%); display: flex; gap: 0.625rem;
		align-items: flex-start;
		background: oklch(0.08 0.01 280 / 85%); backdrop-filter: blur(16px);
		border: 1px solid oklch(1 0 0 / 7%); border-radius: 0.875rem;
		padding: 0.75rem 1rem; max-width: 340px; min-width: 180px;
		animation: tooltip-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1);
		pointer-events: none; z-index: 20;
		box-shadow: 0 8px 32px oklch(0 0 0 / 40%), 0 0 60px color-mix(in srgb, var(--c) 6%, transparent);
	}
	@keyframes tooltip-enter {
		from { opacity: 0; transform: translateX(-50%) translateY(8px) scale(0.96); }
		to { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
	}

	.memory-tooltip-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; margin-top: 4px; box-shadow: 0 0 8px currentColor; }
	.memory-tooltip-body { display: flex; flex-direction: column; gap: 0.2rem; min-width: 0; }
	.memory-tooltip-name { font-family: var(--font-mono); font-size: 0.72rem; font-weight: 500; letter-spacing: 0.03em; color: oklch(0.92 0.02 75 / 85%); }
	.memory-tooltip-summary { font-family: var(--font-body); font-size: 0.66rem; color: oklch(0.88 0.02 75 / 45%); line-height: 1.4; }
	.memory-tooltip-meta { font-family: var(--font-mono); font-size: 0.75rem; color: oklch(0.78 0.12 75 / 28%); }
	.memory-tooltip-files { margin-top: 0.2rem; display: flex; flex-wrap: wrap; gap: 0.25rem; }
	.memory-tooltip-file {
		font-family: var(--font-mono); font-size: 0.7rem; color: oklch(0.88 0.02 75 / 30%);
		background: oklch(1 0 0 / 3%); padding: 0.1rem 0.35rem; border-radius: 0.25rem;
		border: 1px solid oklch(1 0 0 / 4%);
	}
	.memory-tooltip-more { color: oklch(0.78 0.12 75 / 35%); font-style: italic; border: none; background: none; padding: 0.1rem 0; }

	@media (max-width: 640px) {
		.memory-header { padding: 0.5rem 0.75rem 0; }
		.memory-tooltip { left: 0.75rem; right: 0.75rem; transform: none; max-width: none; }
		@keyframes tooltip-enter {
			from { opacity: 0; transform: translateY(8px) scale(0.96); }
			to { opacity: 1; transform: translateY(0) scale(1); }
		}
	}
</style>
