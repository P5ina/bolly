<script lang="ts">
	import Button from "$lib/components/ui/button/button.svelte";
	import Textarea from "$lib/components/ui/textarea/textarea.svelte";
	import SendHorizontal from "@lucide/svelte/icons/send-horizontal";
	import Loader from "@lucide/svelte/icons/loader";

	let {
		onSend,
		disabled = false,
	}: {
		onSend: (content: string) => void;
		disabled?: boolean;
	} = $props();

	let value = $state("");

	function handleSubmit() {
		const trimmed = value.trim();
		if (!trimmed || disabled) return;
		onSend(trimmed);
		value = "";
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}
</script>

<div class="border-t border-border/60 bg-background/80 px-4 py-3 backdrop-blur-sm">
	<div class="mx-auto flex max-w-3xl items-end gap-2">
		<div class="relative flex-1">
			<Textarea
				bind:value
				onkeydown={handleKeydown}
				placeholder="Say something..."
				rows={1}
				class="min-h-[44px] max-h-[160px] resize-none rounded-xl border-border/50 bg-muted/40 pr-4 text-sm placeholder:text-muted-foreground/40 focus-visible:ring-warm/30"
				{disabled}
			/>
		</div>
		<Button
			size="icon"
			onclick={handleSubmit}
			disabled={disabled || !value.trim()}
			class="h-[44px] w-[44px] shrink-0 rounded-xl bg-warm text-warm-foreground transition-all hover:bg-warm/90 disabled:bg-muted disabled:text-muted-foreground/30"
		>
			{#if disabled}
				<Loader class="h-4 w-4 animate-spin" />
			{:else}
				<SendHorizontal class="h-4 w-4" />
			{/if}
		</Button>
	</div>
</div>
