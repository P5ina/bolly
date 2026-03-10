<script lang="ts">
	let { data } = $props();
	let creating = $state(false);
	let slugInput = $state('');
	let selectedPlan = $state<'starter' | 'companion' | 'unlimited'>('starter');
	let errorMsg = $state('');
	let showCreate = $state(false);

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

			if (!res.ok) {
				const err = await res.json().catch(() => ({ message: res.statusText }));
				errorMsg = err.message ?? 'Failed to create companion';
				return;
			}

			// Reload page to show new tenant
			location.reload();
		} catch {
			errorMsg = 'Network error';
		} finally {
			creating = false;
		}
	}

	function companionUrl(flyAppId: string | null) {
		return flyAppId ? `https://${flyAppId}.fly.dev` : '#';
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
							<option value="starter">Starter ($5/mo)</option>
							<option value="companion">Companion ($12/mo)</option>
							<option value="unlimited">Unlimited ($25/mo)</option>
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

		<!-- tenant list -->
		{#if data.tenants.length === 0}
			<div class="text-center py-20">
				<p class="text-text-dim mb-2 font-display italic text-lg">no companions yet</p>
				<p class="text-text-ghost text-sm">Create one to get started.</p>
			</div>
		{:else}
			<div class="grid gap-3">
				{#each data.tenants as tenant}
					<a
						href={companionUrl(tenant.flyAppId)}
						target="_blank"
						class="group flex items-center justify-between p-5 rounded-xl border transition-all duration-300 hover:-translate-y-0.5"
						style="background: var(--color-bg); border-color: var(--color-border);"
					>
						<div class="flex items-center gap-4">
							<div class="w-10 h-10 rounded-lg flex items-center justify-center font-display italic text-warm"
								style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
							>
								{tenant.slug[0]?.toUpperCase()}
							</div>
							<div>
								<div class="text-text font-medium text-sm">{tenant.slug}<span class="text-text-ghost">.bollyai.dev</span></div>
								<div class="text-xs text-text-ghost mt-0.5">{tenant.plan} plan</div>
							</div>
						</div>
						<div class="flex items-center gap-3">
							<span class="inline-flex items-center gap-1.5 text-xs">
								<span
									class="w-1.5 h-1.5 rounded-full"
									class:bg-emerald-400={tenant.status === 'running'}
									class:bg-amber-400={tenant.status === 'provisioning'}
									class:bg-red-400={tenant.status === 'error'}
								></span>
								<span class="text-text-ghost">{tenant.status}</span>
							</span>
							<svg class="w-4 h-4 text-text-ghost group-hover:text-warm transition-colors" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/>
							</svg>
						</div>
					</a>
				{/each}
			</div>
		{/if}
	</div>
</div>
