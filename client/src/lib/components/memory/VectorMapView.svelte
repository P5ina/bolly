<script lang="ts">
	import { fetchMemoryVectors, type VectorPoint } from "$lib/api/client.js";
	import { UMAP } from "umap-js";

	interface Props {
		slug: string;
	}

	let { slug }: Props = $props();

	let points = $state<VectorPoint[]>([]);
	let projected = $state<{ x: number; y: number; point: VectorPoint }[]>([]);
	let loading = $state(true);
	let error = $state("");
	let hovered = $state<number | null>(null);
	let canvas: HTMLCanvasElement | undefined = $state();

	// Camera
	let panX = $state(0);
	let panY = $state(0);
	let zoom = $state(1);
	let dragging = $state(false);
	let dragStart = $state({ x: 0, y: 0 });

	const folderHex: Record<string, string> = {
		about: "#5ba8d4", facts: "#6bc47a", moments: "#d46b6b",
		preferences: "#d4a55a", projects: "#7b7bd4", interests: "#c475d4",
		people: "#d49060", emotions: "#d46ba0", knowledge: "#5ab8a0",
		technical: "#7090c0", media_image: "#d4a55a", media_video: "#d49060",
		media_audio: "#c475d4", "(root)": "#8888a0",
	};

	function getColor(point: VectorPoint): string {
		if (point.source_type.startsWith("media_")) return folderHex[point.source_type] ?? "#888";
		const folder = point.path.split("/")[0] ?? "(root)";
		if (folderHex[folder]) return folderHex[folder];
		let hash = 0;
		for (let i = 0; i < folder.length; i++) hash = folder.charCodeAt(i) + ((hash << 5) - hash);
		return `hsl(${((hash % 360) + 360) % 360}, 50%, 62%)`;
	}

	function getLabel(point: VectorPoint): string {
		if (point.source_type.startsWith("media_")) return point.content_preview || point.path;
		const parts = point.path.split("/");
		return parts[parts.length - 1]?.replace(".md", "") ?? point.path;
	}

	$effect(() => {
		if (!slug) return;
		loading = true;
		error = "";
		fetchMemoryVectors(slug).then(pts => {
			points = pts;
			if (pts.length < 2) {
				projected = pts.map((p, i) => ({ x: 0.5, y: 0.5, point: p }));
				loading = false;
				return;
			}

			const vectors = pts.map(p => p.vector);
			const umap = new UMAP({
				nNeighbors: Math.min(15, Math.max(2, Math.floor(pts.length / 3))),
				minDist: 0.1,
				nComponents: 2,
			});

			const embedding = umap.fit(vectors);

			// Normalize to 0-1
			let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
			for (const [x, y] of embedding) {
				if (x < minX) minX = x;
				if (x > maxX) maxX = x;
				if (y < minY) minY = y;
				if (y > maxY) maxY = y;
			}
			const rangeX = maxX - minX || 1;
			const rangeY = maxY - minY || 1;

			projected = embedding.map(([x, y], i) => ({
				x: (x - minX) / rangeX,
				y: (y - minY) / rangeY,
				point: pts[i],
			}));
			loading = false;
		}).catch(e => {
			error = e.message;
			loading = false;
		});
	});

	$effect(() => {
		if (!canvas || loading || projected.length === 0) return;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;

		const W = canvas.width = canvas.offsetWidth * devicePixelRatio;
		const H = canvas.height = canvas.offsetHeight * devicePixelRatio;
		ctx.scale(devicePixelRatio, devicePixelRatio);
		const w = canvas.offsetWidth;
		const h = canvas.offsetHeight;

		const pad = 60;

		ctx.clearRect(0, 0, w, h);

		// Draw connections between nearby points
		ctx.strokeStyle = "rgba(120, 140, 200, 0.06)";
		ctx.lineWidth = 0.5;
		for (let i = 0; i < projected.length; i++) {
			const a = projected[i];
			const ax = pad + a.x * (w - pad * 2) + panX;
			const ay = pad + a.y * (h - pad * 2) + panY;
			for (let j = i + 1; j < projected.length; j++) {
				const b = projected[j];
				const bx = pad + b.x * (w - pad * 2) + panX;
				const by = pad + b.y * (h - pad * 2) + panY;
				const dist = Math.sqrt((ax - bx) ** 2 + (ay - by) ** 2);
				if (dist < 80 * zoom) {
					ctx.beginPath();
					ctx.moveTo(ax * zoom, ay * zoom);
					ctx.lineTo(bx * zoom, by * zoom);
					ctx.stroke();
				}
			}
		}

		// Draw points
		for (let i = 0; i < projected.length; i++) {
			const p = projected[i];
			const x = (pad + p.x * (w - pad * 2) + panX) * zoom;
			const y = (pad + p.y * (h - pad * 2) + panY) * zoom;
			const color = getColor(p.point);
			const r = hovered === i ? 8 : 5;

			// Glow
			ctx.beginPath();
			ctx.arc(x, y, r + 4, 0, Math.PI * 2);
			ctx.fillStyle = color.replace(")", ", 0.15)").replace("hsl(", "hsla(").replace("#", "#");
			const gradient = ctx.createRadialGradient(x, y, 0, x, y, r + 8);
			gradient.addColorStop(0, color + "33");
			gradient.addColorStop(1, "transparent");
			ctx.fillStyle = gradient;
			ctx.fill();

			// Point
			ctx.beginPath();
			ctx.arc(x, y, r, 0, Math.PI * 2);
			ctx.fillStyle = color;
			ctx.fill();

			// Label
			if (zoom > 0.7 || hovered === i) {
				ctx.font = `${hovered === i ? 11 : 9}px var(--font-mono, monospace)`;
				ctx.fillStyle = hovered === i ? "rgba(255,255,255,0.9)" : "rgba(255,255,255,0.4)";
				ctx.textAlign = "center";
				const label = getLabel(p.point);
				ctx.fillText(label.slice(0, 25), x, y - r - 6);
			}
		}
	});

	function handleMouseMove(e: MouseEvent) {
		if (!canvas) return;
		const rect = canvas.getBoundingClientRect();
		const mx = e.clientX - rect.left;
		const my = e.clientY - rect.top;
		const w = canvas.offsetWidth;
		const h = canvas.offsetHeight;
		const pad = 60;

		if (dragging) {
			panX += (e.clientX - dragStart.x) / zoom;
			panY += (e.clientY - dragStart.y) / zoom;
			dragStart = { x: e.clientX, y: e.clientY };
			return;
		}

		let closest = -1;
		let closestDist = 20;
		for (let i = 0; i < projected.length; i++) {
			const p = projected[i];
			const x = (pad + p.x * (w - pad * 2) + panX) * zoom;
			const y = (pad + p.y * (h - pad * 2) + panY) * zoom;
			const dist = Math.sqrt((mx - x) ** 2 + (my - y) ** 2);
			if (dist < closestDist) {
				closest = i;
				closestDist = dist;
			}
		}
		hovered = closest >= 0 ? closest : null;
	}

	function handleWheel(e: WheelEvent) {
		e.preventDefault();
		const delta = e.deltaY > 0 ? 0.9 : 1.1;
		zoom = Math.max(0.3, Math.min(5, zoom * delta));
	}

	function handleMouseDown(e: MouseEvent) {
		dragging = true;
		dragStart = { x: e.clientX, y: e.clientY };
	}

	function handleMouseUp() {
		dragging = false;
	}
