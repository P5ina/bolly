<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { deleteInstance } from "$lib/api/client.js";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import InstanceOnboarding from "$lib/components/onboarding/InstanceOnboarding.svelte";
	let { children } = $props();

	const slug = $derived(page.params.slug!);
	const instances = getInstances();

	const isNew = $derived(
		!instances.loading && !instances.list.some((i) => i.slug === slug)
	);
	const checking = $derived(instances.loading);

	const tabs = ["chat", "drops", "thoughts", "skills", "settings"] as const;
	const activeTab = $derived(
		tabs.find((t) => page.url.pathname.includes(`/${slug}/${t}`)) ?? "chat"
	);

	let showDeleteConfirm = $state(false);
	let confirmSlug = $state("");
	let deleting = $state(false);

	function handleOnboardingComplete() {
		instances.refresh();
	}

	async function handleDelete() {
		if (confirmSlug !== slug) return;
		deleting = true;
		try {
			await deleteInstance(slug);
			instances.remove(slug);
			showDeleteConfirm = false;
			goto("/");
		} catch (e) {
			console.error("Failed to delete instance:", e);
		} finally {
			deleting = false;
		}
	}

	function openDeleteConfirm() {
		confirmSlug = "";
		showDeleteConfirm = true;
	}

	function closeDeleteConfirm() {
		showDeleteConfirm = false;
		confirmSlug = "";
	}
</script>

