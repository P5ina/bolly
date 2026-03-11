<script lang="ts">
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toasts = getToasts();
</script>

{#if toasts.list.length > 0}
	<div class="toast-container">
		{#each toasts.list as toast (toast.id)}
			<div class="toast toast-{toast.kind}" role="alert">
				<span class="toast-icon">
					{#if toast.kind === "error"}!{:else if toast.kind === "success"}~{:else}·{/if}
				</span>
				<span class="toast-msg">{toast.message}</span>
				<button class="toast-close" onclick={() => toasts.dismiss(toast.id)} aria-label="Dismiss">
					<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" class="w-2.5 h-2.5">
						<path d="M2 2l8 8M10 2l-8 8" stroke-linecap="round"/>
					</svg>
				</button>
			</div>
		{/each}
	</div>
{/if}

<style>
	.toast-container {
		position: fixed;
		bottom: calc(1rem + env(safe-area-inset-bottom, 0px));
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		z-index: 200;
		pointer-events: none;
		max-width: calc(100vw - 2rem);
	}

	.toast {
		pointer-events: auto;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		border-radius: 1rem;
		background: oklch(0.10 0.015 280 / 90%);
		backdrop-filter: blur(16px);
		border: 1px solid oklch(1 0 0 / 6%);
		font-family: var(--font-body);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 80%);
		animation: toast-enter 0.35s cubic-bezier(0.16, 1, 0.3, 1) both;
		white-space: nowrap;
	}

	.toast-error {
		border-color: oklch(0.65 0.12 25 / 20%);
		color: oklch(0.85 0.08 25 / 90%);
	}

	.toast-success {
		border-color: oklch(0.65 0.10 160 / 20%);
		color: oklch(0.80 0.08 160 / 90%);
	}

	.toast-icon {
		font-family: var(--font-mono);
		font-size: 0.8rem;
		opacity: 0.6;
		flex-shrink: 0;
	}

	.toast-msg {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.toast-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.25rem;
		height: 1.25rem;
		border-radius: 50%;
		flex-shrink: 0;
		opacity: 0.3;
		transition: opacity 0.2s;
		cursor: pointer;
	}
	.toast-close:hover {
		opacity: 0.7;
	}

	@keyframes toast-enter {
		from { opacity: 0; transform: translateY(8px); }
		to { opacity: 1; transform: translateY(0); }
	}

	@media (max-width: 720px) {
		.toast-container {
			bottom: calc(4rem + env(safe-area-inset-bottom, 0px));
		}
	}
</style>
