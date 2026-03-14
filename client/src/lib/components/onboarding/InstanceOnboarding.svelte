<script lang="ts">
	import {
		sendMessage,
		fetchSoulTemplates,
		applySoulTemplate,
		fetchSoul,
		setCompanionName,
		fetchConfigStatus,
		updateLlmConfig,
	} from "$lib/api/client.js";
	import type { SoulTemplate } from "$lib/api/types.js";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import { play, preload } from "$lib/sounds.js";

	const toast = getToasts();
	import AsciiRenderer from "$lib/components/chat/AsciiRenderer.svelte";

	let { slug, oncomplete }: { slug: string; oncomplete: () => void } = $props();

	const instances = getInstances();

	type Stage =
		| "reveal"
		| "intro"
		| "picking-provider"
		| "picking-model"
		| "waiting-key"
		| "testing"
		| "picking-language"
		| "naming-user"
		| "naming-companion"
		| "picking-soul"
		| "waiting-first"
		| "sending"
		| "departing";

	let stage = $state<Stage>("reveal");
	let revealed = $state(false);
	let firstMessage = $state("");
	let userNameInput = $state(localStorage.getItem("bolly:preferredName") ?? "");
	let companionNameInput = $state("");
	let messageInput: HTMLTextAreaElement | undefined = $state();
	let userNameInputEl: HTMLInputElement | undefined = $state();
	let nameInputEl: HTMLInputElement | undefined = $state();
	let keyInput: HTMLInputElement | undefined = $state();
	let chosenProvider = $state<"anthropic" | "openai" | "openrouter" | null>(null);
	let chosenModel = $state<string | null>(null);
	let apiKeyValue = $state("");
	let keyError = $state("");
	let chosenLanguage = $state(
		localStorage.getItem("bolly:language") ?? "english",
	);
	let lines = $state<{ text: string; revealed: string; done: boolean }[]>([]);
	let soulTemplates = $state<SoulTemplate[]>([]);

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
		openrouter: [
			{ id: "google/gemini-2.5-flash", label: "gemini 2.5 flash", note: "fast & cheap" },
			{ id: "google/gemini-2.5-pro", label: "gemini 2.5 pro", note: "powerful" },
			{ id: "anthropic/claude-sonnet-4", label: "claude sonnet 4", note: "balanced" },
			{ id: "deepseek/deepseek-r1", label: "deepseek r1", note: "reasoning" },
			{ id: "meta-llama/llama-4-maverick", label: "llama 4 maverick", note: "open source" },
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

	async function runSequence() {
		// Reveal stage
		preload("intro_reveal");
		await pause(600);
		play("intro_reveal");
		revealed = true;
		await pause(1800);
		stage = "intro";
		await pause(400);
		await typewrite(`hey, ${slug}.`);
		await pause(400);
		await typewrite("a new space, just for us.");
		await pause(600);

		// Check if LLM is already configured
		let llmConfigured = false;
		for (let attempt = 0; attempt < 3; attempt++) {
			try {
				const status = await fetchConfigStatus();
				if (status.llm_configured) {
					llmConfigured = true;
					chosenProvider = (status.provider as "anthropic" | "openai" | "openrouter") ?? "anthropic";
					chosenModel = status.model ?? null;
				}
				break;
			} catch {
				if (attempt < 2) await pause(2000);
			}
		}

		if (!llmConfigured) {
			await typewrite("before we begin \u2014 who should i think with?");
			stage = "picking-provider";
		} else {
			await typewrite("what language should we speak?");
			stage = "picking-language";
		}
	}

	function pickProvider(provider: "anthropic" | "openai" | "openrouter") {
		chosenProvider = provider;
		continueAfterProvider();
	}

	async function continueAfterProvider() {
		stage = "intro";
		await pause(300);
		const name = chosenProvider ?? "unknown";
		await typewrite(`${name}. good choice.`);
		await pause(400);
		await typewrite("which mind should i wear?");
		stage = "picking-model";
	}

	async function pickModel(modelId: string) {
		chosenModel = modelId;
		stage = "intro";
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
			keyError = `hmm, that didn\u2019t work. try again? (${err instanceof Error ? err.message : String(err)})`;
			stage = "waiting-key";
			await pause(100);
			keyInput?.focus();
			return;
		}

		await pause(300);
		await typewrite("i can feel it. i\u2019m alive now.");
		await pause(600);
		await typewrite("what language should we speak?");
		stage = "picking-language";
	}

	async function skipConfig() {
		stage = "intro";
		await pause(300);
		await typewrite("alright, we\u2019ll figure that out later.");
		await pause(600);
		await typewrite("what language should we speak?");
		stage = "picking-language";
	}

	function handleKeyKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			submitKey();
		}
	}

	async function pickLanguage(langId: string) {
		chosenLanguage = langId;
		localStorage.setItem("bolly:language", langId);
		stage = "intro";
		await pause(300);
		const lang = LANGUAGES.find((l) => l.id === langId);
		await typewrite(`${lang?.label ?? langId}.`);
		await pause(400);
		await typewrite("what should i call you?");
		stage = "naming-user";
		await pause(100);
		userNameInputEl?.focus();
	}

	async function submitUserName() {
		const name = userNameInput.trim();
		if (!name) return;

		localStorage.setItem("bolly:preferredName", name);
		stage = "intro";
		await pause(200);
		await typewrite(`${name}. nice to meet you.`);
		await pause(400);
		await typewrite("what should i call myself?");
		stage = "naming-companion";
		await pause(100);
		nameInputEl?.focus();
	}

	function handleUserNameKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			submitUserName();
		}
	}

	async function submitCompanionName() {
		const name = companionNameInput.trim();
		if (!name) return;

		stage = "intro";
		await pause(200);
		await typewrite(`${name}. i like that.`);

		// Save companion name to server
		try {
			await setCompanionName(slug, name);
		} catch {
			// will be set later
		}

		await pause(400);

		let hasSoul = false;
		try {
			const soul = await fetchSoul(slug);
			hasSoul = soul.exists && soul.content.trim().length > 0;
		} catch {
			// no soul
		}

		if (hasSoul) {
			await askFirstMessage();
		} else {
			try {
				soulTemplates = await fetchSoulTemplates();
			} catch {
				soulTemplates = [];
			}

			if (soulTemplates.length > 0) {
				await typewrite("who should i be for you?");
				stage = "picking-soul";
			} else {
				await askFirstMessage();
			}
		}
	}

	function handleNameKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			submitCompanionName();
		}
	}

	async function pickSoul(template: SoulTemplate) {
		stage = "intro";
		await pause(200);

		if (template.id !== "custom") {
			try {
				await applySoulTemplate(slug, template.id);
			} catch {
				// will use default
			}
			await typewrite(`${template.name}. i can be that.`);
		} else {
			await typewrite("a blank canvas. you can shape me later.");
		}

		await pause(400);
		await askFirstMessage();
	}

	async function askFirstMessage() {
		await typewrite("tell me something.");
		stage = "waiting-first";
		await pause(100);
		messageInput?.focus();
	}

	async function submitFirst() {
		const content = firstMessage.trim();
		if (!content) return;

		stage = "sending";

		const preferredName =
			localStorage.getItem("bolly:preferredName") ?? "";
		const langLabel =
			LANGUAGES.find((l) => l.id === chosenLanguage)?.label ?? chosenLanguage;
		const setupParts: string[] = [];
		if (preferredName) setupParts.push(`my name is ${preferredName}`);
		setupParts.push(`please speak to me in ${langLabel}`);

		const combined = setupParts.join(". ") + ".\n\n" + content;

		try {
			await sendMessage(slug, combined);
			await instances.refresh();
		} catch {
			toast.error("setup failed — try sending a message after");
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
	class="instance-space"
	class:instance-depart={stage === "departing"}
>
	<!-- living glow -->
	<div class="instance-glow"></div>
	<div class="instance-glow-secondary"></div>

	<!-- particles -->
	<div class="instance-particles">
		{#each Array(6) as _, i}
			<div class="instance-particle" style="--i:{i}; --x:{12 + (i * 13) % 76}; --y:{15 + (i * 19) % 65}"></div>
		{/each}
	</div>

	<div class="relative z-10 w-full max-w-md px-6">
		<!-- companion creature -->
		<div class="mb-10 flex justify-center">
			<div class="instance-creature" class:instance-creature-reveal={revealed}>
				<AsciiRenderer thinking={stage === "reveal" && revealed} mood="calm" />
				{#if stage === "reveal" && revealed}
					<div class="instance-ring instance-ring-1"></div>
					<div class="instance-ring instance-ring-2"></div>
					<div class="instance-ring instance-ring-3"></div>
				{/if}
			</div>
		</div>

		<!-- typewriter lines -->
		<div class="space-y-3 mb-8" class:invisible={stage === "reveal"}>
			{#each lines as line, i}
				<div class="instance-line" style="animation-delay: {i * 50}ms">
					{#if i === 0 && line.done}
						<p class="instance-title">{line.revealed}</p>
					{:else if i === 0}
						<p class="instance-title">{line.revealed}<span class="instance-cursor"></span></p>
					{:else if !line.done}
						<p class="instance-text">{line.revealed}<span class="instance-cursor"></span></p>
					{:else}
						<p class="instance-text">{line.revealed}</p>
					{/if}
				</div>
			{/each}
		</div>

		<!-- provider picker -->
		{#if stage === "picking-provider"}
			<div class="instance-input-enter">
				<div class="flex gap-3">
					<button onclick={() => pickProvider("anthropic")} class="instance-pill flex-1">
						<span class="font-display text-sm italic">anthropic</span>
					</button>
					<button onclick={() => pickProvider("openai")} class="instance-pill flex-1">
						<span class="font-display text-sm italic">openai</span>
					</button>
					<button onclick={() => pickProvider("openrouter")} class="instance-pill flex-1">
						<span class="font-display text-sm italic">openrouter</span>
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
			<div class="instance-input-enter">
				<div class="grid grid-cols-3 gap-2.5">
					{#each MODELS[chosenProvider] as model}
						<button onclick={() => pickModel(model.id)} class="instance-pill flex-col gap-0.5 py-3.5">
							<span class="font-display text-sm italic">{model.label}</span>
							<span class="text-[10px] text-muted-foreground/25">{model.note}</span>
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- api key -->
		{#if stage === "waiting-key"}
			<div class="instance-input-enter">
				<div class="relative">
					<input
						bind:this={keyInput}
						bind:value={apiKeyValue}
						onkeydown={handleKeyKeydown}
						type="password"
						placeholder="sk-..."
						class="instance-name-input font-mono text-sm"
						style="text-align: left;"
					/>
					{#if apiKeyValue.trim()}
						<button onclick={submitKey} aria-label="Submit key" class="instance-send" style="top: 50%; bottom: auto; transform: translateY(-50%);">
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
			<div class="instance-input-enter flex items-center justify-center gap-3">
				<div class="instance-spinner"></div>
				<span class="font-display text-xs italic text-warm/40">waking up</span>
			</div>
		{/if}

		<!-- language picker -->
		{#if stage === "picking-language"}
			<div class="instance-input-enter">
				<div class="grid grid-cols-4 gap-2">
					{#each LANGUAGES as lang}
						<button
							onclick={() => pickLanguage(lang.id)}
							class="instance-pill"
							class:instance-pill-active={chosenLanguage === lang.id}
						>
							{lang.label}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- user name -->
		{#if stage === "naming-user"}
			<div class="instance-input-enter">
				<div class="relative">
					<input
						bind:this={userNameInputEl}
						bind:value={userNameInput}
						onkeydown={handleUserNameKeydown}
						placeholder="your name..."
						class="instance-name-input"
					/>
					{#if userNameInput.trim()}
						<button onclick={submitUserName} aria-label="Confirm" class="instance-send">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
							</svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- companion name -->
		{#if stage === "naming-companion"}
			<div class="instance-input-enter">
				<div class="relative">
					<input
						bind:this={nameInputEl}
						bind:value={companionNameInput}
						onkeydown={handleNameKeydown}
						placeholder="a name..."
						class="instance-name-input"
					/>
					{#if companionNameInput.trim()}
						<button onclick={submitCompanionName} aria-label="Confirm" class="instance-send">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
							</svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- soul picker -->
		{#if stage === "picking-soul"}
			<div class="instance-input-enter">
				<div class="grid grid-cols-2 gap-2.5">
					{#each soulTemplates as template (template.id)}
						<button
							onclick={() => pickSoul(template)}
							class="instance-pill instance-pill-soul"
						>
							<span class="instance-pill-name">{template.name}</span>
							<span class="instance-pill-desc">{template.description}</span>
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
						class="instance-textarea"
					></textarea>
					{#if firstMessage.trim()}
						<button onclick={submitFirst} aria-label="Send" class="instance-send">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
							</svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- sending -->
		{#if stage === "sending"}
			<div class="instance-input-enter flex items-center justify-center gap-3">
				<div class="instance-spinner"></div>
				<span class="font-display text-xs italic text-warm/40">waking up</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.instance-space {
		position: relative;
		display: flex;
		height: 100%;
		align-items: center;
		justify-content: center;
		overflow: hidden;
	}

	.instance-glow {
		position: absolute;
		top: 30%;
		left: 50%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 5%) 0%,
			oklch(0.78 0.12 75 / 2%) 30%,
			transparent 65%
		);
		animation: glow-breathe 6s ease-in-out infinite;
		pointer-events: none;
	}
	.instance-glow-secondary {
		position: absolute;
		top: 35%;
		left: 48%;
		width: 300px;
		height: 300px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.70 0.08 300 / 2%) 0%,
			transparent 60%
		);
		animation: glow-breathe 10s ease-in-out infinite;
		animation-delay: -3s;
		pointer-events: none;
	}
	@keyframes glow-breathe {
		0%, 100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
		50% { opacity: 0.5; transform: translate(-50%, -50%) scale(1.08); }
	}

	/* particles */
	.instance-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
		overflow: hidden;
	}
	.instance-particle {
		position: absolute;
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 20%);
		left: calc(var(--x) * 1%);
		top: calc(var(--y) * 1%);
		animation: particle-drift 14s ease-in-out infinite;
		animation-delay: calc(var(--i) * -2.3s);
	}
	@keyframes particle-drift {
		0%, 100% { transform: translate(0, 0); opacity: 0.2; }
		25% { transform: translate(12px, -18px); opacity: 0.6; }
		50% { transform: translate(-8px, -30px); opacity: 0.3; }
		75% { transform: translate(16px, -12px); opacity: 0.5; }
	}

	.invisible {
		visibility: hidden;
	}

	/* companion creature */
	.instance-creature {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transform: scale(0.3);
		transition: all 1.2s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.instance-creature-reveal {
		opacity: 1;
		transform: scale(1);
	}

	/* reveal rings */
	.instance-ring {
		position: absolute;
		inset: -4px;
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 30%);
		animation: ring-expand 1.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
	}
	.instance-ring-1 { animation-delay: 0ms; }
	.instance-ring-2 { animation-delay: 150ms; border-color: oklch(0.78 0.12 75 / 20%); }
	.instance-ring-3 { animation-delay: 300ms; border-color: oklch(0.78 0.12 75 / 10%); }

	@keyframes ring-expand {
		0% {
			transform: scale(1);
			opacity: 1;
		}
		100% {
			transform: scale(3.5);
			opacity: 0;
		}
	}

	/* text styles */
	.instance-title {
		font-family: var(--font-display);
		font-size: 1.5rem;
		font-weight: 400;
		font-style: italic;
		letter-spacing: -0.01em;
		color: oklch(0.88 0.03 75 / 90%);
		text-align: center;
	}

	.instance-text {
		font-family: var(--font-body);
		font-size: 0.875rem;
		line-height: 1.6;
		color: oklch(0.88 0.03 75 / 50%);
		text-align: center;
	}

	.instance-cursor {
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

	/* pills */
	.instance-pill {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 2rem;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		background: oklch(0.78 0.12 75 / 3%);
		padding: 0.5rem 0.5rem;
		font-family: var(--font-body);
		font-size: 0.75rem;
		color: oklch(0.88 0.03 75 / 60%);
		transition: all 0.3s ease;
		cursor: pointer;
	}
	.instance-pill:hover {
		border-color: oklch(0.78 0.12 75 / 25%);
		background: oklch(0.78 0.12 75 / 8%);
		color: oklch(0.88 0.03 75 / 85%);
		box-shadow: 0 0 20px oklch(0.78 0.12 75 / 6%);
	}
	.instance-pill-active {
		border-color: oklch(0.78 0.12 75 / 35%);
		background: oklch(0.78 0.12 75 / 12%);
		color: oklch(0.88 0.03 75 / 90%);
	}

	.instance-pill-soul {
		flex-direction: column;
		align-items: flex-start;
		gap: 0.25rem;
		padding: 0.75rem 1rem;
		border-radius: 1rem;
	}
	.instance-pill-name {
		font-family: var(--font-display);
		font-size: 0.8rem;
		font-style: italic;
		color: oklch(0.88 0.03 75 / 70%);
	}
	.instance-pill-desc {
		font-size: 0.625rem;
		color: oklch(0.88 0.03 75 / 30%);
		line-height: 1.3;
	}

	/* name input */
	.instance-name-input {
		width: 100%;
		border-radius: 2rem;
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		background: oklch(0.78 0.12 75 / 3%);
		padding: 0.75rem 3rem 0.75rem 1.25rem;
		font-family: var(--font-display);
		font-size: 0.95rem;
		font-style: italic;
		color: oklch(0.88 0.03 75 / 80%);
		outline: none;
		transition: all 0.4s ease;
		text-align: center;
	}
	.instance-name-input::placeholder {
		color: oklch(0.78 0.12 75 / 15%);
		font-style: italic;
	}
	.instance-name-input:focus {
		border-color: oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 40px oklch(0.78 0.12 75 / 6%);
	}

	/* textarea */
	.instance-textarea {
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
	.instance-textarea::placeholder {
		color: oklch(0.78 0.12 75 / 15%);
		font-family: var(--font-display);
		font-style: italic;
	}
	.instance-textarea:focus {
		border-color: oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 40px oklch(0.78 0.12 75 / 6%);
	}

	.instance-send {
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
	.instance-send:hover {
		color: oklch(0.78 0.12 75 / 80%);
		background: oklch(0.78 0.12 75 / 8%);
	}

	/* spinner */
	.instance-spinner {
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

	/* depart */
	.instance-depart {
		animation: depart 0.4s cubic-bezier(0.55, 0, 1, 0.45) forwards;
	}
	@keyframes depart {
		to { opacity: 0; transform: scale(0.98); filter: blur(4px); }
	}
</style>
