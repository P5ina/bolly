<script lang="ts">
	import type { Drop } from "$lib/api/types.js";
	let {
		drop,
		icon,
		time,
		expanded,
		onexpand,
		ondelete,
	}: {
		drop: Drop;
		icon: string;
		time: string;
		expanded: boolean;
		onexpand: () => void;
		ondelete: () => void;
	} = $props();

	const moodColors: Record<string, string> = {
		calm: "oklch(0.70 0.08 220)",
		curious: "oklch(0.75 0.12 200)",
		excited: "oklch(0.80 0.16 75)",
		warm: "oklch(0.78 0.12 75)",
		happy: "oklch(0.82 0.14 95)",
		joyful: "oklch(0.85 0.16 85)",
		reflective: "oklch(0.65 0.08 260)",
		contemplative: "oklch(0.60 0.06 270)",
		melancholy: "oklch(0.55 0.08 250)",
		sad: "oklch(0.50 0.06 260)",
		worried: "oklch(0.60 0.10 40)",
		anxious: "oklch(0.65 0.12 30)",
		playful: "oklch(0.78 0.15 140)",
		mischievous: "oklch(0.75 0.14 150)",
		focused: "oklch(0.70 0.10 230)",
		tired: "oklch(0.55 0.04 260)",
		peaceful: "oklch(0.72 0.06 180)",
		loving: "oklch(0.72 0.14 0)",
		tender: "oklch(0.70 0.10 350)",
		creative: "oklch(0.78 0.16 310)",
		energetic: "oklch(0.82 0.18 65)",
	};

	const accentColor = $derived(moodColors[drop.mood] ?? "oklch(0.78 0.12 75)");
</script>

<button
	class="drop-card"
	class:drop-card-expanded={expanded}
	style="--accent: {accentColor}"
	onclick={onexpand}
>
	<div class="drop-card-header">
		<span class="drop-card-icon">{icon}</span>
		<span class="drop-card-kind">{drop.kind}</span>
		<span class="drop-card-time">{time}</span>
	</div>

	<h3 class="drop-card-title">{drop.title}</h3>

	<div class="drop-card-content" class:drop-card-content-expanded={expanded}>
		{drop.content}
	</div>

	{#if drop.mood}
		<div class="drop-card-mood">
			<span class="drop-card-mood-dot" style="background: {accentColor}"></span>
			{drop.mood}
		</div>
	{/if}

	{#if expanded}
		<span
			role="button"
			tabindex="0"
			class="drop-card-delete"
			onclick={(e) => { e.stopPropagation(); ondelete(); }}
			onkeydown={(e) => { if (e.key === "Enter") { e.stopPropagation(); ondelete(); } }}
		>
			delete
		</span>
	{/if}
</button>

<style>
	.drop-card {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 1rem 1.125rem;
		border-radius: 0.75rem;
		background: oklch(0.09 0.018 278 / 60%);
		border: 1px solid oklch(1 0 0 / 4%);
		cursor: pointer;
		transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
		text-align: left;
		width: 100%;
		animation: drop-emerge 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes drop-emerge {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.drop-card:hover {
		background: oklch(0.10 0.020 278 / 70%);
		border-color: var(--accent, oklch(0.78 0.12 75)) / 15%;
		box-shadow: 0 0 20px color-mix(in oklch, var(--accent) 8%, transparent);
	}

	.drop-card-expanded {
		border-color: color-mix(in oklch, var(--accent) 20%, transparent);
		box-shadow: 0 0 30px color-mix(in oklch, var(--accent) 10%, transparent);
	}

	.drop-card-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.drop-card-icon {
		font-family: var(--font-mono);
		font-size: 0.8rem;
		color: var(--accent, oklch(0.78 0.12 75));
		opacity: 0.7;
	}

	.drop-card-kind {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.drop-card-time {
		margin-left: auto;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 28%);
	}

	.drop-card-title {
		font-family: var(--font-display);
		font-size: 0.875rem;
		font-weight: 500;
		color: oklch(0.88 0.02 75);
		line-height: 1.35;
	}

	.drop-card-content {
		font-size: 0.78rem;
		color: oklch(0.78 0.12 75 / 50%);
		line-height: 1.55;
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		-webkit-box-orient: vertical;
		white-space: pre-wrap;
	}

	.drop-card-content-expanded {
		-webkit-line-clamp: unset;
		color: oklch(0.78 0.12 75 / 65%);
	}

	.drop-card-mood {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 35%);
		margin-top: 0.25rem;
	}

	.drop-card-mood-dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		opacity: 0.6;
	}

	.drop-card-delete {
		align-self: flex-end;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.65 0.15 20 / 50%);
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		transition: all 0.2s ease;
	}

	.drop-card-delete:hover {
		color: oklch(0.65 0.15 20 / 80%);
		background: oklch(0.65 0.15 20 / 8%);
	}
</style>
