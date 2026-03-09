<script lang="ts">
	import { sendMessage } from "$lib/api/client.js";
	import { getInstances } from "$lib/stores/instances.svelte.js";

	let { slug, oncomplete }: { slug: string; oncomplete: () => void } = $props();

	const instances = getInstances();

	type Stage =
		| "intro"
		| "picking-language"
		| "waiting-first"
		| "sending"
		| "departing";

	let stage = $state<Stage>("intro");
	let firstMessage = $state("");
	let messageInput: HTMLTextAreaElement | undefined = $state();
	let chosenLanguage = $state(localStorage.getItem("personality:language") ?? "english");
	let lines = $state<{ text: string; revealed: string; done: boolean }[]>([]);

	const LANGUAGES = [
		{ id: "english", label: "English" },
		{ id: "russian", label: "Русский" },
		{ id: "spanish", label: "Español" },
		{ id: "french", label: "Français" },
		{ id: "german", label: "Deutsch" },
		{ id: "japanese", label: "日本語" },
		{ id: "chinese", label: "中文" },
		{ id: "korean", label: "한국어" },
		{ id: "portuguese", label: "Português" },
		{ id: "italian", label: "Italiano" },
		{ id: "turkish", label: "Türkçe" },
		{ id: "arabic", label: "العربية" },
	];

	function typewrite(text: string, speed = 38): Promise<void> {
		return new Promise((resolve) => {
			const entry = { text, revealed: "", done: false };
			lines = [...lines, entry];
			const idx = lines.length - 1;
			let i = 0;

			function tick() {
				if (i >= text.length) {
					lines[idx].done = true;
					resolve();
					return;
				}
				const char = text[i];
				lines[idx].revealed += char;
				i++;

				let delay = speed;
				if (char === "." || char === "?" || char === "!") delay = speed * 8;
				else if (char === ",") delay = speed * 3;
				else if (char === "\u2014" || char === "\u2013") delay = speed * 4;

				setTimeout(tick, delay);
			}
			setTimeout(tick, speed);
		});
	}

	function pause(ms: number): Promise<void> {
		return new Promise((r) => setTimeout(r, ms));
	}

	async function runSequence() {
		await pause(400);
		await typewrite(`i\u2019m ${slug}.`);
		await pause(400);
		await typewrite("a new space, just for us.");
		await pause(600);
		await typewrite("what language should we speak?");
		stage = "picking-language";
	}

	async function pickLanguage(langId: string) {
		chosenLanguage = langId;
		localStorage.setItem("personality:language", langId);
		stage = "intro";
		await pause(300);
		const lang = LANGUAGES.find((l) => l.id === langId);
		await typewrite(`${lang?.label ?? langId}.`);
		await pause(400);
		await typewrite("tell me something.");
		stage = "waiting-first";
		await pause(100);
		messageInput?.focus();
	}

	async function submitFirst() {
		const content = firstMessage.trim();
		if (!content) return;

		stage = "sending";

		// Send language setup + user message
		const preferredName = localStorage.getItem("personality:preferredName") ?? "";
		const langLabel = LANGUAGES.find((l) => l.id === chosenLanguage)?.label ?? chosenLanguage;
		const setupParts: string[] = [];
		if (preferredName) setupParts.push(`my name is ${preferredName}`);
		setupParts.push(`please speak to me in ${langLabel}`);

		try {
			// Send setup context
			await sendMessage(slug, setupParts.join(". ") + ".");
			// Send actual first message
			await sendMessage(slug, content);
			await instances.refresh();
		} catch {
			// instance created even on failure
		}

		stage = "departing";
		await pause(400);
		oncomplete();
	}

	function handleMessageKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			submitFirst();
		}
	}

	$effect(() => {
		runSequence();
	});
</script>

<div
	class="relative flex h-full items-center justify-center overflow-hidden"
	class:instance-depart={stage === "departing"}
