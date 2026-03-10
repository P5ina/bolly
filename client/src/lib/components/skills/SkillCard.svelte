<script lang="ts">
	import type { Skill } from "$lib/api/types.js";

	let {
		skill,
		ondelete,
	}: {
		skill: Skill;
		ondelete: () => void;
	} = $props();

	let expanded = $state(false);
</script>

<button
	class="skill-card"
	class:skill-card-expanded={expanded}
	class:skill-card-builtin={skill.builtin}
	onclick={() => (expanded = !expanded)}
>
	<div class="skill-card-header">
		<span class="skill-card-icon">{skill.icon || "~"}</span>
		<span class="skill-card-name">{skill.name}</span>
		{#if skill.builtin}
			<span class="skill-card-badge">built-in</span>
		{:else if skill.source}
			<span class="skill-card-badge skill-card-badge-community">community</span>
		{/if}
	</div>

	<p class="skill-card-desc">{skill.description}</p>

	{#if expanded && skill.instructions}
		<div class="skill-card-instructions">
			<span class="skill-card-instructions-label">instructions</span>
			<p class="skill-card-instructions-text">{skill.instructions}</p>
		</div>
	{/if}

	{#if expanded && !skill.builtin}
		<span
			role="button"
			tabindex="0"
			class="skill-card-delete"
			onclick={(e) => {
				e.stopPropagation();
				ondelete();
			}}
			onkeydown={(e) => {
				if (e.key === "Enter") {
					e.stopPropagation();
					ondelete();
				}
			}}
		>
			delete
		</span>
	{/if}
</button>

<style>
	.skill-card {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 1rem 1.125rem;
		border-radius: 0.75rem;
		background: oklch(0.09 0.018 278 / 60%);
		border: 1px solid oklch(1 0 0 / 4%);
		cursor: pointer;
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		text-align: left;
		width: 100%;
		animation: skill-emerge 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes skill-emerge {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.skill-card:hover {
		background: oklch(0.10 0.020 278 / 70%);
		border-color: oklch(0.78 0.12 75 / 10%);
		box-shadow: 0 0 20px oklch(0.78 0.12 75 / 5%);
	}

	.skill-card-expanded {
		border-color: oklch(0.78 0.12 75 / 15%);
		box-shadow: 0 0 30px oklch(0.78 0.12 75 / 8%);
	}

	.skill-card-builtin {
		border-color: oklch(0.78 0.12 75 / 8%);
	}

	.skill-card-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.skill-card-icon {
		font-family: var(--font-mono);
		font-size: 0.85rem;
		color: oklch(0.78 0.12 75 / 60%);
		width: 1.25rem;
		text-align: center;
		flex-shrink: 0;
	}

	.skill-card-name {
		font-family: var(--font-display);
		font-size: 0.85rem;
		font-weight: 500;
		color: oklch(0.88 0.02 75);
	}

	.skill-card-badge {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.78 0.12 75 / 35%);
		background: oklch(0.78 0.12 75 / 6%);
		padding: 0.15rem 0.45rem;
		border-radius: 0.25rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.skill-card-desc {
		font-size: 0.78rem;
		color: oklch(0.78 0.12 75 / 45%);
		line-height: 1.5;
	}

	.skill-card-instructions {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		padding-top: 0.5rem;
		border-top: 1px solid oklch(1 0 0 / 4%);
	}

	.skill-card-instructions-label {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 25%);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.skill-card-instructions-text {
		font-size: 0.72rem;
		color: oklch(0.78 0.12 75 / 40%);
		line-height: 1.55;
		white-space: pre-wrap;
	}

	.skill-card-badge-community {
		color: oklch(0.7 0.08 200 / 50%);
		background: oklch(0.7 0.08 200 / 8%);
	}

	.skill-card-delete {
		align-self: flex-end;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.65 0.15 20 / 50%);
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		transition: all 0.2s ease;
	}

	.skill-card-delete:hover {
		color: oklch(0.65 0.15 20 / 80%);
		background: oklch(0.65 0.15 20 / 8%);
	}
</style>
