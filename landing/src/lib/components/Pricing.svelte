<script lang="ts">
	import Reveal from './Reveal.svelte';

	const BYOK_ENABLED = false; // flip to true to re-enable BYOK pricing
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

		<!-- BYOK toggle (hidden when BYOK_ENABLED=false) -->
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

		<div class="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-[400px] md:max-w-none mx-auto">
			{#each plans as plan, i}
				<Reveal delay={100 + i * 100}>
					<div class="price-card" class:featured={plan.featured}>
						{#if plan.badge}
							<div class="price-badge">{plan.badge}</div>
						{/if}

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
									<span class="w-1 h-1 rounded-full bg-warm-dim shrink-0"></span>
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

	/* --- BYOK toggle --- */

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
		border: 1px solid var(--color-border);
		background: transparent;
		transition: all 0.3s ease;
	}

	.byok-toggle:hover {
		border-color: oklch(1 0 0 / 12%);
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

	/* --- price cards --- */

	.price-card {
		padding: 2.5rem 2rem;
		border-radius: 1rem;
		border: 1px solid var(--color-border);
		background: var(--color-bg);
		transition: all 0.4s ease;
		position: relative;
		display: flex;
		flex-direction: column;
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
		border-color: oklch(0.78 0.12 75 / 40%);
		box-shadow: 0 0 60px oklch(0.78 0.12 75 / 5%);
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
		background: oklch(0.78 0.12 75 / 10%);
		border: 1px solid var(--color-border-warm);
		color: var(--color-warm);
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
