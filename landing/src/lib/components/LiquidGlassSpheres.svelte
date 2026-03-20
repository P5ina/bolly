<script lang="ts">
	import { onMount } from 'svelte';

	// ── Sphere refraction displacement map (Snell's law) ──
	function generateSphereMap(size: number, ior: number, thickness: number): string {
		const canvas = document.createElement('canvas');
		canvas.width = size;
		canvas.height = size;
		const ctx = canvas.getContext('2d')!;
		const data = new Uint8ClampedArray(size * size * 4);

		for (let py = 0; py < size; py++) {
			for (let px = 0; px < size; px++) {
				const idx = (py * size + px) * 4;
				const nx = (px / size) * 2 - 1;
				const ny = (py / size) * 2 - 1;
				const r2 = nx * nx + ny * ny;

				if (r2 < 0.95) {
					const nz = Math.sqrt(1.0 - Math.min(r2, 1.0));
					const eta = 1.0 / ior;
					const k = 1.0 - eta * eta * (1.0 - nz * nz);
					if (k > 0) {
						const rx = -eta * nx;
						const ry = -eta * ny;
						const edgeFalloff = Math.pow(nz, 0.4);
						data[idx]     = Math.round(128 + rx * thickness * edgeFalloff * 127);
						data[idx + 1] = Math.round(128 + ry * thickness * edgeFalloff * 127);
					} else {
						data[idx] = 128;
						data[idx + 1] = 128;
					}
				} else {
					data[idx] = 128;
					data[idx + 1] = 128;
				}
				data[idx + 2] = 0;
				data[idx + 3] = 255;
			}
		}

		ctx.putImageData(new ImageData(data, size, size), 0, 0);
		return canvas.toDataURL();
	}

	// ── Config ──
	const configs = [
		{ size: 200, ior: 1.45, thickness: 0.7, scale: 55, orbitRx: 0.35, orbitRy: 0.08, speed: 1.0, phase: 0, yBase: 0.5 },
		{ size: 120, ior: 1.50, thickness: 0.6, scale: 40, orbitRx: 0.40, orbitRy: 0.10, speed: 1.3, phase: Math.PI * 0.7, yBase: 0.45 },
		{ size: 70,  ior: 1.55, thickness: 0.55, scale: 30, orbitRx: 0.30, orbitRy: 0.06, speed: 1.7, phase: Math.PI * 1.4, yBase: 0.55 },
	];

	let mapUrls = $state<string[]>([]);
	let sphereEls: (HTMLDivElement | undefined)[] = [undefined, undefined, undefined];
	let isSafari = false;

	let scrollProgress = 0;
	let smoothScroll = 0;
	let mouseX = 0.5;
	let mouseY = 0.5;
	let targetMX = 0.5;
	let targetMY = 0.5;

	onMount(() => {
		// Detect Safari
		isSafari = /^((?!chrome|android).)*safari/i.test(navigator.userAgent);

		// Generate displacement maps
		mapUrls = configs.map(c => generateSphereMap(c.size, c.ior, c.thickness));

		// Apply styles after maps are ready
		requestAnimationFrame(() => {
			configs.forEach((cfg, i) => {
				const el = sphereEls[i];
				if (!el) return;

				const glassEl = el.querySelector('.sphere-glass') as HTMLElement;
				const filterEl = el.querySelector('.sphere-filter') as HTMLElement;
				if (!glassEl) return;

				if (!isSafari && filterEl) {
					// Chrome/Edge: SVG filter for displacement
					filterEl.style.filter = `url(#sphere-refract-${i})`;
				}
			});
		});

		const onScroll = () => {
			const max = document.body.scrollHeight - window.innerHeight;
			scrollProgress = max > 0 ? window.scrollY / max : 0;
		};
		const onMouse = (e: MouseEvent) => {
			targetMX = e.clientX / window.innerWidth;
			targetMY = e.clientY / window.innerHeight;
		};
		window.addEventListener('scroll', onScroll, { passive: true });
		window.addEventListener('mousemove', onMouse, { passive: true });
		onScroll();

		let raf: number;
		function tick() {
			const t = performance.now() / 1000;
			smoothScroll += (scrollProgress - smoothScroll) * 0.06;
			mouseX += (targetMX - mouseX) * 0.04;
			mouseY += (targetMY - mouseY) * 0.04;

			const scrollAngle = smoothScroll * Math.PI * 6;
			const vw = window.innerWidth;
			const vh = window.innerHeight;

			configs.forEach((cfg, i) => {
				const el = sphereEls[i];
				if (!el) return;

				const angle = scrollAngle * cfg.speed + cfg.phase + t * 0.08;
				const cx = 0.5 + cfg.orbitRx * Math.cos(angle) + (mouseX - 0.5) * 0.15;
				const cy = cfg.yBase + cfg.orbitRy * Math.sin(scrollAngle * 0.5 + t * 0.3) + Math.sin(angle * 0.7) * 0.05 + (mouseY - 0.5) * -0.08;

				const px = cx * vw - cfg.size / 2;
				const py = cy * vh - cfg.size / 2;
				el.style.transform = `translate3d(${px}px, ${py}px, 0)`;
			});

			raf = requestAnimationFrame(tick);
		}
		raf = requestAnimationFrame(tick);

		return () => {
			cancelAnimationFrame(raf);
			window.removeEventListener('scroll', onScroll);
			window.removeEventListener('mousemove', onMouse);
		};
	});