</script>

<div class="vector-map">
	{#if loading}
		<div class="loading">
			<div class="pulse"></div>
			<span>computing layout...</span>
		</div>
	{:else if error}
		<div class="error">{error}</div>
	{:else if projected.length === 0}
		<div class="empty">no vectors indexed yet</div>
	{:else}
		<canvas
			bind:this={canvas}
			onmousemove={handleMouseMove}
			onmousedown={handleMouseDown}
			onmouseup={handleMouseUp}
			onmouseleave={handleMouseUp}
			onwheel={handleWheel}
			style="cursor: {dragging ? 'grabbing' : 'grab'}"
		></canvas>
		<div class="stats">{projected.length} vectors</div>
		{#if hovered !== null && projected[hovered]}
			{@const p = projected[hovered].point}
			<div class="tooltip">
				<div class="tooltip-path">{p.path}</div>
				<div class="tooltip-type">{p.source_type}</div>
				{#if p.content_preview}
					<div class="tooltip-preview">{p.content_preview.slice(0, 200)}</div>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.vector-map {
		position: relative;
		width: 100%;
		height: 100%;
		background: oklch(0.08 0.01 250);
		border-radius: 1rem;
		overflow: hidden;
	}
	canvas {
		width: 100%;
		height: 100%;
		display: block;
	}
	.loading, .error, .empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 1rem;
		font-family: var(--font-mono);
		font-size: 0.8rem;
		color: oklch(0.55 0.08 240 / 40%);
	}
	.pulse {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: oklch(0.55 0.08 240 / 30%);
		animation: pulse 1.5s ease-in-out infinite;
	}
	@keyframes pulse {
		0%, 100% { transform: scale(1); opacity: 0.3; }
		50% { transform: scale(1.5); opacity: 0.6; }
	}
	.stats {
		position: absolute;
		top: 0.75rem;
		right: 0.75rem;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.55 0.08 240 / 30%);
	}
	.tooltip {
		position: absolute;
		bottom: 1rem;
		left: 1rem;
		right: 1rem;
		padding: 0.75rem;
		background: oklch(0.12 0.02 250 / 90%);
		backdrop-filter: blur(12px);
		border: 1px solid oklch(1 0 0 / 10%);
		border-radius: 0.75rem;
		pointer-events: none;
	}
	.tooltip-path {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.9 0.02 75 / 70%);
		margin-bottom: 0.25rem;
	}
	.tooltip-type {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.55 0.08 240 / 40%);
		margin-bottom: 0.25rem;
	}
	.tooltip-preview {
		font-family: var(--font-body);
		font-size: 0.72rem;
		color: oklch(0.88 0.02 75 / 45%);
		line-height: 1.4;
		overflow: hidden;
		text-overflow: ellipsis;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		-webkit-box-orient: vertical;
	}
</style>
