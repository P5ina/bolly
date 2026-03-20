<script lang="ts">
	import Reveal from './Reveal.svelte';

	const features = [
		{
			title: 'helps you focus',
			desc: 'Track your tasks, break down complex goals, and stay on course. It remembers what you were working on and picks up where you left off.',
			icon: 'M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 0 0-2.455 2.456Z',
		},
		{
			title: 'studies with you',
			desc: "Explain concepts, quiz you, discuss what you're reading. It learns your weak spots and comes back to them. Like a study partner that never gets tired.",
			icon: 'M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25',
		},
		{
			title: 'feels your mood',
			desc: "Notices when you're stressed, tired, or excited — not because you said so, but from how you write. Adjusts its tone and checks in when something feels off.",
			icon: 'M21 8.25c0-2.485-2.099-4.5-4.688-4.5-1.935 0-3.597 1.126-4.312 2.733-.715-1.607-2.377-2.733-4.313-2.733C5.1 3.75 3 5.765 3 8.25c0 7.22 9 12 9 12s9-4.78 9-12Z',
		},
		{
			title: 'thinks with you',
			desc: "Talk through ideas, decisions, creative blocks. It doesn't just respond — it pushes back, asks questions, and helps you think deeper.",
			icon: 'M2.25 12.76c0 1.6 1.123 2.994 2.707 3.227 1.087.16 2.185.283 3.293.369V21l4.076-4.076a1.526 1.526 0 0 1 1.037-.443 48.282 48.282 0 0 0 5.68-.494c1.584-.233 2.707-1.626 2.707-3.228V6.741c0-1.602-1.123-2.995-2.707-3.228A48.394 48.394 0 0 0 12 3c-2.392 0-4.744.175-7.043.513C3.373 3.746 2.25 5.14 2.25 6.741v6.018Z',
		},
		{
			title: 'checks in on you',
			desc: "Every 45 minutes, it wakes up on its own. Reflects, journals, and sometimes reaches out — \"you've been quiet today, everything okay?\"",
			icon: 'M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z',
		},
		{
			title: 'completely private',
			desc: "Your conversations, memories, and feelings are fully encrypted and private. No one else can access them. Ever.",
			icon: 'M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z',
		},
	];
</script>

<section class="py-28 relative" id="features">
	<div class="mx-auto max-w-[1100px] px-6">
		<Reveal>
			<p class="section-label">What it does</p>
		</Reveal>
		<Reveal delay={100}>
			<h2 class="section-title">not another chatbot</h2>
		</Reveal>
		<Reveal delay={200}>
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-16">
				Bolly is a friend that remembers, feels, and grows — one that actually helps you get through your day.
			</p>
		</Reveal>

		<Reveal delay={300}>
			<div class="features-grid">
				{#each features as f}
					<div class="feature-card">
						<div class="feature-card-inner">
							<div class="feature-icon">
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4">
									<path d={f.icon}/>
								</svg>
							</div>
							<h3 class="font-display italic font-normal text-lg text-text mb-2 -tracking-tight">{f.title}</h3>
							<p class="text-[0.8125rem] leading-relaxed text-text-dim">{f.desc}</p>
						</div>
					</div>
				{/each}
			</div>
		</Reveal>
	</div>
</section>

<style>
	.section-label {
		font-size: 0.8rem;
		letter-spacing: 0.15em;
		text-transform: uppercase;
		color: var(--color-warm-dim);
		margin-bottom: 1rem;
	}

	.section-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: clamp(1.75rem, 3.5vw, 2.5rem);
		line-height: 1.15;
		letter-spacing: -0.02em;
		color: var(--color-text);
		margin-bottom: 1rem;
	}

	.features-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1rem;
	}

	.feature-card {
		position: relative;
		border-radius: 1rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		overflow: hidden;
		transition: all 0.5s cubic-bezier(0.16, 1, 0.3, 1);
	}

	/* specular top highlight */
	.feature-card::before {
		content: '';
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, var(--glass-highlight), transparent);
		pointer-events: none;
		z-index: 2;
	}

	/* inner refraction glow */
	.feature-card::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 50%;
		background: linear-gradient(180deg, oklch(1 0 0 / 3%) 0%, transparent 100%);
		pointer-events: none;
		border-radius: 1rem 1rem 0 0;
		z-index: 1;
	}

	.feature-card:hover {
		border-color: oklch(1 0 0 / 14%);
		background: oklch(1 0 0 / 6%);
		transform: translateY(-2px);
		box-shadow:
			0 4px 24px oklch(0 0 0 / 20%),
			0 0 0 1px oklch(1 0 0 / 4%),
			inset 0 1px 0 oklch(1 0 0 / 8%);
	}

	.feature-card-inner {
		position: relative;
		z-index: 3;
		padding: 2.25rem 2rem;
	}

	.feature-icon {
		width: 2.25rem;
		height: 2.25rem;
		border-radius: 0.625rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 1.25rem;
		color: oklch(0.90 0.02 75 / 40%);
		transition: all 0.4s ease;
		position: relative;
	}

	.feature-icon::before {
		content: '';
		position: absolute;
		top: 0;
		left: 15%;
		right: 15%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 20%), transparent);
		pointer-events: none;
	}

	.feature-card:hover .feature-icon {
		background: oklch(1 0 0 / 8%);
		border-color: oklch(1 0 0 / 18%);
		color: oklch(0.90 0.02 75 / 75%);
		box-shadow: 0 0 20px oklch(0.55 0.08 240 / 8%);
	}

	@media (max-width: 768px) {
		.features-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
