<section class="hero">
	<!-- Breathing ambient glows -->
	<div class="hero-glow" aria-hidden="true"></div>
	<div class="hero-glow-secondary" aria-hidden="true"></div>

	<!-- Floating glass particles -->
	<div class="hero-particles" aria-hidden="true">
		<div class="particle p1"></div>
		<div class="particle p2"></div>
		<div class="particle p3"></div>
		<div class="particle p4"></div>
		<div class="particle p5"></div>
	</div>

	<!-- Content -->
	<div class="hero-content">
		<!-- Badge -->
		<div class="hero-badge" style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both;">
			<span class="hero-badge-dot"></span>
			now in beta
		</div>

		<!-- Headline -->
		<h1 class="hero-title" style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.12s;">
			a friend that helps you<br><span class="hero-accent">think, work & feel</span>
		</h1>

		<!-- Subtitle -->
		<p class="hero-subtitle" style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.22s;">
			Not a chatbot. A presence that remembers your goals, notices your mood,
			helps you study, and checks in when you've been quiet too long.
		</p>

		<!-- CTA -->
		<div class="hero-actions" style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.32s;">
			<a href="#pricing" class="btn-primary">
				Meet yours
				<svg class="hero-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
				</svg>
			</a>
		</div>

		<!-- Glass orb -->
		<div class="hero-orb-wrap" style="animation: fade-up 1s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.45s;">
			<div class="hero-orb">
				<video
					class="hero-orb-video"
					autoplay
					muted
					loop
					playsinline
					aria-hidden="true"
					poster="/assets/hero-orb.webp"
				>
					<source src="/assets/hero-orb.mp4" type="video/mp4" />
				</video>
				<div class="hero-orb-glow"></div>
			</div>
		</div>

		<!-- Trust signals -->
		<div class="hero-trust" style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.55s;">
			<span class="hero-trust-item">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
				your data stays yours
			</span>
			<span class="hero-trust-sep"></span>
			<span class="hero-trust-item">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
				remembers everything
			</span>
			<span class="hero-trust-sep"></span>
			<span class="hero-trust-item">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25"/></svg>
				alive 24/7
			</span>
		</div>
	</div>

	<!-- Scroll indicator -->
	<div class="hero-scroll" style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.7s;">
		<div class="hero-scroll-line"></div>
	</div>
</section>

