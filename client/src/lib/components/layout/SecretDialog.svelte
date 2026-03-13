<script lang="ts">
	import { submitSecret, cancelSecret } from "$lib/api/client.js";

	interface Props {
		instanceSlug: string;
		requestId: string;
		prompt: string;
		target: string;
		onclose: () => void;
	}

	let { instanceSlug, requestId, prompt, target, onclose }: Props = $props();

	let value = $state("");
	let submitting = $state(false);
	let error = $state("");

	async function handleSubmit() {
		if (!value.trim()) return;
		submitting = true;
		error = "";
		try {
			await submitSecret(instanceSlug, requestId, value);
			onclose();
		} catch (e) {
			error = e instanceof Error ? e.message : "failed to submit";
		} finally {
			submitting = false;
		}
	}

	function handleCancel() {
		cancelSecret(instanceSlug, requestId).catch(() => {});
		onclose();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Escape") handleCancel();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={handleCancel}>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog" onclick={(e) => e.stopPropagation()}>
		<div class="header">
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="icon">
				<rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
				<path d="M7 11V7a5 5 0 0 1 10 0v4" />
			</svg>
			<span class="title">secret required</span>
		</div>

		<p class="prompt">{prompt}</p>
		<p class="target">{target}</p>

		<form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
			<input
				type="password"
				bind:value
				placeholder="enter value…"
				autocomplete="off"
				disabled={submitting}
			/>
			{#if error}
				<p class="error">{error}</p>
			{/if}
			<div class="actions">
				<button type="button" class="btn-cancel" onclick={handleCancel} disabled={submitting}>
					cancel
				</button>
				<button type="submit" class="btn-submit" disabled={submitting || !value.trim()}>
					{submitting ? "saving…" : "save"}
				</button>
			</div>
		</form>
	</div>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 200;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0 0 0 / 60%);
		backdrop-filter: blur(8px);
		animation: fade-in 0.2s ease;
	}

	@keyframes fade-in {
		from { opacity: 0; }
	}

	.dialog {
		width: min(24rem, calc(100vw - 2rem));
		padding: 1.5rem;
		border-radius: 1rem;
		background: oklch(0.08 0.015 280);
		border: 1px solid oklch(0.2 0.02 280);
		animation: dialog-enter 0.25s cubic-bezier(0.16, 1, 0.3, 1);
	}

	@keyframes dialog-enter {
		from { opacity: 0; transform: scale(0.95) translateY(8px); }
	}

	.header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}

	.icon {
		width: 1.125rem;
		height: 1.125rem;
		color: oklch(0.78 0.12 75 / 80%);
	}

	.title {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.78 0.12 75 / 70%);
	}

	.prompt {
		font-size: 0.875rem;
		color: oklch(0.85 0.02 280);
		margin-bottom: 0.375rem;
		line-height: 1.4;
	}

	.target {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.5 0.02 280);
		margin-bottom: 1rem;
	}

	input {
		width: 100%;
		padding: 0.6rem 0.75rem;
		border-radius: 0.5rem;
		background: oklch(0.05 0.01 280);
		border: 1px solid oklch(0.2 0.02 280);
		color: oklch(0.9 0.02 280);
		font-family: var(--font-mono);
		font-size: 0.8rem;
		outline: none;
		transition: border-color 0.2s;
	}

	input:focus {
		border-color: oklch(0.78 0.12 75 / 40%);
	}

	input:disabled {
		opacity: 0.5;
	}

	.error {
		font-size: 0.75rem;
		color: oklch(0.7 0.15 25);
		margin-top: 0.375rem;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 1rem;
	}

	.btn-cancel,
	.btn-submit {
		padding: 0.4rem 0.875rem;
		border-radius: 0.5rem;
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		cursor: pointer;
		transition: all 0.2s;
	}

	.btn-cancel {
		background: transparent;
		border: 1px solid oklch(0.25 0.02 280);
		color: oklch(0.55 0.02 280);
	}
	.btn-cancel:hover {
		border-color: oklch(0.35 0.02 280);
		color: oklch(0.7 0.02 280);
	}

	.btn-submit {
		background: oklch(0.78 0.12 75 / 15%);
		border: 1px solid oklch(0.78 0.12 75 / 25%);
		color: oklch(0.78 0.12 75 / 90%);
	}
	.btn-submit:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 25%);
		border-color: oklch(0.78 0.12 75 / 40%);
	}
	.btn-submit:disabled {
		opacity: 0.4;
		cursor: default;
	}
</style>
