<script lang="ts">
	import type { ChatMessage } from "$lib/api/types.js";
	import { Marked } from "marked";

	let {
		message,
		index = 0,
		prevMessage,
		mood = "calm",
		active = false,
	}: {
		message: ChatMessage;
		index?: number;
		prevMessage?: ChatMessage;
		mood?: string;
		active?: boolean;
	} = $props();

	const isUser = $derived(message.role === "user");
	const time = $derived(() => {
		const ms = Number(message.created_at);
		if (Number.isNaN(ms)) return "";
		const d = new Date(ms);
		return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
	});

	const isConsecutive = $derived(() => {
		if (!prevMessage) return false;
		if (prevMessage.role !== message.role) return false;
		if (isUser) return false;
		const gap = Math.abs(Number(message.created_at) - Number(prevMessage.created_at));
		return gap < 30_000;
	});

	const marked = new Marked({
		breaks: true,
		gfm: true,
	});

	const html = $derived(isUser ? message.content : (marked.parse(message.content) as string));
</script>

<div
	class="msg"
	class:msg-user={isUser}
	class:msg-companion={!isUser}
	class:msg-consecutive={isConsecutive()}
	class:msg-active={!isUser && active}
	data-mood={mood}
	style="animation-delay: {Math.min(index * 20, 300)}ms"
