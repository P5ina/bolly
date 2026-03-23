<script lang="ts">
	import { fetchMemory, fetchMemoryContent, searchMemory, deleteMemoryFile, fetchVectors, getAuthToken, type MemorySearchResult, type VectorEntry } from "$lib/api/client.js";
	import { Play, Music, FileText } from "@lucide/svelte";
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

	// Debug state
	let debugOpen = $state(false);
	let vectors = $state<VectorEntry[]>([]);
	let vectorsLoading = $state(false);

	async function loadVectors() {
		vectorsLoading = true;
		try {
			vectors = await fetchVectors(slug);
		} catch {
			toast.error("failed to load vectors");
		}
		vectorsLoading = false;
	}

	function toggleDebug() {
		debugOpen = !debugOpen;
		if (debugOpen && vectors.length === 0) loadVectors();
	}

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
						ctx!.strokeStyle = `rgba(120, 140, 200, ${alpha})`;
						ctx!.lineWidth = 0.5;
						ctx!.stroke();
					}
				}
			}

			for (const p of particles) {
				ctx!.beginPath();
				ctx!.arc(p.x, p.y, p.r, 0, Math.PI * 2);
				ctx!.fillStyle = `rgba(120, 140, 200, ${Math.max(0, p.opacity)})`;
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

	const IMAGE_EXTS = ['.jpg', '.jpeg', '.png', '.gif', '.webp', '.svg', '.bmp'];
	const VIDEO_EXTS = ['.mp4', '.mov', '.webm'];
	const AUDIO_EXTS = ['.mp3', '.wav', '.ogg'];
	const PDF_EXTS = ['.pdf'];
	const MEDIA_EXTS = [...IMAGE_EXTS, ...VIDEO_EXTS, ...AUDIO_EXTS, ...PDF_EXTS];

	function isMediaFile(path: string): boolean {
		const lower = path.toLowerCase();
		return MEDIA_EXTS.some(ext => lower.endsWith(ext));
	}

	function mediaType(path: string): 'image' | 'video' | 'audio' | 'pdf' | 'text' {
		const lower = path.toLowerCase();
		if (IMAGE_EXTS.some(ext => lower.endsWith(ext))) return 'image';
		if (VIDEO_EXTS.some(ext => lower.endsWith(ext))) return 'video';
		if (AUDIO_EXTS.some(ext => lower.endsWith(ext))) return 'audio';
		if (PDF_EXTS.some(ext => lower.endsWith(ext))) return 'pdf';
		return 'text';
	}

	function mediaUrl(path: string): string {
		const token = getAuthToken() ?? '';
		return `/api/instances/${encodeURIComponent(slug)}/memory/${path}${token ? `?token=${encodeURIComponent(token)}` : ''}`;
	}

	async function openDocument(entry: MemoryEntry) {
		viewingEntry = entry;
		if (isMediaFile(entry.path)) {
			// Media files rendered inline — no need to fetch content
			viewingContent = "";
			viewingLoading = false;
			return;
		}
		viewingLoading = true;
		try {
			viewingContent = await fetchMemoryContent(slug, entry.path);
		} catch {
			viewingContent = "(failed to load)";
		} finally {
			viewingLoading = false;
		}
	}

	async function handleDelete() {
		if (!viewingEntry) return;
		const path = viewingEntry.path;
		try {
			await deleteMemoryFile(slug, path);
			toast.show("deleted");
			viewingEntry = null;
			viewingContent = "";
			await load();
		} catch {
			toast.error("failed to delete");
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
			<button class="memory-delete" onclick={handleDelete} title="delete memory">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
					<path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
		</div>
		<div class="doc-viewer">
			{#if viewingLoading}
				<div class="doc-loading">loading...</div>
			{:else if mediaType(viewingEntry.path) === 'image'}
				<img src={mediaUrl(viewingEntry.path)} alt={viewingEntry.path} class="doc-media-img" />
			{:else if mediaType(viewingEntry.path) === 'video'}
				<video src={mediaUrl(viewingEntry.path)} controls playsinline class="doc-media-video">
					<track kind="captions" />
				</video>
			{:else if mediaType(viewingEntry.path) === 'audio'}
				<div class="doc-media-audio-wrap">
					<div class="doc-audio-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32">
							<path d="M9 18V5l12-2v13" stroke-linecap="round" stroke-linejoin="round"/>
							<circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
						</svg>
					</div>
					<audio src={mediaUrl(viewingEntry.path)} controls class="doc-media-audio"></audio>
				</div>
			{:else if mediaType(viewingEntry.path) === 'pdf'}
				<iframe src={mediaUrl(viewingEntry.path)} class="doc-media-pdf" title="PDF viewer"></iframe>
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
			<button class="search-toggle" class:search-toggle-active={debugOpen} onclick={toggleDebug} title="vector debug">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
					<path d="M12 20h9M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" stroke-linecap="round" stroke-linejoin="round"/>
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
						{@const isMedia = result.source_type?.startsWith("media_")}
						{@const basePath = result.path.split("#")[0]}
						{@const folder = isMedia ? result.source_type?.replace("media_", "") ?? "media" : (basePath.split("/")[0] ?? "(root)")}
						{@const preview = result.text.trim().slice(0, 200)}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="search-result"
							class:search-result-media={isMedia}
							style="--c: {isMedia ? '#d4a55a' : getHex(folder)}"
							onclick={() => {
								if (isMedia && result.media_url) {
									window.open(result.media_url, '_blank');
								} else {
									const entry = entries.find(e => e.path === basePath);
									if (entry) openDocument(entry);
								}
							}}
						>
							{#if isMedia && result.media_url && result.source_type === "media_image"}
								<img class="search-result-thumb" src={result.media_url} alt={basePath} />
							{/if}
							<div class="search-result-body">
								<div class="search-result-path">
									{#if isMedia}
										<span class="search-result-folder" style="color: var(--c)">{result.source_type === "media_image" ? "image" : result.source_type === "media_video" ? "video" : "audio"}</span>
										{preview || basePath}
									{:else}
										<span class="search-result-folder" style="color: {getHex(folder)}">{folder}/</span>{fileName(basePath)}
									{/if}
								</div>
								{#if !isMedia}
									<div class="search-result-summary">{preview}</div>
								{/if}
								<div class="search-result-meta">score: {result.score.toFixed(4)}</div>
							</div>
						</div>
					{/each}
				</div>
			{:else if searchQuery.length >= 2 && !searchLoading}
				<div class="search-empty">no matches</div>
			{/if}
		{/if}

		{#if debugOpen}
			<div class="debug-panel">
				<div class="debug-header">
					<span class="debug-title">vectors ({vectors.length})</span>
					<button class="debug-refresh" onclick={loadVectors}>{vectorsLoading ? "..." : "refresh"}</button>
				</div>
				<div class="debug-list">
					{#each vectors as v}
						<div class="debug-entry" class:debug-media={v.source_type.startsWith("media_")}>
							<div class="debug-path">{v.path}</div>
							<div class="debug-meta">
								<span class="debug-type">{v.source_type}</span>
								{#if v.upload_id && v.upload_id !== v.path}
									<span class="debug-upload">upload: {v.upload_id}</span>
								{/if}
							</div>
							{#if v.content_preview}
								<div class="debug-preview">{v.content_preview.slice(0, 150)}</div>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		{/if}

		{#if !searchOpen && !debugOpen}
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
						{@const isImage = !isFolderView && circle.entry && IMAGE_EXTS.some(ext => circle.entry!.path.toLowerCase().endsWith(ext))}
						{@const fileType = !isFolderView && circle.entry ? mediaType(circle.entry.path) : 'text'}
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
								{#if isImage}
									<img class="bubble-thumb" src={mediaUrl(circle.entry?.path ?? '')} alt="" loading="lazy" />
								{:else if fileType === 'video'}
									<video class="bubble-thumb" src={mediaUrl(circle.entry?.path ?? '')} autoplay muted loop playsinline></video>
								{:else if fileType === 'audio'}
									<div class="bubble-type-icon"><Music size={20} /></div>
								{:else if fileType === 'pdf'}
									<div class="bubble-type-icon"><FileText size={20} /></div>
								{:else}
									<div class="bubble-core"></div>
								{/if}
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
		border: 1px solid oklch(0.55 0.08 240 / 15%);
		animation: loading-spin 3s linear infinite;
	}
	.memory-loading-ring::after {
		content: ""; position: absolute; top: -1px; left: 50%;
		width: 6px; height: 2px; background: oklch(0.55 0.08 240 / 60%);
		border-radius: 1px; transform: translateX(-50%);
	}
	.memory-loading-ring-2 { inset: 8px; animation-direction: reverse; animation-duration: 2s; border-color: oklch(0.55 0.08 240 / 10%); }
	.memory-loading-dot {
		position: absolute; top: 50%; left: 50%; width: 4px; height: 4px;
		border-radius: 50%; background: oklch(0.55 0.08 240 / 40%);
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
		border: 1px solid oklch(0.55 0.08 240 / 12%);
		background: radial-gradient(circle at 35% 35%, oklch(0.55 0.08 240 / 8%), transparent 70%);
	}
	.empty-orb-1 { width: 40px; height: 40px; left: 20px; top: 0; animation: breathe-slow 4s ease-in-out infinite; }
	.empty-orb-2 { width: 24px; height: 24px; left: 0; top: 28px; animation: breathe-slow 5s ease-in-out infinite 0.5s; }
	.empty-orb-3 { width: 18px; height: 18px; right: 4px; top: 34px; animation: breathe-slow 3.5s ease-in-out infinite 1s; }
	@keyframes breathe-slow { 0%, 100% { opacity: 0.4; transform: scale(1); } 50% { opacity: 0.8; transform: scale(1.08); } }
	.memory-empty-text { font-family: var(--font-display); font-size: 0.95rem; font-style: italic; color: oklch(0.55 0.08 240 / 40%); }
	.memory-empty-hint { font-size: 0.7rem; color: oklch(0.55 0.08 240 / 35%); max-width: 26ch; line-height: 1.5; }

	/* ═══════ Header ═══════ */

	.memory-header {
		display: flex; align-items: center; gap: 0.625rem;
		padding: 0.75rem 1.5rem 0; flex-shrink: 0; z-index: 2;
	}
	.memory-back {
		display: flex; align-items: center; justify-content: center;
		width: 28px; height: 28px; color: oklch(0.65 0.04 240 / 50%);
		background: linear-gradient(145deg, oklch(1 0 0 / 6%) 0%, oklch(0.5 0.02 240 / 8%) 100%);
		border: 1px solid oklch(1 0 0 / 10%); border-top-color: oklch(1 0 0 / 16%);
		border-radius: 50%; cursor: pointer; transition: all 0.25s ease;
		box-shadow: inset 0 1px 0 oklch(1 0 0 / 8%);
	}
	.memory-back:hover {
		color: oklch(0.80 0.04 240 / 75%);
		border-color: oklch(1 0 0 / 18%);
		background: linear-gradient(145deg, oklch(1 0 0 / 9%) 0%, oklch(0.5 0.02 240 / 12%) 100%);
	}
	.memory-breadcrumb { font-family: var(--font-mono); font-size: 0.68rem; color: oklch(0.55 0.08 240 / 50%); }
	.memory-count { font-family: var(--font-mono); font-size: 0.7rem; color: oklch(0.55 0.08 240 / 28%); letter-spacing: 0.04em; margin-left: auto; }
	.memory-delete {
		display: flex; align-items: center; justify-content: center;
		width: 28px; height: 28px; color: oklch(0.55 0.08 15 / 50%);
		background: none; border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 50%; cursor: pointer; transition: all 0.25s ease;
		margin-left: 6px;
	}
	.memory-delete:hover {
		color: oklch(0.70 0.15 15);
		border-color: oklch(0.70 0.15 15 / 30%);
		background: oklch(0.70 0.15 15 / 8%);
	}

	/* ═══════ Search ═══════ */

	.search-toggle {
		display: flex; align-items: center; justify-content: center;
		width: 28px; height: 28px; color: oklch(0.55 0.08 240 / 30%);
		background: none; border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 50%; cursor: pointer; transition: all 0.25s ease;
		flex-shrink: 0;
	}
	.search-toggle:hover, .search-toggle-active {
		color: oklch(0.55 0.08 240 / 65%);
		border-color: oklch(0.55 0.08 240 / 28%);
		background: oklch(0.55 0.08 240 / 6%);
	}

	.search-bar {
		display: flex; align-items: center; gap: 0.5rem;
		padding: 0.5rem 1.5rem; flex-shrink: 0; z-index: 2;
	}
	.search-input {
		flex: 1; font-family: var(--font-mono); font-size: 0.7rem;
		background: linear-gradient(155deg, oklch(1 0 0 / 5%) 0%, oklch(0.5 0.02 250 / 8%) 50%, oklch(1 0 0 / 3%) 100%);
		backdrop-filter: blur(16px) saturate(140%);
		-webkit-backdrop-filter: blur(16px) saturate(140%);
		border: 1px solid oklch(1 0 0 / 10%); border-top-color: oklch(1 0 0 / 16%);
		border-radius: 0.625rem; padding: 0.4rem 0.75rem;
		color: oklch(0.90 0.02 75 / 80%); outline: none;
		transition: border-color 0.2s ease;
		box-shadow: inset 0 1px 0 oklch(1 0 0 / 6%);
	}
	.search-input:focus { border-color: oklch(1 0 0 / 18%); box-shadow: 0 0 0 3px oklch(0.5 0.06 240 / 8%), inset 0 1px 0 oklch(1 0 0 / 8%); }
	.search-input::placeholder { color: oklch(0.55 0.08 240 / 35%); }
	.search-count {
		font-family: var(--font-mono); font-size: 0.75rem;
		color: oklch(0.55 0.08 240 / 30%); white-space: nowrap;
	}

	.search-results {
		flex: 1; min-height: 0; overflow-y: auto; z-index: 2;
		padding: 0.25rem 1.5rem 1.5rem; display: flex; flex-direction: column; gap: 0.35rem;
	}
	.search-result {
		position: relative;
		padding: 0.6rem 0.75rem; border-radius: 0.75rem;
		border: 1px solid oklch(1 0 0 / 8%); border-top-color: oklch(1 0 0 / 14%);
		background: linear-gradient(150deg, oklch(1 0 0 / 5%) 0%, oklch(0.5 0.02 250 / 6%) 50%, oklch(1 0 0 / 3%) 100%);
		backdrop-filter: blur(12px) saturate(140%);
		-webkit-backdrop-filter: blur(12px) saturate(140%);
		cursor: pointer; transition: all 0.2s ease;
		display: flex; flex-direction: column; gap: 0.15rem;
		box-shadow: inset 0 1px 0 oklch(1 0 0 / 6%);
		overflow: hidden;
	}
	.search-result:hover {
		border-color: oklch(1 0 0 / 14%);
		background: linear-gradient(150deg, oklch(1 0 0 / 7%) 0%, color-mix(in srgb, var(--c) 8%, transparent) 50%, oklch(1 0 0 / 4%) 100%);
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
	.search-result-media {
		flex-direction: row; align-items: center; gap: 0.6rem;
	}
	.search-result-thumb {
		width: 48px; height: 48px; border-radius: 0.5rem;
		object-fit: cover; flex-shrink: 0;
		border: 1px solid oklch(1 0 0 / 10%);
	}
	.search-result-body {
		flex: 1; min-width: 0;
		display: flex; flex-direction: column; gap: 0.15rem;
	}
	.search-result-meta {
		font-family: var(--font-mono); font-size: 0.7rem;
		color: oklch(0.55 0.08 240 / 15%);
	}
	.search-empty {
		font-family: var(--font-mono); font-size: 0.75rem;
		color: oklch(0.55 0.08 240 / 28%); text-align: center;
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
		color: oklch(0.55 0.08 240 / 35%);
		padding: 2rem;
		text-align: center;
	}

	.doc-media-img {
		max-width: 100%;
		max-height: 70vh;
		border-radius: 0.75rem;
		object-fit: contain;
	}

	.doc-media-video {
		max-width: 100%;
		max-height: 70vh;
		border-radius: 0.75rem;
	}

	.doc-media-audio-wrap {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
		padding: 3rem 0;
	}

	.doc-audio-icon {
		color: oklch(0.78 0.12 75 / 30%);
	}

	.doc-media-audio {
		width: 100%;
		max-width: 400px;
	}

	.doc-media-pdf {
		width: 100%;
		height: 80vh;
		border: none;
		border-radius: 0.75rem;
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
		background: linear-gradient(
			145deg,
			oklch(1 0 0 / 8%) 0%,
			color-mix(in srgb, var(--c) 8%, transparent) 40%,
			oklch(1 0 0 / 4%) 70%,
			color-mix(in srgb, var(--c) 5%, transparent) 100%
		);
		backdrop-filter: blur(16px) saturate(150%) brightness(1.05);
		-webkit-backdrop-filter: blur(16px) saturate(150%) brightness(1.05);
		border: 1px solid oklch(1 0 0 / 10%);
		border-top-color: oklch(1 0 0 / 20%);
		box-shadow:
			0 4px 24px oklch(0 0 0 / 15%),
			0 0 40px color-mix(in srgb, var(--c) 5%, transparent),
			inset 0 1px 0 oklch(1 0 0 / 10%),
			inset 0 -1px 0 oklch(0 0 0 / 5%);
		animation: bubble-float var(--float-dur) ease-in-out infinite var(--float-delay);
		transition: border-color 0.3s ease, box-shadow 0.3s ease;
		overflow: hidden;
	}

	/* Specular highlight on glass sphere */
	.bubble::before {
		content: "";
		position: absolute;
		top: 8%;
		left: 20%;
		width: 35%;
		height: 20%;
		border-radius: 50%;
		background: radial-gradient(ellipse, oklch(1 0 0 / 15%) 0%, transparent 70%);
		transform: rotate(-20deg);
		pointer-events: none;
	}

	.bubble-clickable { cursor: pointer; }

	.bubble-hovered {
		border-color: oklch(1 0 0 / 18%);
		border-top-color: oklch(1 0 0 / 28%);
		box-shadow:
			0 4px 32px oklch(0 0 0 / 20%),
			0 0 60px color-mix(in srgb, var(--c) 10%, transparent),
			0 0 120px color-mix(in srgb, var(--c) 4%, transparent),
			inset 0 1px 0 oklch(1 0 0 / 14%);
		z-index: 5;
	}

	@keyframes bubble-float {
		0%, 100% { transform: translate(0, 0); }
		33% { transform: translate(var(--float-x), var(--float-y)); }
		66% { transform: translate(calc(var(--float-x) * -0.6), calc(var(--float-y) * -0.4)); }
	}

	.bubble-thumb {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: 50%;
		opacity: 0.85;
	}

	.bubble-type-icon {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		color: oklch(0.78 0.12 75 / 40%);
		opacity: 0.6;
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
		background: linear-gradient(155deg, oklch(1 0 0 / 6%) 0%, oklch(0.5 0.02 250 / 10%) 40%, oklch(1 0 0 / 4%) 100%);
		backdrop-filter: blur(28px) saturate(160%) brightness(1.06);
		-webkit-backdrop-filter: blur(28px) saturate(160%) brightness(1.06);
		border: 1px solid oklch(1 0 0 / 10%); border-top-color: oklch(1 0 0 / 18%);
		border-radius: 1rem;
		padding: 0.75rem 1rem; max-width: 340px; min-width: 180px;
		animation: tooltip-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1);
		pointer-events: none; z-index: 20;
		box-shadow: 0 8px 32px oklch(0 0 0 / 30%), inset 0 1px 0 oklch(1 0 0 / 8%);
		overflow: hidden;
	}
	@keyframes tooltip-enter {
		from { opacity: 0; transform: translateX(-50%) translateY(8px) scale(0.96); }
		to { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
	}

	.memory-tooltip-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; margin-top: 4px; box-shadow: 0 0 8px currentColor; }
	.memory-tooltip-body { display: flex; flex-direction: column; gap: 0.2rem; min-width: 0; }
	.memory-tooltip-name { font-family: var(--font-mono); font-size: 0.72rem; font-weight: 500; letter-spacing: 0.03em; color: oklch(0.92 0.02 75 / 85%); }
	.memory-tooltip-summary { font-family: var(--font-body); font-size: 0.66rem; color: oklch(0.88 0.02 75 / 45%); line-height: 1.4; }
	.memory-tooltip-meta { font-family: var(--font-mono); font-size: 0.75rem; color: oklch(0.55 0.08 240 / 28%); }
	.memory-tooltip-files { margin-top: 0.2rem; display: flex; flex-wrap: wrap; gap: 0.25rem; }
	.memory-tooltip-file {
		font-family: var(--font-mono); font-size: 0.7rem; color: oklch(0.88 0.02 75 / 30%);
		background: oklch(1 0 0 / 3%); padding: 0.1rem 0.35rem; border-radius: 0.25rem;
		border: 1px solid oklch(1 0 0 / 4%);
	}
	.memory-tooltip-more { color: oklch(0.55 0.08 240 / 35%); font-style: italic; border: none; background: none; padding: 0.1rem 0; }

	@media (max-width: 640px) {
		.memory-header { padding: 0.5rem 0.75rem 0; }
		.memory-tooltip { left: 0.75rem; right: 0.75rem; transform: none; max-width: none; }
		@keyframes tooltip-enter {
			from { opacity: 0; transform: translateY(8px) scale(0.96); }
			to { opacity: 1; transform: translateY(0) scale(1); }
		}
	}

	/* ═══════ Debug panel ═══════ */

	.debug-panel {
		flex: 1; overflow-y: auto; padding: 0.5rem 1rem;
	}
	.debug-header {
		display: flex; align-items: center; justify-content: space-between;
		margin-bottom: 0.5rem;
	}
	.debug-title {
		font-family: var(--font-mono); font-size: 0.7rem;
		color: oklch(0.55 0.08 240 / 50%); letter-spacing: 0.04em;
	}
	.debug-refresh {
		font-family: var(--font-mono); font-size: 0.65rem;
		color: oklch(0.6 0.08 190 / 50%); background: none; border: 1px solid oklch(1 0 0 / 8%);
		border-radius: 4px; padding: 0.15rem 0.5rem; cursor: pointer;
	}
	.debug-refresh:hover { border-color: oklch(1 0 0 / 15%); color: oklch(0.7 0.08 190 / 70%); }
	.debug-list { display: flex; flex-direction: column; gap: 0.3rem; }
	.debug-entry {
		padding: 0.4rem 0.5rem; border-radius: 0.35rem;
		background: oklch(0.08 0.015 240 / 40%); border: 1px solid oklch(1 0 0 / 4%);
	}
	.debug-media { border-left: 2px solid oklch(0.65 0.12 75 / 40%); }
	.debug-path {
		font-family: var(--font-mono); font-size: 0.68rem;
		color: oklch(0.7 0.04 220 / 65%); word-break: break-all;
	}
	.debug-meta {
		display: flex; gap: 0.5rem; margin-top: 0.15rem;
	}
	.debug-type {
		font-family: var(--font-mono); font-size: 0.6rem;
		color: oklch(0.55 0.08 190 / 50%);
	}
	.debug-upload {
		font-family: var(--font-mono); font-size: 0.6rem;
		color: oklch(0.5 0.04 220 / 35%);
	}
	.debug-preview {
		font-family: var(--font-mono); font-size: 0.6rem;
		color: oklch(0.5 0.03 220 / 40%); margin-top: 0.2rem;
		line-height: 1.4; white-space: pre-wrap; word-break: break-word;
	}
</style>
