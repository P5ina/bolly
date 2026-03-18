<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { clearContext, fetchChats, fetchCompanionName, fetchGoogleAccounts, fetchMessages, fetchMood, sendMessage, stopAgent, uploadFile } from "$lib/api/client.js";
	import type { ChatMessage, ChatSummary, ServerEvent } from "$lib/api/types.js";
	import { getWebSocket } from "$lib/stores/websocket.svelte.js";
	import MessageBubble from "./MessageBubble.svelte";
	import ChatInput from "./ChatInput.svelte";
	import AsciiRenderer from "./AsciiRenderer.svelte";
	import CreatureBubble from "./CreatureBubble.svelte";
	import StreamActivity from "./StreamActivity.svelte";
	import ContextStats from "./ContextStats.svelte";
	import HeartbeatUpdateBanner from "./HeartbeatUpdateBanner.svelte";
import McpAppViewer from "./McpAppViewer.svelte";
	import { play, playImmediate, preload } from "$lib/sounds.js";
	import { hapticMedium, hapticDouble, hapticError } from "$lib/haptics.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
	import TerminalSquare from "@lucide/svelte/icons/terminal-square";
	import BarChart3 from "@lucide/svelte/icons/bar-chart-3";
	import Eraser from "@lucide/svelte/icons/eraser";
	import Minimize2 from "@lucide/svelte/icons/minimize-2";

	const toast = getToasts();

	let { slug, chatId }: { slug: string; chatId: string } = $props();

	type StreamItem =
		| { type: "message"; data: ChatMessage }
		| { type: "activity"; id: string; kind: "tool" | "mood" | "state" | "output"; label: string; timestamp: string }