>
	{#if isUser}
		<div class="msg-content msg-content-user">
			{message.content}
		</div>
	{:else}
		<div class="msg-companion-wrap">
			{#if !isConsecutive()}
				<div class="msg-presence-line">
					<span class="msg-presence-dot"></span>
					<span class="msg-presence-label">{mood}</span>
				</div>
			{/if}
			<div class="msg-content msg-content-companion prose">
				{@html html}
			</div>
		</div>
	{/if}
	{#if !isConsecutive()}
		<span class="msg-time" class:msg-time-right={isUser}>
			{time()}
		</span>
	{/if}
</div>

<style>
	.msg {
		padding: 0.375rem 0;
		animation: msg-enter 0.45s cubic-bezier(0.16, 1, 0.3, 1) both;
		--msg-accent: oklch(0.78 0.12 75 / 18%);
	}

	.msg[data-mood="focused"] { --msg-accent: oklch(0.76 0.12 170 / 18%); }
	.msg[data-mood="playful"] { --msg-accent: oklch(0.78 0.14 145 / 18%); }
	.msg[data-mood="loving"] { --msg-accent: oklch(0.8 0.12 20 / 18%); }
	.msg[data-mood="warm"] { --msg-accent: oklch(0.8 0.12 55 / 18%); }
	.msg[data-mood="reflective"] { --msg-accent: oklch(0.72 0.08 300 / 18%); }

	.msg-consecutive {
		padding: 0.0625rem 0;
	}

	.msg-active .msg-content-companion {
		border-color: var(--msg-accent);
		box-shadow: 0 0 0 1px var(--msg-accent), 0 10px 24px oklch(0.02 0.01 280 / 10%);
	}

	@keyframes msg-enter {
		from {
			opacity: 0;
			transform: translateY(5px);
			filter: blur(1.5px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
			filter: blur(0);
		}
	}

	.msg-content {
		font-size: 0.875rem;
		line-height: 1.7;
		letter-spacing: 0.005em;
		max-width: 85%;
		word-break: break-word;
	}

	.msg-content-user {
		white-space: pre-wrap;
	}

	.msg-companion-wrap {
		display: flex;
		flex-direction: column;
		gap: 0.28rem;
	}

	.msg-presence-line {
		display: inline-flex;
		align-items: center;
		gap: 0.38rem;
		font-family: var(--font-mono);
		font-size: 0.58rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.76 0.03 75 / 45%);
	}

	.msg-presence-dot {
		width: 5px;
		height: 5px;
		border-radius: 999px;
		background: var(--msg-accent);
		box-shadow: 0 0 10px var(--msg-accent);
	}

	.msg-content-companion {
		color: oklch(0.88 0.03 75 / 90%);
		font-family: var(--font-body);
		padding: 0.1rem 0.2rem 0.1rem 0;
		border-left: 1px solid transparent;
		transition: border-color 0.35s ease, box-shadow 0.35s ease;
	}

	.prose :global(p) {
		margin: 0.25em 0;
	}
	.prose :global(p:first-child) {
		margin-top: 0;
	}
	.prose :global(p:last-child) {
		margin-bottom: 0;
	}
	.prose :global(h1),
	.prose :global(h2),
	.prose :global(h3),
	.prose :global(h4) {
		font-family: var(--font-display);
		color: oklch(0.92 0.04 75 / 95%);
		margin: 0.75em 0 0.25em;
		line-height: 1.3;
		font-weight: 600;
	}
	.prose :global(h1) { font-size: 1.15em; }
	.prose :global(h2) { font-size: 1.05em; }
	.prose :global(h3) { font-size: 0.95em; }
	.prose :global(strong) {
		color: oklch(0.92 0.04 75);
		font-weight: 600;
	}
	.prose :global(em) {
		font-style: italic;
		color: oklch(0.85 0.05 75 / 80%);
	}
	.prose :global(a) {
		color: oklch(0.78 0.12 75);
		text-decoration: underline;
		text-decoration-color: oklch(0.78 0.12 75 / 30%);
		text-underline-offset: 2px;
		transition: text-decoration-color 0.2s;
	}
	.prose :global(a:hover) {
		text-decoration-color: oklch(0.78 0.12 75 / 70%);
	}
	.prose :global(code) {
		font-family: var(--font-mono);
		font-size: 0.8em;
		background: oklch(0.12 0.01 280 / 60%);
		padding: 0.15em 0.35em;
		border-radius: 4px;
		color: oklch(0.82 0.06 75);
	}
	.prose :global(pre) {
		background: oklch(0.08 0.01 280 / 80%);
		border: 1px solid oklch(0.2 0.01 280 / 30%);
		border-radius: 6px;
		padding: 0.75em 1em;
		margin: 0.5em 0;
		overflow-x: auto;
	}
	.prose :global(pre code) {
		background: none;
		padding: 0;
		font-size: 0.78em;
		color: oklch(0.80 0.02 75 / 85%);
		line-height: 1.5;
	}
	.prose :global(ul),
	.prose :global(ol) {
		margin: 0.35em 0;
		padding-left: 1.4em;
	}
	.prose :global(li) {
		margin: 0.15em 0;
	}
	.prose :global(li::marker) {
		color: oklch(0.78 0.12 75 / 35%);
	}
	.prose :global(blockquote) {
		border-left: 2px solid oklch(0.78 0.12 75 / 25%);
		padding-left: 0.75em;
		margin: 0.4em 0;
		color: oklch(0.80 0.03 75 / 70%);
		font-style: italic;
	}
	.prose :global(hr) {
		border: none;
		border-top: 1px solid oklch(0.78 0.12 75 / 12%);
		margin: 0.75em 0;
	}
	.prose :global(table) {
		border-collapse: collapse;
		margin: 0.5em 0;
		font-size: 0.82em;
		width: 100%;
	}
	.prose :global(th),
	.prose :global(td) {
		border: 1px solid oklch(0.2 0.01 280 / 30%);
		padding: 0.35em 0.6em;
		text-align: left;
	}
	.prose :global(th) {
		background: oklch(0.10 0.01 280 / 50%);
		color: oklch(0.90 0.04 75);
		font-weight: 600;
	}

	.msg-user {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
	}

	.msg-content-user {
		color: oklch(0.70 0.02 260 / 50%);
		font-family: var(--font-body);
		font-size: 0.8125rem;
		text-align: right;
	}

	.msg-time {
		font-size: 0.5625rem;
		color: oklch(0.78 0.12 75 / 12%);
		margin-top: 0.125rem;
		font-family: var(--font-mono);
		letter-spacing: 0.03em;
		transition: color 0.3s ease;
	}
	.msg:hover .msg-time {
		color: oklch(0.78 0.12 75 / 25%);
	}
	.msg-time-right {
		text-align: right;
	}
</style>
