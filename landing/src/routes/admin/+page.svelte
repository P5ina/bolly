<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import {
		Server,
		Play,
		Square,
		ExternalLink,
		AlertTriangle,
		Loader,
		Cpu,
		HardDrive,
		MessageSquare,
		Zap,
		RefreshCw,
	} from 'lucide-svelte';

	const PROVIDERS = ['anthropic', 'openrouter', 'openai'] as const;

	const MODELS: Record<string, { id: string; label: string }[]> = {
		anthropic: [
			{ id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
			{ id: 'claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5' },
		],
		openrouter: [
			{ id: 'anthropic/claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
			{ id: 'anthropic/claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5' },
			{ id: 'moonshotai/kimi-k2.5', label: 'Kimi K2.5' },
			{ id: 'google/gemini-2.5-flash', label: 'Gemini 2.5 Flash' },
		],
		openai: [
			{ id: 'gpt-4o', label: 'GPT-4o' },
			{ id: 'gpt-4o-mini', label: 'GPT-4o Mini' },
		],
	};

	let { data } = $props();
	let updating = $state<string | null>(null);
	let stopping = $state<string | null>(null);
	let starting = $state<string | null>(null);
	let provisioning = $state<string | null>(null);
	let actionError = $state<string | null>(null);
	let actionSuccess = $state<string | null>(null);
	let refreshing = $state(false);
	let syncing = $state(false);
	let updatingAll = $state(false);
	let resetting = $state<string | null>(null);
	let migrating = $state(false);
	let notifying = $state(false);
	let patching = $state(false);
	let priceMessage = $state('');

	function currentModel(tenant: (typeof data.tenants)[number]) {
		if (tenant.machine?.model) return tenant.machine.model;
		return 'default';
	}

	function currentProvider(tenant: (typeof data.tenants)[number]) {
		if (tenant.machine?.provider) return tenant.machine.provider;
		return 'openrouter';
	}

	function formatTokens(n: number) {
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
		return n.toString();
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
	}

	function statusColor(status: string) {
		switch (status) {
			case 'running': return 'bg-emerald-400';
			case 'stopped': return 'bg-amber-400';
			case 'provisioning': return 'bg-blue-400 animate-pulse';
			case 'error': return 'bg-red-400';
			default: return 'bg-neutral-500';
		}
	}

	async function refresh() {
		refreshing = true;
		await invalidateAll();
		refreshing = false;
	}
</script>

<div class="min-h-dvh bg-bg">
	<header class="border-b border-border" style="background: oklch(0.05 0.015 280 / 70%); backdrop-filter: blur(24px);">
		<div class="mx-auto max-w-[1100px] px-6 py-4 flex items-center justify-between">
			<div class="flex items-center gap-3">
				<a href="/" class="flex items-center gap-2.5">
					<div class="w-7 h-7 rounded-md flex items-center justify-center font-display italic text-sm text-warm"
						style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
					>b</div>
					<span class="font-display italic text-lg text-text">bolly</span>
				</a>
				<span class="text-xs px-2 py-0.5 rounded-full"
					style="background: oklch(0.65 0.15 25 / 12%); border: 1px solid oklch(0.65 0.15 25 / 25%); color: oklch(0.75 0.12 25);"
				>admin</span>
			</div>
			<div class="flex items-center gap-4">
				<button
					onclick={refresh}
					disabled={refreshing}
					class="text-xs text-text-ghost hover:text-text-dim transition-colors inline-flex items-center gap-1"
				>
					<RefreshCw size={12} class={refreshing ? 'animate-spin' : ''} />
					refresh
				</button>
				<a href="/dashboard" class="text-xs text-text-ghost hover:text-text-dim transition-colors">dashboard</a>
			</div>
		</div>
	</header>

	<div class="mx-auto max-w-[1100px] px-6 py-10">
		<!-- Stats row -->
		<div class="grid grid-cols-4 gap-4 mb-10">
			{#each [
				{ label: 'total', value: data.tenants.length, color: 'var(--color-text-dim)' },
				{ label: 'running', value: data.tenants.filter(t => t.status === 'running').length, color: 'oklch(0.72 0.15 155)' },
				{ label: 'stopped', value: data.tenants.filter(t => t.status === 'stopped').length, color: 'oklch(0.78 0.12 75)' },
				{ label: 'errors', value: data.tenants.filter(t => t.status === 'error').length, color: 'oklch(0.65 0.15 25)' },
			] as stat}
				<div class="p-4 rounded-xl border border-border" style="background: var(--color-bg);">
					<div class="text-2xl font-display italic" style="color: {stat.color};">{stat.value}</div>
					<div class="text-xs text-text-ghost mt-1">{stat.label}</div>
				</div>
			{/each}
		</div>

		{#if actionError}
			<div class="mb-4 p-3 rounded-lg text-xs text-red-400/80 border" style="background: oklch(0.65 0.15 25 / 8%); border-color: oklch(0.65 0.15 25 / 20%);">
				{actionError}
				<button onclick={() => actionError = null} class="ml-2 underline">dismiss</button>
			</div>
		{/if}

		{#if actionSuccess}
			<div class="mb-4 p-3 rounded-lg text-xs text-emerald-400/80 border" style="background: oklch(0.72 0.15 155 / 8%); border-color: oklch(0.72 0.15 155 / 20%);">
				{actionSuccess}
				<button onclick={() => actionSuccess = null} class="ml-2 underline">dismiss</button>
			</div>
		{/if}

		<!-- Create Machine -->
		<div class="mb-6 p-4 rounded-xl" style="background: oklch(0.72 0.12 200 / 4%); border: 1px solid oklch(0.72 0.12 200 / 12%);">
			<h3 class="text-sm font-mono mb-3" style="color: oklch(0.72 0.12 200 / 70%);">create machine</h3>
			<form method="POST" action="?/createMachine" use:enhance={() => {
				actionError = null;
				actionSuccess = null;
				return async ({ result, update }) => {
					if (result.type === 'failure') {
						actionError = (result.data as { error?: string })?.error ?? 'Failed';
					} else if (result.type === 'success') {
						actionSuccess = 'Machine created';
					}
					await update();
				};
			}}>
				<div class="flex items-end gap-3 flex-wrap">
					<div class="flex flex-col gap-1">
						<label for="cm-user" class="text-xs font-mono" style="color: var(--color-text-ghost);">user</label>
						<select id="cm-user" name="userId" required class="text-xs py-1.5 px-2 rounded-lg font-mono" style="background: oklch(1 0 0 / 4%); border: 1px solid oklch(1 0 0 / 8%); color: var(--color-text);">
							{#each data.users as user}
								<option value={user.id}>{user.email}</option>
							{/each}
						</select>
					</div>
					<div class="flex flex-col gap-1">
						<label for="cm-slug" class="text-xs font-mono" style="color: var(--color-text-ghost);">slug</label>
						<input id="cm-slug" name="slug" required placeholder="my-companion" class="text-xs py-1.5 px-2 rounded-lg font-mono w-40" style="background: oklch(1 0 0 / 4%); border: 1px solid oklch(1 0 0 / 8%); color: var(--color-text);" />
					</div>
					<div class="flex flex-col gap-1">
						<label for="cm-plan" class="text-xs font-mono" style="color: var(--color-text-ghost);">plan</label>
						<select id="cm-plan" name="plan" class="text-xs py-1.5 px-2 rounded-lg font-mono" style="background: oklch(1 0 0 / 4%); border: 1px solid oklch(1 0 0 / 8%); color: var(--color-text);">
							<option value="starter">Starter</option>
							<option value="companion" selected>Companion</option>
							<option value="unlimited">Unlimited</option>
						</select>
					</div>
					<button type="submit" class="inline-flex items-center gap-1.5 text-xs py-1.5 px-4 rounded-lg transition-all duration-300" style="color: oklch(0.72 0.12 200); background: oklch(0.72 0.12 200 / 10%); border: 1px solid oklch(0.72 0.12 200 / 25%);">
						<Server size={12} />
						create
					</button>
				</div>
			</form>
		</div>

		<div class="flex items-center justify-between mb-6">
			<h2 class="font-display italic text-xl text-text">instances</h2>
			<div class="flex items-center gap-2">
				<form method="POST" action="?/updateAllImages" use:enhance={() => {
					updatingAll = true;
					actionError = null;
					actionSuccess = null;
					return async ({ result, update }) => {
						updatingAll = false;
						if (result.type === 'failure') {
							actionError = (result.data as { error?: string })?.error ?? 'Update failed';
						} else if (result.type === 'success') {
							const data = result.data as { updated?: number };
							actionSuccess = `Updated image on ${data?.updated ?? 0} machine(s)`;
						}
						await update();
					};
				}}>
					<button
						type="submit"
						disabled={updatingAll}
						class="text-xs py-1.5 px-4 rounded-lg transition-all duration-300 disabled:opacity-40 inline-flex items-center gap-1.5"
						style="color: var(--color-text-dim); border: 1px solid var(--color-border);"
					>
						{#if updatingAll}
							<Loader size={12} class="animate-spin" />
						{/if}
						update all images
					</button>
				</form>
				<form method="POST" action="?/syncPlanLimits" use:enhance={() => {
					syncing = true;
					actionError = null;
					actionSuccess = null;
					return async ({ result, update }) => {
						syncing = false;
						if (result.type === 'failure') {
							actionError = (result.data as { error?: string })?.error ?? 'Sync failed';
						} else if (result.type === 'success') {
							const data = result.data as { updated?: number };
							actionSuccess = `Synced plan limits for ${data?.updated ?? 0} tenant(s)`;
						}
						await update();
					};
				}}>
					<button
						type="submit"
						disabled={syncing}
						class="text-xs py-1.5 px-4 rounded-lg transition-all duration-300 disabled:opacity-40 inline-flex items-center gap-1.5"
						style="color: var(--color-text-dim); border: 1px solid var(--color-border);"
					>
						{#if syncing}
							<Loader size={12} class="animate-spin" />
						{/if}
						sync plan limits
					</button>
				</form>
				<form method="POST" action="?/patchEnv" use:enhance={() => {
					patching = true;
					actionError = null;
					actionSuccess = null;
					return async ({ result, update }) => {
						patching = false;
						if (result.type === 'failure') {
							actionError = (result.data as { error?: string })?.error ?? 'Patch failed';
						} else if (result.type === 'success') {
							const data = result.data as { patched?: number };
							actionSuccess = `Patched env on ${data?.patched ?? 0} machine(s)`;
						}
						await update();
					};
				}}>
					<button
						type="submit"
						disabled={patching}
						class="text-xs py-1.5 px-4 rounded-lg transition-all duration-300 disabled:opacity-40 inline-flex items-center gap-1.5"
						style="color: var(--color-warm); border: 1px solid var(--color-border-warm);"
					>
						{#if patching}
							<Loader size={12} class="animate-spin" />
						{/if}
						patch env (all machines)
					</button>
				</form>
			</div>
		</div>

		<!-- Billing actions -->
		<div class="mb-10 p-5 rounded-xl border border-border" style="background: var(--color-bg);">
			<h3 class="font-display italic text-base text-text mb-4">billing</h3>
			<div class="flex flex-col gap-4">
				<form method="POST" action="?/notifyPriceChange" class="flex items-end gap-2" use:enhance={() => {
					notifying = true;
					actionError = null;
					actionSuccess = null;
					return async ({ result, update }) => {
						notifying = false;
						if (result.type === 'failure') {
							actionError = (result.data as { error?: string })?.error ?? 'Notify failed';
						} else if (result.type === 'success') {
							const data = result.data as { sent?: number };
							actionSuccess = `Sent price change email to ${data?.sent ?? 0} user(s)`;
						}
						await update();
					};
				}}>
					<div class="flex-1">
						<label class="block text-xs text-text-ghost mb-1">custom message (optional)</label>
						<input
							type="text"
							name="message"
							bind:value={priceMessage}
							placeholder="e.g. New pricing reflects increased model costs..."
							class="w-full py-2 px-3 rounded-lg text-xs text-text outline-none"
							style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
						/>
					</div>
					<button
						type="submit"
						disabled={notifying}
						class="text-xs py-2 px-4 rounded-lg transition-all duration-300 disabled:opacity-40 inline-flex items-center gap-1.5 whitespace-nowrap"
						style="color: var(--color-text-dim); border: 1px solid var(--color-border);"
					>
						{#if notifying}
							<Loader size={12} class="animate-spin" />
						{/if}
						notify price change
					</button>
				</form>
				<div class="flex items-center gap-2 pt-3" style="border-top: 1px solid var(--color-border);">
					<span class="text-xs text-text-ghost flex-1">migrate all active subscriptions to current Stripe price IDs</span>
					<form method="POST" action="?/migrateSubscriptions" use:enhance={() => {
						if (!confirm('This will update all active subscriptions to the new prices. Continue?')) { return async () => {}; }
						migrating = true;
						actionError = null;
						actionSuccess = null;
						return async ({ result, update }) => {
							migrating = false;
							if (result.type === 'failure') {
								actionError = (result.data as { error?: string })?.error ?? 'Migration failed';
							} else if (result.type === 'success') {
								const data = result.data as { migrated?: number };
								actionSuccess = `Migrated ${data?.migrated ?? 0} subscription(s)`;
							}
							await update();
						};
					}}>
						<button
							type="submit"
							disabled={migrating}
							class="text-xs py-2 px-4 rounded-lg transition-all duration-300 disabled:opacity-40 inline-flex items-center gap-1.5 whitespace-nowrap"
							style="color: oklch(0.65 0.15 25); background: oklch(0.65 0.15 25 / 8%); border: 1px solid oklch(0.65 0.15 25 / 20%);"
						>
							{#if migrating}
								<Loader size={12} class="animate-spin" />
							{/if}
							migrate subscriptions
						</button>
					</form>
				</div>
			</div>
		</div>

		{#if data.tenants.length === 0}
			<div class="text-center py-20">
				<p class="text-text-dim font-display italic text-lg">no instances</p>
			</div>
		{:else}
			<div class="grid gap-3">
				{#each data.tenants as tenant}
					<div
						class="rounded-xl border transition-all duration-300"
						style="background: var(--color-bg); border-color: var(--color-border);"
					>
						<!-- Instance header -->
						<div class="p-5">
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-4">
									<div class="w-10 h-10 rounded-lg flex items-center justify-center font-display italic text-warm"
										style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
									>
										{tenant.slug[0]?.toUpperCase()}
									</div>
									<div>
										<div class="flex items-center gap-2">
											<span class="text-text font-medium text-sm">{tenant.slug}<span class="text-text-ghost">.bollyai.dev</span></span>
											<span class="inline-flex items-center gap-1.5 text-xs">
												<span class="w-1.5 h-1.5 rounded-full {statusColor(tenant.status)}"></span>
												<span class="text-text-ghost">{tenant.status}</span>
											</span>
										</div>
										<div class="flex items-center gap-3 mt-1 text-xs text-text-ghost">
											<span>{tenant.userEmail}</span>
											<span class="px-1.5 py-0.5 rounded text-[0.625rem]"
												style="background: var(--color-warm-ghost); border: 1px solid var(--color-border);"
											>{tenant.plan}</span>
											<span>{tenant.imageChannel}</span>
											<span>created {formatDate(tenant.createdAt)}</span>
										</div>
									</div>
								</div>

								<div class="flex items-center gap-2">
									{#if tenant.status === 'running'}
										<form method="POST" action="?/stopMachine" use:enhance={() => {
											stopping = tenant.id;
											actionError = null;
											return async ({ result, update }) => {
												stopping = null;
												if (result.type === 'failure') {
													actionError = (result.data as { error?: string })?.error ?? 'Failed to stop';
												}
												await update();
											};
										}}>
											<input type="hidden" name="tenantId" value={tenant.id} />
											<button
												type="submit"
												disabled={stopping === tenant.id}
												class="inline-flex items-center gap-1 text-xs py-1.5 px-3 rounded-lg transition-all duration-300 disabled:opacity-40"
												style="color: oklch(0.75 0.12 25); background: oklch(0.65 0.15 25 / 8%); border: 1px solid oklch(0.65 0.15 25 / 20%);"
												title="Stop machine"
											>
												{#if stopping === tenant.id}
													<Loader size={12} class="animate-spin" />
												{:else}
													<Square size={12} />
												{/if}
												stop
											</button>
										</form>
									{/if}
									{#if tenant.status === 'stopped'}
										<form method="POST" action="?/startMachine" use:enhance={() => {
											starting = tenant.id;
											actionError = null;
											return async ({ result, update }) => {
												starting = null;
												if (result.type === 'failure') {
													actionError = (result.data as { error?: string })?.error ?? 'Failed to start';
												}
												await update();
											};
										}}>
											<input type="hidden" name="tenantId" value={tenant.id} />
											<button
												type="submit"
												disabled={starting === tenant.id}
												class="inline-flex items-center gap-1 text-xs py-1.5 px-3 rounded-lg transition-all duration-300 disabled:opacity-40"
												style="color: oklch(0.72 0.15 155); background: oklch(0.72 0.15 155 / 8%); border: 1px solid oklch(0.72 0.15 155 / 20%);"
												title="Start machine"
											>
												{#if starting === tenant.id}
													<Loader size={12} class="animate-spin" />
												{:else}
													<Play size={12} />
												{/if}
												start
											</button>
										</form>
									{/if}
									{#if tenant.errorMessage}
										<span class="inline-flex items-center gap-1 text-xs text-red-400/70">
											<AlertTriangle size={12} />
											{tenant.errorMessage}
										</span>
									{/if}
								</div>
							</div>

							<!-- Details row -->
							<div class="mt-4 pt-4 flex items-center gap-6 text-xs text-text-ghost" style="border-top: 1px solid var(--color-border);">
								<span class="inline-flex items-center gap-1.5">
									<Zap size={12} />
									{formatTokens(tenant.tokensPerMonth)}/mo
								</span>
								<span class="inline-flex items-center gap-1.5">
									<HardDrive size={12} />
									{(tenant.storageLimit / 1024).toFixed(0)} GB
								</span>
								{#if tenant.rateLimit}
									<span class="inline-flex items-center gap-1.5">
										<MessageSquare size={12} />
										{formatTokens(tenant.rateLimit.tokensThisMonth)} tokens used
									</span>
									<form method="POST" action="?/resetLimits" class="inline-flex" use:enhance={() => {
										resetting = tenant.id;
										actionError = null;
										actionSuccess = null;
										return async ({ result, update }) => {
											resetting = null;
											if (result.type === 'failure') {
												actionError = (result.data as { error?: string })?.error ?? 'Reset failed';
											} else if (result.type === 'success') {
												actionSuccess = `Limits reset for ${tenant.slug}`;
											}
											await update();
										};
									}}>
										<input type="hidden" name="tenantId" value={tenant.id} />
										<button
											type="submit"
											disabled={resetting === tenant.id}
											class="inline-flex items-center gap-1 text-[0.625rem] py-0.5 px-2 rounded-md transition-all duration-300 disabled:opacity-40"
											style="color: oklch(0.78 0.12 75 / 60%); border: 1px solid oklch(0.78 0.12 75 / 15%);"
										>
											{#if resetting === tenant.id}
												<Loader size={10} class="animate-spin" />
											{/if}
											reset
										</button>
									</form>
								{/if}
							</div>

							<!-- Model selector -->
							{#if tenant.status === 'running' || tenant.status === 'stopped'}
								<div class="mt-4 pt-4" style="border-top: 1px solid var(--color-border);">
									<div class="flex items-center gap-2 mb-3">
										<Server size={12} class="text-text-ghost" />
										<span class="text-xs text-text-ghost">current:</span>
										<span class="text-xs font-mono text-text-dim">{currentProvider(tenant)}</span>
										<span class="text-xs text-text-ghost">/</span>
										<span class="text-xs font-mono text-text-dim">{currentModel(tenant)}</span>
									</div>
									<form
										method="POST"
										action="?/updateModel"
										class="flex items-center gap-2"
										use:enhance={() => {
											updating = tenant.id;
											actionError = null;
											actionSuccess = null;
											return async ({ result, update }) => {
												updating = null;
												if (result.type === 'failure') {
													actionError = (result.data as { error?: string })?.error ?? 'Failed to update';
												} else if (result.type === 'success') {
													actionSuccess = `Model updated for ${tenant.slug}. Machine will restart.`;
												}
												await update();
											};
										}}
									>
										<input type="hidden" name="tenantId" value={tenant.id} />
										<select
											name="provider"
											class="py-2 px-3 rounded-lg text-xs text-text outline-none font-mono"
											style="background: var(--color-bg-raised); border: 1px solid var(--color-border); min-width: 8rem;"
											value={currentProvider(tenant)}
											onchange={(e) => {
												const form = (e.target as HTMLElement).closest('form')!;
												const modelSelect = form.querySelector('select[name="model"]') as HTMLSelectElement;
												const provider = (e.target as HTMLSelectElement).value;
												const models = MODELS[provider] ?? [];
												modelSelect.innerHTML = models.map(m =>
													`<option value="${m.id}">${m.label}</option>`
												).join('');
											}}
										>
											{#each PROVIDERS as p}
												<option value={p} selected={currentProvider(tenant) === p}>{p}</option>
											{/each}
										</select>
										<select
											name="model"
											class="flex-1 py-2 px-3 rounded-lg text-xs text-text outline-none font-mono"
											style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
										>
											{#each (MODELS[currentProvider(tenant)] ?? []) as m}
												<option value={m.id} selected={currentModel(tenant) === m.id}>{m.label}</option>
											{/each}
										</select>
										<button
											type="submit"
											disabled={updating === tenant.id}
											class="py-2 px-4 rounded-lg text-xs font-medium text-warm transition-all duration-300 disabled:opacity-40"
											style="background: oklch(0.78 0.12 75 / 12%); border: 1px solid oklch(0.78 0.12 75 / 20%);"
										>
											{#if updating === tenant.id}
												<Loader size={12} class="animate-spin" />
											{:else}
												apply
											{/if}
										</button>
									</form>
								</div>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
