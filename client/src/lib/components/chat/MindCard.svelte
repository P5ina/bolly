<script lang="ts">
	let {
		title,
		value,
		hint = "",
		accent = "soft",
	}: {
		title: string;
		value: string;
		hint?: string;
		accent?: "soft" | "warm" | "cool" | "glow";
	} = $props();
</script>

<div class="mind-card" data-accent={accent}>
	<div class="mind-card-title">{title}</div>
	<div class="mind-card-value">{value}</div>
	{#if hint}
		<div class="mind-card-hint">{hint}</div>
	{/if}
</div>

<style>
	.mind-card {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 0.85rem 0.95rem;
		border-radius: 14px;
		border: 1px solid oklch(0.78 0.12 75 / 8%);
		background: linear-gradient(180deg, oklch(0.1 0.01 280 / 34%), oklch(0.08 0.01 280 / 20%));
		transition: transform 0.3s ease, border-color 0.3s ease, background 0.3s ease;
	}

	.mind-card:hover {
		transform: translateY(-1px);
		border-color: oklch(0.78 0.12 75 / 16%);
	}

	.mind-card[data-accent="warm"] {
		background:
			linear-gradient(180deg, oklch(0.1 0.01 280 / 36%), oklch(0.08 0.01 280 / 20%)),
			radial-gradient(circle at top right, oklch(0.78 0.12 75 / 9%), transparent 48%);
	}

	.mind-card[data-accent="cool"] {
		background:
			linear-gradient(180deg, oklch(0.1 0.01 280 / 36%), oklch(0.08 0.01 280 / 20%)),
			radial-gradient(circle at top right, oklch(0.76 0.12 170 / 8%), transparent 48%);
	}

	.mind-card[data-accent="glow"] {
		background:
			linear-gradient(180deg, oklch(0.1 0.01 280 / 38%), oklch(0.08 0.01 280 / 22%)),
			radial-gradient(circle at top right, oklch(0.84 0.1 75 / 12%), transparent 54%);
	}

	.mind-card-title {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.78 0.02 280 / 45%);
	}

	.mind-card-value {
		font-size: 0.9rem;
		line-height: 1.45;
		color: oklch(0.9 0.03 75 / 90%);
	}

	.mind-card-hint {
		font-size: 0.74rem;
		line-height: 1.4;
		color: oklch(0.78 0.02 280 / 58%);
	}
</style>
