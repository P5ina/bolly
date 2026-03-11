<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { clearContext, fetchChats, fetchCompanionName, fetchMessages, fetchMood, sendMessage, stopAgent, uploadFile } from "$lib/api/client.js";
	import type { ChatMessage, ChatSummary, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import MessageBubble from "./MessageBubble.svelte";
	import ChatInput from "./ChatInput.svelte";
	import AsciiRenderer from "./AsciiRenderer.svelte";
	import StreamActivity from "./StreamActivity.svelte";
	import { play } from "$lib/sounds.js";
	import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";

	let { slug, chatId }: { slug: string; chatId: string } = $props();

	type StreamItem =
		| { type: "message"; data: ChatMessage }
		| { type: "activity"; id: string; kind: "tool" | "mood" | "state" | "output"; label: string; timestamp: string };

	let activeChatId = $derived(chatId);
	let chats = $state<ChatSummary[]>([]);
	let companionName = $state("");
	let messages = $state<ChatMessage[]>([]);
	let stream = $state<StreamItem[]>([]);
	let loading = $state(true);
	let sending = $state(false);
	let agentRunning = $state(false);
	let mood = $state("calm");
	let scrollContainer: HTMLDivElement | undefined = $state();
	let isConnected = $state(false);
	let showChatList = $state(false);

	const ws = getWebSocket();

	function scrollToBottom() {
		requestAnimationFrame(() => {
			if (scrollContainer) {
				scrollContainer.scrollTop = scrollContainer.scrollHeight;
			}
		});
	}

	function now() {
		return new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
	}

	function pushActivity(kind: "tool" | "mood" | "state" | "output", label: string) {
		stream = [...stream, {
			type: "activity",
			id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
			kind,
			label,
			timestamp: now(),
		}];
		scrollToBottom();
	}

	function addMessage(msg: ChatMessage) {
		if (!messages.some((m) => m.id === msg.id)) {
			messages = [...messages, msg];
			stream = [...stream, { type: "message", data: msg }];
			scrollToBottom();
		}
	}

	function isToolActivity(msg: ChatMessage): boolean {
		return msg.role === "assistant" && msg.content.startsWith("[tool activity]");
	}

	function toolActivityToStreamItems(msg: ChatMessage): StreamItem[] {
		return msg.content
			.split("\n")
			.filter((line) => line.startsWith("• "))
			.map((line, idx) => ({
				type: "activity" as const,
				id: `${msg.id}-${idx}`,
				kind: "tool" as const,
				label: line.slice(2).replace(/ →.*/, ""), // show tool name + args, trim result
				timestamp: new Date(Number(msg.created_at)).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
			}));
	}

	function messagesToStream(msgs: ChatMessage[]): StreamItem[] {
		return msgs.flatMap((m) =>
			isToolActivity(m)
				? toolActivityToStreamItems(m)
				: [{ type: "message" as const, data: m }]
		);
	}

	function loadChat(id: string) {
		showChatList = false;
		const path = id === "default" ? `/${slug}/chat` : `/${slug}/chat/${id}`;
		goto(path);
	}

	function refreshChatList() {
		fetchChats(slug)
			.then((res) => { chats = res; })
			.catch(() => {});
	}

	function newChat() {
		const id = `chat_${Date.now()}`;
		loadChat(id);
		// Will be created server-side on first message
		refreshChatList();
	}

	$effect(() => {
		const currentSlug = slug;
		const currentChat = chatId;
		untrack(() => {
			messages = [];
			stream = [];
			loading = true;
			isConnected = false;

			refreshChatList();

			fetchMessages(currentSlug, currentChat)
				.then((res) => {
					messages = res.messages.filter((m) => !isToolActivity(m));
					stream = messagesToStream(res.messages);
					agentRunning = res.agent_running;
					if (agentRunning) pushActivity("state", "thinking...");
					scrollToBottom();
				})
				.catch(() => { messages = []; })
				.finally(() => { loading = false; });

			fetchMood(currentSlug)
				.then((res) => { if (res.mood) mood = res.mood; })
				.catch(() => {});

			fetchCompanionName(currentSlug)
				.then((res) => { if (res.name) companionName = res.name; })
				.catch(() => {});
		});

		const unsub = ws.subscribe((event: ServerEvent) => {
			isConnected = true;
			if (event.type === "instance_discovered") return;
			if (event.instance_slug !== currentSlug) return;

			// Filter chat-specific events by chat_id
			const eventChatId = "chat_id" in event ? event.chat_id : undefined;
			if (eventChatId && eventChatId !== currentChat) return;

			if (event.type === "chat_message_created") {
				if (event.message.role === "assistant") play("message_receive");
				addMessage(event.message);
				refreshChatList();
			} else if (event.type === "mood_updated") {
				play("mood_shift");
				mood = event.mood;
				pushActivity("mood", `mood → ${event.mood}`);
			} else if (event.type === "agent_running") {
				agentRunning = true;
				pushActivity("state", "thinking...");
			} else if (event.type === "agent_stopped") {
				agentRunning = false;
				sending = false;
			} else if (event.type === "tool_activity") {
				// Skip tool_activity for set_mood — the dedicated mood_updated event handles it
				if (event.summary.startsWith("mood →")) return;
				const isOutput = event.tool_name.endsWith("_output");
				pushActivity(isOutput ? "output" : "tool", event.summary);
			} else if (event.type === "drop_created") {
				pushActivity("tool", `dropped: ${event.drop.title}`);
			} else if (event.type === "context_compacting") {
				pushActivity("state", `compacting ${event.messages_compacted} messages...`);
			}
		});
		return unsub;
	});

	async function handleSend(content: string, files?: File[]) {
		sending = true;
		try {
			// Upload files first, then reference them in the message
			let finalContent = content;
			if (files && files.length > 0) {
				const uploadResults = await Promise.all(
					files.map((f) => uploadFile(slug, f)),
				);
				const refs = uploadResults
					.map((u) => `[attached: ${u.original_name} (${u.id})]`)
					.join("\n");
				finalContent = finalContent ? `${finalContent}\n\n${refs}` : refs;
			}
			const res = await sendMessage(slug, finalContent, activeChatId);
			for (const msg of res.messages) addMessage(msg);
		} catch {
			sending = false;
		}
	}

	async function handleStop() {
		await stopAgent(slug, activeChatId);
	}

	async function handleClear() {
		await clearContext(slug, activeChatId);
		messages = [];
		stream = [];
	}

	function streamKey(item: StreamItem): string {
		return item.type === "message" ? item.data.id : item.id;
	}

	function getPrev(item: StreamItem, index: number): ChatMessage | undefined {
		if (item.type !== "message") return undefined;
		for (let i = index - 1; i >= 0; i--) {
			if (stream[i].type === "message") return (stream[i] as { type: "message"; data: ChatMessage }).data;
		}
		return undefined;
	}

	function chatLabel(chat: ChatSummary): string {
		if (chat.title && chat.title !== "untitled") return chat.title;
		if (chat.id === "default") return "default";
		return chat.id.replace("chat_", "#");
	}
</script>

<div class="chat-space">
	<div class="chat-glow" class:chat-glow-active={sending || agentRunning}></div>

	<header class="chat-bar">
		<div class="bar-left">
			<div class="bar-led" class:bar-led-on={isConnected}></div>
			<span class="bar-name">{companionName || slug}</span>
			<span class="bar-sep">/</span>
			<span class="bar-mood">{mood}</span>
		</div>
		<div class="bar-right">
			{#if sending || agentRunning}
				<span class="bar-status">working</span>
			{:else}
				<span class="bar-status">{messages.length} msgs</span>
			{/if}
			<button onclick={() => showChatList = !showChatList} onmousedown={(e) => e.preventDefault()} class="bar-btn" title="Chats">
				<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" class="w-3 h-3">
					<path d="M2 3h12M2 7h8M2 11h10" stroke-linecap="round"/>
				</svg>
			</button>
			<button onclick={newChat} onmousedown={(e) => e.preventDefault()} class="bar-btn" title="New chat">
				<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" class="w-3 h-3">
					<path d="M8 3v10M3 8h10" stroke-linecap="round"/>
				</svg>
			</button>
			<AlertDialog.Root>
				<AlertDialog.Trigger class="bar-btn bar-clear" title="Clear context">
					<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" class="w-3 h-3">
						<path d="M2 4h12M5.5 4V2.5h5V4M6 7v5M10 7v5M3.5 4l.75 9.5h7.5L12.5 4" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</AlertDialog.Trigger>
				<AlertDialog.Portal>
					<AlertDialog.Overlay class="clear-dialog-overlay" />
					<AlertDialog.Content class="clear-dialog">
						<AlertDialog.Header>
							<AlertDialog.Title class="clear-dialog-title">clear context</AlertDialog.Title>
							<AlertDialog.Description class="clear-dialog-desc">
								this will erase all messages in this conversation. this cannot be undone.
							</AlertDialog.Description>
						</AlertDialog.Header>
						<AlertDialog.Footer class="clear-dialog-footer">
							<AlertDialog.Cancel class="clear-dialog-btn clear-dialog-cancel">cancel</AlertDialog.Cancel>
							<AlertDialog.Action class="clear-dialog-btn clear-dialog-confirm" onclick={handleClear}>clear</AlertDialog.Action>
						</AlertDialog.Footer>
					</AlertDialog.Content>
				</AlertDialog.Portal>
			</AlertDialog.Root>
		</div>
	</header>

	<!-- Chat list dropdown -->
	{#if showChatList}
		<div class="chat-list-overlay" onclick={() => showChatList = false} role="presentation"></div>
		<div class="chat-list">
			{#each chats as chat (chat.id)}
				<button
					class="chat-list-item"
					class:chat-list-active={chat.id === activeChatId}
					onclick={() => loadChat(chat.id)}
				>
					<span class="chat-list-label">{chatLabel(chat)}</span>
					<span class="chat-list-count">{chat.message_count}</span>
				</button>
			{:else}
				<div class="chat-list-empty">no chats yet</div>
			{/each}
		</div>
	{/if}

	<div class="chat-columns">
		<div class="chat-main">
			<div class="chat-stream" bind:this={scrollContainer}>
				<div class="stream-inner">
					{#if loading}
						<div class="chat-loading"><div class="loading-dot"></div></div>
					{:else if stream.length === 0}
						<div class="chat-empty"><p>say something.</p></div>
					{:else}
						{#each stream as item, i (streamKey(item))}
							{#if item.type === "message"}
								<MessageBubble message={item.data} {slug} index={i} prevMessage={getPrev(item, i)} />
							{:else}
								<StreamActivity kind={item.kind} label={item.label} timestamp={item.timestamp} />
							{/if}
						{/each}
					{/if}

					{#if sending || agentRunning}
						<div class="chat-thinking">
							<div class="think-dot" style="animation-delay: 0ms"></div>
							<div class="think-dot" style="animation-delay: 200ms"></div>
							<div class="think-dot" style="animation-delay: 400ms"></div>
						</div>
					{/if}
				</div>
			</div>

			<ChatInput onSend={handleSend} onStop={handleStop} disabled={sending || agentRunning} {agentRunning} />
		</div>

		<aside class="chat-creature">
			<AsciiRenderer thinking={sending || agentRunning} {mood} />
		</aside>
	</div>
</div>

<style>
	.chat-space {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.chat-glow {
		position: absolute;
		top: -80px;
		left: 50%;
		width: 600px;
		height: 400px;
		transform: translateX(-50%);
		border-radius: 50%;
		background: radial-gradient(ellipse, oklch(0.78 0.12 75 / 4%) 0%, transparent 65%);
		animation: breathe 7s ease-in-out infinite;
		pointer-events: none;
		z-index: 0;
	}

	.chat-glow-active {
		animation: breathe-fast 2.5s ease-in-out infinite;
		background: radial-gradient(ellipse, oklch(0.78 0.12 75 / 7%) 0%, transparent 65%);
	}

	@keyframes breathe {
		0%, 100% { transform: translateX(-50%) scale(1); opacity: 0.6; }
		50% { transform: translateX(-50%) scale(1.05); opacity: 1; }
	}

	@keyframes breathe-fast {
		0%, 100% { transform: translateX(-50%) scale(1); opacity: 0.7; }
		30% { transform: translateX(-50%) scale(1.1); opacity: 1; }
		60% { transform: translateX(-50%) scale(0.97); opacity: 0.5; }
	}

	/* --- bar --- */

	header.chat-bar {
		position: relative;
		z-index: 4;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.6rem 1.25rem;
		border-bottom: 1px solid oklch(0.78 0.12 75 / 6%);
		flex-shrink: 0;
	}

	.bar-left, .bar-right {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.04em;
	}

	.bar-led {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.40 0.01 280 / 40%);
		transition: all 0.4s ease;
	}

	.bar-led-on {
		background: oklch(0.78 0.12 75 / 85%);
		box-shadow: 0 0 8px oklch(0.78 0.12 75 / 25%);
	}

	.bar-name { color: oklch(0.90 0.04 75 / 90%); }
	.bar-sep { color: oklch(0.50 0.02 280 / 25%); }
	.bar-mood { color: oklch(0.72 0.06 75 / 60%); }

	.bar-status {
		color: oklch(0.58 0.02 280 / 45%);
		font-size: 0.6rem;
	}

	.bar-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		color: oklch(0.55 0.02 280 / 35%);
		border-radius: 4px;
		transition: all 0.2s ease;
	}

	.bar-btn:hover {
		color: oklch(0.78 0.12 75 / 70%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	.bar-clear:hover {
		color: oklch(0.65 0.08 25 / 70%);
		background: oklch(0.65 0.08 25 / 8%);
	}

	/* --- chat list --- */

	.chat-list-overlay {
		position: fixed;
		inset: 0;
		z-index: 3;
	}

	.chat-list {
		position: absolute;
		top: 2.6rem;
		right: 1.25rem;
		z-index: 5;
		min-width: 180px;
		max-height: 280px;
		overflow-y: auto;
		background: oklch(0.14 0.01 280);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		border-radius: 8px;
		padding: 0.25rem;
		box-shadow: 0 8px 32px oklch(0 0 0 / 40%);
		animation: list-enter 0.15s ease both;
	}

	@keyframes list-enter {
		from { opacity: 0; transform: translateY(-4px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.chat-list-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 0.4rem 0.6rem;
		border-radius: 5px;
		font-family: var(--font-mono);
		font-size: 0.65rem;
		color: oklch(0.75 0.03 75 / 60%);
		transition: all 0.15s ease;
		text-align: left;
	}

	.chat-list-item:hover {
		background: oklch(0.78 0.12 75 / 6%);
		color: oklch(0.88 0.04 75 / 85%);
	}

	.chat-list-active {
		background: oklch(0.78 0.12 75 / 8%);
		color: oklch(0.90 0.05 75 / 90%);
	}

	.chat-list-label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 130px;
	}

	.chat-list-count {
		font-size: 0.55rem;
		color: oklch(0.55 0.02 280 / 35%);
		flex-shrink: 0;
	}

	.chat-list-empty {
		padding: 0.6rem;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.50 0.02 280 / 35%);
		text-align: center;
	}

	/* --- columns --- */

	.chat-columns {
		position: relative;
		z-index: 2;
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: 1fr 1fr;
	}

	.chat-main {
		display: flex;
		flex-direction: column;
		min-height: 0;
		border-right: 1px solid oklch(0.78 0.12 75 / 5%);
	}

	.chat-creature {
		display: flex;
		align-items: center;
		justify-content: center;
		transform: scale(2.2);
		opacity: 0.5;
		pointer-events: none;
	}

	/* --- stream --- */

	.chat-stream {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
	}

	.stream-inner {
		max-width: 640px;
		margin: 0 auto;
		padding: 0.75rem 1.25rem 2rem;
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}

	.chat-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 3rem 0;
	}

	.loading-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 25%);
		animation: pulse 2s ease-in-out infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(0.7); }
	}

	.chat-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 5rem 0;
		animation: fade-up 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.4s both;
	}

	.chat-empty p {
		font-family: var(--font-display);
		font-size: 0.85rem;
		font-style: italic;
		color: oklch(0.60 0.03 75 / 35%);
		margin: 0;
	}

	@keyframes fade-up {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.chat-thinking {
		display: flex;
		gap: 0.35rem;
		padding: 0.6rem 0;
		animation: fade-up 0.3s ease both;
	}

	.think-dot {
		width: 3.5px;
		height: 3.5px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 35%);
		animation: bounce 1.4s ease-in-out infinite;
	}

	@keyframes bounce {
		0%, 60%, 100% { transform: translateY(0); opacity: 0.25; }
		30% { transform: translateY(-5px); opacity: 1; }
	}

	/* --- responsive --- */

	@media (max-width: 900px) {
		.chat-columns {
			grid-template-columns: 1fr;
		}
		.chat-creature {
			display: none;
		}
		.chat-main {
			border-right: none;
		}
	}

	@media (max-width: 720px) {
		.stream-inner {
			padding-inline: 0.75rem;
		}
		header.chat-bar {
			padding: 0.5rem 0.75rem;
		}
		.bar-right {
			max-width: 50%;
		}
	}

	/* --- clear context dialog --- */

	:global(.clear-dialog-overlay) {
		position: fixed;
		inset: 0;
		z-index: 50;
		background: oklch(0 0 0 / 60%);
		backdrop-filter: blur(4px);
		animation: overlay-in 0.2s ease both;
	}

	@keyframes overlay-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	:global(.clear-dialog) {
		position: fixed;
		top: 50%;
		left: 50%;
		z-index: 51;
		transform: translate(-50%, -50%);
		width: min(360px, calc(100vw - 2rem));
		background: oklch(0.12 0.01 280);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		border-radius: 12px;
		padding: 1.5rem;
		box-shadow: 0 16px 64px oklch(0 0 0 / 50%);
		animation: dialog-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes dialog-in {
		from { opacity: 0; transform: translate(-50%, -48%) scale(0.96); }
		to { opacity: 1; transform: translate(-50%, -50%) scale(1); }
	}

	:global(.clear-dialog-title) {
		font-family: var(--font-mono);
		font-size: 0.8rem;
		letter-spacing: 0.04em;
		color: oklch(0.90 0.04 75 / 90%);
		margin: 0;
	}

	:global(.clear-dialog-desc) {
		font-family: var(--font-body);
		font-size: 0.75rem;
		line-height: 1.5;
		color: oklch(0.70 0.02 280 / 60%);
		margin-top: 0.5rem;
	}

	:global(.clear-dialog-footer) {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 1.25rem;
	}

	:global(.clear-dialog-btn) {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.04em;
		padding: 0.4rem 1rem;
		border-radius: 6px;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	:global(.clear-dialog-cancel) {
		color: oklch(0.70 0.02 280 / 60%);
		background: oklch(1 0 0 / 4%);
		border: 1px solid oklch(1 0 0 / 8%);
	}

	:global(.clear-dialog-cancel:hover) {
		background: oklch(1 0 0 / 8%);
		color: oklch(0.85 0.02 280 / 80%);
	}

	:global(.clear-dialog-confirm) {
		color: oklch(0.90 0.08 25 / 90%);
		background: oklch(0.65 0.12 25 / 15%);
		border: 1px solid oklch(0.65 0.12 25 / 25%);
	}

	:global(.clear-dialog-confirm:hover) {
		background: oklch(0.65 0.12 25 / 25%);
	}
</style>
