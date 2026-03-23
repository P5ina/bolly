<script lang="ts">
	import type { ChatMessage } from "$lib/api/types.js";
	import { uploadFileUrl } from "$lib/api/client.js";
	import { Marked } from "marked";

	let {
		message,
		slug = "",
		index = 0,
		prevMessage,
		nextMessage,
		mood = "calm",
		active = false,
		speaking = false,
		revealProgress = 1,
		streaming = false,
	}: {
		message: ChatMessage;
		slug?: string;
		index?: number;
		prevMessage?: ChatMessage;
		nextMessage?: ChatMessage;
		mood?: string;
		active?: boolean;
		speaking?: boolean;
		revealProgress?: number;
		streaming?: boolean;
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
		return gap < 60_000;
	});

	const isLastInGroup = $derived(() => {
		if (!nextMessage) return true;
		if (nextMessage.role !== message.role) return true;
		const gap = Math.abs(Number(nextMessage.created_at) - Number(message.created_at));
		return gap >= 60_000;
	});

	const marked = new Marked({
		breaks: true,
		gfm: true,
	});

	interface Attachment {
		name: string;
		id: string;
		isImage: boolean;
		url: string;
	}

	const ATTACH_RE = /\[attached:\s*(.+?)\s*\((\w+)\)\]/g;
	const IMAGE_EXTS = ["jpg", "jpeg", "png", "gif", "webp", "svg"];

	const attachments = $derived.by(() => {
		const results: Attachment[] = [];
		if (!slug) return results;
		for (const match of message.content.matchAll(ATTACH_RE)) {
			const name = match[1];
			const id = match[2];
			const ext = name.split(".").pop()?.toLowerCase() ?? "";
			results.push({
				name,
				id,
				isImage: IMAGE_EXTS.includes(ext),
				url: uploadFileUrl(slug, id),
			});
		}
		return results;
	});

	const textContent = $derived(
		message.content.replace(ATTACH_RE, "").trim()
	);

	const html = $derived(
		isUser || streaming ? textContent : (marked.parse(textContent) as string)
	);

	/** Words for voice reveal (only used when speaking). */
	const words = $derived(textContent.split(/(\s+)/));
	const revealCount = $derived(
		speaking ? Math.ceil(revealProgress * words.filter(w => w.trim()).length) : words.length
	);

	const modelLabel = $derived.by(() => {
		if (!message.model) return "";
		const m = message.model.toLowerCase();
		if (m.includes("haiku")) return "fast";
		if (m.includes("sonnet")) return "heavy";
		if (m.includes("opus")) return "heavy";
		if (m.includes("mini")) return "fast";
		if (m.includes("gpt-5.2")) return "fast";
		if (m.includes("flash")) return "fast";
		return "";
	});
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
		{#if textContent}
			<div class="msg-content msg-content-user">
				{textContent}
			</div>
		{/if}
		{#if attachments.length > 0}
			<div class="msg-attachments" class:msg-attachments-right={isUser}>
				{#each attachments as a (a.id)}
					{#if a.isImage}
						<a href={a.url} target="_blank" class="msg-img-link">
							<img src={a.url} alt={a.name} class="msg-img" loading="lazy" />
						</a>
					{:else}
						<a href={a.url} target="_blank" class="msg-file-link" download={a.name}>
							<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" class="w-3 h-3">
								<path d="M4 1h5.5L13 4.5V14a1 1 0 01-1 1H4a1 1 0 01-1-1V2a1 1 0 011-1z" stroke-linejoin="round"/>
								<path d="M9 1v4h4" stroke-linejoin="round"/>
							</svg>
							<span>{a.name}</span>
						</a>
					{/if}
				{/each}
			</div>
		{/if}
	{:else}
		<div class="msg-companion-wrap">
			{#if !isConsecutive()}
				<div class="msg-presence-line">
					<span class="msg-presence-dot"></span>
				</div>
			{/if}
			{#if textContent}
				{#if speaking}
					<div class="msg-content msg-content-companion msg-voice-reveal">
						{#each words as word, wi}
							{#if word.trim()}
								{@const wordIdx = words.slice(0, wi + 1).filter(w => w.trim()).length}
								<span class="voice-word" class:voice-word-visible={wordIdx <= revealCount}>{word}</span>
							{:else}
								{@html word.includes("\n") ? "<br>" : " "}
							{/if}
						{/each}
					</div>
				{:else}
					<div class="msg-content msg-content-companion prose" class:msg-streaming={streaming}>
						{@html html}
					</div>
				{/if}
			{/if}
			{#if attachments.length > 0}
				<div class="msg-attachments">
					{#each attachments as a (a.id)}
						{#if a.isImage}
							<a href={a.url} target="_blank" class="msg-img-link">
								<img src={a.url} alt={a.name} class="msg-img" loading="lazy" />
							</a>
						{:else}
							<a href={a.url} target="_blank" class="msg-file-link" download={a.name}>
								<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" class="w-3 h-3">
									<path d="M4 1h5.5L13 4.5V14a1 1 0 01-1 1H4a1 1 0 01-1-1V2a1 1 0 011-1z" stroke-linejoin="round"/>
									<path d="M9 1v4h4" stroke-linejoin="round"/>
								</svg>
								<span>{a.name}</span>
							</a>
						{/if}
					{/each}
				</div>
			{/if}
		</div>
	{/if}
	{#if isLastInGroup()}
		<span class="msg-time" class:msg-time-right={!isUser}>
			{time()}
			{#if modelLabel && !isUser}
				<span class="msg-model" class:msg-model-fast={modelLabel === "fast"}>{modelLabel}</span>
			{/if}
		</span>
	{/if}
</div>

<style>
	.msg {
		padding: 0.5rem 0;
		animation: msg-enter 0.55s cubic-bezier(0.16, 1, 0.3, 1) both;
		--msg-accent: oklch(0.6 0.1 190);
		--msg-glass-bg: oklch(0.13 0.025 200 / 30%);
		--msg-glass-border: oklch(0.5 0.06 200 / 10%);
	}

	/* mood accents — teal-metallic spectrum */
	.msg[data-mood="focused"] { --msg-accent: oklch(0.68 0.1 180); }
	.msg[data-mood="playful"] { --msg-accent: oklch(0.72 0.12 160); }
	.msg[data-mood="loving"] { --msg-accent: oklch(0.7 0.1 20); --msg-glass-bg: oklch(0.13 0.02 10 / 25%); }
	.msg[data-mood="warm"] { --msg-accent: oklch(0.72 0.1 65); --msg-glass-bg: oklch(0.13 0.02 55 / 25%); }
	.msg[data-mood="reflective"] { --msg-accent: oklch(0.6 0.08 280); --msg-glass-bg: oklch(0.13 0.02 270 / 25%); }
	.msg[data-mood="excited"] { --msg-accent: oklch(0.75 0.12 85); }
	.msg[data-mood="curious"] { --msg-accent: oklch(0.7 0.1 200); }
	.msg[data-mood="creative"] { --msg-accent: oklch(0.72 0.12 155); }
	.msg[data-mood="melancholy"] { --msg-accent: oklch(0.5 0.06 250); --msg-glass-bg: oklch(0.11 0.02 250 / 25%); }
	.msg[data-mood="sad"] { --msg-accent: oklch(0.45 0.05 245); }
	.msg[data-mood="anxious"] { --msg-accent: oklch(0.6 0.1 30); }
	.msg[data-mood="energetic"] { --msg-accent: oklch(0.75 0.14 100); }
	.msg[data-mood="peaceful"] { --msg-accent: oklch(0.65 0.08 170); }
	.msg[data-mood="tired"] { --msg-accent: oklch(0.45 0.03 260); }

	.msg-consecutive {
		padding: 0.125rem 0;
	}

	@keyframes msg-enter {
		from {
			opacity: 0;
			transform: translateY(10px) scale(0.97);
			filter: blur(4px);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
			filter: blur(0);
		}
	}

	/* --- Glass card base --- */

	.msg-content {
		font-size: 0.875rem;
		line-height: 1.7;
		letter-spacing: 0.005em;
		max-width: 85%;
		word-break: break-word;
		overflow-wrap: anywhere;
		overflow-x: hidden;
	}

	.msg-content-user {
		white-space: pre-wrap;
	}

	/* User messages: right-aligned, subdued glass */
	.msg-user {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		padding-right: 0.25rem;
	}

	.msg-content-user {
		position: relative;
		color: oklch(0.72 0.025 220 / 60%);
		font-family: var(--font-body);
		font-size: 0.8125rem;
		padding: 0.55rem 0.9rem;
		background: linear-gradient(
			160deg,
			oklch(1 0 0 / 6%) 0%,
			oklch(0.5 0.02 220 / 8%) 40%,
			oklch(1 0 0 / 3%) 100%
		);
		backdrop-filter: blur(20px) saturate(150%) brightness(1.05);
		-webkit-backdrop-filter: blur(20px) saturate(150%) brightness(1.05);
		border: 1px solid oklch(1 0 0 / 8%);
		border-top-color: oklch(1 0 0 / 15%);
		border-radius: 14px 14px 4px 14px;
		max-width: 65%;
		box-shadow:
			0 2px 8px oklch(0 0 0 / 12%),
			inset 0 1px 0 oklch(1 0 0 / 8%),
			inset 0 -1px 0 oklch(0 0 0 / 5%);
		overflow: hidden;
	}

	/* Specular highlight sweep */
	.msg-content-user::before {
		content: "";
		position: absolute;
		top: 0;
		left: 8%;
		right: 8%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 25%), transparent);
		pointer-events: none;
	}

	/* Companion wrap: left-aligned */
	.msg-companion-wrap {
		display: flex;
		flex-direction: column;
		gap: 0.28rem;
		align-items: flex-start;
	}

	.msg-presence-line {
		display: inline-flex;
		align-items: center;
		gap: 0.38rem;
		font-family: var(--font-mono);
		font-size: 0.6rem;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: oklch(0.55 0.05 200 / 40%);
		padding-right: 0.5rem;
	}

	.msg-presence-dot {
		width: 5px;
		height: 5px;
		border-radius: 999px;
		background: var(--msg-accent);
		box-shadow: 0 0 10px var(--msg-accent), 0 0 20px oklch(from var(--msg-accent) l c h / 25%);
		animation: presence-beacon 3s ease-in-out infinite;
	}

	/* Companion liquid glass card */
	.msg-content-companion {
		position: relative;
		color: oklch(0.9 0.025 75 / 92%);
		font-family: var(--font-body);
		padding: 0.7rem 1rem;
		background: linear-gradient(
			145deg,
			oklch(1 0 0 / 7%) 0%,
			var(--msg-glass-bg) 35%,
			oklch(1 0 0 / 4%) 70%,
			oklch(0.5 0.03 200 / 10%) 100%
		);
		backdrop-filter: blur(24px) saturate(170%) brightness(1.08);
		-webkit-backdrop-filter: blur(24px) saturate(170%) brightness(1.08);
		border: 1px solid oklch(1 0 0 / 10%);
		border-top-color: oklch(1 0 0 / 20%);
		border-bottom-color: oklch(0 0 0 / 8%);
		border-radius: 16px 16px 4px 16px;
		max-width: 80%;
		box-shadow:
			0 2px 16px oklch(0 0 0 / 20%),
			0 8px 32px oklch(0 0 0 / 8%),
			inset 0 1px 0 oklch(1 0 0 / 10%),
			inset 0 -1px 0 oklch(0 0 0 / 6%);
		transition: border-color 0.4s ease, box-shadow 0.4s ease;
		overflow: hidden;
	}

	/* Specular highlight — bright line along top edge */
	.msg-content-companion::before {
		content: "";
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 35%), oklch(1 0 0 / 15%), transparent);
		pointer-events: none;
	}

	/* Secondary inner refraction glow */
	.msg-content-companion::after {
		content: "";
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 50%;
		background: linear-gradient(
			180deg,
			oklch(1 0 0 / 4%) 0%,
			transparent 100%
		);
		pointer-events: none;
		border-radius: 16px 16px 0 0;
	}

	.msg-active .msg-content-companion {
		border-color: oklch(1 0 0 / 15%);
		border-top-color: oklch(1 0 0 / 25%);
		box-shadow:
			0 4px 24px oklch(0 0 0 / 25%),
			0 0 30px oklch(0.6 0.08 200 / 8%),
			inset 0 1px 0 oklch(1 0 0 / 12%),
			inset 0 -1px 0 oklch(0 0 0 / 6%);
	}

	/* --- Prose (markdown) --- */

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
		color: oklch(0.7 0.1 190);
		text-decoration: underline;
		text-decoration-color: oklch(0.7 0.1 190 / 30%);
		text-underline-offset: 2px;
		transition: text-decoration-color 0.2s;
	}
	.prose :global(a:hover) {
		text-decoration-color: oklch(0.7 0.1 190 / 70%);
	}
	.prose :global(code) {
		font-family: var(--font-mono);
		font-size: 0.8em;
		background: oklch(0.1 0.015 200 / 50%);
		padding: 0.15em 0.35em;
		border-radius: 4px;
		color: oklch(0.78 0.06 190);
		border: 1px solid oklch(0.5 0.04 200 / 10%);
	}
	.prose :global(pre) {
		background: oklch(0.07 0.015 210 / 60%);
		backdrop-filter: blur(8px);
		-webkit-backdrop-filter: blur(8px);
		border: 1px solid oklch(0.4 0.04 200 / 12%);
		border-radius: 10px;
		padding: 0.75em 1em;
		margin: 0.5em 0;
		overflow-x: auto;
		max-width: 100%;
	}
	.prose :global(pre code) {
		background: none;
		padding: 0;
		font-size: 0.78em;
		color: oklch(0.80 0.02 75 / 85%);
		line-height: 1.5;
		border: none;
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
		color: oklch(0.6 0.08 200 / 40%);
	}
	.prose :global(blockquote) {
		border-left: 2px solid oklch(0.6 0.08 200 / 30%);
		padding-left: 0.75em;
		margin: 0.4em 0;
		color: oklch(0.80 0.03 75 / 70%);
		font-style: italic;
	}
	.prose :global(hr) {
		border: none;
		border-top: 1px solid oklch(0.5 0.06 200 / 12%);
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
		border: 1px solid oklch(0.4 0.04 200 / 15%);
		padding: 0.35em 0.6em;
		text-align: left;
	}
	.prose :global(th) {
		background: oklch(0.1 0.02 200 / 35%);
		color: oklch(0.90 0.04 75);
		font-weight: 600;
	}


	/* --- Timestamp --- */
	.msg-time {
		display: block;
		font-size: 0.5625rem;
		color: oklch(0.5 0.04 200 / 15%);
		margin-top: 0.2rem;
		font-family: var(--font-mono);
		letter-spacing: 0.04em;
		transition: color 0.3s ease;
	}
	.msg:hover .msg-time {
		color: oklch(0.6 0.06 200 / 40%);
	}
	.msg-time-right {
		text-align: left;
	}

	/* --- Attachments --- */
	.msg-attachments {
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
		margin-top: 0.25rem;
		max-width: 85%;
	}
	.msg-attachments-right {
		justify-content: flex-start;
	}

	.msg-img-link {
		display: block;
		border-radius: 10px;
		overflow: hidden;
		transition: opacity 0.2s ease;
	}
	.msg-img-link:hover {
		opacity: 0.85;
	}

	.msg-img {
		max-width: 280px;
		max-height: 200px;
		border-radius: 10px;
		object-fit: cover;
		border: 1px solid oklch(0.5 0.06 200 / 12%);
	}

	.msg-file-link {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.3rem 0.6rem;
		border-radius: 8px;
		background: oklch(0.14 0.02 200 / 25%);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border: 1px solid oklch(0.5 0.06 200 / 10%);
		color: oklch(0.65 0.08 200 / 60%);
		font-family: var(--font-mono);
		font-size: 0.75rem;
		text-decoration: none;
		transition: all 0.2s ease;
	}
	.msg-file-link:hover {
		background: oklch(0.14 0.02 200 / 40%);
		color: oklch(0.75 0.1 200 / 85%);
		border-color: oklch(0.5 0.06 200 / 18%);
	}

	.msg-model {
		display: inline-block;
		margin-left: 0.3rem;
		padding: 0 0.25rem;
		border-radius: 3px;
		font-size: 0.5rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		background: oklch(0.55 0.08 200 / 12%);
		color: oklch(0.55 0.08 200 / 45%);
		vertical-align: middle;
	}
	.msg-model-fast {
		background: oklch(0.65 0.1 170 / 12%);
		color: oklch(0.65 0.1 170 / 45%);
	}
	.msg:hover .msg-model {
		color: oklch(0.55 0.08 200 / 65%);
	}
	.msg:hover .msg-model-fast {
		color: oklch(0.65 0.1 170 / 65%);
	}

	/* --- Voice word reveal --- */
	.msg-voice-reveal {
		font-family: var(--font-body);
		color: oklch(0.9 0.025 75 / 92%);
		padding: 0.7rem 1rem;
		background: linear-gradient(135deg, var(--msg-glass-bg) 0%, oklch(0.1 0.018 220 / 20%) 100%);
		backdrop-filter: blur(20px) saturate(140%);
		-webkit-backdrop-filter: blur(20px) saturate(140%);
		border: 1px solid var(--msg-glass-border);
		border-radius: 16px 16px 4px 16px;
		max-width: 80%;
		box-shadow: 0 2px 12px oklch(0 0 0 / 18%), inset 0 1px 0 oklch(1 0 0 / 3%);
	}

	.voice-word {
		opacity: 0.06;
		transition: opacity 0.18s ease;
	}

	.voice-word-visible {
		opacity: 1;
	}
</style>
