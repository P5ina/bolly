<script lang="ts">
	import {
		fetchSoul,
		updateSoul,
		fetchSoulTemplates,
		applySoulTemplate,
	} from "$lib/api/client.js";
	import type { Soul, SoulTemplate } from "$lib/api/types.js";
	import X from "@lucide/svelte/icons/x";
	import Save from "@lucide/svelte/icons/save";
	import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
	import Sparkles from "@lucide/svelte/icons/sparkles";
	import ChevronLeft from "@lucide/svelte/icons/chevron-left";

	let { slug, onclose }: { slug: string; onclose: () => void } = $props();

	let soul = $state<Soul | null>(null);
	let templates = $state<SoulTemplate[]>([]);
	let editContent = $state("");
	let loading = $state(true);
	let saving = $state(false);
	let saved = $state(false);
	let view = $state<"editor" | "templates">("editor");
	let dirty = $derived(soul !== null && editContent !== soul.content);

	$effect(() => {
		loading = true;
		Promise.all([fetchSoul(slug), fetchSoulTemplates()])
			.then(([s, t]) => {
				soul = s;
				editContent = s.content;
				templates = t;
				if (!s.exists) {
					view = "templates";
				}
			})
			.catch(() => {})
			.finally(() => {
				loading = false;
			});
	});

	async function save() {
		if (!dirty) return;
		saving = true;
		try {
			const updated = await updateSoul(slug, editContent);
			soul = updated;
			saved = true;
			setTimeout(() => (saved = false), 2000);
		} finally {
			saving = false;
		}
	}

	async function pickTemplate(template: SoulTemplate) {
		if (template.id === "custom") {
			editContent = template.content;
			soul = { content: template.content, exists: false };
			view = "editor";
			return;
		}

		saving = true;
		try {
			const updated = await applySoulTemplate(slug, template.id);
			soul = updated;
			editContent = updated.content;
			view = "editor";
		} finally {
			saving = false;
		}
	}

	function reset() {
		if (soul) {
			editContent = soul.content;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key === "s") {
			e.preventDefault();
			save();
		}
		if (e.key === "Escape") {
			onclose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="soul-panel">
	<!-- header -->
	<div class="flex items-center gap-3 border-b border-border/60 px-5 py-3.5">
		{#if view === "templates"}
			<button
				onclick={() => (view = "editor")}
				class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground/60 transition-colors hover:bg-muted/50 hover:text-foreground"
				disabled={!soul?.exists}
			>
				<ChevronLeft class="h-4 w-4" />
			</button>
		{:else}
			<div class="flex h-7 w-7 items-center justify-center rounded-lg bg-warm/10">
				<Sparkles class="h-3.5 w-3.5 text-warm" />
			</div>
		{/if}

		<div class="flex-1">
			<h2 class="font-display text-sm font-semibold tracking-tight">
				{view === "templates" ? "choose a soul" : "soul"}
			</h2>
			<p class="text-[11px] text-muted-foreground/50">
				{view === "templates"
					? "pick a personality template"
					: soul?.exists
						? "defines who your companion is"
						: "no soul yet"}
			</p>
		</div>

		<div class="flex items-center gap-1.5">
			{#if view === "editor" && soul?.exists}
				<button
					onclick={() => (view = "templates")}
					class="soul-header-btn"
					title="Browse templates"
				>
					<Sparkles class="h-3.5 w-3.5" />
				</button>
			{/if}

			{#if view === "editor" && dirty}
				<button onclick={reset} class="soul-header-btn" title="Discard changes">
					<RotateCcw class="h-3.5 w-3.5" />
				</button>
				<button
					onclick={save}
					disabled={saving}
					class="flex items-center gap-1.5 rounded-md bg-warm px-2.5 py-1.5 text-xs font-medium text-warm-foreground transition-colors hover:bg-warm/90 disabled:opacity-50"
				>
					<Save class="h-3 w-3" />
					{saving ? "saving..." : "save"}
				</button>
			{/if}

			{#if saved && !dirty}
				<span class="text-xs text-emerald-400/80">saved</span>
			{/if}

			<button onclick={onclose} class="soul-header-btn ml-1" title="Close">
				<X class="h-3.5 w-3.5" />
			</button>
		</div>
	</div>

	<!-- body -->
	{#if loading}
		<div class="flex flex-1 items-center justify-center">
			<div
				class="h-5 w-5 animate-spin rounded-full border-2 border-warm/30 border-t-warm"
			></div>
		</div>
	{:else if view === "templates"}
		<div class="flex-1 overflow-y-auto p-5">
			<div class="grid gap-3">
				{#each templates as template (template.id)}
					<button
						onclick={() => pickTemplate(template)}
						class="soul-template-card"
						disabled={saving}
					>
						<div class="flex items-start gap-3">
							<div
								class="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-warm/8"
							>
								<Sparkles class="h-4 w-4 text-warm/60" />
							</div>
							<div class="text-left">
								<p class="font-display text-sm font-medium text-foreground">
									{template.name}
								</p>
								<p class="mt-0.5 text-xs text-muted-foreground/60">
									{template.description}
								</p>
							</div>
						</div>
					</button>
				{/each}
			</div>
		</div>
	{:else}
		<div class="flex flex-1 flex-col overflow-hidden">
			<textarea
				bind:value={editContent}
				placeholder={"# soul\n\ndefine who your companion is...\n\n## voice\nhow do they speak?\n\n## personality\nwhat drives them?"}
				spellcheck={false}
				class="soul-textarea"
			></textarea>

			<div
				class="flex items-center justify-between border-t border-border/40 px-4 py-2"
			>
				<span class="text-[11px] text-muted-foreground/40">
					markdown &middot; {editContent.length} chars
				</span>
				<span class="text-[11px] text-muted-foreground/40">
					{#if dirty}unsaved changes{:else}&nbsp;{/if}
				</span>
			</div>
		</div>
	{/if}
</div>

<style>
	.soul-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		animation: soul-enter 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes soul-enter {
		from {
			opacity: 0;
			transform: translateX(12px);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}

	.soul-header-btn {
		display: flex;
		height: 1.75rem;
		width: 1.75rem;
		align-items: center;
		justify-content: center;
		border-radius: 0.375rem;
		color: oklch(var(--muted-foreground) / 0.6);
		transition: all 0.15s ease;
	}
	.soul-header-btn:hover {
		background: oklch(var(--muted) / 0.5);
		color: var(--foreground);
	}

	.soul-textarea {
		flex: 1;
		resize: none;
		padding: 1.25rem;
		font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
		font-size: 0.8125rem;
		line-height: 1.7;
		color: var(--foreground);
		background: transparent;
		outline: none;
		tab-size: 2;
	}
	.soul-textarea::placeholder {
		color: oklch(var(--muted-foreground) / 0.2);
	}

	.soul-template-card {
		width: 100%;
		border-radius: 0.75rem;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		background: oklch(0.78 0.12 75 / 3%);
		padding: 0.875rem 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.soul-template-card:hover {
		border-color: oklch(0.78 0.12 75 / 35%);
		background: oklch(0.78 0.12 75 / 7%);
		box-shadow: 0 0 20px -5px oklch(0.78 0.12 75 / 8%);
	}
	.soul-template-card:disabled {
		opacity: 0.5;
		cursor: wait;
	}
</style>
