<script lang="ts">
	import { page } from '$app/state';
	import { invalidateAll } from '$app/navigation';
	import { ExternalLink, AlertTriangle, Loader, Mail, CreditCard, CalendarClock, XCircle, RotateCw, RefreshCw, Share2, Check } from 'lucide-svelte';
	import { enhance } from '$app/forms';

	let shared = $state<string | null>(null);

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
				body: JSON.stringify({ slug: slugInput.trim().toLowerCase(), plan: selectedPlan }),
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

		<!-- create form -->
		{#if showCreate}
			<div class="mb-8 p-6 rounded-xl border border-border-warm" style="background: var(--color-bg-raised);">
				<div class="grid gap-4 md:grid-cols-[1fr_auto_auto] items-end">
					<div>
						<label for="slug" class="block text-xs text-text-ghost mb-1.5 tracking-wide">Subdomain</label>
						<div class="flex items-center gap-0">
							<input
								id="slug"
								bind:value={slugInput}
								placeholder="my-companion"
								class="flex-1 py-2.5 px-4 rounded-l-lg text-sm text-text outline-none"
								style="background: var(--color-bg); border: 1px solid var(--color-border); border-right: none;"
							/>
							<span class="py-2.5 px-3 text-xs text-text-ghost rounded-r-lg" style="background: var(--color-bg); border: 1px solid var(--color-border);">
								.bollyai.dev
							</span>
						</div>
					</div>
					<div>
						<label for="plan" class="block text-xs text-text-ghost mb-1.5 tracking-wide">Plan</label>
						<select
							id="plan"
							bind:value={selectedPlan}
							class="py-2.5 px-4 rounded-lg text-sm text-text outline-none"
							style="background: var(--color-bg); border: 1px solid var(--color-border);"
						>
							<option value="starter">Starter ($5/mo) — 100 msgs/day</option>
							<option value="companion">Companion ($12/mo) — 300 msgs/day</option>
							<option value="unlimited">Unlimited ($25/mo) — no limits</option>
						</select>
					</div>
					<button
						onclick={createTenant}
						disabled={creating || !slugInput.trim()}
						class="py-2.5 px-6 rounded-lg text-sm font-medium text-warm transition-all duration-300 disabled:opacity-40"
						style="background: oklch(0.78 0.12 75 / 12%); border: 1px solid oklch(0.78 0.12 75 / 20%);"
					>
						{creating ? 'Creating...' : 'Create'}
					</button>
				</div>
				{#if errorMsg}
					<p class="mt-3 text-xs text-red-400/70 italic">{errorMsg}</p>
				{/if}
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
											<form method="POST" action="?/switchChannel" use:enhance={() => {
												switchingChannel = tenant.id;
												return async ({ update }) => {
													switchingChannel = null;
													await update();
												};
											}} class="inline-flex items-center">
												<input type="hidden" name="tenantId" value={tenant.id} />
												<input type="hidden" name="channel" value={tenant.imageChannel === 'stable' ? 'nightly' : 'stable'} />
												<button
													type="submit"
													disabled={switchingChannel === tenant.id}
													class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full transition-all duration-300 disabled:opacity-40"
													style="background: {tenant.imageChannel === 'nightly' ? 'oklch(0.65 0.15 300 / 12%)' : 'oklch(0.5 0 0 / 8%)'}; border: 1px solid {tenant.imageChannel === 'nightly' ? 'oklch(0.65 0.15 300 / 25%)' : 'oklch(0.5 0 0 / 15%)'}; color: {tenant.imageChannel === 'nightly' ? 'oklch(0.75 0.12 300)' : 'var(--color-text-ghost)'};"
													title="Switch to {tenant.imageChannel === 'stable' ? 'nightly' : 'stable'} channel"
												>
													<RefreshCw size={10} class={switchingChannel === tenant.id ? 'animate-spin' : ''} />
													{tenant.imageChannel}
												</button>
											</form>
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
											{tenant.planName} — {formatPrice(tenant.priceMonthly)}/mo
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
