<script lang="ts">
	import Reveal from './Reveal.svelte';

	const tools = [
		'read_file', 'write_file', 'run_command', 'install_package',
		'send_email', 'read_email', 'remember', 'recall', 'journal',
		'edit_soul', 'set_mood', 'create_drop', 'web_search',
		'schedule_message', 'search_code',
	];
</script>

<section class="py-28">
	<div class="mx-auto max-w-[1100px] px-6">
		<div class="demo-layout">
			<Reveal>
				<div class="demo-terminal">
					<div class="demo-bar">
						<div class="demo-dot"></div>
						<div class="demo-dot"></div>
						<div class="demo-dot"></div>
						<span class="ml-2 text-[0.6875rem] text-text-ghost">bolly — heartbeat</span>
					</div>
					<div class="demo-body">
						<span class="line"><span class="prompt">heartbeat</span> <span class="out">waking up...</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;mood:</span> <span class="text-warm">curious</span> <span class="out">→</span> <span class="text-warm">contemplative</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;journal:</span> <span class="text-violet">wrote 3 paragraphs</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;memory:</span> <span class="text-violet">recalled 2 facts</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;drop:</span> <span class="text-warm">created "on repetition"</span></span>
						<span class="line">&nbsp;</span>
						<span class="line"><span class="prompt">drop</span> <span class="out">thought — "on repetition"</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;</span><span class="cmd">there's something beautiful about</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;</span><span class="cmd">the way you come back to the same</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;</span><span class="cmd">problems. not because you're stuck —</span></span>
						<span class="line"><span class="out">&nbsp;&nbsp;</span><span class="cmd">because you care enough to circle.</span></span>
						<span class="line">&nbsp;</span>
						<span class="line"><span class="prompt">next heartbeat</span> <span class="out">in 43m</span><span class="cursor"></span></span>
					</div>
				</div>
			</Reveal>

			<Reveal delay={200}>
				<div>
					<p class="section-label">Capabilities</p>
					<h2 class="font-display font-normal italic text-text mb-5 leading-[1.15]"
						style="font-size: clamp(1.5rem, 3vw, 2.25rem); letter-spacing: -0.02em;"
					>
						it doesn't just talk.<br><em>it does things.</em>
					</h2>
					<p class="text-[0.9375rem] text-text-dim leading-relaxed mb-3">
						Your companion has real tools connected to its runtime.
						It can read your codebase, run shell commands, manage projects,
						send you emails, and install what it needs.
					</p>
					<p class="text-[0.9375rem] text-text-dim leading-relaxed mb-5">
						During heartbeat cycles, it reflects on your conversations,
						writes in its journal, and creates drops — small creative artifacts
						that emerge from its thinking.
					</p>
					<div class="flex flex-wrap gap-1.5">
						{#each tools as tool}
							<span class="tool-tag">{tool}</span>
						{/each}
					</div>
				</div>
			</Reveal>
		</div>
	</div>
</section>

<style>
	.section-label {
		font-size: 0.6875rem;
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

	.demo-terminal {
		background: oklch(0.04 0.01 280);
		border: 1px solid var(--color-border);
		border-radius: 0.75rem;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: 0.8125rem;
	}

	.demo-bar {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		padding: 0.75rem 1rem;
		background: oklch(0.06 0.01 280);
		border-bottom: 1px solid var(--color-border);
	}

	.demo-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(1 0 0 / 6%);
	}

	.demo-body {
		padding: 1.25rem;
		line-height: 1.8;
		min-height: 280px;
	}

	.line { display: block; min-height: 1.8em; }
	.prompt { color: var(--color-warm-dim); }
	.cmd { color: var(--color-text-dim); }
	.out { color: var(--color-text-ghost); }

	.cursor {
		display: inline-block;
		width: 7px;
		height: 14px;
		background: var(--color-warm-dim);
		vertical-align: text-bottom;
		margin-left: 2px;
		animation: cursor-blink 1s steps(2) infinite;
	}

	.tool-tag {
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		padding: 0.25rem 0.625rem;
		border-radius: 0.25rem;
		background: var(--color-warm-ghost);
		border: 1px solid var(--color-border);
		color: var(--color-text-ghost);
		transition: all 0.3s ease;
	}

	.tool-tag:hover {
		border-color: var(--color-border-warm);
		color: var(--color-warm-dim);
	}

	@media (max-width: 768px) {
		.demo-layout {
			grid-template-columns: 1fr;
			gap: 2.5rem;
		}
		.demo-layout > :last-child { order: -1; }
	}
</style>
