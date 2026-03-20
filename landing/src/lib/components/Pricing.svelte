<script lang="ts">
	import Reveal from './Reveal.svelte';

	const BYOK_ENABLED = false;
	let byok = $state(false);

	const plans = [
		{
			name: 'starter',
			desc: 'See if it clicks',
			price: 12,
			byokPrice: 5,
			features: ['1M tokens / month', '10 GB storage', 'Memory & mood tracking'],
			featured: false,
		},
		{
			name: 'companion',
			desc: 'For everyday life',
			price: 29,
			byokPrice: 10,
			features: ['3M tokens / month', '20 GB storage', 'Web browsing', 'Email integration'],
			featured: true,
			badge: 'popular',
		},
		{
			name: 'real friend',
			desc: 'No limits',
			price: 59,
			byokPrice: 19,
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
			<p class="text-[0.9375rem] text-text-dim max-w-[480px] mb-10">
				Every plan includes full privacy, persistent memory, and a companion that never sleeps.
				{#if BYOK_ENABLED && byok}
					Bring your own API key — pay only for hosting.
				{:else}
					AI included — no API key needed.
				{/if}
			</p>
		</Reveal>

		{#if BYOK_ENABLED}
		<Reveal delay={250}>
			<div class="byok-toggle-row">
				<button
					class="byok-toggle"
					class:byok-active={byok}
					onclick={() => byok = !byok}
				>
					<span class="byok-track">
						<span class="byok-thumb"></span>
					</span>
					<span class="byok-label">I have my own API key</span>
				</button>
				{#if BYOK_ENABLED && byok}
					<span class="byok-hint">hosting-only pricing · no rate limits</span>
				{/if}
			</div>
		</Reveal>
		{/if}

		<Reveal delay={200}>
			<div class="pricing-grid">
				{#each plans as plan, i}
					<div class="price-card" class:featured={plan.featured}>
						{#if plan.badge}
							<div class="price-badge">{plan.badge}</div>
						{/if}

						<div class="price-card-inner">
							<div class="font-display italic text-xl text-text mb-1">{plan.name}</div>
							<div class="text-xs text-text-ghost mb-6">{plan.desc}</div>

							<div class="price-amount">
								<span class="text-xl text-text-dim font-light">$</span>
								{#key byok}
									<span class="font-display italic text-5xl text-text leading-none -tracking-wider price-num">
										{byok ? plan.byokPrice : plan.price}
									</span>
								{/key}
							</div>
							<div class="text-[0.8125rem] text-text-ghost mb-1">per month</div>
							{#if BYOK_ENABLED && byok}
								<div class="price-was">was ${plan.price}/mo</div>
							{/if}
							<div class={BYOK_ENABLED && byok ? 'mb-4' : 'mb-8'}></div>

							<ul class="list-none mb-8 space-y-1.5 flex-1">
								{#each plan.features as feature}
									<li class="text-[0.8125rem] text-text-dim flex items-center gap-2">
										<span class="w-1 h-1 rounded-full shrink-0" style="background: oklch(0.55 0.08 240 / 30%);"></span>
										{feature}
									</li>
								{/each}
								{#if BYOK_ENABLED && byok}
									<li class="text-[0.8125rem] flex items-center gap-2 byok-feature">
										<span class="w-1 h-1 rounded-full shrink-0" style="background: oklch(0.70 0.14 145);"></span>
										No rate limits
									</li>
								{/if}
							</ul>

							<a
								href="/signup"
								class="price-btn"
								class:price-btn-featured={plan.featured}
							>
								Get started
							</a>
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

	/* ── BYOK toggle ── */
	.byok-toggle-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin-bottom: 2.5rem;
	}

	.byok-toggle {
		display: flex;
		align-items: center;
		gap: 0.625rem;
		cursor: pointer;
		padding: 0.5rem 1rem 0.5rem 0.5rem;
		border-radius: 2rem;
		border: 1px solid var(--glass-border);
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		transition: all 0.3s ease;
	}

	.byok-toggle:hover {
		border-color: oklch(1 0 0 / 14%);
	}

	.byok-toggle.byok-active {
		border-color: oklch(0.70 0.14 145 / 30%);
		background: oklch(0.70 0.14 145 / 5%);
	}

	.byok-track {
		position: relative;
		width: 2rem;
		height: 1.125rem;
		border-radius: 1rem;
		background: oklch(1 0 0 / 8%);
		transition: background 0.3s ease;
		flex-shrink: 0;
	}

	.byok-active .byok-track {
		background: oklch(0.70 0.14 145 / 30%);
	}

	.byok-thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 0.875rem;
		height: 0.875rem;
		border-radius: 50%;
		background: oklch(0.55 0.02 280);
		transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
	}

	.byok-active .byok-thumb {
		left: calc(100% - 2px - 0.875rem);
		background: oklch(0.75 0.12 145);
	}

	.byok-label {
		font-size: 0.9rem;
		color: var(--color-text-dim);
		letter-spacing: 0.01em;
	}

	.byok-active .byok-label {
		color: oklch(0.80 0.10 145);
	}

	.byok-hint {
		font-size: 0.8rem;
		color: oklch(0.65 0.08 145 / 60%);
		letter-spacing: 0.02em;
		animation: hint-in 0.3s ease both;
	}

	@keyframes hint-in {
		from { opacity: 0; transform: translateX(-4px); }
		to { opacity: 1; transform: translateX(0); }
	}

	/* ── Grid ── */
	.pricing-grid {
		display: grid;
		grid-template-columns: 1fr;
		gap: 1.5rem;
		max-width: 400px;
		margin: 0 auto;
		padding-top: 0.75rem; /* room for badge */
	}

	@media (min-width: 768px) {
		.pricing-grid {
			grid-template-columns: repeat(3, 1fr);
			max-width: none;
			align-items: stretch;
		}
	}

	/* ── Glass price cards ── */
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

	/* specular top line — on inner to avoid clipping badge */
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

	/* inner refraction */
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

	.price-num {
		animation: price-swap 0.35s cubic-bezier(0.34, 1.56, 0.64, 1) both;
	}

	@keyframes price-swap {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.price-was {
		font-size: 0.8rem;
		color: oklch(0.55 0.02 280 / 40%);
		text-decoration: line-through;
		animation: hint-in 0.3s ease both;
	}

	.byok-feature {
		color: oklch(0.75 0.10 145);
		animation: hint-in 0.3s ease both;
	}

	/* ── Glass buttons ── */
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

	.price-btn-featured:hover {
		background: oklch(0.78 0.12 75 / 15%);
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow:
			0 0 30px oklch(0.78 0.12 75 / 8%),
			inset 0 1px 0 oklch(0.78 0.12 75 / 12%);
	}
</style>
