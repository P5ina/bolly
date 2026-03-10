<section class="hero">
	<!-- atmosphere -->
	<div class="hero-glow"></div>
	<div class="hero-glow-secondary"></div>
	<div class="hero-glow-tertiary"></div>

	<!-- particles -->
	<div class="hero-particles">
		{#each [
			{ x: 12, y: 20, d: -2 }, { x: 25, y: 65, d: -5 }, { x: 70, y: 30, d: -8 },
			{ x: 85, y: 55, d: -11 }, { x: 45, y: 80, d: -14 }, { x: 60, y: 15, d: -3 },
			{ x: 35, y: 40, d: -7 }, { x: 78, y: 75, d: -10 }, { x: 18, y: 50, d: -6 },
			{ x: 92, y: 25, d: -13 }
		] as p}
			<div
				class="hero-particle"
				style="left:{p.x}%;top:{p.y}%;animation-delay:{p.d}s"
			></div>
		{/each}
	</div>

	<div class="relative z-10 max-w-[750px] mx-auto text-center">
		<!-- orb -->
		<div class="flex justify-center mb-12">
			<div class="relative w-20 h-20">
				<div class="hero-orb-core"></div>
				<div class="hero-orb-ring"></div>
				<div class="hero-orb-ring-outer"></div>
				<div class="hero-orb-alive"></div>
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
			a friend that helps you<br><span class="text-warm">think, work & feel</span>
		</h1>

		<p
			class="text-[1.0625rem] leading-relaxed text-text-dim max-w-[540px] mx-auto mb-10"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.2s;"
		>
			Not a chatbot. A presence that remembers your goals, notices your mood,
			helps you study, and checks in when you've been quiet too long.
		</p>

		<div
			class="flex items-center justify-center gap-4 flex-wrap"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.3s;"
		>
			<a href="#pricing" class="btn-primary">
				Meet yours
				<svg class="w-4 h-4 transition-transform group-hover:translate-x-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
				</svg>
			</a>
			<a href="https://github.com/P5ina/bolly" target="_blank" class="btn-ghost">
				or self-host <code class="font-mono text-xs text-text-ghost bg-warm-ghost px-2 py-0.5 rounded border border-border">docker pull</code>
			</a>
		</div>

		<!-- trust line -->
		<div
			class="hero-trust"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.5s;"
		>
			<span class="hero-trust-item">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3 h-3"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" stroke-linecap="round" stroke-linejoin="round"/></svg>
				your data stays yours
			</span>
			<span class="hero-trust-sep"></span>
			<span class="hero-trust-item">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3 h-3"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" stroke-linecap="round" stroke-linejoin="round"/></svg>
				remembers everything
			</span>
			<span class="hero-trust-sep"></span>
			<span class="hero-trust-item">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3 h-3"><path d="M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25" stroke-linecap="round" stroke-linejoin="round"/></svg>
				alive 24/7
			</span>
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

	.hero-glow-tertiary {
		position: absolute;
		top: 55%;
		left: 58%;
		width: 400px;
		height: 400px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.65 0.10 160 / 2%) 0%, transparent 55%);
		animation: breathe 14s ease-in-out infinite;
		animation-delay: -7s;
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

	.hero-orb-alive {
		position: absolute;
		inset: -4px;
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 6%);
		animation: alive-expand 3.5s ease-in-out infinite;
		pointer-events: none;
	}
	@keyframes alive-expand {
		0%, 100% { transform: scale(1); opacity: 1; }
		50% { transform: scale(1.5); opacity: 0; }
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

	.hero-trust {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 1rem;
		margin-top: 2.5rem;
	}

	.hero-trust-item {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		font-size: 0.6875rem;
		letter-spacing: 0.04em;
		color: var(--color-text-ghost);
	}

	.hero-trust-item svg {
		color: oklch(0.78 0.12 75 / 25%);
	}

	.hero-trust-sep {
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 15%);
		flex-shrink: 0;
	}

	@media (max-width: 540px) {
		.hero-trust {
			flex-direction: column;
			gap: 0.5rem;
		}
		.hero-trust-sep {
			display: none;
		}
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
