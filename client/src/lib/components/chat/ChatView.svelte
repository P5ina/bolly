<script lang="ts">
	import { fetchMessages, sendMessage } from "$lib/api/client.js";
	import type { ChatMessage, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import MessageBubble from "./MessageBubble.svelte";
	import ChatInput from "./ChatInput.svelte";
	import SoulEditor from "$lib/components/soul/SoulEditor.svelte";
	import ScrollArea from "$lib/components/ui/scroll-area/scroll-area.svelte";
	import Bot from "@lucide/svelte/icons/bot";
	import Sparkles from "@lucide/svelte/icons/sparkles";

	let { slug }: { slug: string } = $props();

	let soulOpen = $state(false);

	let messages = $state<ChatMessage[]>([]);
	let loading = $state(true);
	let sending = $state(false);
	let scrollContainer: HTMLDivElement | undefined = $state();

	const ws = getWebSocket();

	function scrollToBottom() {
		requestAnimationFrame(() => {
			if (scrollContainer) {
				scrollContainer.scrollTop = scrollContainer.scrollHeight;
			}
		});
	}

	$effect(() => {
		// reset on slug change
		messages = [];
		loading = true;

		fetchMessages(slug)
			.then((res) => {
				messages = res.messages;
				scrollToBottom();
			})
			.catch(() => {
				messages = [];
			})
			.finally(() => {
				loading = false;
			});

		const unsub = ws.subscribe((event: ServerEvent) => {
			if (
				event.type === "chat_message_created" &&
				event.instance_slug === slug
			) {
				const exists = messages.some((m) => m.id === event.message.id);
				if (!exists) {
					messages = [...messages, event.message];
					scrollToBottom();
				}
			}
		});

		return unsub;
	});

	async function handleSend(content: string) {
		sending = true;
		try {
			const res = await sendMessage(slug, content);
			// merge new messages, avoiding duplicates from ws
			for (const msg of res.messages) {
				if (!messages.some((m) => m.id === msg.id)) {
					messages = [...messages, msg];
				}
			}
			scrollToBottom();
		} finally {
			sending = false;
		}
	}
</script>

<div class="page-enter flex h-full">
	<!-- chat column -->
	<div class="flex flex-1 flex-col {soulOpen ? 'border-r border-border/60' : ''}">
		<!-- header -->
		<div class="flex items-center gap-3 border-b border-border/60 px-5 py-3.5">
			<div class="flex h-8 w-8 items-center justify-center rounded-lg bg-warm/10 text-warm font-display text-sm font-bold">
				{slug[0]?.toUpperCase() ?? "?"}
			</div>
			<div class="flex-1">
				<h2 class="font-display text-sm font-semibold tracking-tight">{slug}</h2>
				<p class="text-[11px] text-muted-foreground/50">
					{messages.length} message{messages.length !== 1 ? "s" : ""}
				</p>
			</div>
			<button
				onclick={() => (soulOpen = !soulOpen)}
				class="flex h-8 w-8 items-center justify-center rounded-lg transition-colors
					{soulOpen ? 'bg-warm/15 text-warm' : 'text-muted-foreground/50 hover:bg-muted/50 hover:text-foreground'}"
				title="Edit soul"
			>
				<Sparkles class="h-4 w-4" />
			</button>
		</div>

		<!-- messages -->
		<div class="flex-1 overflow-y-auto" bind:this={scrollContainer}>
			<div class="mx-auto max-w-3xl space-y-4 px-4 py-6">
				{#if loading}
					<div class="flex items-center justify-center py-16">
						<div class="h-5 w-5 animate-spin rounded-full border-2 border-warm/30 border-t-warm"></div>
					</div>
				{:else if messages.length === 0}
					<div class="flex flex-col items-center justify-center py-24 text-center">
						<div class="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-warm/8">
							<Bot class="h-7 w-7 text-warm/50" />
						</div>
						<p class="font-display text-base text-muted-foreground/50">Start a conversation</p>
						<p class="mt-1.5 flex items-center gap-1 text-xs text-muted-foreground/30">
							<Sparkles class="h-3 w-3" />
							Your companion is listening
						</p>
					</div>
				{:else}
					{#each messages as message (message.id)}
						<MessageBubble {message} />
					{/each}
				{/if}

				{#if sending}
					<div class="flex gap-3">
						<div class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-warm/12">
							<Bot class="h-3.5 w-3.5 text-warm" />
						</div>
						<div class="rounded-2xl rounded-tl-sm border border-warm/10 bg-warm-muted px-4 py-3">
							<div class="flex gap-1">
								<div class="h-1.5 w-1.5 animate-bounce rounded-full bg-warm/40" style="animation-delay: 0ms"></div>
								<div class="h-1.5 w-1.5 animate-bounce rounded-full bg-warm/40" style="animation-delay: 150ms"></div>
								<div class="h-1.5 w-1.5 animate-bounce rounded-full bg-warm/40" style="animation-delay: 300ms"></div>
							</div>
						</div>
					</div>
				{/if}
			</div>
		</div>

		<!-- input -->
		<ChatInput onSend={handleSend} disabled={sending} />
	</div>

	<!-- soul panel -->
	{#if soulOpen}
		<div class="w-[420px] shrink-0">
			<SoulEditor {slug} onclose={() => (soulOpen = false)} />
		</div>
	{/if}
</div>
