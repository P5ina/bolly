<script lang="ts">
	import { goto } from "$app/navigation";
	import { sendMessage, updateLlmConfig } from "$lib/api/client.js";
	import { getInstances } from "$lib/stores/instances.svelte.js";

	const instances = getInstances();

	// --- state machine ---
	type Stage =
		| "waking"
		| "greeting"
		| "picking-provider"
		| "waiting-key"
		| "testing"
		| "asking-name"
		| "waiting-name"
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
	let keyError = $state("");

	// visible lines of companion text, each with typewriter state
	let lines = $state<{ text: string; revealed: string; done: boolean }[]>([]);

	// -- typewriter engine --
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

				// natural pauses on punctuation
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

	// -- orchestrator --
	async function runSequence() {
		// waking glow
		await pause(800);

		stage = "greeting";
		await typewrite("hello.");
		await pause(600);
		await typewrite("i\u2019m here now.");
		await pause(400);
		await typewrite("this is your space.");
		await pause(800);

		// config setup
		await typewrite("before we begin \u2014 who should i think with?");
		stage = "picking-provider";
	}

	function pickProvider(provider: "anthropic" | "openai") {
		chosenProvider = provider;
		continueAfterProvider();
	}

	async function continueAfterProvider() {
		stage = "greeting"; // hide picker while typing
		await pause(300);
		const name = chosenProvider === "anthropic" ? "anthropic" : "openai";
		await typewrite(`${name}. good choice.`);
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
		stage = "asking-name";
		await typewrite("what should i call you?");
		stage = "waiting-name";

		await pause(100);
		nameInput?.focus();
	}

	async function skipConfig() {
		stage = "greeting"; // hide picker
		await pause(300);
		await typewrite("alright, we\u2019ll figure that out later.");
		await pause(600);

		stage = "asking-name";
		await typewrite("what should i call you?");
		stage = "waiting-name";

		await pause(100);
		nameInput?.focus();
	}

	async function submitName() {
		const slug = slugify(nameValue);
		if (!slug) return;
		chosenSlug = slug;

		stage = "confirming";
		await pause(300);

		// If LLM is configured, let the AI respond to the name
		if (chosenProvider) {
			try {
				const res = await sendMessage(chosenSlug, `my name is ${nameValue.trim()}`);
				const aiReply = res.messages.find((m) => m.role === "assistant");
				if (aiReply) {
					await typewrite(aiReply.content);
				} else {
					await typewrite(`${nameValue.trim()}. i like that.`);
				}
			} catch {
				await typewrite(`${nameValue.trim()}. i like that.`);
			}
		} else {
			await typewrite(`${nameValue.trim()}.`);
			await pause(500);
			await typewrite("i like that.");
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

			// If LLM is configured, typewrite the AI response
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
	class="relative flex h-full items-center justify-center overflow-hidden"
	class:onboard-depart={stage === "departing"}
>
	<!-- ambient glow -->
	<div class="onboard-glow" class:onboard-glow-active={stage !== "waking"}></div>

	<!-- floating particles -->
	<div class="onboard-particles" class:onboard-particles-active={stage !== "waking"}>
		{#each Array(6) as _, i}
			<div class="onboard-particle" style="--i:{i}"></div>
		{/each}
	</div>

	<!-- content column -->
	<div class="relative z-10 w-full max-w-lg px-6">
		<!-- companion lines -->
		<div class="space-y-3 mb-8">
			{#each lines as line, i}
				<div
					class="onboard-line"
					style="animation-delay: {i * 50}ms"
				>
					{#if i === 0 && line.done}
						<!-- first line is the big greeting -->
						<p class="font-display text-4xl font-bold tracking-tight text-foreground">
							{line.revealed}
						</p>
					{:else if i === 0}
						<p class="font-display text-4xl font-bold tracking-tight text-foreground">
							{line.revealed}<span class="onboard-cursor"></span>
						</p>
					{:else if !line.done}
						<p class="text-base leading-relaxed text-muted-foreground">
							{line.revealed}<span class="onboard-cursor"></span>
						</p>
					{:else}
						<p class="text-base leading-relaxed text-muted-foreground">
							{line.revealed}
						</p>
					{/if}
				</div>
			{/each}
		</div>

		<!-- provider picker -->
		{#if stage === "picking-provider"}
			<div class="onboard-input-enter">
				<div class="flex gap-3">
					<button
						onclick={() => pickProvider("anthropic")}
						class="flex-1 rounded-xl border border-warm/20 bg-warm/5 px-5 py-3.5 font-display text-sm text-foreground transition-all duration-300 hover:border-warm/40 hover:bg-warm/10 hover:shadow-[0_0_30px_-5px] hover:shadow-warm/15"
					>
						anthropic
					</button>
					<button
						onclick={() => pickProvider("openai")}
						class="flex-1 rounded-xl border border-warm/20 bg-warm/5 px-5 py-3.5 font-display text-sm text-foreground transition-all duration-300 hover:border-warm/40 hover:bg-warm/10 hover:shadow-[0_0_30px_-5px] hover:shadow-warm/15"
					>
						openai
					</button>
				</div>
				<button
					onclick={skipConfig}
					class="mt-3 w-full text-xs text-muted-foreground/40 transition-colors hover:text-muted-foreground/70"
				>
					skip for now
				</button>
			</div>
		{/if}

		<!-- api key input -->
		{#if stage === "waiting-key"}
			<div class="onboard-input-enter">
				<div class="relative">
					<input
						bind:this={keyInput}
						bind:value={apiKeyValue}
						onkeydown={handleKeyKeydown}
						type="password"
						placeholder="sk-..."
						class="w-full rounded-xl border border-warm/20 bg-warm/5 px-5 py-3.5 font-mono text-sm text-foreground placeholder:text-muted-foreground/25 outline-none transition-all duration-300 focus:border-warm/40 focus:shadow-[0_0_30px_-5px] focus:shadow-warm/15"
					/>
					{#if apiKeyValue.trim()}
						<button
							onclick={submitKey}
							aria-label="Submit key"
							class="absolute right-2 top-1/2 -translate-y-1/2 flex h-8 w-8 items-center justify-center rounded-lg bg-warm text-warm-foreground transition-all hover:bg-warm/90"
						>
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
						</button>
					{/if}
				</div>
				{#if keyError}
					<p class="mt-2 text-xs text-red-400/80">{keyError}</p>
				{/if}
			</div>
		{/if}

		<!-- testing spinner -->
		{#if stage === "testing"}
			<div class="onboard-input-enter flex items-center gap-2.5 text-sm text-warm/60">
				<div class="onboard-spinner"></div>
				<span class="font-mono text-xs">waking up</span>
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
						class="w-full rounded-xl border border-warm/20 bg-warm/5 px-5 py-3.5 font-display text-lg text-foreground placeholder:text-muted-foreground/25 outline-none transition-all duration-300 focus:border-warm/40 focus:shadow-[0_0_30px_-5px] focus:shadow-warm/15"
					/>
					{#if nameValue.trim()}
						<button
							onclick={submitName}
							aria-label="Continue"
							class="absolute right-2 top-1/2 -translate-y-1/2 flex h-8 w-8 items-center justify-center rounded-lg bg-warm text-warm-foreground transition-all hover:bg-warm/90"
						>
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
						</button>
					{/if}
				</div>
			</div>
		{/if}

		<!-- first message input -->
		{#if stage === "waiting-first"}
			<div class="onboard-input-enter">
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

		<!-- sending state -->
		{#if stage === "sending"}
			<div class="onboard-input-enter flex items-center gap-2.5 text-sm text-warm/60">
				<div class="onboard-spinner"></div>
				<span class="font-mono text-xs">creating your space</span>
			</div>
		{/if}
	</div>
</div>

<style>
	/* ambient glow */
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
	.onboard-glow-active {
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 6%) 0%,
			oklch(0.78 0.12 75 / 2%) 30%,
			transparent 70%
		);
		animation: glow-breathe 5s ease-in-out infinite;
	}
	@keyframes glow-breathe {
		0%, 100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
		50% { opacity: 0.6; transform: translate(-50%, -50%) scale(1.08); }
	}

	/* floating particles */
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
		transition: background 2s ease;
	}
	.onboard-particles-active .onboard-particle {
		background: oklch(0.78 0.12 75 / 25%);
		animation: particle-float 12s ease-in-out infinite;
		animation-delay: calc(var(--i) * -2s);
	}
	.onboard-particle:nth-child(1) { left: 15%; top: 20%; }
	.onboard-particle:nth-child(2) { left: 80%; top: 15%; }
	.onboard-particle:nth-child(3) { left: 25%; top: 75%; }
	.onboard-particle:nth-child(4) { left: 70%; top: 65%; }
	.onboard-particle:nth-child(5) { left: 50%; top: 30%; }
	.onboard-particle:nth-child(6) { left: 40%; top: 85%; }
	@keyframes particle-float {
		0%, 100% { transform: translate(0, 0); opacity: 0.3; }
		25% { transform: translate(15px, -20px); opacity: 1; }
		50% { transform: translate(-10px, -35px); opacity: 0.5; }
		75% { transform: translate(20px, -15px); opacity: 0.8; }
	}

	/* typewriter cursor */
	.onboard-cursor {
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

	/* departure transition */
	.onboard-depart {
		animation: depart 0.6s cubic-bezier(0.55, 0, 1, 0.45) forwards;
	}
	@keyframes depart {
		to { opacity: 0; transform: scale(0.98) translateY(-10px); filter: blur(4px); }
	}

	/* spinner */
	.onboard-spinner {
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
