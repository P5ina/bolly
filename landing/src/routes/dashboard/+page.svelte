<script lang="ts">
	import { page } from '$app/state';
	import { invalidateAll } from '$app/navigation';
	import { ExternalLink, AlertTriangle, Loader, Mail, CreditCard, CalendarClock, XCircle, RotateCw, RefreshCw, Share2, Check, Key, ChevronDown, ChevronUp, Trash2 } from 'lucide-svelte';
	import { enhance } from '$app/forms';

	const BYOK_ENABLED = false; // flip to true to re-enable BYOK UI

	let shared = $state<string | null>(null);
	let byokOpen = $state<string | null>(null);
	let byokSaving = $state<string | null>(null);
	let byokRemoving = $state<string | null>(null);
	let byokError = $state<string | null>(null);

	async function shareLink(slug: string, shareToken: string | null) {
		const base = `${window.location.origin}/connect/${slug}`;
		const url = shareToken ? `${base}?share=${shareToken}` : base;
		if (navigator.share) {
			await navigator.share({ title: `bolly — ${slug}`, url });
		} else {
			await navigator.clipboard.writeText(url);
			shared = slug;
			setTimeout(() => (shared = null), 2000);
		}
	}

	function formatPrice(cents: number) {
		return `$${(cents / 100).toFixed(0)}`;
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
	}

	let { data } = $props();
	let retrying = $state<string | null>(null);
	let cancelling = $state<string | null>(null);
	let switchingChannel = $state<string | null>(null);
	let creating = $state(false);
	let slugInput = $state('');
	let selectedPlan = $state<'starter' | 'companion' | 'unlimited'>('starter');
	let createByok = $state(false);
	let errorMsg = $state('');
	let showCreate = $state(false);
	let provisioning = $state(page.url.searchParams.get('checkout') === 'success');

	// Poll for tenant status after checkout
	$effect(() => {
		if (!provisioning) return;
		const interval = setInterval(async () => {
			await invalidateAll();
			const hasReady = data.tenants.some((t) => t.status === 'running' || t.status === 'error');
			if (hasReady) {
				provisioning = false;
				clearInterval(interval);
			}
		}, 3000);
		return () => clearInterval(interval);
	});

	async function createTenant() {
		if (!slugInput.trim()) return;
		creating = true;
		errorMsg = '';

		try {
			const res = await fetch('/api/tenants', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ slug: slugInput.trim().toLowerCase(), plan: selectedPlan, byok: createByok }),
			});

			const body = await res.json().catch(() => ({ message: res.statusText }));

			if (!res.ok) {
				errorMsg = body.message ?? 'Failed to create companion';
				return;
			}

			const { checkoutUrl } = body;
			if (checkoutUrl) {
				window.location.href = checkoutUrl;
			}
		} catch {
			errorMsg = 'Network error';
		} finally {
			creating = false;
		}
	}

	function connectUrl(slug: string) {
		return `/connect/${slug}`;
	}
</script>

