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

	const MODEL_PRESETS = [
		{ provider: 'openrouter', model: 'anthropic/claude-sonnet-4-6', label: 'Claude Sonnet 4.6', cost: '$3 / $15' },
		{ provider: 'openrouter', model: 'anthropic/claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5', cost: '$0.80 / $4' },
		{ provider: 'openrouter', model: 'moonshotai/kimi-k2.5', label: 'Kimi K2.5', cost: '$0.14 / $0.42' },
		{ provider: 'openrouter', model: 'google/gemini-2.5-flash', label: 'Gemini 2.5 Flash', cost: '$0.15 / $0.60' },
		{ provider: 'anthropic', model: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6 (direct)', cost: '$3 / $15' },
	] as const;

	let { data } = $props();
	let updating = $state<string | null>(null);
	let stopping = $state<string | null>(null);
	let starting = $state<string | null>(null);
	let actionError = $state<string | null>(null);
	let actionSuccess = $state<string | null>(null);
	let refreshing = $state(false);
	let syncing = $state(false);
	let updatingAll = $state(false);

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
										<a
											href="https://{tenant.slug}.bollyai.dev"
											target="_blank"
											class="inline-flex items-center gap-1 text-xs py-1.5 px-3 rounded-lg text-text-ghost transition-all duration-300 hover:text-text-dim"
											style="border: 1px solid var(--color-border);"
										>
											<ExternalLink size={12} />
										</a>
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
									{:else if tenant.status === 'stopped'}
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
									{:else if tenant.status === 'error'}
										<span class="inline-flex items-center gap-1 text-xs text-red-400/70">
											<AlertTriangle size={12} />
											{tenant.errorMessage ?? 'unknown error'}
										</span>
									{/if}
								</div>
							</div>

							<!-- Details row -->
							<div class="mt-4 pt-4 flex items-center gap-6 text-xs text-text-ghost" style="border-top: 1px solid var(--color-border);">
								<span class="inline-flex items-center gap-1.5">
									<Cpu size={12} />
									{tenant.messagesPerDay === -1 ? 'unlimited' : `${tenant.messagesPerDay}/day`}
								</span>
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
										{tenant.rateLimit.messagesToday} today / {formatTokens(tenant.rateLimit.tokensThisMonth)} tokens
									</span>
								{/if}
							</div>

							<!-- Model selector -->
							{#if tenant.status === 'running' || tenant.status === 'stopped'}
								<div class="mt-4 pt-4" style="border-top: 1px solid var(--color-border);">
									<div class="flex items-center gap-2 mb-2">
										<Server size={12} class="text-text-ghost" />
										<span class="text-xs text-text-ghost">model</span>
										{#if tenant.machine?.model}
											<span class="text-xs font-mono text-text-dim">{tenant.machine.model}</span>
										{:else}
											<span class="text-xs font-mono text-text-ghost italic">default (from env)</span>
										{/if}
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
											name="model"
											class="flex-1 py-2 px-3 rounded-lg text-xs text-text outline-none font-mono"
											style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
										>
											{#each MODEL_PRESETS as preset}
												<option
													value={preset.model}
													selected={currentModel(tenant) === preset.model}
												>
													{preset.label} — {preset.cost}
												</option>
											{/each}
										</select>
										<input type="hidden" name="provider" value="" />
										<button
											type="submit"
											disabled={updating === tenant.id}
											class="py-2 px-4 rounded-lg text-xs font-medium text-warm transition-all duration-300 disabled:opacity-40"
											style="background: oklch(0.78 0.12 75 / 12%); border: 1px solid oklch(0.78 0.12 75 / 20%);"
											onclick={(e) => {
												const form = (e.target as HTMLElement).closest('form')!;
												const select = form.querySelector('select[name="model"]') as HTMLSelectElement;
												const preset = MODEL_PRESETS.find(p => p.model === select.value);
												if (preset) {
													(form.querySelector('input[name="provider"]') as HTMLInputElement).value = preset.provider;
												}
											}}
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
