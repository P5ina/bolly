<script lang="ts">
	let scrolled = $state(false);
	let mobileOpen = $state(false);

	$effect(() => {
		function onScroll() {
			scrolled = window.scrollY > 50;
		}
		window.addEventListener('scroll', onScroll, { passive: true });
		return () => window.removeEventListener('scroll', onScroll);
	});

	function closeMobile() {
		mobileOpen = false;
	}
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

		<!-- desktop -->
		<ul class="hidden md:flex items-center gap-8 list-none">
			<li><a href="/#features" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">Features</a></li>
			<li><a href="/#how" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">How it works</a></li>
			<li><a href="/#pricing" class="text-[0.8125rem] text-text-dim tracking-wide hover:text-text transition-colors">Pricing</a></li>
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

		<!-- mobile toggle -->
		<button
			class="mobile-toggle md:hidden"
			onclick={() => mobileOpen = !mobileOpen}
			aria-label={mobileOpen ? 'Close menu' : 'Open menu'}
		>
			<span class="toggle-line" class:toggle-open={mobileOpen}></span>
			<span class="toggle-line" class:toggle-open={mobileOpen}></span>
		</button>
	</div>
</nav>

<!-- mobile menu -->
{#if mobileOpen}
	<button class="mobile-backdrop" onclick={closeMobile} aria-label="Close menu"></button>

	<div class="mobile-menu">
		<a href="/#features" onclick={closeMobile} class="mobile-link">Features</a>
		<a href="/#how" onclick={closeMobile} class="mobile-link">How it works</a>
		<a href="/#pricing" onclick={closeMobile} class="mobile-link">Pricing</a>
		<a href="/skills" class="mobile-link">Skills</a>
		<a href="https://github.com/P5ina/bolly" target="_blank" class="mobile-link">GitHub</a>

		<div class="mobile-divider"></div>

		<a href="/login" class="mobile-link">Log in</a>
		<a href="/signup" class="mobile-cta">Get started</a>
	</div>
{/if}

<style>
	.mobile-toggle {
		display: flex;
		flex-direction: column;
		gap: 5px;
		padding: 0.5rem;
		cursor: pointer;
	}

	.toggle-line {
		width: 18px;
		height: 1.5px;
		background: oklch(0.78 0.12 75 / 50%);
		border-radius: 1px;
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		transform-origin: center;
	}

	.toggle-line.toggle-open:first-child {
		transform: translateY(3.25px) rotate(45deg);
	}
	.toggle-line.toggle-open:last-child {
		transform: translateY(-3.25px) rotate(-45deg);
	}

	.mobile-backdrop {
		position: fixed;
		inset: 0;
		z-index: 90;
		background: oklch(0 0 0 / 40%);
		backdrop-filter: blur(4px);
		animation: fade-in 0.2s ease both;
		border: none;
		cursor: default;
	}

	.mobile-menu {
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(280px, 80vw);
		z-index: 95;
		background: oklch(0.06 0.015 280 / 95%);
		backdrop-filter: blur(24px);
		border-left: 1px solid oklch(1 0 0 / 6%);
		padding: 5rem 1.5rem 2rem;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		animation: slide-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.mobile-link {
		display: block;
		padding: 0.75rem 0.75rem;
		font-size: 0.875rem;
		color: oklch(0.88 0.02 75 / 60%);
		border-radius: 0.5rem;
		transition: all 0.2s ease;
		text-decoration: none;
	}
	.mobile-link:hover {
		color: oklch(0.88 0.02 75 / 90%);
		background: oklch(1 0 0 / 3%);
	}

	.mobile-divider {
		height: 1px;
		background: oklch(1 0 0 / 6%);
		margin: 0.5rem 0.75rem;
	}

	.mobile-cta {
		display: block;
		text-align: center;
		margin: 0.5rem 0.75rem 0;
		padding: 0.75rem 1.5rem;
		border-radius: 2rem;
		font-size: 0.875rem;
		color: oklch(0.78 0.12 75 / 90%);
		background: oklch(0.78 0.12 75 / 8%);
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		transition: all 0.2s ease;
		text-decoration: none;
	}
	.mobile-cta:hover {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 25%);
	}

	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	@keyframes slide-in {
		from { transform: translateX(100%); }
		to { transform: translateX(0); }
	}
</style>