</script>

<!-- SVG filters -->
<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" style="position:absolute">
	<defs>
		{#each configs as cfg, i}
			{#if mapUrls[i]}
				<filter
					id="sphere-refract-{i}"
					filterUnits="userSpaceOnUse"
					color-interpolation-filters="sRGB"
					x="0" y="0"
					width={cfg.size}
					height={cfg.size}
				>
					<feImage
						href={mapUrls[i]}
						width={cfg.size}
						height={cfg.size}
						result="dispMap"
					/>
					<feDisplacementMap
						in="SourceGraphic"
						in2="dispMap"
						scale={cfg.scale}
						xChannelSelector="R"
						yChannelSelector="G"
					/>
				</filter>
			{/if}
		{/each}
	</defs>
</svg>

<!-- Glass spheres -->
<div class="spheres-container">
	{#each configs as cfg, i}
		<div
			bind:this={sphereEls[i]}
			class="sphere-wrapper"
			style:width="{cfg.size}px"
			style:height="{cfg.size}px"
		>
			<!--
				Chrome: .sphere-filter has filter:url(#svg) which distorts the glass backdrop result
				Safari: .sphere-filter has no filter, just the backdrop glass look
			-->
			<div class="sphere-filter">
				<div class="sphere-glass"></div>
			</div>
			<!-- Decorations outside filter -->
			<div class="sphere-decor">
				<div class="specular"></div>
				<div class="fresnel"></div>
			</div>
		</div>
	{/each}
</div>

<style>
	.spheres-container {
		position: fixed;
		top: 0;
		left: 0;
		width: 0;
		height: 0;
		z-index: 50;
		pointer-events: none;
	}

	.sphere-wrapper {
		position: absolute;
		top: 0;
		left: 0;
		pointer-events: none;
		will-change: transform;
	}

	/* Filter wrapper — gets filter:url(#svg) set via JS on Chrome only */
	.sphere-filter {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		overflow: hidden;
		/* filter is applied via JS for Chrome */
	}

	/* Glass backdrop */
	.sphere-glass {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		backdrop-filter: blur(1px) saturate(1.5) brightness(1.15);
		-webkit-backdrop-filter: blur(1px) saturate(1.5) brightness(1.15);
	}

	/* Decorations */
	.sphere-decor {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		pointer-events: none;
		box-shadow:
			0 4px 24px oklch(0 0 0 / 15%),
			0 8px 48px oklch(0 0 0 / 8%),
			inset 0 2px 6px oklch(1 0 0 / 14%),
			inset 0 -10px 28px oklch(0 0 0 / 10%),
			0 0 0 0.5px oklch(1 0 0 / 10%);
	}

	.specular {
		position: absolute;
		top: 8%;
		left: 18%;
		width: 48%;
		height: 30%;
		border-radius: 50%;
		background: radial-gradient(
			ellipse at 50% 65%,
			oklch(1 0 0 / 28%) 0%,
			oklch(1 0 0 / 8%) 40%,
			transparent 70%
		);
		pointer-events: none;
		transform: rotate(-15deg);
	}

	.fresnel {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		background: radial-gradient(
			circle at 50% 50%,
			transparent 50%,
			oklch(1 0 0 / 4%) 65%,
			oklch(1 0 0 / 10%) 80%,
			oklch(1 0 0 / 18%) 93%,
			oklch(1 0 0 / 8%) 100%
		);
		pointer-events: none;
	}
</style>
