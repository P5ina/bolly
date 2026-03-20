<script lang="ts">
	import GlassScene from './GlassScene.svelte';
</script>

<section class="hero">
	<!-- 3D starfield background -->
	<div class="hero-scene">
		<GlassScene />
	</div>

	<!-- radial ambient glows -->
	<div class="hero-glow"></div>
	<div class="hero-glow-secondary"></div>

	<div class="relative z-10 max-w-[750px] mx-auto text-center">
		<!-- badge -->
		<div class="hero-badge">
			<span class="hero-badge-dot"></span>
			now in beta
		</div>

		<h1 class="font-display font-normal italic text-text mb-6 leading-[1.08] tracking-tight"
			style="font-size: clamp(2.75rem, 6vw, 4.5rem); letter-spacing: -0.035em; animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.15s;"
		>
			a friend that helps you<br><span class="hero-accent">think, work & feel</span>
		</h1>

		<p
			class="text-[1.0625rem] leading-relaxed text-text-dim max-w-[540px] mx-auto mb-10"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.25s;"
		>
			Not a chatbot. A presence that remembers your goals, notices your mood,
			helps you study, and checks in when you've been quiet too long.
		</p>

		<div
			class="flex items-center justify-center gap-4 flex-wrap"
			style="animation: fade-up 0.8s cubic-bezier(0.16,1,0.3,1) both; animation-delay: 0.35s;"
		>
			<a href="#pricing" class="btn-primary">
				Meet yours
				<svg class="w-4 h-4 transition-transform group-hover:translate-x-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
				</svg>
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
		background: oklch(0.02 0.01 260);
	}

	.hero-scene {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}

	.hero-glow {
		position: absolute;
		top: 35%;
		left: 50%;
		width: 700px;
		height: 700px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.55 0.08 240 / 6%) 0%, oklch(0.55 0.08 240 / 2%) 30%, transparent 65%);
		animation: breathe 8s ease-in-out infinite;
		pointer-events: none;
		z-index: 1;
	}

	.hero-glow-secondary {
		position: absolute;
		top: 50%;
		left: 40%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.70 0.08 300 / 3%) 0%, transparent 60%);
		animation: breathe-slow 12s ease-in-out infinite;
		animation-delay: -4s;
		pointer-events: none;
		z-index: 1;
	}

	.hero-accent {
		background: linear-gradient(135deg, oklch(0.78 0.12 75), oklch(0.72 0.10 55), oklch(0.80 0.14 85));
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}

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
		animation: fade-up 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
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
		font-size: 0.8rem;
		letter-spacing: 0.04em;
		color: oklch(0.90 0.02 75 / 25%);
	}

	.hero-trust-item svg {
		color: oklch(0.78 0.12 75 / 30%);
	}

	.hero-trust-sep {
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(1 0 0 / 12%);
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
		z-index: 5;
	}

	.hero-scroll-line {
		width: 1px;
		height: 40px;
		background: linear-gradient(to bottom, oklch(1 0 0 / 12%), transparent);
		animation: scroll-pulse 2s ease-in-out infinite;
	}
</style>
