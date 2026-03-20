<script lang="ts">
	import Reveal from './Reveal.svelte';

	const steps = [
		{
			num: '01',
			title: 'sign up',
			desc: 'Pick a plan and your private environment spins up in seconds. No setup, no servers to manage.',
		},
		{
			num: '02',
			title: 'shape who they are',
			desc: "Choose a personality template or write your own. Define how your friend thinks, speaks, and what they care about.",
		},
		{
			num: '03',
			title: 'just talk',
			desc: "That's it. They remember everything. Their mood shifts. They start journaling, reaching out, and growing — on their own.",
		},
	];
</script>

<section class="how" id="how">
	<div class="mx-auto max-w-[1100px] px-6">
		<Reveal>
			<p class="section-label">How it works</p>
		</Reveal>
		<Reveal delay={100}>
			<h2 class="section-title">three minutes to someone who gets you</h2>
		</Reveal>
		<Reveal delay={200}>
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-16">
				No complicated setup. Sign up, give them a name, start talking.
			</p>
		</Reveal>

		<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
			{#each steps as step, i}
				<Reveal delay={100 + i * 100}>
					<div class="step-card">
						<div class="step-card-inner">
							<div class="step-num">{step.num}</div>
							<h3 class="font-display italic font-normal text-xl text-text mb-2.5 -tracking-tight">
								{step.title}
							</h3>
							<p class="text-[0.8125rem] text-text-dim leading-relaxed">
								{step.desc}
							</p>
						</div>

						{#if i < steps.length - 1}
							<div class="step-connector"></div>
						{/if}
					</div>
				</Reveal>
			{/each}
		</div>
	</div>
</section>

<style>
	.how {
		padding: 7rem 0;
		background: oklch(0.03 0.012 260);
		border-top: 1px solid var(--glass-border);
		border-bottom: 1px solid var(--glass-border);
		position: relative;
	}

	.how::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 8%), transparent);
		pointer-events: none;
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

	.step-card {
		position: relative;
		border-radius: 1rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		overflow: hidden;
		transition: all 0.4s ease;
	}

	.step-card::before {
		content: '';
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 18%), transparent);
		pointer-events: none;
		z-index: 2;
	}

	.step-card::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 40%;
		background: linear-gradient(180deg, oklch(1 0 0 / 3%) 0%, transparent 100%);
		pointer-events: none;
		z-index: 1;
	}

	.step-card:hover {
		border-color: oklch(1 0 0 / 14%);
		transform: translateY(-2px);
		box-shadow: 0 4px 24px oklch(0 0 0 / 20%);
	}

	.step-card-inner {
		position: relative;
		z-index: 3;
		padding: 2.25rem 2rem;
	}

	.step-num {
		font-family: var(--font-display);
		font-style: italic;
		font-size: 3rem;
		line-height: 1;
		margin-bottom: 1rem;
		color: oklch(1 0 0 / 6%);
	}

	.step-connector {
		display: none;
	}

	@media (min-width: 768px) {
		.step-connector {
			display: block;
			position: absolute;
			top: 3rem;
			right: -0.75rem;
			width: 1.5rem;
			height: 1px;
			background: linear-gradient(90deg, oklch(1 0 0 / 10%), oklch(1 0 0 / 4%));
			z-index: 10;
		}
	}
</style>
