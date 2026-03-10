<script lang="ts">
	let email = $state('');
	let password = $state('');
	let errorMsg = $state('');
	let loading = $state(false);

	async function submit() {
		if (!email || !password) return;
		loading = true;
		errorMsg = '';

		try {
			const res = await fetch('/api/auth/login', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ email, password }),
			});

			if (!res.ok) {
				const err = await res.json().catch(() => ({ message: res.statusText }));
				errorMsg = err.message ?? 'Login failed';
				return;
			}

			location.href = '/dashboard';
		} catch {
			errorMsg = 'Network error';
		} finally {
			loading = false;
		}
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') submit();
	}
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

		<div class="space-y-4">
			<div>
				<input
					bind:value={email}
					{onkeydown}
					type="email"
					placeholder="Email"
					class="w-full py-3 px-4 rounded-lg text-sm text-text outline-none transition-all duration-300"
					style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
				/>
			</div>
			<div>
				<input
					bind:value={password}
					{onkeydown}
					type="password"
					placeholder="Password"
					class="w-full py-3 px-4 rounded-lg text-sm text-text outline-none transition-all duration-300"
					style="background: var(--color-bg-raised); border: 1px solid var(--color-border);"
				/>
			</div>

			{#if errorMsg}
				<p class="text-xs text-red-400/70 italic">{errorMsg}</p>
			{/if}

			<button
				onclick={submit}
				disabled={loading || !email || !password}
				class="w-full py-3 rounded-lg text-sm font-medium text-warm transition-all duration-300 disabled:opacity-40"
				style="background: oklch(0.78 0.12 75 / 12%); border: 1px solid oklch(0.78 0.12 75 / 20%);"
			>
				{loading ? 'Signing in...' : 'Sign in'}
			</button>
		</div>

		<p class="text-center mt-6 text-xs text-text-ghost">
			Don't have an account? <a href="/signup" class="text-warm-dim hover:text-warm transition-colors">Sign up</a>
		</p>
	</div>
</div>
