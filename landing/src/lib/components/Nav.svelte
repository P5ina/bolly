<script lang="ts">
	let scrolled = $state(false);

	$effect(() => {
		function onScroll() {
			scrolled = window.scrollY > 50;
		}
		window.addEventListener('scroll', onScroll, { passive: true });
		return () => window.removeEventListener('scroll', onScroll);
	});
</script>

<nav
	class="fixed top-0 left-0 right-0 z-100 py-4 backdrop-blur-[24px] transition-all duration-400"
	style="background: oklch(0.05 0.015 280 / 70%); border-bottom: 1px solid {scrolled ? 'oklch(1 0 0 / 8%)' : 'oklch(1 0 0 / 5%)'};"
>
	<div class="mx-auto max-w-[1100px] px-6 flex items-center justify-between">
		<a href="/" class="flex items-center gap-2.5">
			<div
				class="w-8 h-8 rounded-lg flex items-center justify-center font-display italic text-base text-warm"
				style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
			>
				b
			</div>
			<span class="font-display italic text-xl text-text tracking-tight">bolly</span>
		</a>

		<ul class="hidden md:flex items-center gap-8 list-none">
			<li><a href="#features" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">Features</a></li>
			<li><a href="#how" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">How it works</a></li>
			<li><a href="#pricing" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">Pricing</a></li>
			<li><a href="/skills" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">Skills</a></li>
			<li><a href="https://github.com/P5ina/bolly" target="_blank" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">GitHub</a></li>
			<li><a href="/login" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">Log in</a></li>
			<li>
				<a
					href="/signup"
					class="text-[0.8125rem] py-2 px-5 rounded-full text-warm transition-all duration-300 hover:shadow-[0_0_30px_oklch(0.78_0.12_75/8%)]"
					style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
				>
					Get started
				</a>
			</li>
		</ul>
	</div>
</nav>
