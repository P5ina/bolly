<script lang="ts">
	import { fetchSkills, deleteSkill } from "$lib/api/client.js";
	import type { Skill } from "$lib/api/types.js";
	import SkillCard from "./SkillCard.svelte";
	import CreateSkillModal from "./CreateSkillModal.svelte";

	let skills = $state<Skill[]>([]);
	let loading = $state(true);
	let showCreate = $state(false);

	async function load() {
		loading = true;
		try {
			skills = await fetchSkills();
		} catch (e) {
			console.error("failed to load skills", e);
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();
	});

	async function handleDelete(skillId: string) {
		try {
			await deleteSkill(skillId);
			skills = skills.filter((s) => s.id !== skillId);
		} catch (e) {
			console.error("failed to delete skill", e);
		}
	}

	function handleCreated(skill: Skill) {
		skills = [...skills, skill];
		showCreate = false;
	}
</script>

<div class="skills-container">
	{#if loading}
		<div class="skills-loading">
			<div class="skills-loading-dot"></div>
		</div>
	{:else}
		<div class="skills-header">
			<span class="skills-count"
				>{skills.length} skill{skills.length !== 1 ? "s" : ""}</span
			>
			<button class="skills-add" onclick={() => (showCreate = true)}>
				+ new skill
			</button>
		</div>

		{#if skills.length === 0}
			<div class="skills-empty">
				<div class="skills-empty-icon">+</div>
				<p class="skills-empty-text">no skills yet</p>
				<p class="skills-empty-hint">
					skills extend what your companion can do — teach it new behaviors,
					workflows, and abilities.
				</p>
			</div>
		{:else}
			<div class="skills-grid">
				{#each skills as skill (skill.id)}
					<SkillCard
						{skill}
						ondelete={() => handleDelete(skill.id)}
					/>
				{/each}
			</div>
		{/if}
	{/if}

	{#if showCreate}
		<CreateSkillModal
			onclose={() => (showCreate = false)}
			oncreated={handleCreated}
		/>
	{/if}
</div>

<style>
	.skills-container {
		height: 100%;
		overflow-y: auto;
		padding: 2rem 1.5rem;
	}

	.skills-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.skills-loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	.skills-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 1.25rem;
	}

	.skills-count {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 30%);
		letter-spacing: 0.05em;
	}

	.skills-add {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.78 0.12 75 / 40%);
		background: oklch(0.78 0.12 75 / 5%);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		padding: 0.35rem 0.75rem;
		border-radius: 0.5rem;
		cursor: pointer;
		letter-spacing: 0.04em;
		transition: all 0.2s ease;
	}

	.skills-add:hover {
		color: oklch(0.78 0.12 75 / 65%);
		background: oklch(0.78 0.12 75 / 10%);
		border-color: oklch(0.78 0.12 75 / 20%);
	}

	.skills-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: calc(100% - 3rem);
		gap: 0.75rem;
		text-align: center;
	}

	.skills-empty-icon {
		font-family: var(--font-mono);
		font-size: 1.5rem;
		color: oklch(0.78 0.12 75 / 20%);
		animation: pulse-alive 3s ease-in-out infinite;
	}

	.skills-empty-text {
		font-family: var(--font-display);
		font-size: 0.95rem;
		color: oklch(0.78 0.12 75 / 50%);
	}

	.skills-empty-hint {
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 25%);
		max-width: 30ch;
		line-height: 1.5;
	}

	.skills-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: 0.75rem;
	}

	@media (max-width: 640px) {
		.skills-grid {
			grid-template-columns: 1fr;
		}
		.skills-container {
			padding: 1.5rem 1rem;
		}
	}
</style>
