<script lang="ts">
	import { fetchStats } from "$lib/api/client.js";
	import type { Stats } from "$lib/api/types.js";
	import { getToasts } from "$lib/stores/toast.svelte.js";

	const toast = getToasts();
	let { slug }: { slug: string } = $props();

	let stats = $state<Stats | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		try {
			stats = await fetchStats(slug);
		} catch {
			toast.error("failed to load stats");
		} finally {
			loading = false;
		}
	}

	$effect(() => { load(); });

	// --- derived data ---

	const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
	const HOUR_LABELS = Array.from({ length: 24 }, (_, i) =>
		i === 0 ? "12a" : i < 12 ? `${i}a` : i === 12 ? "12p" : `${i - 12}p`
	);

	let peakHour = $derived.by(() => {
		if (!stats) return 0;
		return stats.hourly_activity.indexOf(Math.max(...stats.hourly_activity));
	});

	let peakDay = $derived.by(() => {
		if (!stats) return 0;
		return stats.daily_activity.indexOf(Math.max(...stats.daily_activity));
	});

	let topMoods = $derived.by(() => {
		if (!stats) return [];
		return Object.entries(stats.mood_counts)
			.sort((a, b) => b[1] - a[1])
			.slice(0, 6);
	});

	let totalMoodCount = $derived(topMoods.reduce((s, [, c]) => s + c, 0));

	let daysSinceFirst = $derived.by(() => {
		if (!stats?.first_message_at) return 0;
		const first = parseInt(stats.first_message_at);
		return Math.floor((Date.now() - first) / 86400000);
	});

	// Heatmap: 7 rows (days) x 24 cols (hours) — need to derive from daily_history
	// For now, use hourly_activity × daily_activity as a rough 2D heatmap
	let heatmapMax = $derived.by(() => {
		if (!stats) return 1;
		return Math.max(...stats.hourly_activity, 1);
	});

	// Activity sparkline from daily_history (last 60 days)
	let sparkline = $derived.by(() => {
		if (!stats) return [];
		const last60 = stats.daily_history.slice(-60);
		const max = Math.max(...last60.map(([, c]) => c), 1);
		return last60.map(([date, count]) => ({ date, count, pct: count / max }));
	});

	// Contribution heatmap (GitHub-style) — last 52 weeks
	let contributionWeeks = $derived.by(() => {
		if (!stats) return [];
		const map = new Map(stats.daily_history.map(([d, c]) => [d, c]));
		const maxCount = Math.max(...stats.daily_history.map(([, c]) => c), 1);
		const weeks: { date: string; count: number; level: number }[][] = [];
		const today = new Date();
		// Start from 52 weeks ago, aligned to Monday
		const start = new Date(today);
		start.setDate(start.getDate() - 363 - start.getDay() + 1);

		let currentWeek: { date: string; count: number; level: number }[] = [];
		for (let d = new Date(start); d <= today; d.setDate(d.getDate() + 1)) {
			const ds = d.toISOString().slice(0, 10);
			const count = map.get(ds) ?? 0;
			const level = count === 0 ? 0 : Math.min(4, Math.ceil((count / maxCount) * 4));
			currentWeek.push({ date: ds, count, level });
			if (currentWeek.length === 7) {
				weeks.push(currentWeek);
				currentWeek = [];
			}
		}
		if (currentWeek.length > 0) weeks.push(currentWeek);
		return weeks;
	});

	const moodColors: Record<string, string> = {
		calm: "#5ba8d4", curious: "#5bb8d0", excited: "#d4a55a",
		warm: "#d4a55a", happy: "#7bc47a", playful: "#6bc4a0",
		thoughtful: "#8888b8", focused: "#7090c0", tender: "#d07888",
		loving: "#d46b8a", creative: "#c475d4", energetic: "#d4b040",
		melancholic: "#8080a0", anxious: "#c09060", grateful: "#70b880",
		nostalgic: "#a08890",
	};

	function getMoodColor(mood: string): string {
		return moodColors[mood] ?? "#8888a0";
	}

	function formatInterval(secs: number): string {
		if (secs < 60) return `${Math.round(secs)}s`;
		if (secs < 3600) return `${Math.round(secs / 60)}m`;
		return `${(secs / 3600).toFixed(1)}h`;
	}
</script>