<div class="min-h-dvh bg-bg">
	<!-- header -->
	<header class="border-b border-border" style="background: oklch(0.05 0.015 280 / 70%); backdrop-filter: blur(24px);">
		<div class="mx-auto max-w-[900px] px-6 py-4 flex items-center justify-between">
			<a href="/" class="flex items-center gap-2.5">
				<div class="w-7 h-7 rounded-md flex items-center justify-center font-display italic text-sm text-warm"
					style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
				>b</div>
				<span class="font-display italic text-lg text-text">bolly</span>
			</a>
			<div class="flex items-center gap-4">
				<span class="text-xs text-text-ghost">{data.user.email}</span>
				<form method="POST" action="?/logout">
					<button type="submit" class="text-xs text-text-ghost hover:text-text-dim transition-colors">
						Log out
					</button>
				</form>
			</div>
		</div>
	</header>

	<div class="mx-auto max-w-[900px] px-6 py-12">
		<div class="flex items-center justify-between mb-8">
			<h1 class="font-display italic text-2xl text-text">your companions</h1>
			<button
				onclick={() => showCreate = !showCreate}
				class="text-sm py-2 px-5 rounded-full text-warm transition-all duration-300"
				style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
			>
				{showCreate ? 'cancel' : '+ new companion'}
			</button>
		</div>

		<!-- create ritual overlay -->
		{#if showCreate}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="create-overlay" onkeydown={(e) => e.key === 'Escape' && (showCreate = false)}>
				<div class="create-ritual">
					<!-- ambient glow -->
					<div class="create-glow"></div>

					<!-- close -->
					<button class="create-close" onclick={() => showCreate = false}>
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-5 h-5"><path d="M6 18L18 6M6 6l12 12" stroke-linecap="round"/></svg>
					</button>

					<!-- step 1: name -->
					<div class="create-step create-step-name">
						<p class="create-hint">give it a name</p>
						<div class="create-name-row">
							<input
								bind:value={slugInput}
								placeholder="my-companion"
								class="create-name-input"
								oninput={(e) => { slugInput = (e.target as HTMLInputElement).value.toLowerCase().replace(/[^a-z0-9-]/g, ''); }}
							/>
							<span class="create-name-suffix">.bollyai.dev</span>
						</div>
					</div>

					<!-- step 2: plan -->
					<div class="create-step create-step-plan">
						<p class="create-hint">choose your plan</p>
						<div class="create-plan-grid">
							{#each [
								{ id: 'starter', name: 'Starter', price: 12, byokPrice: 5, tokens: '1M', desc: 'for exploring' },
								{ id: 'companion', name: 'Companion', price: 29, byokPrice: 10, tokens: '3M', desc: 'for daily use', featured: true },
								{ id: 'unlimited', name: 'Real Friend', price: 59, byokPrice: 19, tokens: '10M', desc: 'no limits' },
							] as plan}
								<button
									class="create-plan-card"
									class:create-plan-selected={selectedPlan === plan.id}
									class:create-plan-featured={plan.featured}
									onclick={() => selectedPlan = plan.id as typeof selectedPlan}
								>
									{#if plan.featured}
										<span class="create-plan-badge">popular</span>
									{/if}
									<span class="create-plan-name">{plan.name}</span>
									<span class="create-plan-price">
										\${BYOK_ENABLED && createByok ? plan.byokPrice : plan.price}<span class="create-plan-period">/mo</span>
									</span>
									<span class="create-plan-tokens">{plan.tokens} tokens</span>
									<span class="create-plan-desc">{plan.desc}</span>
								</button>
							{/each}
						</div>
						{#if BYOK_ENABLED}<label class="create-byok-toggle">
							<input type="checkbox" bind:checked={createByok} />
							<span>bring your own API key</span>
							{#if createByok}
								<span class="create-byok-note">hosting-only pricing</span>
							{/if}
						</label>{/if}
					</div>

					<!-- action -->
					<div class="create-step create-step-action">
						{#if errorMsg}
							<p class="create-error">{errorMsg}</p>
						{/if}
						<button
							onclick={createTenant}
							disabled={creating || !slugInput.trim()}
							class="create-submit"
							class:create-submit-loading={creating}
						>
							{#if creating}
								<div class="create-submit-spinner"></div>
								creating...
							{:else}
								begin — ${createByok
									? (selectedPlan === 'starter' ? 5 : selectedPlan === 'companion' ? 10 : 19)
									: (selectedPlan === 'starter' ? 12 : selectedPlan === 'companion' ? 29 : 59)}/mo
							{/if}
						</button>
					</div>
				</div>
			</div>
		{/if}

		{#if provisioning}
			<div class="mb-6 p-4 rounded-xl border flex items-center gap-3" style="background: var(--color-warm-ghost); border-color: var(--color-border-warm);">
				<div class="w-2 h-2 rounded-full bg-warm animate-pulse"></div>
				<p class="text-sm text-text-dim">Payment received — provisioning your companion...</p>
			</div>
		{/if}

		<!-- tenant list -->
		{#if data.tenants.length === 0 && !provisioning}
			<div class="text-center py-20">
				<p class="text-text-dim mb-2 font-display italic text-lg">no companions yet</p>
				<p class="text-text-ghost text-sm">Create one to get started.</p>
			</div>
		{:else}
			<div class="grid gap-3">
				{#each data.tenants as tenant}
					{#if tenant.status === 'error'}
						<div
							class="p-5 rounded-xl border"
							style="background: var(--color-bg); border-color: oklch(0.65 0.15 25 / 30%);"
						>
							<div class="flex items-center justify-between mb-3">
								<div class="flex items-center gap-4">
									<div class="w-10 h-10 rounded-lg flex items-center justify-center text-red-400/80"
										style="background: oklch(0.65 0.15 25 / 10%); border: 1px solid oklch(0.65 0.15 25 / 20%);"
									>
										<AlertTriangle size={18} />
									</div>
									<div>
										<div class="text-text font-medium text-sm">{tenant.slug}<span class="text-text-ghost">.bollyai.dev</span></div>
										<div class="text-xs text-red-400/70 mt-0.5">provisioning failed</div>
									</div>
								</div>
								<form method="POST" action="?/retryProvision" use:enhance={() => {
									retrying = tenant.slug;
									return async ({ update }) => {
										retrying = null;
										await update();
									};
								}}>
									<input type="hidden" name="slug" value={tenant.slug} />
									<button
										type="submit"
										disabled={retrying === tenant.slug}
										class="inline-flex items-center gap-1.5 text-xs py-2 px-4 rounded-lg text-warm transition-all duration-300 disabled:opacity-40"
										style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
									>
										<RotateCw size={13} class={retrying === tenant.slug ? 'animate-spin' : ''} />
										{retrying === tenant.slug ? 'Retrying...' : 'Retry'}
									</button>
								</form>
							</div>
							<p class="text-xs text-text-ghost leading-relaxed">
								Something went wrong while setting up your companion. You can retry or contact
								<a href="mailto:support@bollyai.dev" class="inline-flex items-center gap-1 text-warm underline underline-offset-2">
									<Mail size={11} />support@bollyai.dev</a>
								for help.
							</p>
						</div>
					{:else if tenant.status === 'provisioning'}
						<div
							class="p-5 rounded-xl border"
							style="background: var(--color-bg); border-color: var(--color-border-warm);"
						>
							<div class="flex items-center gap-4">
								<div class="w-10 h-10 rounded-lg flex items-center justify-center text-warm animate-spin"
									style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
								>
									<Loader size={18} />
								</div>
								<div>
									<div class="text-text font-medium text-sm">{tenant.slug}<span class="text-text-ghost">.bollyai.dev</span></div>
									<div class="text-xs text-text-ghost mt-0.5">provisioning...</div>
								</div>
							</div>
						</div>
					{:else}
						<div
							class="p-5 rounded-xl border"
							style="background: var(--color-bg); border-color: var(--color-border);"
						>
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-4">
									<div class="w-10 h-10 rounded-lg flex items-center justify-center font-display italic text-warm"
										style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
									>
										{tenant.slug[0]?.toUpperCase()}
									</div>
									<div>
										<div class="text-text font-medium text-sm">{tenant.slug}<span class="text-text-ghost">.bollyai.dev</span></div>
										<div class="flex items-center gap-3 mt-0.5">
											<span class="inline-flex items-center gap-1.5 text-xs">
												<span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
												<span class="text-text-ghost">running</span>
											</span>
										</div>
									</div>
								</div>
								<div class="flex items-center gap-2">
									<button
										onclick={() => shareLink(tenant.slug, tenant.shareToken)}
										class="inline-flex items-center gap-1.5 text-xs py-2 px-3 rounded-lg transition-all duration-300"
										style="color: var(--color-text-ghost); border: 1px solid var(--color-border);"
										title="Share connection link"
									>
										{#if shared === tenant.slug}
											<Check size={13} /> copied
										{:else}
											<Share2 size={13} /> share
										{/if}
									</button>
									<a
										href={connectUrl(tenant.slug)}
										target="_blank"
										class="group inline-flex items-center gap-1.5 text-xs py-2 px-4 rounded-lg text-warm transition-all duration-300 hover:-translate-y-0.5"
										style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
									>
										Open <ExternalLink size={13} />
									</a>
								</div>
							</div>

							<!-- Subscription details -->
							{#if tenant.subscription}
								<div class="mt-4 pt-4 flex items-center justify-between" style="border-top: 1px solid var(--color-border);">
									<div class="flex items-center gap-4 text-xs text-text-ghost">
										<span class="inline-flex items-center gap-1.5">
											<CreditCard size={13} class="text-text-ghost" />
											{tenant.planName}{BYOK_ENABLED && tenant.byok ? ' BYOK' : ''} — {formatPrice(tenant.subscription.amount)}/mo
										</span>
										<span class="inline-flex items-center gap-1.5">
											<CalendarClock size={13} class="text-text-ghost" />
											{#if tenant.subscription.cancelAtPeriodEnd}
												<span class="text-amber-400">cancels {formatDate(tenant.subscription.currentPeriodEnd)}</span>
											{:else}
												renews {formatDate(tenant.subscription.currentPeriodEnd)}
											{/if}
										</span>
									</div>
									<div class="flex items-center gap-3">
										{#if !tenant.subscription.cancelAtPeriodEnd}
											<form method="POST" action="?/cancelSubscription" use:enhance={() => {
												cancelling = tenant.subscription!.id;
												return async ({ update }) => {
													cancelling = null;
													await update();
												};
											}}>
												<input type="hidden" name="subscriptionId" value={tenant.subscription.id} />
												<button
													type="submit"
													disabled={cancelling === tenant.subscription.id}
													class="text-xs text-red-400/60 hover:text-red-400 transition-colors underline underline-offset-2 disabled:opacity-40"
												>
													{cancelling === tenant.subscription.id ? 'cancelling...' : 'cancel'}
												</button>
											</form>
										{/if}
										<form method="POST" action="?/billing" use:enhance>
											<button
												type="submit"
												class="text-xs text-text-ghost hover:text-text-dim transition-colors underline underline-offset-2"
											>
												manage billing
											</button>
										</form>
									</div>
								</div>
							{/if}

						<!-- BYOK section (hidden when BYOK_ENABLED=false) -->
						{#if BYOK_ENABLED}
						<div class="mt-3 pt-3" style="border-top: 1px solid var(--color-border);">
							{#if tenant.byok}
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-3 text-xs">
										<span class="inline-flex items-center gap-1.5 py-1 px-2.5 rounded-full" style="background: oklch(0.70 0.14 145 / 10%); border: 1px solid oklch(0.70 0.14 145 / 20%); color: oklch(0.75 0.12 145);">
											<Key size={11} /> BYOK active
										</span>
										<span class="text-text-ghost">
											{tenant.byok.provider} &middot; {tenant.byok.keyHint}
											{#if tenant.byok.model}&middot; {tenant.byok.model}{/if}
										</span>
									</div>
									<div class="flex items-center gap-2">
										<button onclick={() => { byokOpen = byokOpen === tenant.id ? null : tenant.id; byokError = null; }} class="text-xs text-text-ghost hover:text-text-dim transition-colors underline underline-offset-2">change</button>
										<form method="POST" action="?/removeBYOK" use:enhance={() => { byokRemoving = tenant.id; byokError = null; return async ({ result, update }) => { byokRemoving = null; if (result.type === 'failure') { byokError = (result.data as { error?: string })?.error ?? 'Failed'; } else { await update(); } }; }}>
											<input type="hidden" name="tenantId" value={tenant.id} />
											<button type="submit" disabled={byokRemoving === tenant.id} class="inline-flex items-center gap-1 text-xs text-red-400/60 hover:text-red-400 transition-colors disabled:opacity-40">
												<Trash2 size={11} /> {byokRemoving === tenant.id ? 'removing...' : 'remove'}
											</button>
										</form>
									</div>
								</div>
							{:else}
								<button onclick={() => { byokOpen = byokOpen === tenant.id ? null : tenant.id; byokError = null; }} class="flex items-center gap-2 text-xs text-text-ghost hover:text-text-dim transition-colors">
									<Key size={12} /> Use your own API key
									{#if byokOpen === tenant.id}<ChevronUp size={12} />{:else}<ChevronDown size={12} />{/if}
								</button>
							{/if}

							{#if byokOpen === tenant.id}
								<form method="POST" action="?/saveBYOK" class="mt-3 grid gap-3" use:enhance={() => { byokSaving = tenant.id; byokError = null; return async ({ result, update }) => { byokSaving = null; if (result.type === 'failure') { byokError = (result.data as { error?: string })?.error ?? 'Failed'; } else { byokOpen = null; await update(); } }; }}>
									<input type="hidden" name="tenantId" value={tenant.id} />
									<div class="grid gap-3 md:grid-cols-3">
										<div>
											<label for="byok-provider-{tenant.id}" class="block text-xs text-text-ghost mb-1">Provider</label>
											<select id="byok-provider-{tenant.id}" name="provider" class="w-full py-2 px-3 rounded-lg text-sm text-text outline-none" style="background: var(--color-bg-raised); border: 1px solid var(--color-border);">
												<option value="anthropic" selected={tenant.byok?.provider === 'anthropic'}>Anthropic</option>
												<option value="openai" selected={tenant.byok?.provider === 'openai'}>OpenAI</option>
												<option value="openrouter" selected={tenant.byok?.provider === 'openrouter'}>OpenRouter</option>
											</select>
										</div>
										<div>
											<label for="byok-key-{tenant.id}" class="block text-xs text-text-ghost mb-1">API Key</label>
											<input id="byok-key-{tenant.id}" name="apiKey" type="password" required placeholder={tenant.byok ? tenant.byok.keyHint : 'sk-...'} class="w-full py-2 px-3 rounded-lg text-sm text-text outline-none" style="background: var(--color-bg-raised); border: 1px solid var(--color-border);" />
										</div>
										<div>
											<label for="byok-model-{tenant.id}" class="block text-xs text-text-ghost mb-1">Model <span class="text-text-ghost/50">(optional)</span></label>
											<input id="byok-model-{tenant.id}" name="model" type="text" placeholder="default" value={tenant.byok?.model ?? ''} class="w-full py-2 px-3 rounded-lg text-sm text-text outline-none" style="background: var(--color-bg-raised); border: 1px solid var(--color-border);" />
										</div>
									</div>
									<p class="text-xs text-text-ghost/50 leading-relaxed">Anthropic: supports API keys and Claude subscription tokens (<code class="text-text-ghost/70">claude setup-token</code>). Subscription tokens may be revoked by Anthropic at any time — use at your own risk.</p>
									<div class="flex items-center justify-between">
										<p class="text-xs text-text-ghost/60 max-w-md">Your key will be validated and your subscription switches to hosting-only pricing. The companion will restart.</p>
										<button type="submit" disabled={byokSaving === tenant.id} class="text-xs py-2 px-5 rounded-lg text-warm transition-all duration-300 disabled:opacity-40" style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);">
											{byokSaving === tenant.id ? 'Validating...' : 'Save & activate'}
										</button>
									</div>
									{#if byokError}<p class="text-xs text-red-400/70 italic">{byokError}</p>{/if}
								</form>
							{/if}
						</div>
tttttt{/if}
						</div>
					{/if}
				{/each}
			</div>
		{/if}

		<!-- Orphaned subscriptions -->
		{#if data.orphanedSubscriptions.length > 0}
			<div class="mt-10">
				<h2 class="font-display italic text-lg text-text mb-4">unused subscriptions</h2>
				<p class="text-xs text-text-ghost mb-4">
					These subscriptions are active but not linked to any companion. This can happen if provisioning failed. You can cancel them to stop being charged.
				</p>
				<div class="grid gap-3">
					{#each data.orphanedSubscriptions as sub}
						<div
							class="p-5 rounded-xl border flex items-center justify-between"
							style="background: var(--color-bg); border-color: oklch(0.65 0.15 25 / 30%);"
						>
							<div class="flex items-center gap-4">
								<div class="w-10 h-10 rounded-lg flex items-center justify-center text-amber-400/80"
									style="background: oklch(0.78 0.12 75 / 8%); border: 1px solid oklch(0.78 0.12 75 / 15%);"
								>
									<CreditCard size={18} />
								</div>
								<div>
									<div class="text-text font-medium text-sm">
										{sub.productName ?? 'Bolly subscription'}
										{#if sub.metadata?.slug}
											<span class="text-text-ghost"> — {sub.metadata.slug}</span>
										{/if}
									</div>
									<div class="flex items-center gap-3 mt-0.5 text-xs text-text-ghost">
										<span>{formatPrice(sub.amount)}/mo</span>
										<span class="inline-flex items-center gap-1">
											<CalendarClock size={11} />
											{#if sub.cancelAtPeriodEnd}
												<span class="text-amber-400">cancels {formatDate(sub.currentPeriodEnd)}</span>
											{:else}
												renews {formatDate(sub.currentPeriodEnd)}
											{/if}
										</span>
									</div>
								</div>
							</div>
							{#if !sub.cancelAtPeriodEnd}
								<form method="POST" action="?/cancelSubscription" use:enhance={() => {
									cancelling = sub.id;
									return async ({ update }) => {
										cancelling = null;
										await update();
									};
								}}>
									<input type="hidden" name="subscriptionId" value={sub.id} />
									<button
										type="submit"
										disabled={cancelling === sub.id}
										class="inline-flex items-center gap-1.5 text-xs py-2 px-4 rounded-lg transition-all duration-300 text-red-400/80 hover:text-red-400 disabled:opacity-40"
										style="background: oklch(0.65 0.15 25 / 10%); border: 1px solid oklch(0.65 0.15 25 / 20%);"
									>
										<XCircle size={13} />
										{cancelling === sub.id ? 'Cancelling...' : 'Cancel'}
									</button>
								</form>
							{:else}
								<span class="text-xs text-amber-400/70 italic">cancellation pending</span>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	/* ═══ Creation Ritual ═══ */

	.create-overlay {
		position: fixed;
		inset: 0;
		z-index: 200;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.03 0.015 280 / 92%);
		backdrop-filter: blur(40px) saturate(1.2);
		animation: ritual-fade 0.4s cubic-bezier(0.16, 1, 0.3, 1);
	}

	@keyframes ritual-fade {
		from { opacity: 0; }
	}

	.create-ritual {
		position: relative;
		width: min(480px, calc(100vw - 3rem));
		display: flex;
		flex-direction: column;
		gap: 2.5rem;
		padding: 3rem 0;
		animation: ritual-rise 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes ritual-rise {
		from {
			opacity: 0;
			transform: translateY(20px) scale(0.97);
			filter: blur(4px);
		}
	}

	.create-glow {
		position: absolute;
		top: -60px;
		left: 50%;
		width: 300px;
		height: 300px;
		transform: translateX(-50%);
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 10%) 0%, transparent 70%);
		pointer-events: none;
		animation: glow-breathe 4s ease-in-out infinite;
	}

	@keyframes glow-breathe {
		0%, 100% { opacity: 0.6; transform: translateX(-50%) scale(1); }
		50% { opacity: 1; transform: translateX(-50%) scale(1.1); }
	}

	.create-close {
		position: absolute;
		top: 0;
		right: -3rem;
		color: oklch(0.88 0.02 75 / 25%);
		cursor: pointer;
		transition: color 0.2s;
		background: none;
		border: none;
	}
	.create-close:hover { color: oklch(0.88 0.02 75 / 60%); }

	/* ═══ Steps ═══ */

	.create-step {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.create-hint {
		font-family: 'Instrument Serif', Georgia, serif;
		font-style: italic;
		font-size: 1rem;
		color: oklch(0.88 0.02 75 / 40%);
		letter-spacing: 0.01em;
	}

	/* ═══ Name Input ═══ */

	.create-name-row {
		display: flex;
		align-items: stretch;
		border-radius: 0.75rem;
		overflow: hidden;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		transition: border-color 0.3s;
	}
	.create-name-row:focus-within {
		border-color: oklch(0.78 0.12 75 / 35%);
	}

	.create-name-input {
		flex: 1;
		padding: 1rem 1.25rem;
		font-family: 'JetBrains Mono', monospace;
		font-size: 1.1rem;
		color: oklch(0.92 0.04 75);
		background: oklch(0.08 0.015 280);
		border: none;
		outline: none;
		letter-spacing: 0.02em;
	}
	.create-name-input::placeholder {
		color: oklch(0.88 0.02 75 / 18%);
	}

	.create-name-suffix {
		display: flex;
		align-items: center;
		padding: 0 1rem;
		font-family: 'JetBrains Mono', monospace;
		font-size: 0.85rem;
		color: oklch(0.78 0.12 75 / 30%);
		background: oklch(0.06 0.015 280);
		border-left: 1px solid oklch(1 0 0 / 6%);
		white-space: nowrap;
	}

	/* ═══ Plan Cards ═══ */

	.create-plan-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 0.75rem;
	}

	.create-plan-card {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.3rem;
		padding: 1.5rem 1rem;
		border-radius: 0.875rem;
		border: 1px solid oklch(1 0 0 / 6%);
		background: oklch(0.07 0.015 280);
		cursor: pointer;
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		text-align: center;
	}
	.create-plan-card:hover {
		border-color: oklch(0.78 0.12 75 / 20%);
		transform: translateY(-2px);
	}

	.create-plan-selected {
		border-color: oklch(0.78 0.12 75 / 40%);
		background: oklch(0.09 0.02 75);
		box-shadow: 0 0 30px oklch(0.78 0.12 75 / 8%), inset 0 1px 0 oklch(0.78 0.12 75 / 10%);
	}

	.create-plan-badge {
		position: absolute;
		top: -0.5rem;
		font-family: 'JetBrains Mono', monospace;
		font-size: 0.6rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		padding: 0.15rem 0.5rem;
		border-radius: 1rem;
		background: oklch(0.78 0.12 75 / 15%);
		color: oklch(0.78 0.12 75);
		border: 1px solid oklch(0.78 0.12 75 / 25%);
	}

	.create-plan-name {
		font-family: 'DM Sans', sans-serif;
		font-size: 0.9rem;
		font-weight: 500;
		color: oklch(0.88 0.02 75 / 80%);
	}

	.create-plan-price {
		font-family: 'Instrument Serif', Georgia, serif;
		font-style: italic;
		font-size: 1.75rem;
		color: oklch(0.78 0.12 75);
		line-height: 1;
		margin: 0.25rem 0;
	}
	.create-plan-period {
		font-size: 0.8rem;
		color: oklch(0.78 0.12 75 / 50%);
	}

	.create-plan-tokens {
		font-family: 'JetBrains Mono', monospace;
		font-size: 0.72rem;
		color: oklch(0.88 0.02 75 / 35%);
	}

	.create-plan-desc {
		font-family: 'DM Sans', sans-serif;
		font-size: 0.78rem;
		color: oklch(0.88 0.02 75 / 25%);
		font-style: italic;
	}

	/* ═══ BYOK Toggle ═══ */

	.create-byok-toggle {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		font-family: 'JetBrains Mono', monospace;
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 35%);
		margin-top: 0.25rem;
	}
	.create-byok-toggle input {
		accent-color: oklch(0.78 0.12 75);
	}

	.create-byok-note {
		font-size: 0.65rem;
		color: oklch(0.72 0.15 155 / 60%);
		padding: 0.1rem 0.4rem;
		border-radius: 0.25rem;
		background: oklch(0.72 0.15 155 / 8%);
	}

	/* ═══ Submit ═══ */

	.create-error {
		font-family: 'JetBrains Mono', monospace;
		font-size: 0.78rem;
		color: oklch(0.7 0.18 25);
		text-align: center;
	}

	.create-submit {
		width: 100%;
		padding: 1rem;
		border-radius: 0.75rem;
		font-family: 'DM Sans', sans-serif;
		font-size: 1rem;
		font-weight: 500;
		letter-spacing: 0.02em;
		color: oklch(0.065 0.015 280);
		background: oklch(0.78 0.12 75);
		border: none;
		cursor: pointer;
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
	}
	.create-submit:hover:not(:disabled) {
		transform: translateY(-1px);
		box-shadow: 0 8px 30px oklch(0.78 0.12 75 / 25%), 0 2px 8px oklch(0.78 0.12 75 / 15%);
	}
	.create-submit:disabled {
		opacity: 0.3;
		cursor: not-allowed;
	}
	.create-submit-loading {
		opacity: 0.7;
	}

	.create-submit-spinner {
		width: 14px;
		height: 14px;
		border: 2px solid oklch(0.065 0.015 280 / 30%);
		border-top-color: oklch(0.065 0.015 280);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	/* ═══ Responsive ═══ */

	@media (max-width: 540px) {
		.create-plan-grid {
			grid-template-columns: 1fr;
		}
		.create-plan-card {
			flex-direction: row;
			justify-content: space-between;
			padding: 1rem 1.25rem;
		}
		.create-plan-price { font-size: 1.3rem; margin: 0; }
		.create-close { right: 0; top: -2rem; }
	}
</style>
