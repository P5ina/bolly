<script lang="ts">
	import Reveal from './Reveal.svelte';

	const features = [
		{
			title: 'knows you',
			desc: 'Semantic memory that extracts facts from every conversation. It remembers what matters and recalls it when relevant.',
			icon: 'M12 2a5 5 0 0 1 5 5c0 2.76-2.24 5-5 5s-5-2.24-5-5a5 5 0 0 1 5-5zM19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2',
		},
		{
			title: 'feels',
			desc: "Mood shifts naturally based on conversation. Sentiment tracking reads your emotional state. A living 3D blob reflects how it feels.",
			icon: 'M12 22c5.52 0 10-4.48 10-10S17.52 2 12 2 2 6.48 2 12s4.48 10 10 10zM8 14s1.5 2 4 2 4-2 4-2M9 9h.01M15 9h.01',
		},
		{
			title: 'creates',
			desc: "Drops — autonomous creative artifacts. Poems, ideas, observations, stories. Generated during heartbeat cycles while you're away.",
			icon: 'M12 2l3.09 6.26L22 9.27l-5 4.87L18.18 21 12 17.77 5.82 21 7 14.14l-5-4.87 6.91-1.01L12 2z',
		},
		{
			title: 'acts',
			desc: '25+ tools with full system access. Reads files, runs commands, installs packages, sends emails. Real capabilities, not pretend.',
			icon: 'M2 3h20v14H2zM8 21h8M12 17v4',
		},
		{
			title: 'heartbeat',
			desc: 'Wakes every 45 minutes on its own. Reflects, journals, updates mood, creates drops. Alive even when you\'re not there.',
			icon: 'M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z',
		},
		{
			title: 'your server',
			desc: "Complete isolation. Your companion runs in its own environment — own filesystem, own memory, own soul. Nobody else can access it.",
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
			<h2 class="section-title">not another AI wrapper</h2>
		</Reveal>
		<Reveal delay={200}>
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-16">
				Bolly is a living system — it remembers, feels, creates, and acts autonomously.
			</p>
		</Reveal>

		<Reveal delay={300}>
			<div class="features-grid">
				{#each features as f}
					<div class="feature-card">
						<div class="feature-icon">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4">
								<path d={f.icon}/>
							</svg>
						</div>
						<h3 class="font-display italic font-normal text-lg text-text mb-2 -tracking-tight">{f.title}</h3>
						<p class="text-[0.8125rem] leading-relaxed text-text-dim">{f.desc}</p>
					</div>
				{/each}
			</div>
		</Reveal>
	</div>
</section>

<style>
	.section-label {
		font-size: 0.6875rem;
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
		gap: 1px;
		background: var(--color-border);
		border: 1px solid var(--color-border);
		border-radius: 1rem;
		overflow: hidden;
	}

	.feature-card {
		background: var(--color-bg);
		padding: 2.25rem 2rem;
		transition: all 0.5s cubic-bezier(0.16, 1, 0.3, 1);
		position: relative;
	}

	.feature-card::before {
		content: '';
		position: absolute;
		inset: 0;
		background: radial-gradient(ellipse at 50% 0%, oklch(0.78 0.12 75 / 3%) 0%, transparent 70%);
		opacity: 0;
		transition: opacity 0.5s ease;
		pointer-events: none;
	}

	.feature-card:hover::before {
		opacity: 1;
	}

	.feature-icon {
		width: 2.25rem;
		height: 2.25rem;
		border-radius: 0.625rem;
		background: var(--color-warm-ghost);
		border: 1px solid var(--color-border-warm);
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 1.25rem;
		color: var(--color-warm-dim);
		transition: all 0.4s ease;
	}

	.feature-card:hover .feature-icon {
		background: oklch(0.78 0.12 75 / 8%);
		border-color: oklch(0.78 0.12 75 / 20%);
		color: var(--color-warm);
		box-shadow: 0 0 20px oklch(0.78 0.12 75 / 6%);
	}

	@media (max-width: 768px) {
		.features-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