{#if checking}
	<div class="flex h-full items-center justify-center">
		<div class="companion-waking-dot"></div>
	</div>
{:else if isNew}
	{#key slug}
		<InstanceOnboarding {slug} oncomplete={handleOnboardingComplete} />
	{/key}
{:else}
	<div class="instance-view">
		<nav class="instance-tabs">
			<a
				href="/"
				class="instance-tab instance-tab-home"
				title="all companions"
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="instance-tab-icon">
					<path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
			{#each tabs as tab}
				<a
					href="/{slug}/{tab}"
					class="instance-tab"
					class:instance-tab-active={activeTab === tab}
				>
					{tab}
				</a>
			{/each}
			<div class="instance-tab-spacer"></div>
			<button
				class="instance-tab instance-tab-delete"
				onclick={openDeleteConfirm}
				title="Delete instance"
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="instance-tab-icon">
					<path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
		</nav>

		<div class="instance-content">
			{@render children()}
		</div>
	</div>

	{#if showDeleteConfirm}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="delete-overlay" onkeydown={(e) => e.key === 'Escape' && closeDeleteConfirm()} onclick={closeDeleteConfirm}>
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="delete-modal" onclick={(e) => e.stopPropagation()}>
				<div class="delete-modal-icon">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-6 h-6">
						<path d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</div>

				<h3 class="delete-modal-title">Delete "{slug}"?</h3>

				<div class="delete-modal-warnings">
					<p class="delete-modal-warning">This will <strong>permanently delete</strong> all data associated with this instance:</p>
					<ul class="delete-modal-list">
						<li>All chat history and conversations</li>
						<li>Soul definition and personality</li>
						<li>Memory and learned facts</li>
						<li>All drops and creative artifacts</li>
						<li>Uploaded files and skin</li>
						<li>Configuration and state</li>
					</ul>
					<p class="delete-modal-warning delete-modal-warning-strong">This action cannot be undone.</p>
				</div>

				<div class="delete-modal-confirm">
					<label class="delete-modal-label" for="confirm-slug">
						Type <strong>{slug}</strong> to confirm:
					</label>
					<input
						id="confirm-slug"
						bind:value={confirmSlug}
						placeholder={slug}
						class="delete-modal-input"
						autocomplete="off"
						onkeydown={(e) => e.key === 'Enter' && handleDelete()}
					/>
				</div>

				<div class="delete-modal-actions">
					<button class="delete-modal-cancel" onclick={closeDeleteConfirm}>Cancel</button>
					<button
						class="delete-modal-destroy"
						disabled={confirmSlug !== slug || deleting}
						onclick={handleDelete}
					>
						{deleting ? "Deleting..." : "Delete permanently"}
					</button>
				</div>
			</div>
		</div>
	{/if}
{/if}

<style>
	.companion-waking-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: waking 2s ease-in-out infinite;
	}
	@keyframes waking {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(0.7); }
	}

	.instance-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		max-width: 100%;
		overflow: hidden;
	}

	.instance-tabs {
		display: flex;
		gap: 0;
		padding: 0.5rem 1.5rem 0;
		border-bottom: 1px solid oklch(1 0 0 / 4%);
		flex-shrink: 0;
		z-index: 10;
		overflow-x: auto;
		scrollbar-width: none;
		-webkit-overflow-scrolling: touch;
	}
	.instance-tabs::-webkit-scrollbar {
		display: none;
	}

	@media (max-width: 720px) {
		.instance-tabs {
			padding: 0.4rem 0.75rem 0;
		}
	}

	.instance-tab {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.05em;
		color: oklch(0.78 0.12 75 / 30%);
		background: none;
		border: none;
		padding: 0.5rem 1rem 0.625rem;
		cursor: pointer;
		position: relative;
		transition: color 0.3s ease;
		text-decoration: none;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.instance-tab:hover {
		color: oklch(0.78 0.12 75 / 55%);
	}

	.instance-tab-active {
		color: oklch(0.78 0.12 75 / 75%);
	}

	.instance-tab-active::after {
		content: "";
		position: absolute;
		bottom: -1px;
		left: 1rem;
		right: 1rem;
		height: 1px;
		background: oklch(0.78 0.12 75 / 30%);
	}

	.instance-tab-home {
		display: flex;
		align-items: center;
		padding: 0.5rem 0.75rem 0.625rem;
	}

	.instance-tab-icon {
		width: 0.8rem;
		height: 0.8rem;
	}

	.instance-content {
		flex: 1;
		min-height: 0;
		min-width: 0;
		overflow: hidden;
	}

	.instance-tab-spacer {
		flex: 1;
	}

	.instance-tab-delete {
		color: oklch(0.65 0.05 20 / 35%);
		padding: 0.5rem 0.6rem 0.625rem;
		display: flex;
		align-items: center;
	}
	.instance-tab-delete:hover {
		color: oklch(0.65 0.15 25 / 80%);
	}

	/* --- delete modal --- */
	.delete-overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.065 0.015 280 / 80%);
		backdrop-filter: blur(4px);
		animation: fade-in 0.15s ease;
	}
	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	.delete-modal {
		width: 100%;
		max-width: 420px;
		margin: 1rem;
		padding: 1.75rem;
		border-radius: 1rem;
		border: 1px solid oklch(0.65 0.15 25 / 15%);
		background: oklch(0.10 0.01 280);
		animation: modal-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes modal-enter {
		from { opacity: 0; transform: scale(0.95) translateY(8px); }
		to { opacity: 1; transform: scale(1) translateY(0); }
	}

	.delete-modal-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 0.625rem;
		background: oklch(0.65 0.15 25 / 10%);
		color: oklch(0.65 0.15 25 / 70%);
		margin-bottom: 1rem;
	}

	.delete-modal-title {
		font-family: var(--font-display);
		font-size: 1.1rem;
		font-weight: 500;
		color: oklch(0.90 0.02 75 / 90%);
		margin-bottom: 1rem;
	}

	.delete-modal-warnings {
		margin-bottom: 1.25rem;
	}

	.delete-modal-warning {
		font-family: var(--font-body);
		font-size: 0.8rem;
		line-height: 1.5;
		color: oklch(0.88 0.02 75 / 55%);
	}
	.delete-modal-warning strong {
		color: oklch(0.65 0.15 25 / 85%);
	}
	.delete-modal-warning-strong {
		margin-top: 0.75rem;
		color: oklch(0.65 0.15 25 / 70%);
		font-weight: 500;
	}

	.delete-modal-list {
		list-style: none;
		padding: 0;
		margin: 0.625rem 0 0;
	}
	.delete-modal-list li {
		font-family: var(--font-body);
		font-size: 0.75rem;
		line-height: 1.8;
		color: oklch(0.88 0.02 75 / 40%);
		padding-left: 1rem;
		position: relative;
	}
	.delete-modal-list li::before {
		content: "";
		position: absolute;
		left: 0;
		top: 50%;
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: oklch(0.65 0.15 25 / 30%);
		transform: translateY(-50%);
	}

	.delete-modal-confirm {
		margin-bottom: 1.25rem;
	}
	.delete-modal-label {
		display: block;
		font-family: var(--font-body);
		font-size: 0.75rem;
		color: oklch(0.88 0.02 75 / 50%);
		margin-bottom: 0.5rem;
	}
	.delete-modal-label strong {
		color: oklch(0.88 0.02 75 / 80%);
		font-family: var(--font-mono);
		font-size: 0.75rem;
	}
	.delete-modal-input {
		width: 100%;
		padding: 0.5rem 0.75rem;
		border-radius: 0.5rem;
		border: 1px solid oklch(1 0 0 / 8%);
		background: oklch(1 0 0 / 3%);
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 0.8rem;
		outline: none;
		transition: border-color 0.2s ease;
	}
	.delete-modal-input:focus {
		border-color: oklch(0.65 0.15 25 / 30%);
	}
	.delete-modal-input::placeholder {
		color: oklch(1 0 0 / 12%);
	}

	.delete-modal-actions {
		display: flex;
		gap: 0.625rem;
		justify-content: flex-end;
	}
	.delete-modal-cancel {
		font-family: var(--font-body);
		font-size: 0.8rem;
		padding: 0.5rem 1rem;
		border-radius: 0.5rem;
		border: 1px solid oklch(1 0 0 / 8%);
		background: none;
		color: oklch(0.88 0.02 75 / 50%);
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.delete-modal-cancel:hover {
		border-color: oklch(1 0 0 / 15%);
		color: oklch(0.88 0.02 75 / 75%);
	}
	.delete-modal-destroy {
		font-family: var(--font-body);
		font-size: 0.8rem;
		font-weight: 500;
		padding: 0.5rem 1rem;
		border-radius: 0.5rem;
		border: 1px solid oklch(0.65 0.15 25 / 30%);
		background: oklch(0.65 0.15 25 / 12%);
		color: oklch(0.65 0.15 25 / 85%);
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.delete-modal-destroy:hover:not(:disabled) {
		background: oklch(0.65 0.15 25 / 22%);
		border-color: oklch(0.65 0.15 25 / 50%);
	}
	.delete-modal-destroy:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}
</style>
