<script lang="ts">
	import { enhance } from '$app/forms';

	let { form } = $props();
	let loading = $state(false);
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
			<h1 class="font-display italic text-2xl text-text">welcome back</h1>
		</div>

		<form method="POST" use:enhance={() => { loading = true; return async ({ update }) => { loading = false; await update(); }; }} class="space-y-4">
			<div>
				<input
					name="email"
					type="email"
					placeholder="Email"
					value={form?.email ?? ''}
					class="w-full py-3 px-4 rounded-lg text-sm text-text outline-none transition-all duration-300"
					style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
				/>
			</div>
			<div>
				<input
					name="password"
					type="password"
					placeholder="Password"
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
				{loading ? 'Signing in...' : 'Sign in'}
			</button>
		</form>

		<div class="text-center mt-6 space-y-2">
			<p class="text-xs text-text-ghost">
				<a href="/forgot-password" class="text-warm-dim hover:text-warm transition-colors">Forgot password?</a>
			</p>
			<p class="text-xs text-text-ghost">
				Don't have an account? <a href="/signup" class="text-warm-dim hover:text-warm transition-colors">Sign up</a>
			</p>
		</div>
	</div>
</div>