<div class="stats-container">
	{#if loading}
		<div class="stats-loading"><div class="stats-loading-dot"></div></div>
	{:else if stats}
		<div class="stats-scroll">
			<!-- Hero numbers -->
			<div class="stats-hero">
				<div class="hero-stat">
					<span class="hero-value">{stats.total_messages.toLocaleString()}</span>
					<span class="hero-label">messages</span>
				</div>
				<div class="hero-stat">
					<span class="hero-value">{daysSinceFirst}</span>
					<span class="hero-label">days together</span>
				</div>
				<div class="hero-stat">
					<span class="hero-value">{stats.streak_days}</span>
					<span class="hero-label">day streak</span>
				</div>
			</div>

			<!-- Contribution heatmap -->
			<div class="stats-section">
				<h3 class="section-title">activity</h3>
				<div class="heatmap-container">
					<div class="heatmap-grid">
						{#each contributionWeeks as week}
							<div class="heatmap-col">
								{#each week as day}
									<div
										class="heatmap-cell"
										class:heatmap-0={day.level === 0}
										class:heatmap-1={day.level === 1}
										class:heatmap-2={day.level === 2}
										class:heatmap-3={day.level === 3}
										class:heatmap-4={day.level === 4}
										title="{day.date}: {day.count} messages"
									></div>
								{/each}
							</div>
						{/each}
					</div>
					<div class="heatmap-legend">
						<span class="heatmap-legend-label">less</span>
						<div class="heatmap-cell heatmap-0"></div>
						<div class="heatmap-cell heatmap-1"></div>
						<div class="heatmap-cell heatmap-2"></div>
						<div class="heatmap-cell heatmap-3"></div>
						<div class="heatmap-cell heatmap-4"></div>
						<span class="heatmap-legend-label">more</span>
					</div>
				</div>
			</div>

			<!-- Hourly activity -->
			<div class="stats-section">
				<h3 class="section-title">
					peak hours
					<span class="section-hint">most active at {HOUR_LABELS[peakHour]}</span>
				</h3>
				<div class="bar-chart">
					{#each stats.hourly_activity as count, i}
						{@const pct = count / heatmapMax}
						<div class="bar-col" title="{HOUR_LABELS[i]}: {count} messages">
							<div class="bar-fill" style="height: {pct * 100}%; opacity: {0.3 + pct * 0.7}"></div>
							{#if i % 3 === 0}
								<span class="bar-label">{HOUR_LABELS[i]}</span>
							{/if}
						</div>
					{/each}
				</div>
			</div>

			<!-- Day of week -->
			<div class="stats-section">
				<h3 class="section-title">
					day of week
					<span class="section-hint">{DAY_LABELS[peakDay]} is your day</span>
				</h3>
				<div class="day-bars">
					{#each stats.daily_activity as count, i}
						{@const max = Math.max(...stats.daily_activity, 1)}
						{@const pct = count / max}
						<div class="day-row">
							<span class="day-label">{DAY_LABELS[i]}</span>
							<div class="day-track">
								<div class="day-fill" style="width: {pct * 100}%"></div>
							</div>
							<span class="day-count">{count}</span>
						</div>
					{/each}
				</div>
			</div>

			<!-- Mood distribution -->
			{#if topMoods.length > 0}
				<div class="stats-section">
					<h3 class="section-title">mood palette</h3>
					<div class="mood-chart">
						{#each topMoods as [mood, count]}
							{@const pct = (count / totalMoodCount) * 100}
							<div class="mood-row">
								<div class="mood-dot" style="background: {getMoodColor(mood)}"></div>
								<span class="mood-name">{mood}</span>
								<div class="mood-track">
									<div class="mood-fill" style="width: {pct}%; background: {getMoodColor(mood)}"></div>
								</div>
								<span class="mood-pct">{pct.toFixed(0)}%</span>
							</div>
						{/each}
					</div>
				</div>
			{/if}

			<!-- Quick stats -->
			<div class="stats-section">
				<h3 class="section-title">details</h3>
				<div class="detail-grid">
					<div class="detail-item">
						<span class="detail-value">{Math.round(stats.avg_message_length)}</span>
						<span class="detail-label">avg chars/message</span>
					</div>
					<div class="detail-item">
						<span class="detail-value">{formatInterval(stats.avg_response_interval_secs)}</span>
						<span class="detail-label">avg between messages</span>
					</div>
					<div class="detail-item">
						<span class="detail-value">{stats.daily_history.length}</span>
						<span class="detail-label">active days</span>
					</div>
					<div class="detail-item">
						<span class="detail-value">{stats.total_messages > 0 && stats.daily_history.length > 0 ? (stats.total_messages / stats.daily_history.length).toFixed(1) : "0"}</span>
						<span class="detail-label">msgs/active day</span>
					</div>
				</div>
			</div>
		</div>
	{:else}
		<div class="stats-empty">no data yet</div>
	{/if}
</div>

<style>
	.stats-container {
		height: 100%;
		overflow: hidden;
		position: relative;
	}

	.stats-loading {
		display: flex; align-items: center; justify-content: center; height: 100%;
	}

	.stats-loading-dot {
		width: 6px; height: 6px; border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	.stats-empty {
		display: flex; align-items: center; justify-content: center; height: 100%;
		font-family: var(--font-mono); font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 25%);
	}

	.stats-scroll {
		height: 100%;
		overflow-y: auto;
		padding: 1.5rem;
		max-width: 520px;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 2rem;
	}

	/* ═══ Hero ═══ */

	.stats-hero {
		display: flex;
		justify-content: center;
		gap: 2.5rem;
		padding: 1rem 0;
	}

	.hero-stat {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.25rem;
	}

	.hero-value {
		font-family: var(--font-display);
		font-size: 1.8rem;
		font-weight: 300;
		color: oklch(0.78 0.12 75 / 70%);
		line-height: 1;
	}

	.hero-label {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.78 0.12 75 / 25%);
		letter-spacing: 0.06em;
	}

	/* ═══ Sections ═══ */

	.stats-section {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.section-title {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		font-weight: 400;
		color: oklch(0.78 0.12 75 / 35%);
		letter-spacing: 0.06em;
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
	}

	.section-hint {
		font-family: var(--font-body);
		font-size: 0.58rem;
		color: oklch(0.78 0.12 75 / 18%);
		font-style: italic;
	}

	/* ═══ Heatmap ═══ */

	.heatmap-container {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.heatmap-grid {
		display: flex;
		gap: 2px;
		overflow-x: auto;
		scrollbar-width: none;
	}

	.heatmap-grid::-webkit-scrollbar { display: none; }

	.heatmap-col {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.heatmap-cell {
		width: 9px;
		height: 9px;
		border-radius: 2px;
	}

	.heatmap-0 { background: oklch(1 0 0 / 4%); }
	.heatmap-1 { background: oklch(0.78 0.12 75 / 12%); }
	.heatmap-2 { background: oklch(0.78 0.12 75 / 25%); }
	.heatmap-3 { background: oklch(0.78 0.12 75 / 45%); }
	.heatmap-4 { background: oklch(0.78 0.12 75 / 70%); }

	.heatmap-legend {
		display: flex;
		align-items: center;
		gap: 3px;
		justify-content: flex-end;
	}

	.heatmap-legend-label {
		font-family: var(--font-mono);
		font-size: 0.48rem;
		color: oklch(0.78 0.12 75 / 18%);
		margin: 0 2px;
	}

	.heatmap-legend .heatmap-cell {
		width: 8px;
		height: 8px;
	}

	/* ═══ Bar chart (hourly) ═══ */

	.bar-chart {
		display: flex;
		align-items: flex-end;
		gap: 2px;
		height: 80px;
	}

	.bar-col {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		position: relative;
		justify-content: flex-end;
	}

	.bar-fill {
		width: 100%;
		border-radius: 2px 2px 0 0;
		background: oklch(0.78 0.12 75);
		min-height: 1px;
		transition: height 0.3s ease;
	}

	.bar-label {
		position: absolute;
		bottom: -14px;
		font-family: var(--font-mono);
		font-size: 0.42rem;
		color: oklch(0.78 0.12 75 / 18%);
	}

	/* ═══ Day bars ═══ */

	.day-bars {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.day-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.day-label {
		font-family: var(--font-mono);
		font-size: 0.55rem;
		color: oklch(0.78 0.12 75 / 30%);
		width: 28px;
		text-align: right;
	}

	.day-track {
		flex: 1;
		height: 8px;
		border-radius: 4px;
		background: oklch(1 0 0 / 3%);
		overflow: hidden;
	}

	.day-fill {
		height: 100%;
		border-radius: 4px;
		background: oklch(0.78 0.12 75 / 40%);
		transition: width 0.5s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.day-count {
		font-family: var(--font-mono);
		font-size: 0.5rem;
		color: oklch(0.78 0.12 75 / 20%);
		width: 28px;
	}

	/* ═══ Mood chart ═══ */

	.mood-chart {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.mood-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.mood-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.mood-name {
		font-family: var(--font-mono);
		font-size: 0.58rem;
		color: oklch(0.88 0.02 75 / 50%);
		width: 72px;
	}

	.mood-track {
		flex: 1;
		height: 6px;
		border-radius: 3px;
		background: oklch(1 0 0 / 3%);
		overflow: hidden;
	}

	.mood-fill {
		height: 100%;
		border-radius: 3px;
		opacity: 0.5;
		transition: width 0.5s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.mood-pct {
		font-family: var(--font-mono);
		font-size: 0.5rem;
		color: oklch(0.78 0.12 75 / 22%);
		width: 28px;
		text-align: right;
	}

	/* ═══ Details grid ═══ */

	.detail-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.75rem;
	}

	.detail-item {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		padding: 0.625rem;
		border-radius: 0.5rem;
		background: oklch(1 0 0 / 2%);
		border: 1px solid oklch(1 0 0 / 4%);
	}

	.detail-value {
		font-family: var(--font-display);
		font-size: 1.1rem;
		font-weight: 300;
		color: oklch(0.78 0.12 75 / 55%);
	}

	.detail-label {
		font-family: var(--font-mono);
		font-size: 0.5rem;
		color: oklch(0.78 0.12 75 / 20%);
		letter-spacing: 0.04em;
	}

	@media (max-width: 640px) {
		.stats-scroll { padding: 1rem; }
		.stats-hero { gap: 1.5rem; }
		.hero-value { font-size: 1.4rem; }
	}
</style>
