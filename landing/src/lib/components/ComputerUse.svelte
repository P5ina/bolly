<script lang="ts">
	import Reveal from './Reveal.svelte';

	const cards = [
		{
			title: 'multi-machine',
			desc: 'One agent, one memory. Deploy child agents across your devices — each wakes and sleeps on its own.',
		},
		{
			title: 'full desktop control',
			desc: 'Screenshots, mouse, keyboard, terminal, files. Not just browser automation — real computer use.',
		},
		{
			title: 'coordinated autonomy',
			desc: 'Child agents make decisions independently, then report to the main agent for correction. Shared + private memory.',
		},
	];
</script>

<section class="computer-use">
	<div class="computer-use-inner">
		<Reveal>
			<p class="section-label">Beyond chat</p>
		</Reveal>
		<Reveal delay={100}>
			<h2 class="section-title">lives on your machines</h2>
		</Reveal>
		<Reveal delay={200}>
			<p class="section-subtitle">
				Bolly can take screenshots, move the mouse, type, edit files, and run terminal commands.
				macOS, Windows, Linux — multiple machines at once, each managed by autonomous child agents.
			</p>
		</Reveal>

		<div class="cards">
			{#each cards as card, i}
				<Reveal delay={250 + i * 100}>
					<div class="card">
						<h3 class="card-title">{card.title}</h3>
						<p class="card-desc">{card.desc}</p>
					</div>
				</Reveal>
			{/each}
		</div>
	</div>
</section>

<style>
	.computer-use {
		padding: 6rem 0;
	}

	.computer-use-inner {
		max-width: 1100px;
		margin: 0 auto;
		padding: 0 1.5rem;
	}

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

	.section-subtitle {
		font-size: 0.9375rem;
		line-height: 1.7;
		color: var(--color-text-dim);
		max-width: 560px;
		margin-bottom: 3rem;
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1.5rem;
	}

	.card {
		padding: 2rem 1.75rem;
		border-radius: 1rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		position: relative;
		overflow: hidden;
		transition: all 0.4s ease;
	}

	.card::before {
		content: '';
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 15%), transparent);
		pointer-events: none;
	}

	.card::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 40%;
		background: linear-gradient(180deg, oklch(1 0 0 / 3%) 0%, transparent 100%);
		pointer-events: none;
		border-radius: 1rem 1rem 0 0;
	}

	.card:hover {
		border-color: oklch(1 0 0 / 14%);
		transform: translateY(-2px);
		box-shadow:
			0 4px 24px oklch(0 0 0 / 20%),
			inset 0 1px 0 oklch(1 0 0 / 6%);
	}

	.card-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: 1.25rem;
		color: var(--color-text);
		margin-bottom: 0.75rem;
		position: relative;
		z-index: 1;
	}

	.card-desc {
		font-size: 0.875rem;
		line-height: 1.65;
		color: var(--color-text-dim);
		position: relative;
		z-index: 1;
	}

	@media (max-width: 768px) {
		.cards {
			grid-template-columns: 1fr;
		}
	}
</style>
