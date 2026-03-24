<script lang="ts">
	import {
		sendMessage,
		fetchSoulTemplates,
		applySoulTemplate,
		fetchSoul,
		setCompanionName,
	} from "$lib/api/client.js";
	import type { SoulTemplate } from "$lib/api/types.js";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import { getSceneStore } from "$lib/stores/scene.svelte.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";
	import { play, playImmediate, preload } from "$lib/sounds.js";
	import { hapticReveal } from "$lib/haptics.js";

	const toast = getToasts();
	const scene = getSceneStore();

	let { slug, oncomplete }: { slug: string; oncomplete: () => void } = $props();

	const instances = getInstances();

	type Stage =
		| "reveal"
		| "intro"
		| "waiting-key"
		| "testing"
		| "picking-language"
		| "naming-companion"
		| "picking-soul"
		| "waiting-first"
		| "sending"
		| "departing";

	let stage = $state<Stage>("reveal");
	let revealed = $state(false);
	let firstMessage = $state("");
	let companionNameInput = $state("");
	let messageInput: HTMLTextAreaElement | undefined = $state();
	let nameInputEl: HTMLInputElement | undefined = $state();
	let chosenLanguage = $state(
		typeof localStorage !== "undefined" ? (localStorage.getItem("bolly:language") ?? "english") : "english",
	);
	let lines = $state<{ text: string; revealed: string; done: boolean }[]>([]);
	let soulTemplates = $state<SoulTemplate[]>([]);

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
				if (i >= text.length) { lines[idx].done = true; resolve(); return; }
				const char = text[i];
				lines[idx].revealed += char;
				i++;
				if (i % 3 === 0) playImmediate("typewriter", { pitchRange: [0.88, 1.15] });
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
		// Tell scene to show a centered sphere for this slug
		scene.enterOnboarding(slug);

		preload("intro_reveal", "typewriter");
		await pause(600);
		play("intro_reveal");
		hapticReveal();
		revealed = true;
		await pause(1800);
		stage = "intro";
		await pause(400);
		const userName = typeof localStorage !== "undefined" ? localStorage.getItem("bolly:preferredName") || slug : slug;
		await typewrite(`hey, ${userName}.`);
		await pause(400);
		await typewrite("a new space, just for us.");
		await pause(600);

		await typewrite("what language should we speak?");
		stage = "picking-language";
	}

	async function pickLanguage(langId: string) {
		chosenLanguage = langId;
		localStorage.setItem("bolly:language", langId);
		stage = "intro";
		await pause(300);
		const lang = LANGUAGES.find((l) => l.id === langId);
		await typewrite(`${lang?.label ?? langId}.`);
		await pause(400);
		await typewrite("what should i call myself?");
		stage = "naming-companion";
		await pause(100);
		nameInputEl?.focus();
	}

	async function submitCompanionName() {
		const name = companionNameInput.trim();
		if (!name) return;
		stage = "intro";
		await pause(200);
		await typewrite(`${name}. i like that.`);
		try { await setCompanionName(slug, name); } catch {}
		await pause(400);

		let hasSoul = false;
		try {
			const soul = await fetchSoul(slug);
			hasSoul = soul.exists && soul.content.trim().length > 0;
		} catch {}

		if (hasSoul) {
			await askFirstMessage();
		} else {
			try { soulTemplates = await fetchSoulTemplates(); } catch { soulTemplates = []; }
			if (soulTemplates.length > 0) {
				await typewrite("who should i be for you?");
				stage = "picking-soul";
			} else {
				await askFirstMessage();
			}
		}
	}

	function handleNameKeydown(e: KeyboardEvent) { if (e.key === "Enter") { e.preventDefault(); submitCompanionName(); } }

	async function pickSoul(template: SoulTemplate) {
		stage = "intro";
		await pause(200);
		if (template.id !== "custom") {
			try { await applySoulTemplate(slug, template.id); } catch {}
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
		const langLabel = LANGUAGES.find((l) => l.id === chosenLanguage)?.label ?? chosenLanguage;
		const preferredName = typeof localStorage !== "undefined" ? localStorage.getItem("bolly:preferredName") || slug : slug;
		const combined = `my name is ${preferredName}. please speak to me in ${langLabel}.\n\n${content}`;
		try {
			await sendMessage(slug, combined);
			await instances.refresh();
		} catch {
			toast.error("setup failed — try sending a message after");
		}
		// Start cinematic intro, then complete
		scene.finishOnboarding();
		stage = "departing";
		await pause(600);
		oncomplete();
	}

	function handleMessageKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submitFirst(); }
	}

	$effect(() => { runSequence(); });
</script>

