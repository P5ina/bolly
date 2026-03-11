<script lang="ts">
	import {
		fetchSkills,
		deleteSkill,
		fetchRegistry,
		installRegistrySkill,
	} from "$lib/api/client.js";
	import type { Skill, RegistryEntry } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import SkillCard from "./SkillCard.svelte";
	import RegistryCard from "./RegistryCard.svelte";
	import CreateSkillModal from "./CreateSkillModal.svelte";

	const toast = getToasts();

	let skills = $state<Skill[]>([]);
	let loading = $state(true);
	let showCreate = $state(false);

	let mode = $state<"installed" | "browse">("installed");
	let registry = $state<RegistryEntry[]>([]);
	let registryLoading = $state(false);
	let registryError = $state("");
	let installingId = $state<string | null>(null);

	async function load() {
		loading = true;
		try {
			skills = await fetchSkills();
		} catch {
			toast.error("failed to load skills");
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();
	});

	async function loadRegistry() {
		if (registry.length > 0) return;
		registryLoading = true;
		registryError = "";
		try {
			registry = await fetchRegistry();
		} catch (e) {
			console.error("failed to load registry", e);
			registryError =
				e instanceof Error ? e.message : "failed to load registry";
		} finally {
			registryLoading = false;
		}
	}

	function switchMode(m: "installed" | "browse") {
		mode = m;
		if (m === "browse") loadRegistry();
	}

	async function handleDelete(skillId: string) {
		try {
			await deleteSkill(skillId);
			skills = skills.filter((s) => s.id !== skillId);
			// Update registry installed status
			registry = registry.map((e) =>
				e.id === skillId ? { ...e, installed: false } : e,
			);
		} catch {
			toast.error("failed to delete skill");
		}
	}

	function handleCreated(skill: Skill) {
		skills = [...skills, skill];
		showCreate = false;
	}

	async function handleInstall(id: string) {
		installingId = id;
		try {
			const skill = await installRegistrySkill(id);
			skills = [...skills, skill];
			registry = registry.map((e) =>
				e.id === id ? { ...e, installed: true } : e,
			);
		} catch {
			toast.error("failed to install skill");
		} finally {
			installingId = null;
		}
	}
</script>

<div class="skills-container">
	{#if loading}
		<div class="skills-loading">
			<div class="skills-loading-dot"></div>
		</div>
	{:else}
		<div class="skills-header">
			<div class="skills-tabs">
				<button
					class="skills-tab"
					class:skills-tab-active={mode === "installed"}
					onclick={() => switchMode("installed")}
				>
					installed
					<span class="skills-tab-count">{skills.length}</span>
				</button>
				<button
					class="skills-tab"
					class:skills-tab-active={mode === "browse"}
					onclick={() => switchMode("browse")}
				>
					browse
				</button>
			</div>
			{#if mode === "installed"}
				<button class="skills-add" onclick={() => (showCreate = true)}>
					+ new skill
				</button>
			{/if}
		</div>

		{#if mode === "installed"}
			{#if skills.length === 0}
				<div class="skills-empty">
					<div class="skills-empty-icon">+</div>
					<p class="skills-empty-text">no skills yet</p>
					<p class="skills-empty-hint">
						skills extend what your companion can do — teach it new
						behaviors, workflows, and abilities.
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
		{:else if mode === "browse"}
			{#if registryLoading}
				<div class="skills-loading">
					<div class="skills-loading-dot"></div>
				</div>
			{:else if registryError}
				<div class="skills-empty">
					<p class="skills-empty-text">couldn't load registry</p>
					<p class="skills-empty-hint">{registryError}</p>
					<button
						class="skills-add"
						onclick={() => {
							registry = [];
							loadRegistry();
						}}
					>
						retry
					</button>
				</div>
			{:else if registry.length === 0}
				<div class="skills-empty">
					<p class="skills-empty-text">no community skills available</p>
					<p class="skills-empty-hint">
						the registry is empty — check back later or set a custom
						registry URL in config.toml
					</p>
				</div>
			{:else}
				<div class="skills-grid">
					{#each registry as entry (entry.id)}
						<RegistryCard
							{entry}
							installing={installingId === entry.id}
							oninstall={handleInstall}
						/>
					{/each}
				</div>
			{/if}
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
		flex-wrap: wrap;
		gap: 0.5rem;
		margin-bottom: 1.25rem;
	}

	.skills-tabs {
		display: flex;
		gap: 0.125rem;
		background: oklch(1 0 0 / 3%);
		border-radius: 0.5rem;
		padding: 0.15rem;
	}

	.skills-tab {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.78 0.12 75 / 25%);
		background: none;
		border: none;
		padding: 0.3rem 0.65rem;
		border-radius: 0.375rem;
		cursor: pointer;
		letter-spacing: 0.04em;
		transition: all 0.2s ease;
		display: flex;
		align-items: center;
		gap: 0.35rem;
	}

	.skills-tab:hover {
		color: oklch(0.78 0.12 75 / 45%);
	}

	.skills-tab-active {
		color: oklch(0.78 0.12 75 / 60%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	.skills-tab-count {
		font-size: 0.55rem;
		color: oklch(0.78 0.12 75 / 30%);
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
