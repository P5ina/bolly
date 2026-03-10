<section class="hero">
	<!-- atmosphere -->
	<div class="hero-glow"></div>
	<div class="hero-glow-secondary"></div>

	<!-- particles -->
	<div class="hero-particles">
		{#each [
			{ x: 12, y: 20, d: -2 }, { x: 25, y: 65, d: -5 }, { x: 70, y: 30, d: -8 },
			{ x: 85, y: 55, d: -11 }, { x: 45, y: 80, d: -14 }, { x: 60, y: 15, d: -3 },
			{ x: 35, y: 40, d: -7 }, { x: 78, y: 75, d: -10 }
		] as p}
			<div
				class="hero-particle"
				style="left:{p.x}%;top:{p.y}%;animation-delay:{p.d}s"
			></div>
		{/each}
	</div>

	<div class="relative z-10 max-w-[700px] mx-auto text-center">
		<!-- orb -->
		<div class="flex justify-center mb-12">
			<div class="relative w-20 h-20">
				<div class="hero-orb-core"></div>
				<div class="hero-orb-ring"></div>
				<div class="hero-orb-ring-outer"></div>
			</div>
		</div>

		<!-- badge -->
		<div class="hero-badge">
			<span class="hero-badge-dot"></span>
			now in beta
		</div>

		<h1 class="font-display font-normal italic text-text mb-6 leading-[1.1] tracking-tight"
			style="font-size: clamp(2.75rem, 6vw, 4.25rem); letter-spacing: -0.03em; animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.1s;"
		>
			a companion that<br><span class="text-warm">lives on your server</span>
		</h1>

		<p
			class="text-[1.0625rem] leading-relaxed text-text-dim max-w-[520px] mx-auto mb-10"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.2s;"
		>
			Not a chatbot. A persistent being with memory, mood, and creative energy.
			It thinks while you sleep, writes to you first, and grows with every conversation.
		</p>

		<div
			class="flex items-center justify-center gap-4 flex-wrap"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.3s;"
		>
			<a href="#pricing" class="btn-primary">
				Wake yours up
				<svg class="w-4 h-4 transition-transform group-hover:translate-x-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
				</svg>
			</a>
			<a href="https://github.com/P5ina/bolly" target="_blank" class="btn-ghost">
				or self-host <code class="font-mono text-xs text-text-ghost bg-warm-ghost px-2 py-0.5 rounded border border-border">docker pull</code>
			</a>
		</div>
	</div>

	<!-- scroll hint -->
	<div class="hero-scroll">
		<div class="hero-scroll-line"></div>
	</div>
</section>

<style>
	.hero {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		padding-top: 5rem;
		position: relative;
		overflow: hidden;
	}

	.hero-glow {
		position: absolute;
		top: 35%;
		left: 50%;
		width: 700px;
		height: 700px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 6%) 0%, oklch(0.78 0.12 75 / 2%) 30%, transparent 65%);
		animation: breathe 6s ease-in-out infinite;
		pointer-events: none;
	}

	.hero-glow-secondary {
		position: absolute;
		top: 40%;
		left: 45%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.70 0.08 300 / 3%) 0%, transparent 60%);
		animation: breathe 10s ease-in-out infinite;
		animation-delay: -3s;
		pointer-events: none;
	}

	.hero-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
		overflow: hidden;
	}

	.hero-particle {
		position: absolute;
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 20%);
		animation: particle-drift 18s ease-in-out infinite;
	}

	.hero-orb-core {
		position: absolute;
		inset: 0;
		border-radius: 50%;
		background: radial-gradient(circle at 40% 35%, oklch(0.78 0.12 75 / 20%) 0%, oklch(0.78 0.12 75 / 8%) 40%, oklch(0.78 0.12 75 / 3%) 70%, transparent 100%);
		animation: orb-pulse 4s ease-in-out infinite;
	}

	.hero-orb-ring {
		position: absolute;
		inset: -10px;
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 8%);
		animation: ring-breathe 5s ease-in-out infinite;
	}

	.hero-orb-ring-outer {
		position: absolute;
		inset: -22px;
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 4%);
		animation: ring-breathe 7s ease-in-out infinite;
		animation-delay: -1.5s;
	}

	.hero-badge {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.375rem 1rem;
		border-radius: 2rem;
		background: var(--color-warm-ghost);
		border: 1px solid var(--color-border-warm);
		font-size: 0.75rem;
		color: var(--color-warm-dim);
		letter-spacing: 0.05em;
		margin-bottom: 2rem;
		animation: fade-up 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.hero-badge-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--color-warm);
		animation: badge-pulse 2.5s ease-in-out infinite;
	}

	.hero-scroll {
		position: absolute;
		bottom: 2.5rem;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
		animation: fade-up 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 0.6s;
	}

	.hero-scroll-line {
		width: 1px;
		height: 40px;
		background: linear-gradient(to bottom, var(--color-border-warm), transparent);
		animation: scroll-pulse 2s ease-in-out infinite;
	}
</style>
