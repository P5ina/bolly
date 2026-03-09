<script lang="ts">
	import type { InstanceSummary } from "$lib/api/types.js";
	import Sparkles from "@lucide/svelte/icons/sparkles";
	import Brain from "@lucide/svelte/icons/brain";
	import FileText from "@lucide/svelte/icons/file-text";
	import Palette from "@lucide/svelte/icons/palette";
	import ArrowRight from "@lucide/svelte/icons/arrow-right";

	let { instance }: { instance: InstanceSummary } = $props();
</script>

<a
	href="/{instance.slug}"
	class="group relative flex flex-col gap-4 rounded-xl border border-border/60 bg-card p-5 transition-all duration-300 hover:border-warm/25 hover:bg-card/80 hover:shadow-lg hover:shadow-warm/5"
>
	<div class="flex items-start justify-between">
		<div class="flex items-center gap-3">
			<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-warm/10 text-warm font-display text-lg font-bold">
				{instance.slug[0]?.toUpperCase() ?? "?"}
			</div>
			<div>
				<h3 class="font-display text-base font-semibold tracking-tight text-card-foreground">
					{instance.slug}
				</h3>
				<p class="text-xs text-muted-foreground">
					{instance.drops_count} drop{instance.drops_count !== 1 ? "s" : ""}
				</p>
			</div>
		</div>
		<ArrowRight class="h-4 w-4 text-muted-foreground/30 transition-all duration-300 group-hover:translate-x-0.5 group-hover:text-warm/60" />
	</div>

	<div class="flex items-center gap-3 text-xs text-muted-foreground/60">
		{#if instance.soul_exists}
			<span class="flex items-center gap-1">
				<Sparkles class="h-3 w-3 text-warm/50" />
				soul
			</span>
		{/if}
		{#if instance.has_memory}
			<span class="flex items-center gap-1">
				<Brain class="h-3 w-3" />
				memory
			</span>
		{/if}
		{#if instance.has_skin}
			<span class="flex items-center gap-1">
				<Palette class="h-3 w-3" />
				skin
			</span>
		{/if}
		{#if instance.drops_count > 0}
			<span class="flex items-center gap-1">
				<FileText class="h-3 w-3" />
				{instance.drops_count}
			</span>
		{/if}
	</div>

	<!-- warm accent line at bottom -->
	<div class="absolute bottom-0 left-4 right-4 h-px bg-gradient-to-r from-transparent via-warm/0 to-transparent transition-all duration-500 group-hover:via-warm/30"></div>
</a>
