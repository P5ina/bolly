<script lang="ts">
	import Reveal from './Reveal.svelte';

	const plans = [
		{
			name: 'starter',
			desc: 'See if it clicks',
			price: 12,
			features: ['1M tokens / month', '10 GB storage', 'Memory & mood tracking'],
			featured: false,
		},
		{
			name: 'companion',
			desc: 'For everyday life',
			price: 29,
			features: ['3M tokens / month', '20 GB storage', 'Web browsing', 'Email integration'],
			featured: true,
			badge: 'popular',
		},
		{
			name: 'unlimited',
			desc: 'No limits',
			price: 59,
			features: ['10M tokens / month', '50 GB storage', 'Web browsing', 'Early access to new features'],
			featured: false,
		},
	];
</script>

<section class="py-28" id="pricing">
	<div class="mx-auto max-w-[1100px] px-6">
		<Reveal>
			<p class="section-label">Pricing</p>
		</Reveal>
		<Reveal delay={100}>
			<h2 class="section-title">simple, transparent</h2>
		</Reveal>
		<Reveal delay={200}>
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-16">
				Every plan includes full privacy, persistent memory, and a companion that never sleeps.
				AI included — no API key needed.
			</p>
		</Reveal>

		<div class="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-[400px] md:max-w-none mx-auto">
			{#each plans as plan, i}
				<Reveal delay={100 + i * 100}>
					<div class="price-card" class:featured={plan.featured}>
						{#if plan.badge}
							<div class="price-badge">{plan.badge}</div>
						{/if}

						<div class="font-display italic text-xl text-text mb-1">{plan.name}</div>
						<div class="text-xs text-text-ghost mb-6">{plan.desc}</div>

						<div class="flex items-baseline gap-1 mb-1">
							<span class="text-xl text-text-dim font-light">$</span>
							<span class="font-display italic text-5xl text-text leading-none -tracking-wider">{plan.price}</span>
						</div>
						<div class="text-[0.8125rem] text-text-ghost mb-8">per month</div>

						<ul class="list-none mb-8 space-y-1.5">
							{#each plan.features as feature}
								<li class="text-[0.8125rem] text-text-dim flex items-center gap-2">
									<span class="w-1 h-1 rounded-full bg-warm-dim shrink-0"></span>
									{feature}
								</li>
							{/each}
						</ul>

						<a
							href="/signup"
							class="block w-full py-3 rounded-lg text-[0.8125rem] font-medium tracking-wide text-center transition-all duration-300"
							class:price-btn-primary={plan.featured}
							class:price-btn-default={!plan.featured}
						>
							Get started
						</a>
					</div>
				</Reveal>
			{/each}
		</div>
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

	.price-card {
		padding: 2.5rem 2rem;
		border-radius: 1rem;
		border: 1px solid var(--color-border);
		background: var(--color-bg);
		transition: all 0.4s ease;
		position: relative;
	}

	.price-card:hover {
		border-color: oklch(1 0 0 / 8%);
		transform: translateY(-2px);
	}

	.price-card.featured {
		border-color: var(--color-border-warm);
		background: linear-gradient(180deg, oklch(0.78 0.12 75 / 3%) 0%, var(--color-bg) 40%);
	}

	.price-card.featured:hover {
		border-color: oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 60px oklch(0.78 0.12 75 / 5%);
	}

	.price-badge {
		position: absolute;
		top: -0.6rem;
		left: 50%;
		transform: translateX(-50%);
		font-size: 0.625rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		padding: 0.25rem 0.875rem;
		border-radius: 1rem;
		background: oklch(0.78 0.12 75 / 10%);
		border: 1px solid var(--color-border-warm);
		color: var(--color-warm);
	}

	.price-btn-default {
		background: var(--color-warm-ghost);
		border: 1px solid var(--color-border);
		color: var(--color-text-dim);
	}

	.price-btn-default:hover {
		border-color: var(--color-border-warm);
		color: var(--color-text);
	}

	.price-btn-primary {
		background: oklch(0.78 0.12 75 / 12%);
		border: 1px solid oklch(0.78 0.12 75 / 20%);
		color: var(--color-warm);
	}

	.price-btn-primary:hover {
		background: oklch(0.78 0.12 75 / 18%);
		border-color: oklch(0.78 0.12 75 / 35%);
		box-shadow: 0 0 30px oklch(0.78 0.12 75 / 8%);
	}
</style>
