<script lang="ts">
	let {
		label,
		value,
		timestamp,
	}: {
		label: string;
		value: string;
		timestamp?: string;
	} = $props();
</script>

<div class="activity-row">
	<div class="activity-pulse"></div>
	<div class="activity-copy">
		<div class="activity-label">{label}</div>
		<div class="activity-value">{value}</div>
	</div>
	{#if timestamp}
		<div class="activity-time">{timestamp}</div>
	{/if}
</div>

<style>
	.activity-row {
		display: grid;
		grid-template-columns: auto 1fr auto;
		gap: 0.7rem;
		align-items: start;
		padding: 0.7rem 0.2rem;
		border-top: 1px solid oklch(0.78 0.12 75 / 6%);
	}

	.activity-pulse {
		width: 7px;
		height: 7px;
		margin-top: 0.32rem;
		border-radius: 999px;
		background: oklch(0.78 0.12 75 / 65%);
		box-shadow: 0 0 0 6px oklch(0.78 0.12 75 / 5%);
	}

	.activity-copy {
		display: flex;
		flex-direction: column;
		gap: 0.16rem;
		min-width: 0;
	}

	.activity-label {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: oklch(0.78 0.02 280 / 46%);
	}

	.activity-value {
		font-size: 0.8rem;
		line-height: 1.45;
		color: oklch(0.84 0.03 75 / 82%);
	}

	.activity-time {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(0.78 0.02 280 / 38%);
		white-space: nowrap;
	}
</style>