<style>
	.hero {
		min-height: 100vh;
		min-height: 100dvh;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 6rem 1.5rem 3rem;
		position: relative;
		overflow: hidden;
	}

	/* ── Breathing ambient glows ── */
	.hero-glow {
		position: absolute;
		top: 30%;
		left: 50%;
		width: 800px;
		height: 800px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.55 0.08 240 / 6%) 0%, oklch(0.55 0.08 240 / 2%) 30%, transparent 65%);
		animation: glow-breathe 8s ease-in-out infinite;
		pointer-events: none;
		z-index: 0;
	}

	.hero-glow-secondary {
		position: absolute;
		top: 55%;
		left: 40%;
		width: 600px;
		height: 600px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.70 0.08 300 / 3%) 0%, transparent 60%);
		animation: glow-breathe-slow 12s ease-in-out infinite;
		animation-delay: -4s;
		pointer-events: none;
		z-index: 0;
	}

	@keyframes glow-breathe {
		0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 1; }
		50% { transform: translate(-50%, -50%) scale(1.12); opacity: 0.6; }
	}

	@keyframes glow-breathe-slow {
		0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 1; }
		50% { transform: translate(-50%, -50%) scale(1.08); opacity: 0.5; }
	}

	/* ── Floating glass particles ── */
	.hero-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
		z-index: 1;
	}

	.particle {
		position: absolute;
		border-radius: 50%;
		border: 1px solid oklch(1 0 0 / 6%);
		background: oklch(1 0 0 / 2%);
		backdrop-filter: blur(4px);
	}

	.p1 {
		width: 60px;
		height: 60px;
		top: 18%;
		left: 8%;
		animation: float-1 18s ease-in-out infinite;
	}

	.p2 {
		width: 35px;
		height: 35px;
		top: 65%;
		right: 12%;
		animation: float-2 22s ease-in-out infinite;
		animation-delay: -6s;
	}

	.p3 {
		width: 24px;
		height: 24px;
		top: 30%;
		right: 18%;
		animation: float-3 16s ease-in-out infinite;
		animation-delay: -3s;
	}

	.p4 {
		width: 45px;
		height: 45px;
		bottom: 25%;
		left: 15%;
		animation: float-4 20s ease-in-out infinite;
		animation-delay: -9s;
	}

	.p5 {
		width: 18px;
		height: 18px;
		top: 50%;
		left: 35%;
		animation: float-5 14s ease-in-out infinite;
		animation-delay: -2s;
	}

	@keyframes float-1 {
		0%, 100% { transform: translate(0, 0) rotate(0deg); opacity: 0.4; }
		25% { transform: translate(15px, -25px) rotate(45deg); opacity: 0.7; }
		50% { transform: translate(-10px, -40px) rotate(90deg); opacity: 0.3; }
		75% { transform: translate(20px, -15px) rotate(135deg); opacity: 0.6; }
	}

	@keyframes float-2 {
		0%, 100% { transform: translate(0, 0); opacity: 0.3; }
		33% { transform: translate(-20px, -30px); opacity: 0.6; }
		66% { transform: translate(15px, -15px); opacity: 0.25; }
	}

	@keyframes float-3 {
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 0.5; }
		50% { transform: translate(-25px, 20px) scale(1.2); opacity: 0.2; }
	}

	@keyframes float-4 {
		0%, 100% { transform: translate(0, 0); opacity: 0.35; }
		40% { transform: translate(25px, -20px); opacity: 0.6; }
		70% { transform: translate(-10px, -35px); opacity: 0.2; }
	}

	@keyframes float-5 {
		0%, 100% { transform: translate(0, 0); opacity: 0.3; }
		50% { transform: translate(20px, -30px); opacity: 0.5; }
	}

	/* ── Content ── */
	.hero-content {
		position: relative;
		z-index: 5;
		max-width: 750px;
		text-align: center;
	}

	/* ── Badge ── */
	.hero-badge {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.375rem 1rem;
		border-radius: 2rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		font-size: 0.85rem;
		color: oklch(0.90 0.02 75 / 50%);
		letter-spacing: 0.05em;
		margin-bottom: 2rem;
		position: relative;
	}

	.hero-badge::before {
		content: '';
		position: absolute;
		top: 0;
		left: 15%;
		right: 15%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 20%), transparent);
		pointer-events: none;
	}

	.hero-badge-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75);
		animation: badge-pulse 2.5s ease-in-out infinite;
		box-shadow: 0 0 8px oklch(0.78 0.12 75 / 40%);
	}

	@keyframes badge-pulse {
		0%, 100% { opacity: 1; box-shadow: 0 0 8px oklch(0.78 0.12 75 / 40%); }
		50% { opacity: 0.4; box-shadow: 0 0 4px oklch(0.78 0.12 75 / 15%); }
	}

	/* ── Title ── */
	.hero-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: clamp(2.75rem, 6vw, 4.5rem);
		line-height: 1.08;
		letter-spacing: -0.035em;
		color: var(--color-text);
		margin-bottom: 1.5rem;
	}

	.hero-accent {
		background: linear-gradient(135deg, oklch(0.78 0.12 75), oklch(0.72 0.10 55), oklch(0.80 0.14 85));
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}

	/* ── Subtitle ── */
	.hero-subtitle {
		font-size: 1.0625rem;
		line-height: 1.625;
		color: var(--color-text-dim);
		max-width: 540px;
		margin: 0 auto 2.5rem;
	}

	/* ── CTA ── */
	.hero-actions {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 1rem;
		margin-bottom: 3rem;
	}

	.hero-arrow {
		width: 1rem;
		height: 1rem;
		transition: transform 0.3s ease;
	}

	/* ── Glass orb ── */
	.hero-orb-wrap {
		display: flex;
		justify-content: center;
		margin-bottom: 2.5rem;
	}

	.hero-orb {
		position: relative;
		width: clamp(400px, 65vw, 800px);
		aspect-ratio: 16 / 9;
		animation: orb-breathe 6s ease-in-out infinite;
	}

	.hero-orb-video {
		width: 100%;
		height: 100%;
		object-fit: cover;
		mask-image: radial-gradient(ellipse 60% 70% at 50% 50%, black 20%, transparent 55%);
		-webkit-mask-image: radial-gradient(ellipse 60% 70% at 50% 50%, black 20%, transparent 55%);
		pointer-events: none;
		user-select: none;
	}

	@media (prefers-reduced-motion: reduce) {
		.hero-orb-video { display: none; }
	}

	.hero-orb-glow {
		position: absolute;
		inset: -20%;
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 6%) 0%, transparent 60%);
		pointer-events: none;
		animation: orb-glow-pulse 6s ease-in-out infinite;
		animation-delay: -1s;
	}

	@keyframes orb-breathe {
		0%, 100% { transform: scale(1) translateY(0); }
		50% { transform: scale(1.03) translateY(-4px); }
	}

	@keyframes orb-glow-pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.5; transform: scale(1.1); }
	}

	/* ── Trust signals ── */
	.hero-trust {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 1rem;
	}

	.hero-trust-item {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		font-size: 0.8rem;
		letter-spacing: 0.04em;
		color: oklch(0.90 0.02 75 / 25%);
	}

	.hero-trust-item svg {
		width: 0.75rem;
		height: 0.75rem;
		color: oklch(0.78 0.12 75 / 30%);
		flex-shrink: 0;
	}

	.hero-trust-sep {
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(1 0 0 / 12%);
		flex-shrink: 0;
	}

	/* ── Scroll indicator ── */
	.hero-scroll {
		position: absolute;
		bottom: 2rem;
		left: 50%;
		transform: translateX(-50%);
		z-index: 5;
	}

	.hero-scroll-line {
		width: 1px;
		height: 40px;
		background: linear-gradient(to bottom, oklch(1 0 0 / 12%), transparent);
		animation: scroll-pulse 2.5s ease-in-out infinite;
	}

	@keyframes scroll-pulse {
		0%, 100% { opacity: 0.4; transform: scaleY(1); }
		50% { opacity: 1; transform: scaleY(1.2); }
	}

	/* ── Responsive ── */
	@media (max-width: 768px) {
		.hero {
			padding: 5rem 1.25rem 2.5rem;
		}

		.hero-subtitle {
			font-size: 0.9375rem;
		}

		.hero-glow {
			width: 500px;
			height: 500px;
		}

		.hero-glow-secondary {
			width: 350px;
			height: 350px;
		}

		.particle { display: none; }

		.hero-orb {
			width: clamp(300px, 80vw, 500px);
		}
	}

	@media (max-width: 540px) {
		.hero-trust {
			flex-direction: column;
			gap: 0.5rem;
		}

		.hero-trust-sep {
			display: none;
		}

		.hero-title {
			font-size: clamp(2rem, 8vw, 2.75rem);
		}
	}

	/* ── Reduced motion ── */
	@media (prefers-reduced-motion: reduce) {
		.hero-glow,
		.hero-glow-secondary,
		.particle,
		.hero-orb,
		.hero-orb-glow,
		.hero-badge-dot,
		.hero-scroll-line {
			animation: none !important;
		}
	}

	/* ── Fade up animation ── */
	@keyframes fade-up {
		from { opacity: 0; transform: translateY(24px); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>
