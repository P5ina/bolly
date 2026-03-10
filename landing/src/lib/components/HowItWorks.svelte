<script lang="ts">
	import Reveal from './Reveal.svelte';

	const steps = [
		{
			num: '01',
			title: 'sign up',
			desc: 'Choose a plan. Your isolated environment spins up in seconds — a private server just for your companion.',
		},
		{
			num: '02',
			title: 'shape its soul',
			desc: 'Pick a personality template or write your own soul.md. Define who your companion is, how it speaks, what drives it.',
		},
		{
			num: '03',
			title: 'let it grow',
			desc: 'Talk to it. It remembers everything. Its mood shifts. It starts journaling, creating drops, reaching out on its own.',
		},
	];
</script>

<section class="how" id="how">
	<div class="mx-auto max-w-[1100px] px-6">
		<Reveal>
			<p class="section-label">How it works</p>
		</Reveal>
		<Reveal delay={100}>
			<h2 class="section-title">three minutes to alive</h2>
		</Reveal>
		<Reveal delay={200}>
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-16">
				No configuration needed. Pick a plan, and your companion starts breathing.
			</p>
		</Reveal>

		<div class="grid grid-cols-1 md:grid-cols-3 gap-8 md:gap-8">
			{#each steps as step, i}
				<Reveal delay={100 + i * 100}>
					<div class="relative px-6 py-8">
						<div class="font-display italic text-5xl leading-none mb-4" style="color: oklch(0.78 0.12 75 / 8%);">
							{step.num}
						</div>
						<h3 class="font-display italic font-normal text-xl text-text mb-2.5 -tracking-tight">
							{step.title}
						</h3>
						<p class="text-[0.8125rem] text-text-dim leading-relaxed">
							{step.desc}
						</p>

						{#if i < steps.length - 1}
							<div class="hidden md:block absolute top-14 -right-4 w-8 h-px" style="background: var(--color-border-warm);"></div>
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
		background: var(--color-bg-raised);
		border-top: 1px solid var(--color-border);
		border-bottom: 1px solid var(--color-border);
	}

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
</style>
