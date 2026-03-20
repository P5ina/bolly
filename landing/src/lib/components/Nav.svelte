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
	class="nav"
	class:nav-scrolled={scrolled}
>
	<div class="mx-auto max-w-[1100px] px-6 flex items-center justify-between">
		<a href="/" class="flex items-center gap-2.5">
			<div class="nav-logo">b</div>
			<span class="font-display italic text-xl text-text tracking-tight">bolly</span>
		</a>

		<!-- desktop -->
		<ul class="hidden md:flex items-center gap-8 list-none">
			<li><a href="/#features" class="nav-link">Features</a></li>
			<li><a href="/#how" class="nav-link">How it works</a></li>
			<li><a href="/#pricing" class="nav-link">Pricing</a></li>
			<li><a href="/skills" class="nav-link">Skills</a></li>
			<li><a href="/login" class="nav-link">Log in</a></li>
			<li>
				<a href="/signup" class="nav-cta">
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
		<div class="mobile-divider"></div>

		<a href="/login" class="mobile-link">Log in</a>
		<a href="/signup" class="mobile-cta">Get started</a>
	</div>
{/if}

<style>
	.nav {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 100;
		py: 4;
		padding: 1rem 0;
		backdrop-filter: blur(24px) saturate(160%) brightness(1.04);
		background: oklch(0.04 0.015 260 / 50%);
		border-bottom: 1px solid var(--glass-border);
		transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.nav::after {
		content: '';
		position: absolute;
		bottom: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 8%), transparent);
		pointer-events: none;
	}

	.nav-scrolled {
		background: oklch(0.04 0.015 260 / 70%);
		border-bottom-color: oklch(1 0 0 / 10%);
		box-shadow: 0 8px 32px oklch(0 0 0 / 20%);
	}

	.nav-logo {
		width: 2rem;
		height: 2rem;
		border-radius: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		font-family: var(--font-display);
		font-style: italic;
		font-size: 0.875rem;
		color: var(--color-warm);
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
	}

	.nav-link {
		font-size: 0.8125rem;
		color: oklch(0.90 0.02 75 / 45%);
		letter-spacing: 0.02em;
		transition: color 0.3s ease;
		text-decoration: none;
	}

	.nav-link:hover {
		color: oklch(0.90 0.02 75 / 85%);
	}

	.nav-cta {
		font-size: 0.8125rem;
		padding: 0.5rem 1.25rem;
		border-radius: 2rem;
		color: var(--color-warm);
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		border-top-color: oklch(0.78 0.12 75 / 20%);
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		text-decoration: none;
		position: relative;
		overflow: hidden;
	}

	.nav-cta::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 50%;
		background: linear-gradient(180deg, oklch(0.78 0.12 75 / 6%) 0%, transparent 100%);
		pointer-events: none;
		border-radius: 2rem 2rem 0 0;
	}

	.nav-cta:hover {
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow: 0 0 30px oklch(0.78 0.12 75 / 8%);
	}

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
		background: oklch(0 0 0 / 50%);
		backdrop-filter: blur(8px);
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
		background: oklch(0.05 0.015 260 / 80%);
		backdrop-filter: blur(32px) saturate(160%) brightness(1.04);
		border-left: 1px solid var(--glass-border);
		padding: 5rem 1.5rem 2rem;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		animation: slide-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.mobile-menu::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		bottom: 0;
		width: 1px;
		background: linear-gradient(180deg, oklch(1 0 0 / 12%), transparent 50%);
		pointer-events: none;
	}

	.mobile-link {
		display: block;
		padding: 0.75rem 0.75rem;
		font-size: 0.875rem;
		color: oklch(0.90 0.02 75 / 50%);
		border-radius: 0.75rem;
		transition: all 0.2s ease;
		text-decoration: none;
	}
	.mobile-link:hover {
		color: oklch(0.90 0.02 75 / 85%);
		background: oklch(1 0 0 / 4%);
	}

	.mobile-divider {
		height: 1px;
		background: linear-gradient(90deg, oklch(1 0 0 / 8%), transparent);
		margin: 0.5rem 0.75rem;
	}

	.mobile-cta {
		display: block;
		text-align: center;
		margin: 0.5rem 0.75rem 0;
		padding: 0.75rem 1.5rem;
		border-radius: 2rem;
		font-size: 0.875rem;
		color: var(--color-warm);
		background: var(--glass-bg);
		backdrop-filter: var(--glass-blur);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		transition: all 0.2s ease;
		text-decoration: none;
	}
	.mobile-cta:hover {
		background: oklch(1 0 0 / 8%);
		border-color: oklch(0.78 0.12 75 / 30%);
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