| { type: "mcp_app"; id: string; toolName: string; toolInput: string; toolOutput: string; html: string }
		| { type: "compaction"; id: string; count: number; timestamp: string };

	let activeChatId = $derived(chatId);
	let chats = $state<ChatSummary[]>([]);
	let companionName = $state("");
	let messages = $state<ChatMessage[]>([]);
	let stream = $state<StreamItem[]>([]);
	let loading = $state(true);
	let sending = $state(false);
	let agentRunning = $state(false);
	let needsGoogleReconnect = $state(false);
	let mood = $state(
		(typeof localStorage !== "undefined" && localStorage.getItem("mood:" + slug)) || "calm"
	);
	let scrollContainer: HTMLDivElement | undefined = $state();
	let isConnected = $state(false);
	let showChatList = $state(false);
	let clearDialogOpen = $state(false);
	let showContextStats = $state(false);
	let showToolActivity = $state(
		typeof localStorage !== "undefined"
			? (localStorage.getItem("bolly:showToolActivity") ?? "true") === "true"
			: true,
	);
	let streamingContent = $state("");
	let displayedLength = $state(0);
	let typewriterRaf = 0;
	let lastTypewriterTime = 0;
	let lastTypeSoundChar = 0;

	const CHARS_PER_FRAME = 2;
	const FRAME_INTERVAL = 16; // ~60fps
	const TYPE_SOUND_EVERY = 3; // play sound every N chars

	preload("typewriter", "message_receive", "message_send", "error");

	function startTypewriter() {
		if (typewriterRaf) return;
		lastTypewriterTime = performance.now();
		function tick(now: number) {
			if (displayedLength >= streamingContent.length && !streamingContent) {
				typewriterRaf = 0;
				return;
			}
			const elapsed = now - lastTypewriterTime;
			if (elapsed >= FRAME_INTERVAL) {
				const charsToAdd = Math.max(1, Math.floor(elapsed / FRAME_INTERVAL) * CHARS_PER_FRAME);
				const newLen = Math.min(displayedLength + charsToAdd, streamingContent.length);
				if (newLen !== displayedLength) {
					displayedLength = newLen;
					// Play typing sound every N characters
					const soundsSince = Math.floor(newLen / TYPE_SOUND_EVERY) - Math.floor(lastTypeSoundChar / TYPE_SOUND_EVERY);
					if (soundsSince > 0) {
						playImmediate("typewriter", { pitchRange: [0.88, 1.15] });
						lastTypeSoundChar = newLen;
					}
					updateStreamingBubble();
					scrollToBottom();
				}
				lastTypewriterTime = now;
			}
			if (displayedLength < streamingContent.length) {
				typewriterRaf = requestAnimationFrame(tick);
			} else {
				typewriterRaf = 0;
			}
		}
		typewriterRaf = requestAnimationFrame(tick);
	}

	function updateStreamingBubble() {
		const displayed = streamingContent.slice(0, displayedLength);
		const streamIdx = stream.findIndex((s) => s.type === "message" && s.data.id === "__streaming__");
		const streamingMsg: ChatMessage = {
			id: "__streaming__",
			role: "assistant",
			content: displayed,
			created_at: new Date().toISOString(),
		};
		if (streamIdx >= 0) {
			stream[streamIdx] = { type: "message", data: streamingMsg };
			stream = stream;
		} else {
			stream = [...stream, { type: "message", data: streamingMsg }];
		}
	}

	const ws = getWebSocket();
	let hadConnection = false;

	// Reload full chat after WebSocket reconnect to pick up missed messages
	$effect(() => {
		const isConnected = ws.connected;
		untrack(() => {
			if (!isConnected) return;
			if (!hadConnection) {
				// First connection — skip, the main load effect handles this
				hadConnection = true;
				return;
			}
			// Reconnected — re-fetch to pick up messages we missed
			fetchMessages(slug, chatId)
				.then((res) => {
					messages = res.messages.filter((m) => !isToolActivity(m));
					stream = messagesToStream(res.messages);
					agentRunning = res.agent_running;
					if (agentRunning) pushActivity("state", "thinking...");
					scrollToBottom();
				})
				.catch(() => {});
		});
	});

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

	function pushActivity(kind: "tool" | "mood" | "state" | "output", label: string, idPrefix?: string) {
		// Dedup: skip if the last activity of the same kind has the same label
		const last = stream.findLast((s) => s.type === "activity" && s.kind === kind);
		if (last && last.type === "activity" && last.label === label) return;
		stream = [...stream, {
			type: "activity",
			id: `${idPrefix ?? ""}${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
			kind,
			label,
			timestamp: now(),
		}];
		scrollToBottom();
	}

	function addMessage(msg: ChatMessage) {
		if (!messages.some((m) => m.id === msg.id)) {
			// Clear streaming bubble when first real assistant message arrives
			if (msg.role === "assistant" && streamingContent) {
				streamingContent = "";
				displayedLength = 0;
				if (typewriterRaf) { cancelAnimationFrame(typewriterRaf); typewriterRaf = 0; }
				stream = stream.filter((s) => !(s.type === "message" && s.data.id === "__streaming__"));
			}
			// Replace promoted placeholder with the real message if content matches
			if (msg.role === "assistant") {
				const promotedIdx = stream.findIndex((s) =>
					s.type === "message" && s.data.id.startsWith("__promoted_") && s.data.content === msg.content
				);
				if (promotedIdx >= 0) {
					stream[promotedIdx] = { type: "message", data: msg };
					stream = stream;
					messages = [...messages, msg];
					return;
				}
			}
			messages = [...messages, msg];
			stream = [...stream, { type: "message", data: msg }];
			scrollToBottom();
		}
	}

	function isToolActivity(msg: ChatMessage): boolean {
		if (msg.kind === "tool_call" || msg.kind === "tool_output" || msg.kind === "mcp_app" || msg.kind === "compaction") return true;
		if (msg.content.startsWith("[restart]")) return true;
		return msg.role === "assistant" && (
			msg.content.startsWith("[tool activity]") ||
			msg.content.startsWith("[tool:") ||
			msg.content.startsWith("[system]")
		);
	}

	function toolActivityToStreamItem(msg: ChatMessage): StreamItem | null {
		const ts = new Date(Number(msg.created_at)).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

		if (msg.kind === "compaction") {
			return {
				type: "compaction" as const,
				id: msg.id,
				count: 0,
				timestamp: ts,
			};
		}

		if (msg.kind === "tool_call" || msg.kind === "tool_output") {
			if (msg.tool_name === "set_mood") return null;
			return {
				type: "activity" as const,
				id: msg.id,
				kind: msg.kind === "tool_output" ? "output" as const : "tool" as const,
				label: msg.content,
				timestamp: ts,
			};
		}
		if (msg.content.startsWith("[tool:")) {
			if (msg.content.startsWith("[tool: set_mood]")) return null;
			const isOutput = msg.content.includes(" output]");
			return {
				type: "activity" as const,
				id: msg.id,
				kind: isOutput ? "output" as const : "tool" as const,
				label: msg.content.replace(/^\[tool:[^\]]*\]\s*/, ""),
				timestamp: ts,
			};
		}
		if (msg.content.startsWith("[system]") || msg.content.startsWith("[restart]")) {
			return {
				type: "activity" as const,
				id: msg.id,
				kind: "state" as const,
				label: msg.content.replace(/^\[(system|restart)\]\s*/, ""),
				timestamp: ts,
			};
		}
		// Legacy [tool activity] format — render as single activity item
		return {
			type: "activity" as const,
			id: msg.id,
			kind: "tool" as const,
			label: msg.content.replace(/^\[tool activity\]\s*/, ""),
			timestamp: ts,
		};
	}

	function messagesToStream(msgs: ChatMessage[]): StreamItem[] {
		return msgs.flatMap((m) => {
			if (m.kind === "mcp_app" && m.mcp_app_html && m.tool_name) {
				return [{
					type: "mcp_app" as const,
					id: m.id,
					toolName: m.tool_name,
					toolInput: m.mcp_app_input ?? "{}",
					toolOutput: m.content,
					html: m.mcp_app_html,
				}];
			}
			if (isToolActivity(m)) {
				const item = toolActivityToStreamItem(m);
				return item ? [item] : [];
			}
			return [{ type: "message" as const, data: m }];
		});
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
			streamingContent = "";

			refreshChatList();

			fetchMessages(currentSlug, currentChat)
				.then((res) => {
					messages = res.messages.filter((m) => !isToolActivity(m));
					stream = messagesToStream(res.messages);
					agentRunning = res.agent_running;
					if (agentRunning) pushActivity("state", "thinking...");
					scrollToBottom();
				})
				.catch((e) => {
					messages = [];
					if (!(e instanceof Error && e.message === "unauthorized")) {
						toast.error("failed to load messages");
					}
				})
				.finally(() => { loading = false; });

			fetchMood(currentSlug)
				.then((res) => { if (res.mood) { mood = res.mood; localStorage.setItem("mood:" + currentSlug, res.mood); } })
				.catch(() => {}); // mood is non-critical

			fetchCompanionName(currentSlug)
				.then((res) => { if (res.name) companionName = res.name; })
				.catch(() => {}); // name is non-critical

			// Check if Google accounts need reconnection (missing drive scope)
			fetchGoogleAccounts(currentSlug)
				.then((accounts) => {
					needsGoogleReconnect = accounts.some((a) =>
						a.scopes && !a.scopes.includes("auth/drive ") && !a.scopes.endsWith("auth/drive")
						&& a.scopes.includes("drive.file")
					);
				})
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
				const msg = event.message;
				if (msg.kind === "mcp_app" && msg.mcp_app_html && msg.tool_name) {
					stream = [...stream, {
						type: "mcp_app" as const,
						id: msg.id,
						toolName: msg.tool_name,
						toolInput: msg.mcp_app_input ?? "{}",
						toolOutput: msg.content,
						html: msg.mcp_app_html,
					}];
					scrollToBottom();
				} else if (isToolActivity(msg)) {
					// Promote streaming bubble to a real message so it doesn't vanish
					if (streamingContent) {
						const snapshotContent = streamingContent;
						streamingContent = "";
						displayedLength = 0;
						if (typewriterRaf) { cancelAnimationFrame(typewriterRaf); typewriterRaf = 0; }
						const snapshotId = `__promoted_${Date.now()}`;
						stream = stream.map((s) =>
							s.type === "message" && s.data.id === "__streaming__"
								? { type: "message" as const, data: { id: snapshotId, role: "assistant" as const, content: snapshotContent, created_at: new Date().toISOString() } }
								: s
						);
					}
					const item = toolActivityToStreamItem(msg);
					if (item) {
						// If this is a tool_output and we have a live-streamed output,
						// promote the live output (keep full content) and skip the truncated summary
						if (item.type === "activity" && item.kind === "output") {
							const liveIdx = stream.findLastIndex(
								(s) => s.type === "activity" && s.kind === "output" && s.id.startsWith("__live_")
							);
							if (liveIdx >= 0) {
								const live = stream[liveIdx] as typeof item;
								live.id = item.id;
								stream = stream;
							} else {
								stream = [...stream, item];
							}
						} else {
							stream = [...stream, item];
						}
					}
					scrollToBottom();
				} else {
					if (msg.role === "assistant") { play("message_receive"); hapticMedium(); }
					addMessage(msg);
					refreshChatList();
				}
			} else if (event.type === "mood_updated") {
				play("mood_shift");
				hapticDouble();
				mood = event.mood;
				localStorage.setItem("mood:" + slug, event.mood);
				// Activity is added via ChatMessageCreated "[system] mood → ..."
			} else if (event.type === "agent_running") {
				agentRunning = true;
				pushActivity("state", "thinking...");
			} else if (event.type === "agent_stopped") {
				agentRunning = false;
				sending = false;
				streamingContent = "";
				displayedLength = 0;
				if (typewriterRaf) { cancelAnimationFrame(typewriterRaf); typewriterRaf = 0; }
				// Clean up any promoted streaming messages stuck in wrong position
				stream = stream.filter((s) =>
					!(s.type === "message" && typeof s.data.id === "string" && s.data.id.startsWith("__promoted_"))
				);
			} else if (event.type === "tool_activity") {
				if (event.summary.startsWith("mood →")) return;
				const isOutput = event.tool_name.endsWith("_output");
				pushActivity(isOutput ? "output" : "tool", event.summary);
			} else if (event.type === "drop_created") {
				pushActivity("tool", `dropped: ${event.drop.title}`);
				play("drop_received");
				hapticDouble();
			} else if (event.type === "tool_output_chunk") {
				// Append chunk to live output activity, or create one
				const liveIdx = stream.findLastIndex(
					(s) => s.type === "activity" && s.kind === "output" && s.id.startsWith("__live_")
				);
				if (liveIdx >= 0) {
					const item = stream[liveIdx] as StreamItem & { type: "activity" };
					item.label += event.chunk;
					stream = stream;
				} else {
					pushActivity("output", event.chunk, "__live_");
				}
				scrollToBottom();
			} else if (event.type === "chat_stream_delta") {
				if (!streamingContent) lastTypeSoundChar = 0;
				streamingContent += event.delta;
				startTypewriter();
			} else if (event.type === "mcp_app_start") {
				// MCP App tool call starting — show iframe immediately
				stream = [...stream, {
					type: "mcp_app",
					id: `mcp_live_${Date.now()}`,
					toolName: event.tool_name,
					toolInput: "",
					toolOutput: "",
					html: event.html,
				}];
				scrollToBottom();
			} else if (event.type === "mcp_app_input_delta") {
				// Append JSON delta to the live MCP App stream item
				const liveIdx = stream.findLastIndex((s) => s.type === "mcp_app" && s.id.startsWith("mcp_live_"));
				if (liveIdx >= 0) {
					const item = stream[liveIdx] as StreamItem & { type: "mcp_app" };
					item.toolInput += event.delta;
					stream = stream; // trigger reactivity
				}
			} else if (event.type === "mcp_app_result") {
				// Tool result arrived — update the matching mcp_app stream item
				// First try live item, then by message_id
				let idx = stream.findLastIndex((s) => s.type === "mcp_app" && s.id.startsWith("mcp_live_"));
				if (idx < 0) idx = stream.findIndex((s) => s.type === "mcp_app" && s.id === event.message_id);
				if (idx >= 0) {
					const item = stream[idx] as StreamItem & { type: "mcp_app" };
					item.toolOutput = event.tool_output;
					item.id = event.message_id; // promote to persisted id
					stream = stream; // trigger reactivity
				}
			} else if (event.type === "context_compacting") {
				stream = [...stream, {
					type: "compaction",
					id: `compact_${Date.now()}`,
					count: event.messages_compacted,
					timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
				}];
				scrollToBottom();
			}
		});
		return unsub;
	});

	let uploadProgress = $state<{ fileIndex: number; fileCount: number; loaded: number; total: number } | null>(null);

	async function handleSend(content: string, files?: File[]) {
		sending = true;
		try {
			// Upload files first, then reference them in the message
			let finalContent = content;
			if (files && files.length > 0) {
				const uploadResults = [];
				for (let i = 0; i < files.length; i++) {
					uploadProgress = { fileIndex: i, fileCount: files.length, loaded: 0, total: files[i].size };
					uploadResults.push(await uploadFile(slug, files[i], (loaded, total) => {
						uploadProgress = { fileIndex: i, fileCount: files.length, loaded, total };
					}));
				}
				uploadProgress = null;
				const refs = uploadResults
					.map((u) => `[attached: ${u.original_name} (${u.id})]`)
					.join("\n");
				finalContent = finalContent ? `${finalContent}\n\n${refs}` : refs;
			}
			const res = await sendMessage(slug, finalContent, activeChatId);
			for (const msg of res.messages) addMessage(msg);
		} catch (e) {
			play("error");
			hapticError();
			sending = false;
			const msg = e instanceof Error ? e.message : "failed to send";
			if (msg.includes("rate limit")) {
				try {
					const parsed = JSON.parse(msg);
					toast.error(parsed.detail ?? "rate limited — try again later");
				} catch {
					toast.error("rate limited — try again later");
				}
			} else {
				toast.error(msg);
			}
		}
	}

	async function handleStop() {
		await stopAgent(slug, activeChatId);
	}

	async function handleClear() {
		clearDialogOpen = false;
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
			{#if sending || agentRunning}
				<span class="bar-activity">
					<span class="bar-activity-dot"></span>
					working
				</span>
			{/if}
		</div>
		<div class="bar-right">
			<button onclick={() => { showToolActivity = !showToolActivity; localStorage.setItem("bolly:showToolActivity", String(showToolActivity)); }} onmousedown={(e) => e.preventDefault()} class="bar-btn" class:bar-btn-active={showToolActivity} title="Toggle tool activity">
				<TerminalSquare size={12} />
			</button>
			<button onclick={() => showContextStats = true} onmousedown={(e) => e.preventDefault()} class="bar-btn" title="Context stats">
				<BarChart3 size={13} />
			</button>
			<AlertDialog.Root bind:open={clearDialogOpen}>
				<AlertDialog.Trigger class="bar-btn" title="Clear context">
					<Eraser size={13} />
				</AlertDialog.Trigger>
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
			</AlertDialog.Root>
		</div>
	</header>

	<!-- TODO: re-enable multi-chat when ready -->
	<!-- {#if showChatList}
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
	{/if} -->

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
								<MessageBubble message={item.data} {slug} index={i} prevMessage={getPrev(item, i)} streaming={item.data.id === "__streaming__"} />
							{:else if item.type === "mcp_app"}
								<McpAppViewer
									html={item.html}
									toolName={item.toolName}
									toolInput={item.toolInput}
									toolOutput={item.toolOutput}
								/>
							{:else if item.type === "compaction"}
								<div class="compaction-notice">
									<Minimize2 size={13} class="compaction-icon" />
									<span class="compaction-text">context compacted</span>
									<span class="compaction-time">{item.timestamp}</span>
								</div>
							{:else if showToolActivity}
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

			<ChatInput onSend={handleSend} onStop={handleStop} disabled={sending || agentRunning} {agentRunning} {uploadProgress} />
		</div>

		<aside class="chat-sidebar">
			<div class="sidebar-banners">
				<HeartbeatUpdateBanner {slug} />
				{#if needsGoogleReconnect}
					<CreatureBubble ondismiss={() => needsGoogleReconnect = false}>
						Google Drive access updated — please reconnect your account in
						<a href="/{slug}/settings">settings</a>.
					</CreatureBubble>
				{/if}
			</div>
			<div class="chat-creature">
				<AsciiRenderer thinking={sending || agentRunning} {mood} />
			</div>
		</aside>
	</div>
</div>

{#if showContextStats}
	<ContextStats {slug} chatId={activeChatId} onclose={() => showContextStats = false} />
{/if}

<style>
	.chat-space {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		width: 100%;
		max-width: 100%;
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
		padding: 0.5rem 1.25rem;
		flex-shrink: 0;
	}

	.bar-left {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.03em;
	}

	.bar-right {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}

	.bar-led {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: oklch(0.35 0.01 280 / 30%);
		transition: all 0.4s ease;
	}

	.bar-led-on {
		background: oklch(0.70 0.14 145 / 80%);
		box-shadow: 0 0 6px oklch(0.70 0.14 145 / 20%);
	}

	.bar-name {
		color: oklch(0.82 0.03 75 / 70%);
	}

	.bar-activity {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-family: var(--font-mono);
		font-size: 0.58rem;
		letter-spacing: 0.06em;
		color: oklch(0.68 0.08 75 / 50%);
		animation: fade-up 0.3s ease both;
	}

	.bar-activity-dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 60%);
		animation: pulse-alive 2.5s ease-in-out infinite;
	}

	.bar-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		color: oklch(0.50 0.02 280 / 30%);
		border-radius: 6px;
		transition: all 0.2s ease;
	}

	.bar-btn-active {
		color: oklch(0.72 0.08 75 / 55%);
	}

	.bar-btn:hover {
		color: oklch(0.78 0.08 75 / 65%);
		background: oklch(1 0 0 / 4%);
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
		min-width: 0;
		display: grid;
		grid-template-columns: 1fr 1fr;
	}

	.chat-main {
		display: flex;
		flex-direction: column;
		min-height: 0;
		min-width: 0;
		border-right: 1px solid oklch(0.78 0.12 75 / 5%);
	}

	.chat-sidebar {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 1rem;
		gap: 1rem;
		overflow: hidden;
		position: relative;
	}

	.sidebar-banners {
		width: 100%;
		max-width: 220px;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		z-index: 2;
	}

	.chat-creature {
		display: flex;
		align-items: center;
		justify-content: center;
		transform: scale(2.2);
		opacity: 0.5;
		pointer-events: none;
		margin-top: 2rem;
	}

	/* --- stream --- */

	.chat-stream {
		flex: 1;
		min-height: 0;
		min-width: 0;
		overflow-y: auto;
		overflow-x: hidden;
	}

	.stream-inner {
		max-width: 640px;
		width: 100%;
		margin: 0 auto;
		padding: 0.75rem 1.25rem 2rem;
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
		box-sizing: border-box;
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

	/* --- compaction notice --- */

	.compaction-notice {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		margin: 0.4rem 0;
		border-radius: 0.5rem;
		background: oklch(0.55 0.08 280 / 5%);
		border: 1px dashed oklch(0.55 0.08 280 / 15%);
		animation: act-in 0.35s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.compaction-icon {
		color: oklch(0.60 0.10 280 / 50%);
		flex-shrink: 0;
	}

	.compaction-text {
		font-family: var(--font-mono);
		font-size: 0.65rem;
		letter-spacing: 0.03em;
		color: oklch(0.60 0.06 280 / 55%);
		flex: 1;
	}

	.compaction-time {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.50 0.01 280 / 35%);
		white-space: nowrap;
	}

	/* --- responsive --- */

	@media (max-width: 900px) {
		.chat-columns {
			grid-template-columns: 1fr;
		}
		.chat-sidebar {
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

	:global(.clear-dialog) {
		background: oklch(0.12 0.01 280) !important;
		border: 1px solid oklch(0.78 0.12 75 / 10%) !important;
		border-radius: 12px !important;
		padding: 1.5rem !important;
		box-shadow: 0 16px 64px oklch(0 0 0 / 50%) !important;
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