<div class="ob" class:ob-depart={stage === "departing"} class:ob-hidden={stage === "reveal" && !revealed}>
	<div class="ob-content">
		<!-- Typewriter lines -->
		<div class="ob-lines" class:ob-lines-hidden={stage === "reveal"}>
			{#each lines as line, i}
				<div class="ob-line" style="animation-delay: {i * 40}ms">
					{#if i === 0 && line.done}
						<p class="ob-title">{line.revealed}</p>
					{:else if i === 0}
						<p class="ob-title">{line.revealed}<span class="ob-cursor"></span></p>
					{:else if !line.done}
						<p class="ob-text">{line.revealed}<span class="ob-cursor"></span></p>
					{:else}
						<p class="ob-text">{line.revealed}</p>
					{/if}
				</div>
			{/each}
		</div>

		<!-- Interactive sections -->
		<div class="ob-input-area">
			{#if stage === "sending"}
				<div class="ob-enter ob-center">
					<div class="ob-spinner"></div>
					<span class="ob-spinner-label">thinking</span>
				</div>
			{/if}

			{#if stage === "picking-language"}
				<div class="ob-enter">
					<div class="ob-pills ob-pills-lang">
						{#each LANGUAGES as lang}
							<button onclick={() => pickLanguage(lang.id)} class="ob-pill" class:ob-pill-active={chosenLanguage === lang.id}>{lang.label}</button>
						{/each}
					</div>
				</div>
			{/if}

			{#if stage === "naming-companion"}
				<div class="ob-enter">
					<div class="ob-field">
						<input bind:this={nameInputEl} bind:value={companionNameInput} onkeydown={handleNameKeydown} placeholder="a name..." class="ob-input" />
						{#if companionNameInput.trim()}
							<button onclick={submitCompanionName} class="ob-go" aria-label="Confirm">→</button>
						{/if}
					</div>
				</div>
			{/if}

			{#if stage === "picking-soul"}
				<div class="ob-enter">
					<div class="ob-pills ob-pills-soul">
						{#each soulTemplates as template (template.id)}
							<button onclick={() => pickSoul(template)} class="ob-pill ob-pill-col ob-pill-soul">
								<span class="ob-pill-label">{template.name}</span>
								<span class="ob-pill-note">{template.description}</span>
							</button>
						{/each}
					</div>
				</div>
			{/if}

			{#if stage === "waiting-first"}
				<div class="ob-enter">
					<div class="ob-field">
						<textarea bind:this={messageInput} bind:value={firstMessage} onkeydown={handleMessageKeydown} placeholder="what's on your mind?" rows={3} class="ob-input ob-textarea"></textarea>
						{#if firstMessage.trim()}
							<button onclick={submitFirst} class="ob-go ob-go-textarea" aria-label="Send">→</button>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.ob {
		position: relative;
		display: flex;
		height: 100%;
		align-items: center;
		justify-content: center;
		overflow: hidden;
		pointer-events: none;
		z-index: 10;
		transition: opacity 0.6s ease;
	}
	.ob-hidden { opacity: 0; }

	.ob-content {
		position: relative;
		width: 100%;
		max-width: 420px;
		padding: 0 1.5rem;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
		pointer-events: auto;
	}

	/* ── Lines ── */
	.ob-lines { display: flex; flex-direction: column; gap: 0.625rem; width: 100%; }
	.ob-lines-hidden { visibility: hidden; }

	.ob-line { animation: ob-fade-in 0.4s cubic-bezier(0.16, 1, 0.3, 1) both; }
	@keyframes ob-fade-in {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.ob-title {
		font-family: var(--font-display);
		font-size: 1.35rem;
		font-weight: 400;
		font-style: italic;
		letter-spacing: -0.01em;
		color: oklch(0.90 0.02 75 / 85%);
		text-align: center;
	}

	.ob-text {
		font-family: var(--font-body);
		font-size: 0.85rem;
		line-height: 1.6;
		color: oklch(1 0 0 / 45%);
		text-align: center;
	}

	.ob-cursor {
		display: inline-block;
		width: 1.5px;
		height: 1.05em;
		margin-left: 1px;
		vertical-align: text-bottom;
		background: oklch(1 0 0 / 40%);
		animation: blink 0.8s steps(2) infinite;
	}
	@keyframes blink { 0% { opacity: 1; } 100% { opacity: 0; } }

	/* ── Input area ── */
	.ob-input-area { width: 100%; }
	.ob-enter {
		animation: ob-slide-in 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 80ms;
	}
	@keyframes ob-slide-in {
		from { opacity: 0; transform: translateY(10px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.ob-center { display: flex; align-items: center; justify-content: center; gap: 0.625rem; }

	/* ── Pills (liquid glass) ── */
	.ob-pills { display: flex; gap: 0.5rem; flex-wrap: wrap; }
	.ob-pills-grid { display: grid; grid-template-columns: repeat(3, 1fr); }
	.ob-pills-lang { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.375rem; }
	.ob-pills-soul { display: grid; grid-template-columns: repeat(2, 1fr); }

	.ob-pill {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: 1;
		padding: 0.55rem 0.75rem;
		border-radius: 2rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		-webkit-backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		font-family: var(--font-display);
		font-size: 0.8rem;
		font-style: italic;
		color: oklch(1 0 0 / 50%);
		cursor: pointer;
		transition: all 0.3s ease;
		box-shadow:
			0 1px 4px oklch(0 0 0 / 8%),
			inset 0 1px 0 oklch(1 0 0 / 5%);
	}
	.ob-pill:hover {
		border-color: oklch(1 0 0 / 20%);
		background: oklch(1 0 0 / 8%);
		color: oklch(1 0 0 / 75%);
		box-shadow: 0 2px 12px oklch(0 0 0 / 12%), inset 0 1px 0 oklch(1 0 0 / 8%);
	}
	.ob-pill-active {
		border-color: oklch(1 0 0 / 22%);
		background: oklch(1 0 0 / 10%);
		color: oklch(1 0 0 / 80%);
	}

	.ob-pill-col {
		flex-direction: column;
		gap: 0.2rem;
		padding: 0.65rem 0.75rem;
	}
	.ob-pill-soul {
		border-radius: 1rem;
		align-items: flex-start;
		padding: 0.75rem 1rem;
		gap: 0.25rem;
	}
	.ob-pill-label {
		font-family: var(--font-display);
		font-size: 0.8rem;
		font-style: italic;
		color: oklch(1 0 0 / 60%);
	}
	.ob-pill-note {
		font-family: var(--font-body);
		font-size: 0.6rem;
		font-style: normal;
		color: oklch(1 0 0 / 25%);
	}

	/* ── Input fields (liquid glass) ── */
	.ob-field { position: relative; }

	.ob-input {
		width: 100%;
		padding: 0.75rem 3rem 0.75rem 1.25rem;
		border-radius: 2rem;
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		-webkit-backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		font-family: var(--font-display);
		font-size: 0.9rem;
		font-style: italic;
		color: oklch(1 0 0 / 80%);
		outline: none;
		text-align: center;
		transition: all 0.3s ease;
		box-shadow: inset 0 1px 0 oklch(1 0 0 / 5%), inset 0 -1px 0 oklch(0 0 0 / 4%);
	}
	.ob-input::placeholder { color: oklch(1 0 0 / 18%); font-style: italic; }
	.ob-input:focus {
		border-color: oklch(1 0 0 / 20%);
		box-shadow: 0 0 0 3px oklch(0.40 0.06 240 / 8%), inset 0 1px 0 oklch(1 0 0 / 8%);
	}
	.ob-input-mono { font-family: var(--font-mono); font-size: 0.8rem; font-style: normal; text-align: left; }

	.ob-textarea {
		border-radius: 1rem;
		resize: none;
		text-align: left;
		font-family: var(--font-body);
		font-size: 0.85rem;
		font-style: normal;
		line-height: 1.6;
		padding: 0.875rem 1.25rem;
	}

	.ob-go {
		position: absolute;
		right: 0.5rem;
		top: 50%;
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		font-size: 0.9rem;
		color: oklch(1 0 0 / 40%);
		transition: all 0.3s ease;
		cursor: pointer;
	}
	.ob-go:hover { color: oklch(1 0 0 / 70%); background: oklch(1 0 0 / 6%); }
	.ob-go-textarea { top: auto; bottom: 0.5rem; transform: none; }

	.ob-error { margin-top: 0.5rem; font-size: 0.72rem; color: oklch(0.65 0.15 25 / 60%); font-style: italic; text-align: center; }
	.ob-skip {
		margin-top: 0.75rem;
		width: 100%;
		font-size: 0.68rem;
		color: oklch(1 0 0 / 18%);
		font-style: italic;
		transition: color 0.2s ease;
		cursor: pointer;
		text-align: center;
	}
	.ob-skip:hover { color: oklch(1 0 0 / 40%); }

	/* ── Spinner ── */
	.ob-spinner {
		width: 12px; height: 12px;
		border: 1.5px solid oklch(1 0 0 / 10%);
		border-top-color: oklch(1 0 0 / 40%);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin { to { transform: rotate(360deg); } }
	.ob-spinner-label {
		font-family: var(--font-display);
		font-size: 0.72rem;
		font-style: italic;
		color: oklch(1 0 0 / 30%);
	}

	/* ── Depart ── */
	.ob-depart { animation: depart 0.5s cubic-bezier(0.55, 0, 1, 0.45) forwards; }
	@keyframes depart { to { opacity: 0; transform: scale(0.98); } }
</style>
