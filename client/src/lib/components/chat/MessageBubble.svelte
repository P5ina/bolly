<script lang="ts">
	import type { ChatMessage } from "$lib/api/types.js";
	import Bot from "@lucide/svelte/icons/bot";
	import User from "@lucide/svelte/icons/user";

	let { message }: { message: ChatMessage } = $props();

	const isUser = $derived(message.role === "user");
	const time = $derived(() => {
		const ms = Number(message.created_at);
		if (Number.isNaN(ms)) return "";
		const d = new Date(ms);
		return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
	});
</script>

<div class="flex gap-3 {isUser ? 'flex-row-reverse' : 'flex-row'}">
	<!-- avatar -->
	<div class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full {isUser ? 'bg-muted' : 'bg-warm/12'}">
		{#if isUser}
			<User class="h-3.5 w-3.5 text-muted-foreground" />
		{:else}
			<Bot class="h-3.5 w-3.5 text-warm" />
		{/if}
	</div>

	<!-- bubble -->
	<div class="max-w-[75%] space-y-1">
		<div
			class="rounded-2xl px-4 py-2.5 text-sm leading-relaxed
				{isUser
					? 'rounded-tr-sm bg-muted text-foreground'
					: 'rounded-tl-sm border border-warm/10 bg-warm-muted text-foreground'}"
		>
			{message.content}
		</div>
		<p class="px-1 text-[10px] text-muted-foreground/40 {isUser ? 'text-right' : 'text-left'}">
			{time()}
		</p>
	</div>
</div>
