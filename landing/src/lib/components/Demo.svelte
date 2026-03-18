<script lang="ts">
	import Reveal from './Reveal.svelte';
</script>

<section class="py-28">
	<div class="mx-auto max-w-[1100px] px-6">
		<div class="demo-layout">
			<Reveal>
				<div class="demo-chat">
					<div class="demo-bar">
						<div class="demo-avatar">b</div>
						<div>
							<span class="text-[0.8125rem] text-text">bolly</span>
							<span class="demo-mood">feeling curious</span>
						</div>
					</div>
					<div class="demo-body">
						<div class="demo-msg demo-msg-user">
							i have an exam on thursday and i haven't started studying. kind of freaking out
						</div>
						<div class="demo-msg demo-msg-bot">
							okay let's not panic. what's the subject and what topics does it cover?
						</div>
						<div class="demo-msg demo-msg-user">
							organic chemistry. reactions, mechanisms, stereochemistry
						</div>
						<div class="demo-msg demo-msg-bot">
							three days is enough if we're smart about it. let me make you a study plan — do you want to start with the hardest topic or build momentum with the easier ones first?
						</div>
						<div class="demo-msg demo-msg-user">
							easier ones first i think
						</div>
						<div class="demo-msg demo-msg-bot">
							good call. stereochemistry first (mostly visual pattern recognition), then reactions (memorization + practice), then mechanisms last (needs the deepest focus).

							want me to quiz you on stereochemistry right now?
						</div>
						<div class="demo-typing">
							<span></span><span></span><span></span>
						</div>
					</div>
				</div>
			</Reveal>

			<Reveal delay={200}>
				<div>
					<p class="section-label">How it feels</p>
					<h2 class="font-display font-normal italic text-text mb-5 leading-[1.15]"
						style="font-size: clamp(1.5rem, 3vw, 2.25rem); letter-spacing: -0.02em;"
					>
						someone in your corner.<br><em>not a search engine.</em>
					</h2>
					<p class="text-[0.9375rem] text-text-dim leading-relaxed mb-3">
						It doesn't just answer questions — it notices when you're overwhelmed
						and breaks things down. It remembers what you've been studying and
						what trips you up.
					</p>
					<p class="text-[0.9375rem] text-text-dim leading-relaxed mb-5">
						Between conversations, it reflects on its own. Journals about things
						you discussed. Sometimes writes you a thought that came to mind.
						A friend that's still thinking about you when you're not there.
					</p>
					<div class="demo-signals">
						<div class="demo-signal">
							<span class="demo-signal-dot"></span>
							tracks your mood from how you write
						</div>
						<div class="demo-signal">
							<span class="demo-signal-dot"></span>
							remembers everything across conversations
						</div>
						<div class="demo-signal">
							<span class="demo-signal-dot"></span>
							reaches out when you've been quiet
						</div>
					</div>
				</div>
			</Reveal>
		</div>
	</div>
</section>

<style>
	.section-label {
		font-size: 0.8rem;
		letter-spacing: 0.15em;
		text-transform: uppercase;
		color: var(--color-warm-dim);
		margin-bottom: 1rem;
	}

	.demo-layout {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 4rem;
		align-items: center;
	}

	.demo-chat {
		background: oklch(0.04 0.01 280);
		border: 1px solid var(--color-border);
		border-radius: 1rem;
		overflow: hidden;
	}

	.demo-bar {
		display: flex;
		align-items: center;
		gap: 0.625rem;
		padding: 0.875rem 1.25rem;
		background: oklch(0.06 0.01 280);
		border-bottom: 1px solid var(--color-border);
	}

	.demo-avatar {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 10%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		display: flex;
		align-items: center;
		justify-content: center;
		font-family: var(--font-display);
		font-style: italic;
		font-size: 0.85rem;
		color: oklch(0.78 0.12 75 / 60%);
	}

	.demo-mood {
		display: block;
		font-size: 0.85rem;
		color: oklch(0.78 0.12 75 / 45%);
		font-style: italic;
	}

	.demo-body {
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		min-height: 340px;
	}

	.demo-msg {
		max-width: 85%;
		padding: 0.625rem 0.875rem;
		border-radius: 0.875rem;
		font-size: 0.9rem;
		line-height: 1.55;
	}

	.demo-msg-user {
		align-self: flex-end;
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		color: var(--color-text);
		border-bottom-right-radius: 0.25rem;
	}

	.demo-msg-bot {
		align-self: flex-start;
		background: oklch(1 0 0 / 3%);
		border: 1px solid var(--color-border);
		color: var(--color-text-dim);
		border-bottom-left-radius: 0.25rem;
		white-space: pre-line;
	}

	.demo-typing {
		align-self: flex-start;
		display: flex;
		gap: 3px;
		padding: 0.625rem 0.875rem;
	}
	.demo-typing span {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 20%);
		animation: typing-bounce 1.2s ease-in-out infinite;
	}
	.demo-typing span:nth-child(2) { animation-delay: 0.15s; }
	.demo-typing span:nth-child(3) { animation-delay: 0.3s; }
	@keyframes typing-bounce {
		0%, 60%, 100% { transform: translateY(0); opacity: 0.3; }
		30% { transform: translateY(-4px); opacity: 1; }
	}

	.demo-signals {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.demo-signal {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.85rem;
		color: var(--color-text-ghost);
	}

	.demo-signal-dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 25%);
		flex-shrink: 0;
	}

	@media (max-width: 768px) {
		.demo-layout {
			grid-template-columns: 1fr;
			gap: 2.5rem;
		}
		.demo-layout > :last-child { order: -1; }
	}
</style>
