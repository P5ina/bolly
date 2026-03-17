<script lang="ts">
	import type { Snippet } from "svelte";
	let { ondismiss, children }: { ondismiss?: () => void; children: Snippet } = $props();
</script>

<div class="creature-bubble">
	<div class="creature-bubble-content">
		{@render children()}
	</div>
	{#if ondismiss}
		<button class="creature-bubble-dismiss" onclick={ondismiss}>ok</button>
	{/if}
</div>

<style>
	.creature-bubble {
		margin: 0 0 0.5rem;
		padding: 0.5rem 0.75rem;
		border-radius: 0.625rem;
		background: oklch(0.08 0.015 280 / 80%);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		font-family: var(--font-body);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75 / 55%);
		line-height: 1.5;
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		animation: bubble-in 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes bubble-in {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.creature-bubble-content {
		flex: 1;
		min-width: 0;
	}

	.creature-bubble :global(a) {
		color: oklch(0.78 0.12 75 / 70%);
		text-decoration: underline;
	}

	.creature-bubble-dismiss {
		flex-shrink: 0;
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.78 0.12 75 / 30%);
		background: none;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		border-radius: 0.25rem;
		padding: 0.1rem 0.4rem;
		cursor: pointer;
		margin-top: 0.1rem;
	}

	.creature-bubble-dismiss:hover {
		color: oklch(0.78 0.12 75 / 60%);
		border-color: oklch(0.78 0.12 75 / 30%);
	}
</style>
