<script lang="ts">
	import Reveal from './Reveal.svelte';

	const plans = [
		{
			name: 'companion',
			desc: 'Everything you need',
			price: 10,
			features: [
				'Bring your own API key',
				'No rate limits',
				'20 GB storage',
				'Persistent memory',
				'Web browsing',
				'Email & calendar',
				'Computer use',
			],
			featured: true,
			badge: 'byok',
			icon: '/assets/plan-companion.png',
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
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-10">
				Bring your own Anthropic API key — pay only for hosting. No rate limits, full control.
			</p>
		</Reveal>

		<Reveal delay={200}>
			<div class="pricing-grid">
				{#each plans as plan}
					<div class="price-card" class:featured={plan.featured}>
						{#if plan.badge}
							<div class="price-badge">{plan.badge}</div>
						{/if}

						<div class="price-card-inner">
							<img src={plan.icon} alt="" class="plan-icon" />
							<div class="font-display italic text-xl text-text mb-1">{plan.name}</div>
							<div class="text-xs text-text-ghost mb-6">{plan.desc}</div>

							<div class="price-amount">
								<span class="text-xl text-text-dim font-light">$</span>
								<span class="font-display italic text-5xl text-text leading-none -tracking-wider">
									{plan.price}
								</span>
							</div>
							<div class="text-[0.8125rem] text-text-ghost mb-8">per month</div>

							<ul class="list-none mb-8 space-y-1.5 flex-1">
								{#each plan.features as feature}
									<li class="text-[0.8125rem] text-text-dim flex items-center gap-2">
										<span class="w-1 h-1 rounded-full shrink-0" style="background: oklch(0.55 0.08 240 / 30%);"></span>
										{feature}
									</li>
								{/each}
							</ul>

							<a href="/signup" class="price-btn" class:price-btn-featured={plan.featured}>
								Get started
							</a>
						</div>
					</div>
				{/each}
			</div>
		</Reveal>

		<Reveal delay={300}>
			<p class="oss-note">Open source · MIT license · Self-host free</p>
		</Reveal>
	</div>
</section>

<style>
	.plan-icon {
		width: 48px;
		height: 48px;
		object-fit: contain;
		margin-bottom: 1rem;
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

	.pricing-grid {
		display: grid;
		grid-template-columns: 1fr;
		gap: 1.5rem;
		max-width: 400px;
		margin: 0 auto;
		padding-top: 0.75rem;
	}

	.price-card {
		border-radius: 1rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		transition: all 0.4s ease;
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.price-card-inner::before {
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

	.price-card-inner::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 40%;
		background: linear-gradient(180deg, oklch(1 0 0 / 3%) 0%, transparent 100%);
		pointer-events: none;
		z-index: 1;
		border-radius: 1rem 1rem 0 0;
	}

	.price-card:hover {
		border-color: oklch(1 0 0 / 14%);
		transform: translateY(-2px);
		box-shadow:
			0 4px 24px oklch(0 0 0 / 20%),
			inset 0 1px 0 oklch(1 0 0 / 6%);
	}

	.price-card.featured {
		border-color: oklch(0.78 0.12 75 / 15%);
		border-top-color: oklch(0.78 0.12 75 / 25%);
		background: oklch(0.78 0.12 75 / 3%);
	}

	.price-card.featured .price-card-inner::before {
		background: linear-gradient(90deg, transparent, oklch(0.78 0.12 75 / 25%), transparent);
	}

	.price-card.featured .price-card-inner::after {
		background: linear-gradient(180deg, oklch(0.78 0.12 75 / 5%) 0%, transparent 100%);
	}

	.price-card.featured:hover {
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow:
			0 4px 32px oklch(0.78 0.12 75 / 8%),
			0 0 60px oklch(0.78 0.12 75 / 4%),
			inset 0 1px 0 oklch(0.78 0.12 75 / 10%);
	}

	.price-card-inner {
		position: relative;
		z-index: 3;
		padding: 2.5rem 2rem;
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
		border-radius: 1rem;
	}

	.price-badge {
		position: absolute;
		top: -0.6rem;
		left: 50%;
		transform: translateX(-50%);
		font-size: 0.85rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		padding: 0.25rem 0.875rem;
		border-radius: 1rem;
		background: oklch(0.78 0.12 75 / 8%);
		backdrop-filter: var(--glass-blur);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		border-top-color: oklch(0.78 0.12 75 / 25%);
		color: var(--color-warm);
		z-index: 4;
	}

	.price-amount {
		display: flex;
		align-items: baseline;
		gap: 0.25rem;
		margin-bottom: 0.25rem;
	}

	.price-btn {
		display: block;
		width: 100%;
		padding: 0.75rem;
		border-radius: 0.75rem;
		font-size: 0.8125rem;
		font-weight: 500;
		letter-spacing: 0.02em;
		text-align: center;
		text-decoration: none;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		color: var(--color-text-dim);
		transition: all 0.3s ease;
		position: relative;
		overflow: hidden;
	}

	.price-btn::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 50%;
		background: linear-gradient(180deg, oklch(1 0 0 / 4%) 0%, transparent 100%);
		pointer-events: none;
	}

	.price-btn:hover {
		border-color: oklch(1 0 0 / 16%);
		color: var(--color-text);
		box-shadow: inset 0 1px 0 oklch(1 0 0 / 8%);
	}

	.price-btn-featured {
		background: oklch(0.78 0.12 75 / 10%);
		border-color: oklch(0.78 0.12 75 / 18%);
		border-top-color: oklch(0.78 0.12 75 / 28%);
		color: var(--color-warm);
	}

	.price-btn-featured::before {
		background: linear-gradient(180deg, oklch(0.78 0.12 75 / 6%) 0%, transparent 100%);
	}

	.oss-note {
		text-align: center;
		font-size: 0.8125rem;
		letter-spacing: 0.04em;
		color: oklch(0.90 0.02 75 / 30%);
		margin-top: 2rem;
	}

	.price-btn-featured:hover {
		background: oklch(0.78 0.12 75 / 15%);
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow:
			0 0 30px oklch(0.78 0.12 75 / 8%),
			inset 0 1px 0 oklch(0.78 0.12 75 / 12%);
	}
</style>
