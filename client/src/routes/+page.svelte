<script lang="ts">
	import { goto } from "$app/navigation";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import InstanceCard from "$lib/components/home/InstanceCard.svelte";
	import Onboarding from "$lib/components/onboarding/Onboarding.svelte";
	import Button from "$lib/components/ui/button/button.svelte";
	import Input from "$lib/components/ui/input/input.svelte";
	import Sparkles from "@lucide/svelte/icons/sparkles";
	import Plus from "@lucide/svelte/icons/plus";
	import ArrowRight from "@lucide/svelte/icons/arrow-right";

	const instances = getInstances();

	const showOnboarding = $derived(!instances.loading && instances.list.length === 0);

	let newSlug = $state("");
	let showInput = $state(false);

	function create() {
		const slug = newSlug
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9_-]/g, "-")
			.replace(/-+/g, "-")
			.replace(/^-|-$/g, "");
		if (!slug) return;
		goto(`/${slug}`);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			create();
		}
		if (e.key === "Escape") {
			showInput = false;
			newSlug = "";
		}
	}
</script>

{#if showOnboarding}
	<Onboarding />
{:else}
	<div class="page-enter flex h-full flex-col overflow-y-auto">
		<div class="mx-auto w-full max-w-2xl px-6 py-16">
			<!-- header -->
			<div class="mb-12">
				<div class="mb-4 flex items-center gap-2 text-warm/70">
					<Sparkles class="h-4 w-4" />
					<span class="font-mono text-xs tracking-wide uppercase">companion</span>
				</div>
				<h1 class="font-display text-4xl font-bold tracking-tight text-foreground">
					Your space.
				</h1>
				<p class="mt-3 text-base leading-relaxed text-muted-foreground">
					A companion that lives with you — remembers, thinks, creates.
				</p>
			</div>

			<!-- new instance -->
			<div class="mb-8">
				{#if showInput}
					<div class="flex items-center gap-2">
						<Input
							bind:value={newSlug}
							onkeydown={handleKeydown}
							placeholder="my-companion"
							autofocus
							class="h-10 rounded-lg border-border/50 bg-muted/40 font-mono text-sm placeholder:text-muted-foreground/30 focus-visible:ring-warm/30"
						/>
						<Button
							onclick={create}
							disabled={!newSlug.trim()}
							class="h-10 gap-1.5 rounded-lg bg-warm px-4 text-sm font-medium text-warm-foreground hover:bg-warm/90 disabled:bg-muted disabled:text-muted-foreground/30"
						>
							Go
							<ArrowRight class="h-3.5 w-3.5" />
						</Button>
					</div>
					<p class="mt-2 text-xs text-muted-foreground/40">
						Pick a name for your companion. Letters, numbers, hyphens.
					</p>
				{:else}
					<Button
						variant="outline"
						onclick={() => (showInput = true)}
						class="h-10 gap-2 rounded-lg border-dashed border-border/60 text-sm text-muted-foreground hover:border-warm/30 hover:text-warm"
					>
						<Plus class="h-4 w-4" />
						New instance
					</Button>
				{/if}
			</div>

			<!-- instance grid -->
			{#if instances.list.length > 0}
				<div class="grid gap-3 sm:grid-cols-2">
					{#each instances.list as instance (instance.slug)}
						<InstanceCard {instance} />
					{/each}
				</div>
			{/if}

			{#if instances.loading}
				<div class="grid gap-3 sm:grid-cols-2">
					{#each [1, 2, 3] as _}
						<div class="h-28 animate-pulse rounded-xl border border-border/30 bg-card/50"></div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
{/if}
