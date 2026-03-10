<script lang="ts">
	import type { RegistryEntry } from "$lib/api/types.js";

	let {
		entry,
		installing,
		oninstall,
	}: {
		entry: RegistryEntry;
		installing: boolean;
		oninstall: (id: string) => void;
	} = $props();
</script>

<div class="registry-card" class:registry-card-installed={entry.installed}>
	<div class="registry-card-header">
		<span class="registry-card-icon">{entry.icon || "~"}</span>
		<span class="registry-card-name">{entry.name}</span>
		{#if entry.installed}
			<span class="registry-card-badge">installed</span>
		{/if}
	</div>

	<p class="registry-card-desc">{entry.description}</p>

	<div class="registry-card-footer">
		{#if entry.author}
			<span class="registry-card-author">{entry.author}</span>
		{/if}
		<span class="registry-card-repo">{entry.repo}</span>

		{#if !entry.installed}
			<button
				class="registry-card-install"
				disabled={installing}
				onclick={(e) => {
					e.stopPropagation();
					oninstall(entry.id);
				}}
			>
				{installing ? "installing..." : "install"}
			</button>
		{/if}
	</div>
</div>

<style>
	.registry-card {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 1rem 1.125rem;
		border-radius: 0.75rem;
		background: oklch(0.09 0.018 278 / 60%);
		border: 1px solid oklch(1 0 0 / 4%);
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		animation: registry-emerge 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes registry-emerge {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.registry-card:hover {
		background: oklch(0.1 0.02 278 / 70%);
		border-color: oklch(0.78 0.12 75 / 10%);
		box-shadow: 0 0 20px oklch(0.78 0.12 75 / 5%);
	}

	.registry-card-installed {
		opacity: 0.6;
	}

	.registry-card-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.registry-card-icon {
		font-family: var(--font-mono);
		font-size: 0.85rem;
		color: oklch(0.78 0.12 75 / 60%);
		width: 1.25rem;
		text-align: center;
		flex-shrink: 0;
	}

	.registry-card-name {
		font-family: var(--font-display);
		font-size: 0.85rem;
		font-weight: 500;
		color: oklch(0.88 0.02 75);
	}

	.registry-card-badge {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.7 0.08 150 / 50%);
		background: oklch(0.7 0.08 150 / 8%);
		padding: 0.15rem 0.45rem;
		border-radius: 0.25rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.registry-card-desc {
		font-size: 0.78rem;
		color: oklch(0.78 0.12 75 / 45%);
		line-height: 1.5;
	}

	.registry-card-footer {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding-top: 0.4rem;
		border-top: 1px solid oklch(1 0 0 / 4%);
	}

	.registry-card-author {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 30%);
	}

	.registry-card-repo {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 20%);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.registry-card-install {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 60%);
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		padding: 0.25rem 0.6rem;
		border-radius: 0.35rem;
		cursor: pointer;
		letter-spacing: 0.04em;
		transition: all 0.2s ease;
		flex-shrink: 0;
	}

	.registry-card-install:hover:not(:disabled) {
		color: oklch(0.88 0.02 75);
		background: oklch(0.78 0.12 75 / 18%);
		border-color: oklch(0.78 0.12 75 / 30%);
	}

	.registry-card-install:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
