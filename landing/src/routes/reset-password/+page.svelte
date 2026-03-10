<script lang="ts">
	import { enhance } from '$app/forms';
	import { page } from '$app/state';

	let { form } = $props();
	let loading = $state(false);

	const token = $derived(page.url.searchParams.get('token') ?? '');
</script>

<div class="min-h-dvh bg-bg flex items-center justify-center px-6">
	<div class="w-full max-w-sm">
		<div class="text-center mb-8">
			<a href="/" class="inline-flex items-center gap-2.5 mb-6">
				<div class="w-8 h-8 rounded-lg flex items-center justify-center font-display italic text-base text-warm"
					style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
				>b</div>
				<span class="font-display italic text-xl text-text">bolly</span>
			</a>
			<h1 class="font-display italic text-2xl text-text">reset password</h1>
		</div>

		{#if !token}
			<div class="text-center space-y-4">
				<p class="text-sm text-red-400/70 italic">Invalid reset link.</p>
				<a href="/forgot-password" class="text-sm text-warm-dim hover:text-warm transition-colors">Request a new one</a>
			</div>
		{:else if form?.success}
			<div class="text-center space-y-4">
				<div class="w-12 h-12 mx-auto rounded-full flex items-center justify-center"
					style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
				>
					<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-warm">
						<polyline points="20 6 9 17 4 12" />
					</svg>
				</div>
				<p class="text-sm text-text-dim">Password reset successfully.</p>
				<a href="/login" class="inline-block mt-2 text-sm text-warm-dim hover:text-warm transition-colors">Sign in</a>
			</div>
		{:else}
			<form method="POST" use:enhance={() => { loading = true; return async ({ update }) => { loading = false; await update(); }; }} class="space-y-4">
				<input type="hidden" name="token" value={token} />
				<div>
					<input
						name="password"
						type="password"
						placeholder="New password (8+ characters)"
						class="w-full py-3 px-4 rounded-lg text-sm text-text outline-none transition-all duration-300"
						style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
					/>
				</div>
				<div>
					<input
						name="confirmPassword"
						type="password"
						placeholder="Confirm password"
						class="w-full py-3 px-4 rounded-lg text-sm text-text outline-none transition-all duration-300"
						style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
					/>
				</div>

				{#if form?.message}
					<p class="text-xs text-red-400/70 italic">{form.message}</p>
				{/if}

				<button
					type="submit"
					disabled={loading}
					class="w-full py-3 rounded-lg text-sm font-medium text-warm transition-all duration-300 disabled:opacity-40"
					style="background: oklch(0.78 0.12 75 / 12%); border: 1px solid oklch(0.78 0.12 75 / 20%);"
				>
					{loading ? 'Resetting...' : 'Reset password'}
				</button>
			</form>
		{/if}

		<p class="text-center mt-6 text-xs text-text-ghost">
			<a href="/login" class="text-warm-dim hover:text-warm transition-colors">Back to sign in</a>
		</p>
	</div>
</div>
