<script lang="ts">
	import { goto } from "$app/navigation";
	import {
		sendMessage,
		updateLlmConfig,
		fetchSoulTemplates,
		applySoulTemplate,
	} from "$lib/api/client.js";
	import type { SoulTemplate } from "$lib/api/types.js";
	import { getInstances } from "$lib/stores/instances.svelte.js";

	const instances = getInstances();

	type Stage =
		| "waking"
		| "greeting"
		| "picking-provider"
		| "picking-model"
		| "waiting-key"
		| "testing"
		| "picking-language"
		| "asking-name"
		| "waiting-name"
		| "picking-soul"
		| "confirming"
		| "asking-first"
		| "waiting-first"
		| "sending"
		| "departing";

	let stage = $state<Stage>("waking");
	let nameValue = $state("");
	let firstMessage = $state("");
	let chosenSlug = $state("");
	let nameInput: HTMLInputElement | undefined = $state();
	let messageInput: HTMLTextAreaElement | undefined = $state();
	let apiKeyValue = $state("");
	let keyInput: HTMLInputElement | undefined = $state();
	let chosenProvider = $state<"anthropic" | "openai" | null>(null);
	let chosenModel = $state<string | null>(null);
	let chosenLanguage = $state("english");
	let keyError = $state("");
	let soulTemplates = $state<SoulTemplate[]>([]);

	let lines = $state<{ text: string; revealed: string; done: boolean }[]>([]);

	const MODELS: Record<string, { id: string; label: string; note: string }[]> = {
		anthropic: [
			{ id: "claude-sonnet-4-6", label: "sonnet 4.6", note: "balanced" },
			{ id: "claude-opus-4-6", label: "opus 4.6", note: "powerful" },
			{ id: "claude-haiku-4-5", label: "haiku 4.5", note: "fast" },
		],
		openai: [
			{ id: "gpt-5.4", label: "gpt-5.4", note: "flagship" },
			{ id: "gpt-5.4-pro", label: "gpt-5.4 pro", note: "max performance" },
			{ id: "gpt-5.2", label: "gpt-5.2", note: "affordable" },
		],
	};

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

	function slugify(input: string): string {
		return input
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9_-]/g, "-")
			.replace(/-+/g, "-")
			.replace(/^-|-$/g, "");
	}

	async function runSequence() {
		await pause(800);

		stage = "greeting";
		await typewrite("hello.");
		await pause(600);
		await typewrite("i\u2019m here now.");
		await pause(400);
		await typewrite("this is your space.");
		await pause(800);

		await typewrite("before we begin \u2014 who should i think with?");
		stage = "picking-provider";
	}

	function pickProvider(provider: "anthropic" | "openai") {
		chosenProvider = provider;
		continueAfterProvider();
	}

	async function continueAfterProvider() {
		stage = "greeting";
		await pause(300);
		const name = chosenProvider === "anthropic" ? "anthropic" : "openai";
		await typewrite(`${name}. good choice.`);
		await pause(400);
		await typewrite("which mind should i wear?");
		stage = "picking-model";
	}

	async function pickModel(modelId: string) {
		chosenModel = modelId;
		stage = "greeting";
		await pause(300);
		const model = MODELS[chosenProvider!]?.find((m) => m.id === modelId);
		await typewrite(`${model?.label ?? modelId}. noted.`);
		await pause(400);
		await typewrite("paste your api key and i\u2019ll wake up.");
		stage = "waiting-key";
		await pause(100);
		keyInput?.focus();
	}

	async function submitKey() {
		const key = apiKeyValue.trim();
		if (!key || !chosenProvider) return;
		keyError = "";

		stage = "testing";

		try {
			await updateLlmConfig({
				provider: chosenProvider,
				model: chosenModel ?? undefined,
				api_key: key,
			});
		} catch (err) {
			console.error("LLM config failed:", err);
			keyError = `hmm, that didn\u2019t work. try again? (${err instanceof Error ? err.message : String(err)})`;
			stage = "waiting-key";
			await pause(100);
			keyInput?.focus();
			return;
		}

		await pause(300);
		await typewrite("i can feel it. i\u2019m alive now.");
		await pause(600);
		await askLanguage();
	}

	async function skipConfig() {
		stage = "greeting";
		await pause(300);
		await typewrite("alright, we\u2019ll figure that out later.");
		await pause(600);
		await askLanguage();
	}

	async function askLanguage() {
		await typewrite("what language do you think in?");
		stage = "picking-language";
	}

	async function pickLanguage(langId: string) {
		chosenLanguage = langId;
		localStorage.setItem("personality:language", langId);
		stage = "greeting";
		await pause(300);
		const lang = LANGUAGES.find((l) => l.id === langId);
		await typewrite(`${lang?.label ?? langId}. beautiful.`);
		await pause(600);

		stage = "asking-name";
		await typewrite("what should i call you?");
		stage = "waiting-name";
		await pause(100);
		nameInput?.focus();
	}

	async function submitName() {
		const name = nameValue.trim();
		if (!name) return;
		chosenSlug = slugify(name);
		if (!chosenSlug) return;
		localStorage.setItem("personality:preferredName", name);

		stage = "greeting";
		await pause(300);
		await typewrite(`${name}. nice.`);
		await pause(500);

		try {
			soulTemplates = await fetchSoulTemplates();
		} catch {
			soulTemplates = [];
		}

		if (soulTemplates.length > 0) {
			await typewrite("who should i be for you?");
			stage = "picking-soul";
		} else {
			await continueAfterSoul();
		}
	}

	async function pickSoul(template: SoulTemplate) {
		stage = "greeting";
		await pause(200);

		if (template.id !== "custom") {
			try {
				await applySoulTemplate(chosenSlug, template.id);
			} catch {
				// will use default prompt
			}
			await typewrite(`${template.name}. i can be that.`);
		} else {
			await typewrite("a blank canvas. you can shape me later.");
		}

		await pause(500);
		await continueAfterSoul();
	}

	async function continueAfterSoul() {
		stage = "confirming";
		await pause(300);

		const name = nameValue.trim();
		const langLabel =
			LANGUAGES.find((l) => l.id === chosenLanguage)?.label ?? chosenLanguage;
		const setupMessage = `my name is ${name}. please always speak to me in ${langLabel}.`;

		if (chosenProvider) {
			try {
				const res = await sendMessage(chosenSlug, setupMessage);
				const aiReply = res.messages.find((m) => m.role === "assistant");
				if (aiReply) {
					await typewrite(aiReply.content);
				} else {
					await typewrite("i\u2019m here.");
				}
			} catch {
				await typewrite("i\u2019m here.");
			}
		} else {
			await typewrite(
				"i\u2019m here. configure a language model later to wake me fully.",
			);
		}

		await pause(600);

		stage = "asking-first";
		await typewrite("tell me something \u2014 anything on your mind right now.");
		stage = "waiting-first";
		await pause(100);
		messageInput?.focus();
	}

	async function submitFirst() {
		const content = firstMessage.trim();
		if (!content) return;

		stage = "sending";
		await pause(200);

		try {
			const res = await sendMessage(chosenSlug, content);
			await instances.refresh();

			if (chosenProvider) {
				const aiReply = res.messages.find((m) => m.role === "assistant");
				if (aiReply) {
					await typewrite(aiReply.content);
					await pause(600);
				}
			}
		} catch {
			// instance created even if LLM stub
		}

		await pause(400);
		await typewrite("let\u2019s go.");
		stage = "departing";

		await pause(600);
		goto(`/${chosenSlug}`);
	}

	function handleKeyKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			submitKey();
		}
	}

	function handleNameKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			submitName();
		}
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
	class="onboard-space"
	class:onboard-depart={stage === "departing"}
