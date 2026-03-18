<script lang="ts">
	import { createSkill } from "$lib/api/client.js";
	import type { Skill } from "$lib/api/types.js";

	let {
		onclose,
		oncreated,
	}: {
		onclose: () => void;
		oncreated: (skill: Skill) => void;
	} = $props();

	let name = $state("");
	let description = $state("");
	let instructions = $state("");
	let icon = $state("~");
	let saving = $state(false);
	let error = $state("");

	function toId(name: string): string {
		return name
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "_")
			.replace(/^_|_$/g, "");
	}

	async function handleSubmit() {
		if (!name.trim()) return;

		saving = true;
		error = "";

		try {
			const skill: Skill = {
				id: toId(name),
				name: name.trim(),
				description: description.trim(),
				icon: icon.trim() || "~",
				builtin: false,
				enabled: true,
				instructions: instructions.trim(),
			};
			const created = await createSkill(skill);
			oncreated(created);
		} catch (e) {
			error = e instanceof Error ? e.message : "failed to create skill";
		} finally {
			saving = false;
		}
	}
</script>

<div
	class="modal-backdrop"
	role="button"
	tabindex="-1"
	onclick={onclose}
	onkeydown={(e) => e.key === "Escape" && onclose()}
>
	<!-- svelte-ignore a11y_interactive_supports_focus -->
	<div
		class="modal"
		role="dialog"
		onclick={(e) => e.stopPropagation()}
		onkeydown={() => {}}
	>
		<div class="modal-header">
			<h2 class="modal-title">new skill</h2>
			<button class="modal-close" onclick={onclose}>&times;</button>
		</div>

		<form class="modal-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
			<div class="field-row">
				<div class="field field-icon">
					<label class="field-label" for="skill-icon">icon</label>
					<input
						id="skill-icon"
						class="field-input"
						type="text"
						maxlength="2"
						bind:value={icon}
						placeholder="~"
					/>
				</div>
				<div class="field field-name">
					<label class="field-label" for="skill-name">name</label>
					<input
						id="skill-name"
						class="field-input"
						type="text"
						bind:value={name}
						placeholder="e.g. Code Reviewer"
						required
					/>
				</div>
			</div>

			<div class="field">
				<label class="field-label" for="skill-desc">description</label>
				<input
					id="skill-desc"
					class="field-input"
					type="text"
					bind:value={description}
					placeholder="what does this skill do?"
				/>
			</div>

			<div class="field">
				<label class="field-label" for="skill-instructions">
					instructions
					<span class="field-hint">prompt fragment injected when active</span>
				</label>
				<textarea
					id="skill-instructions"
					class="field-textarea"
					bind:value={instructions}
					placeholder="you are skilled at..."
					rows="5"
				></textarea>
			</div>

			{#if error}
				<p class="modal-error">{error}</p>
			{/if}

			<div class="modal-actions">
				<button type="button" class="btn-cancel" onclick={onclose}>
					cancel
				</button>
				<button type="submit" class="btn-create" disabled={saving || !name.trim()}>
					{saving ? "creating..." : "create skill"}
				</button>
			</div>
		</form>
	</div>
</div>

<style>
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: oklch(0 0 0 / 60%);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
		animation: fade-in 0.2s ease;
	}

	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	.modal {
		width: 90%;
		max-width: 480px;
		background: oklch(0.10 0.015 278);
		border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 1rem;
		padding: 1.5rem;
		animation: modal-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1);
	}

	@keyframes modal-enter {
		from {
			opacity: 0;
			transform: translateY(12px) scale(0.97);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 1.25rem;
	}

	.modal-title {
		font-family: var(--font-display);
		font-size: 0.95rem;
		font-weight: 500;
		color: oklch(0.88 0.02 75);
	}

	.modal-close {
		font-size: 1.1rem;
		color: oklch(0.78 0.12 75 / 30%);
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.25rem;
		line-height: 1;
	}

	.modal-close:hover {
		color: oklch(0.78 0.12 75 / 60%);
	}

	.modal-form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.field-row {
		display: flex;
		gap: 0.75rem;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.field-icon {
		width: 4rem;
		flex-shrink: 0;
	}

	.field-name {
		flex: 1;
	}

	.field-label {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.field-hint {
		text-transform: none;
		letter-spacing: normal;
		color: oklch(0.78 0.12 75 / 28%);
		margin-left: 0.5rem;
	}

	.field-input {
		font-family: var(--font-mono);
		font-size: 0.78rem;
		color: oklch(0.88 0.02 75);
		background: oklch(0.07 0.012 278);
		border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 0.5rem;
		padding: 0.5rem 0.625rem;
		outline: none;
		transition: border-color 0.2s ease;
	}

	.field-input:focus {
		border-color: oklch(0.78 0.12 75 / 35%);
	}

	.field-input::placeholder {
		color: oklch(0.78 0.12 75 / 35%);
	}

	.field-textarea {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75);
		background: oklch(0.07 0.012 278);
		border: 1px solid oklch(1 0 0 / 6%);
		border-radius: 0.5rem;
		padding: 0.5rem 0.625rem;
		outline: none;
		resize: vertical;
		line-height: 1.5;
		transition: border-color 0.2s ease;
	}

	.field-textarea:focus {
		border-color: oklch(0.78 0.12 75 / 35%);
	}

	.field-textarea::placeholder {
		color: oklch(0.78 0.12 75 / 35%);
	}

	.modal-error {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.65 0.15 20 / 70%);
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 0.5rem;
	}

	.btn-cancel {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 35%);
		background: none;
		border: 1px solid oklch(1 0 0 / 6%);
		padding: 0.4rem 0.75rem;
		border-radius: 0.5rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.btn-cancel:hover {
		color: oklch(0.78 0.12 75 / 55%);
		border-color: oklch(1 0 0 / 10%);
	}

	.btn-create {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.88 0.02 75);
		background: oklch(0.78 0.12 75 / 15%);
		border: 1px solid oklch(0.78 0.12 75 / 28%);
		padding: 0.4rem 0.75rem;
		border-radius: 0.5rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.btn-create:hover:not(:disabled) {
		background: oklch(0.78 0.12 75 / 35%);
	}

	.btn-create:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
</style>
