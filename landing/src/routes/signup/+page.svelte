<script lang="ts">
	import { enhance } from '$app/forms';

	let { form } = $props();
	let loading = $state(false);
</script>

<div class="auth-page">
	<div class="auth-glow"></div>
	<div class="w-full max-w-sm relative z-10">
		<div class="text-center mb-8">
			<a href="/" class="inline-flex items-center gap-2.5 mb-6">
				<div class="auth-logo">b</div>
				<span class="font-display italic text-xl text-text">bolly</span>
			</a>
			<h1 class="font-display italic text-2xl text-text">create your account</h1>
		</div>

		<div class="auth-card">
			<form method="POST" use:enhance={() => { loading = true; return async ({ update }) => { loading = false; await update(); }; }} class="space-y-4">
				<div>
					<input
						name="name"
						type="text"
						placeholder="Name (optional)"
						value={form?.name ?? ''}
						class="auth-input"
					/>
				</div>
				<div>
					<input
						name="email"
						type="email"
						placeholder="Email"
						value={form?.email ?? ''}
						class="auth-input"
					/>
				</div>
				<div>
					<input
						name="password"
						type="password"
						placeholder="Password (8+ characters)"
						class="auth-input"
					/>
				</div>

				{#if form?.message}
					<p class="text-xs text-red-400/70 italic">{form.message}</p>
				{/if}

				<button
					type="submit"
					disabled={loading}
					class="auth-submit"
				>
					{loading ? 'Creating account...' : 'Create account'}
				</button>
			</form>
		</div>

		<p class="text-center mt-6 text-xs text-text-ghost">
			Already have an account? <a href="/login" class="text-warm-dim hover:text-warm transition-colors">Sign in</a>
		</p>
	</div>
</div>

<style>
	.auth-page {
		min-height: 100dvh;
		background: oklch(0.04 0.015 260);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.5rem;
		position: relative;
		overflow: hidden;
	}

	.auth-glow {
		position: absolute;
		top: 40%;
		left: 50%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.55 0.08 240 / 4%) 0%, transparent 65%);
		animation: breathe 8s ease-in-out infinite;
		pointer-events: none;
	}

	.auth-logo {
		width: 2rem;
		height: 2rem;
		border-radius: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		font-family: var(--font-display);
		font-style: italic;
		font-size: 0.875rem;
		color: var(--color-warm);
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
	}

	.auth-card {
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur-heavy);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		border-radius: 1rem;
		padding: 2rem;
		position: relative;
		overflow: hidden;
	}

	.auth-card::before {
		content: '';
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 20%), transparent);
		pointer-events: none;
	}

	.auth-card::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 40%;
		background: linear-gradient(180deg, oklch(1 0 0 / 3%) 0%, transparent 100%);
		pointer-events: none;
		border-radius: 1rem 1rem 0 0;
		z-index: -1;
	}

	.auth-input {
		width: 100%;
		padding: 0.75rem 1rem;
		border-radius: 0.75rem;
		font-size: 0.875rem;
		color: var(--color-text);
		background: oklch(1 0 0 / 3%);
		border: 1px solid var(--glass-border);
		outline: none;
		transition: all 0.3s ease;
		backdrop-filter: blur(8px);
	}

	.auth-input:focus {
		border-color: oklch(1 0 0 / 16%);
		box-shadow: 0 0 0 3px oklch(0.55 0.08 240 / 6%);
	}

	.auth-input::placeholder {
		color: oklch(0.90 0.02 75 / 25%);
	}

	.auth-submit {
		width: 100%;
		padding: 0.75rem;
		border-radius: 0.75rem;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-warm);
		background: oklch(0.78 0.12 75 / 10%);
		backdrop-filter: var(--glass-blur);
		border: 1px solid oklch(0.78 0.12 75 / 18%);
		border-top-color: oklch(0.78 0.12 75 / 28%);
		transition: all 0.3s ease;
		cursor: pointer;
		position: relative;
		overflow: hidden;
	}

	.auth-submit::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 50%;
		background: linear-gradient(180deg, oklch(0.78 0.12 75 / 6%) 0%, transparent 100%);
		pointer-events: none;
	}

	.auth-submit:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 15%);
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow: 0 0 30px oklch(0.78 0.12 75 / 8%);
	}

	.auth-submit:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
</style>