>
	<!-- subtle glow -->
	<div class="instance-glow"></div>

	<div class="relative z-10 w-full max-w-md px-6">
		<!-- companion initial -->
		<div class="mb-10 flex justify-center">
			<div class="flex h-16 w-16 items-center justify-center rounded-2xl bg-warm/10 font-display text-2xl font-bold text-warm instance-avatar">
				{slug[0]?.toUpperCase() ?? "?"}
			</div>
		</div>

		<!-- typewriter lines -->
		<div class="space-y-3 mb-8">
			{#each lines as line, i}
				<div class="instance-line" style="animation-delay: {i * 50}ms">
					{#if i === 0 && line.done}
						<p class="font-display text-2xl font-bold tracking-tight text-foreground text-center">
							{line.revealed}
						</p>
					{:else if i === 0}
						<p class="font-display text-2xl font-bold tracking-tight text-foreground text-center">
							{line.revealed}<span class="instance-cursor"></span>
						</p>
					{:else if !line.done}
						<p class="text-sm leading-relaxed text-muted-foreground text-center">
							{line.revealed}<span class="instance-cursor"></span>
						</p>
					{:else}
						<p class="text-sm leading-relaxed text-muted-foreground text-center">
							{line.revealed}
						</p>
					{/if}
				</div>
			{/each}
		</div>

		<!-- language picker -->
		{#if stage === "picking-language"}
			<div class="instance-input-enter">
				<div class="grid grid-cols-4 gap-2">
					{#each LANGUAGES as lang}
						<button
							onclick={() => pickLanguage(lang.id)}
							class="instance-pill text-xs"
							class:instance-pill-active={chosenLanguage === lang.id}
						>
							{lang.label}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- first message -->
		{#if stage === "waiting-first"}
			<div class="instance-input-enter">
				<div class="relative">
					<textarea
						bind:this={messageInput}
						bind:value={firstMessage}
						onkeydown={handleMessageKeydown}
						placeholder="what's on your mind?"
						rows={3}
						class="w-full resize-none rounded-xl border border-warm/20 bg-warm/5 px-5 py-3.5 text-sm leading-relaxed text-foreground placeholder:text-muted-foreground/25 outline-none transition-all duration-300 focus:border-warm/40 focus:shadow-[0_0_30px_-5px] focus:shadow-warm/15"
					></textarea>
					{#if firstMessage.trim()}
						<button
							onclick={submitFirst}
							aria-label="Send"
							class="absolute right-2 bottom-2 flex h-8 w-8 items-center justify-center rounded-lg bg-warm text-warm-foreground transition-all hover:bg-warm/90"
						>
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- sending -->
		{#if stage === "sending"}
			<div class="instance-input-enter flex items-center justify-center gap-2.5 text-sm text-warm/60">
				<div class="instance-spinner"></div>
				<span class="font-mono text-xs">setting up</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.instance-glow {
		position: absolute;
		top: 35%;
		left: 50%;
		width: 400px;
		height: 400px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 5%) 0%,
			oklch(0.78 0.12 75 / 2%) 30%,
			transparent 70%
		);
		animation: glow-breathe 6s ease-in-out infinite;
		pointer-events: none;
	}
	@keyframes glow-breathe {
		0%, 100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
		50% { opacity: 0.5; transform: translate(-50%, -50%) scale(1.05); }
	}

	.instance-avatar {
		animation: avatar-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	@keyframes avatar-enter {
		from { opacity: 0; transform: scale(0.8) translateY(10px); }
		to { opacity: 1; transform: scale(1) translateY(0); }
	}

	.instance-line {
		animation: line-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	@keyframes line-enter {
		from { opacity: 0; transform: translateY(8px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.instance-input-enter {
		animation: input-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 100ms;
	}
	@keyframes input-enter {
		from { opacity: 0; transform: translateY(12px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.instance-cursor {
		display: inline-block;
		width: 2px;
		height: 1.1em;
		margin-left: 1px;
		vertical-align: text-bottom;
		background: oklch(0.78 0.12 75 / 70%);
		animation: cursor-blink 0.8s steps(2) infinite;
	}
	@keyframes cursor-blink {
		0% { opacity: 1; }
		100% { opacity: 0; }
	}

	.instance-depart {
		animation: depart 0.4s cubic-bezier(0.55, 0, 1, 0.45) forwards;
	}
	@keyframes depart {
		to { opacity: 0; transform: scale(0.98); }
	}

	.instance-pill {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 0.75rem;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		background: oklch(0.78 0.12 75 / 4%);
		padding: 0.5rem 0.5rem;
		color: var(--foreground);
		transition: all 0.3s ease;
		cursor: pointer;
	}
	.instance-pill:hover {
		border-color: oklch(0.78 0.12 75 / 35%);
		background: oklch(0.78 0.12 75 / 10%);
		box-shadow: 0 0 20px -5px oklch(0.78 0.12 75 / 10%);
	}
	.instance-pill-active {
		border-color: oklch(0.78 0.12 75 / 50%);
		background: oklch(0.78 0.12 75 / 15%);
	}

	.instance-spinner {
		width: 14px;
		height: 14px;
		border: 2px solid oklch(0.78 0.12 75 / 20%);
		border-top-color: oklch(0.78 0.12 75 / 70%);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}
</style>