>
	<!-- living atmosphere -->
	<div class="onboard-glow" class:onboard-glow-active={stage !== "waking"}></div>
	<div class="onboard-glow-secondary" class:onboard-glow-active={stage !== "waking"}></div>

	<!-- particles -->
	<div class="onboard-particles" class:onboard-particles-active={stage !== "waking"}>
		{#each Array(8) as _, i}
			<div class="onboard-particle" style="--i:{i}; --x:{10 + (i * 11) % 80}; --y:{8 + (i * 17) % 78}"></div>
		{/each}
	</div>

	<!-- content -->
	<div class="relative z-10 w-full max-w-lg px-6">
		<!-- typewriter lines -->
		<div class="space-y-3 mb-8">
			{#each lines as line, i}
				<div class="onboard-line" style="animation-delay: {i * 50}ms">
					{#if i === 0 && line.done}
						<p class="onboard-title">{line.revealed}</p>
					{:else if i === 0}
						<p class="onboard-title">{line.revealed}<span class="onboard-cursor"></span></p>
					{:else if !line.done}
						<p class="onboard-text">{line.revealed}<span class="onboard-cursor"></span></p>
					{:else}
						<p class="onboard-text">{line.revealed}</p>
					{/if}
				</div>
			{/each}
		</div>

		<!-- provider picker -->
		{#if stage === "picking-provider"}
			<div class="onboard-input-enter">
				<div class="flex gap-3">
					<button onclick={() => pickProvider("anthropic")} class="onboard-pill flex-1">
						<span class="font-display text-sm italic">anthropic</span>
					</button>
					<button onclick={() => pickProvider("openai")} class="onboard-pill flex-1">
						<span class="font-display text-sm italic">openai</span>
					</button>
				</div>
				<button
					onclick={skipConfig}
					class="mt-4 w-full text-xs text-muted-foreground/25 transition-colors hover:text-muted-foreground/50 italic"
				>
					skip for now
				</button>
			</div>
		{/if}

		<!-- model picker -->
		{#if stage === "picking-model" && chosenProvider}
			<div class="onboard-input-enter">
				<div class="grid grid-cols-3 gap-2.5">
					{#each MODELS[chosenProvider] as model}
						<button onclick={() => pickModel(model.id)} class="onboard-pill flex-col gap-0.5 py-3.5">
							<span class="font-display text-sm italic">{model.label}</span>
							<span class="text-[10px] text-muted-foreground/25">{model.note}</span>
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- api key -->
		{#if stage === "waiting-key"}
			<div class="onboard-input-enter">
				<div class="relative">
					<input
						bind:this={keyInput}
						bind:value={apiKeyValue}
						onkeydown={handleKeyKeydown}
						type="password"
						placeholder="sk-..."
						class="onboard-text-input font-mono text-sm"
					/>
					{#if apiKeyValue.trim()}
						<button onclick={submitKey} aria-label="Submit key" class="onboard-submit-btn">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
						</button>
					{/if}
				</div>
				{#if keyError}
					<p class="mt-2 text-xs text-red-400/60 italic">{keyError}</p>
				{/if}
			</div>
		{/if}

		<!-- testing -->
		{#if stage === "testing"}
			<div class="onboard-input-enter flex items-center gap-3">
				<div class="onboard-spinner"></div>
				<span class="font-display text-xs italic text-warm/40">waking up</span>
			</div>
		{/if}

		<!-- language picker -->
		{#if stage === "picking-language"}
			<div class="onboard-input-enter">
				<div class="grid grid-cols-4 gap-2">
					{#each LANGUAGES as lang}
						<button
							onclick={() => pickLanguage(lang.id)}
							class="onboard-pill text-xs"
							class:onboard-pill-active={chosenLanguage === lang.id}
						>
							{lang.label}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- name input -->
		{#if stage === "waiting-name"}
			<div class="onboard-input-enter">
				<div class="relative">
					<input
						bind:this={nameInput}
						bind:value={nameValue}
						onkeydown={handleNameKeydown}
						placeholder="your name"
						class="onboard-text-input font-display text-lg italic"
					/>
					{#if nameValue.trim()}
						<button onclick={submitName} aria-label="Continue" class="onboard-submit-btn">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- soul picker -->
		{#if stage === "picking-soul"}
			<div class="onboard-input-enter">
				<div class="grid grid-cols-2 gap-2.5">
					{#each soulTemplates as template (template.id)}
						<button onclick={() => pickSoul(template)} class="onboard-pill onboard-pill-soul">
							<span class="onboard-pill-name">{template.name}</span>
							<span class="onboard-pill-desc">{template.description}</span>
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- first message -->
		{#if stage === "waiting-first"}
			<div class="onboard-input-enter">
				<div class="relative">
					<textarea
						bind:this={messageInput}
						bind:value={firstMessage}
						onkeydown={handleMessageKeydown}
						placeholder="what's on your mind?"
						rows={3}
						class="onboard-textarea"
					></textarea>
					{#if firstMessage.trim()}
						<button onclick={submitFirst} aria-label="Send" class="onboard-send-btn">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- sending -->
		{#if stage === "sending"}
			<div class="onboard-input-enter flex items-center gap-3">
				<div class="onboard-spinner"></div>
				<span class="font-display text-xs italic text-warm/40">creating your space</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.onboard-space {
		position: relative;
		display: flex;
		height: 100%;
		align-items: center;
		justify-content: center;
		overflow: hidden;
	}

	/* atmosphere */
	.onboard-glow {
		position: absolute;
		top: 40%;
		left: 50%;
		width: 600px;
		height: 600px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 0%) 0%,
			transparent 70%
		);
		transition: all 2s cubic-bezier(0.16, 1, 0.3, 1);
		pointer-events: none;
	}
	.onboard-glow-secondary {
		position: absolute;
		top: 45%;
		left: 48%;
		width: 400px;
		height: 400px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.70 0.08 300 / 0%) 0%,
			transparent 60%
		);
		transition: all 2s cubic-bezier(0.16, 1, 0.3, 1);
		pointer-events: none;
	}
	.onboard-glow.onboard-glow-active {
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 5%) 0%,
			oklch(0.78 0.12 75 / 2%) 30%,
			transparent 65%
		);
		animation: glow-breathe 6s ease-in-out infinite;
	}
	.onboard-glow-secondary.onboard-glow-active {
		background: radial-gradient(
			circle,
			oklch(0.70 0.08 300 / 2%) 0%,
			transparent 60%
		);
		animation: glow-breathe 10s ease-in-out infinite;
		animation-delay: -3s;
	}
	@keyframes glow-breathe {
		0%, 100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
		50% { opacity: 0.5; transform: translate(-50%, -50%) scale(1.08); }
	}

	/* particles */
	.onboard-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
		overflow: hidden;
	}
	.onboard-particle {
		position: absolute;
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 0%);
		left: calc(var(--x) * 1%);
		top: calc(var(--y) * 1%);
		transition: background 2s ease;
	}
	.onboard-particles-active .onboard-particle {
		background: oklch(0.78 0.12 75 / 20%);
		animation: particle-drift 14s ease-in-out infinite;
		animation-delay: calc(var(--i) * -1.8s);
	}
	@keyframes particle-drift {
		0%, 100% { transform: translate(0, 0); opacity: 0.2; }
		25% { transform: translate(12px, -18px); opacity: 0.6; }
		50% { transform: translate(-8px, -30px); opacity: 0.3; }
		75% { transform: translate(16px, -12px); opacity: 0.5; }
	}

	/* text styles */
	.onboard-title {
		font-family: var(--font-display);
		font-size: 2.25rem;
		font-weight: 300;
		font-style: italic;
		letter-spacing: -0.02em;
		color: oklch(0.88 0.03 75 / 90%);
	}

	.onboard-text {
		font-family: var(--font-body);
		font-size: 0.9375rem;
		line-height: 1.6;
		color: oklch(0.88 0.03 75 / 50%);
	}

	/* cursor */
	.onboard-cursor {
		display: inline-block;
		width: 1.5px;
		height: 1.1em;
		margin-left: 1px;
		vertical-align: text-bottom;
		background: oklch(0.78 0.12 75 / 50%);
		animation: cursor-blink 0.8s steps(2) infinite;
	}
	@keyframes cursor-blink {
		0% { opacity: 1; }
		100% { opacity: 0; }
	}

	/* line entrance */
	.onboard-line {
		animation: line-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	@keyframes line-enter {
		from { opacity: 0; transform: translateY(8px); }
		to { opacity: 1; transform: translateY(0); }
	}

	/* input entrance */
	.onboard-input-enter {
		animation: input-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 100ms;
	}
	@keyframes input-enter {
		from { opacity: 0; transform: translateY(12px); }
		to { opacity: 1; transform: translateY(0); }
	}

	/* departure */
	.onboard-depart {
		animation: depart 0.6s cubic-bezier(0.55, 0, 1, 0.45) forwards;
	}
	@keyframes depart {
		to { opacity: 0; transform: scale(0.98); filter: blur(4px); }
	}

	/* spinner */
	.onboard-spinner {
		width: 12px;
		height: 12px;
		border: 1.5px solid oklch(0.78 0.12 75 / 15%);
		border-top-color: oklch(0.78 0.12 75 / 50%);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	/* pills */
	.onboard-pill {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 2rem;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		background: oklch(0.78 0.12 75 / 3%);
		padding: 0.65rem 1rem;
		color: oklch(0.88 0.03 75 / 60%);
		transition: all 0.3s ease;
		cursor: pointer;
	}
	.onboard-pill:hover {
		border-color: oklch(0.78 0.12 75 / 25%);
		background: oklch(0.78 0.12 75 / 8%);
		color: oklch(0.88 0.03 75 / 85%);
		box-shadow: 0 0 25px oklch(0.78 0.12 75 / 6%);
	}
	.onboard-pill-active {
		border-color: oklch(0.78 0.12 75 / 35%);
		background: oklch(0.78 0.12 75 / 12%);
		color: oklch(0.88 0.03 75 / 90%);
	}

	.onboard-pill-soul {
		flex-direction: column;
		align-items: flex-start;
		gap: 0.25rem;
		padding: 0.75rem 1rem;
		border-radius: 1rem;
		text-align: left;
	}
	.onboard-pill-name {
		font-family: var(--font-display);
		font-size: 0.8rem;
		font-style: italic;
		color: oklch(0.88 0.03 75 / 70%);
	}
	.onboard-pill-desc {
		font-size: 0.625rem;
		color: oklch(0.88 0.03 75 / 30%);
		line-height: 1.3;
	}

	/* text input */
	.onboard-text-input {
		width: 100%;
		border-radius: 2rem;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		background: oklch(0.78 0.12 75 / 3%);
		padding: 0.875rem 1.25rem;
		color: oklch(0.88 0.03 75 / 80%);
		outline: none;
		transition: all 0.4s ease;
	}
	.onboard-text-input::placeholder {
		color: oklch(0.78 0.12 75 / 15%);
		font-style: italic;
	}
	.onboard-text-input:focus {
		border-color: oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 40px oklch(0.78 0.12 75 / 6%);
	}

	/* submit button inside inputs */
	.onboard-submit-btn {
		position: absolute;
		right: 0.5rem;
		top: 50%;
		transform: translateY(-50%);
		display: flex;
		height: 2rem;
		width: 2rem;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		color: oklch(0.78 0.12 75 / 50%);
		transition: all 0.3s ease;
	}
	.onboard-submit-btn:hover {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	/* textarea */
	.onboard-textarea {
		width: 100%;
		resize: none;
		border-radius: 1rem;
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		background: oklch(0.78 0.12 75 / 3%);
		padding: 0.875rem 1.25rem;
		font-family: var(--font-body);
		font-size: 0.875rem;
		line-height: 1.6;
		color: oklch(0.88 0.03 75 / 80%);
		outline: none;
		transition: all 0.4s ease;
	}
	.onboard-textarea::placeholder {
		color: oklch(0.78 0.12 75 / 15%);
		font-family: var(--font-display);
		font-style: italic;
	}
	.onboard-textarea:focus {
		border-color: oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 40px oklch(0.78 0.12 75 / 6%);
	}

	.onboard-send-btn {
		position: absolute;
		right: 0.5rem;
		bottom: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		color: oklch(0.78 0.12 75 / 50%);
		transition: all 0.3s ease;
	}
	.onboard-send-btn:hover {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
	}
</style>
