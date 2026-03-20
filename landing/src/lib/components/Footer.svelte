<footer class="footer">
	<div class="footer-line"></div>
	<div class="mx-auto max-w-[1100px] px-6 flex flex-col sm:flex-row justify-between items-center gap-4">
		<div class="text-xs" style="color: oklch(0.90 0.02 75 / 20%);">
			&copy; 2026 Bolly &middot; <a href="https://triangleint.com" target="_blank" class="footer-link">Triangle Interactive</a>
		</div>
		<ul class="flex gap-6 list-none">
			<li><a href="/docs" class="footer-link text-xs">Docs</a></li>
			<li><a href="/privacy" class="footer-link text-xs">Privacy</a></li>
			<li><a href="/terms" class="footer-link text-xs">Terms</a></li>
		</ul>
	</div>
</footer>

<style>
	.footer {
		padding: 2.5rem 0;
		position: relative;
		border-top: 1px solid var(--glass-border);
	}

	.footer-line {
		position: absolute;
		top: 0;
		left: 10%;
		right: 10%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 6%), transparent);
		pointer-events: none;
	}

	.footer-link {
		color: oklch(0.90 0.02 75 / 20%);
		transition: color 0.3s ease;
		text-decoration: none;
	}

	.footer-link:hover {
		color: oklch(0.90 0.02 75 / 50%);
	}
</style>
