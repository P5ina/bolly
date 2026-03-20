<script lang="ts">
	import { enhance } from '$app/forms';

	let { form } = $props();
	let loading = $state(false);
</script>

<div class="auth-page">
	<div class="auth-glow"></div>
	<div class="auth-container">
		<div class="auth-header">
			<a href="/" class="auth-brand">
				<div class="auth-logo">b</div>
				<span class="auth-brand-name">bolly</span>
			</a>
			<h1 class="auth-title">create your account</h1>
		</div>

		<div class="auth-card">
			<form method="POST" use:enhance={() => { loading = true; return async ({ update }) => { loading = false; await update(); }; }} class="auth-form">
				<div>
					<input name="name" type="text" placeholder="Name (optional)" value={form?.name ?? ''} class="auth-input" />
				</div>
				<div>
					<input name="email" type="email" placeholder="Email" value={form?.email ?? ''} class="auth-input" />
				</div>
				<div>
					<input name="password" type="password" placeholder="Password (8+ characters)" class="auth-input" />
				</div>

				{#if form?.message}
					<p class="auth-error">{form.message}</p>
				{/if}

				<button type="submit" disabled={loading} class="auth-submit">
					{loading ? 'Creating account...' : 'Create account'}
				</button>
			</form>
		</div>

		<div class="auth-footer">
			<p>Already have an account? <a href="/login" class="auth-link">Sign in</a></p>
		</div>
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
		pointer-events: none;
	}

	.auth-container { width: 100%; max-width: 24rem; position: relative; z-index: 1; }
	.auth-header { text-align: center; margin-bottom: 2rem; }
	.auth-brand { display: inline-flex; align-items: center; gap: 0.625rem; margin-bottom: 1.5rem; text-decoration: none; }
	.auth-logo { width: 2rem; height: 2rem; border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; font-family: var(--font-display); font-style: italic; font-size: 0.875rem; color: var(--color-warm); background: oklch(1 0 0 / 5%); border: 1px solid oklch(1 0 0 / 10%); }
	.auth-brand-name { font-family: var(--font-display); font-style: italic; font-size: 1.25rem; color: var(--color-text); }
	.auth-title { font-family: var(--font-display); font-style: italic; font-size: 1.5rem; font-weight: 400; color: var(--color-text); }
	.auth-card { background: oklch(1 0 0 / 4%); border: 1px solid oklch(1 0 0 / 8%); border-top-color: oklch(1 0 0 / 16%); border-radius: 1rem; padding: 2rem; position: relative; overflow: hidden; }
	.auth-card::before { content: ''; position: absolute; top: 0; left: 10%; right: 10%; height: 1px; background: linear-gradient(90deg, transparent, oklch(1 0 0 / 20%), transparent); pointer-events: none; }
	.auth-form { display: flex; flex-direction: column; gap: 1rem; }
	.auth-input { width: 100%; padding: 0.75rem 1rem; border-radius: 0.75rem; font-size: 0.875rem; font-family: var(--font-body); color: var(--color-text); background: oklch(1 0 0 / 3%); border: 1px solid oklch(1 0 0 / 8%); outline: none; transition: all 0.3s ease; }
	.auth-input:focus { border-color: oklch(1 0 0 / 16%); box-shadow: 0 0 0 3px oklch(0.55 0.08 240 / 6%); }
	.auth-input::placeholder { color: oklch(0.90 0.02 75 / 25%); }
	.auth-error { font-size: 0.75rem; color: oklch(0.65 0.15 25 / 70%); font-style: italic; }
	.auth-submit { width: 100%; padding: 0.75rem; border-radius: 0.75rem; font-size: 0.875rem; font-weight: 500; font-family: var(--font-body); color: var(--color-warm); background: oklch(0.78 0.12 75 / 10%); border: 1px solid oklch(0.78 0.12 75 / 18%); border-top-color: oklch(0.78 0.12 75 / 28%); transition: all 0.3s ease; cursor: pointer; }
	.auth-submit:hover:not(:disabled) { background: oklch(0.78 0.12 75 / 15%); border-color: oklch(0.78 0.12 75 / 30%); box-shadow: 0 0 30px oklch(0.78 0.12 75 / 8%); }
	.auth-submit:disabled { opacity: 0.4; cursor: not-allowed; }
	.auth-footer { text-align: center; margin-top: 1.5rem; font-size: 0.75rem; color: oklch(0.90 0.02 75 / 25%); }
	.auth-link { color: oklch(0.78 0.12 75 / 40%); text-decoration: none; transition: color 0.3s ease; }
	.auth-link:hover { color: var(--color-warm); }
</style>
