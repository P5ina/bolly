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
		return Math.floor((Date.now() - first) / 86400000) + 1;
	});

	let heatmapMax = $derived.by(() => {
		if (!stats) return 1;
		return Math.max(...stats.hourly_activity, 1);
	});

	const MONTH_LABELS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

	function localDateStr(d: Date): string {
		const y = d.getFullYear();
		const m = String(d.getMonth() + 1).padStart(2, "0");
		const day = String(d.getDate()).padStart(2, "0");
		return `${y}-${m}-${day}`;
	}

	let contributionWeeks = $derived.by(() => {
		if (!stats) return [];
		const map = new Map(stats.daily_history.map(([d, c]) => [d, c]));
		const maxCount = Math.max(...stats.daily_history.map(([, c]) => c), 1);
		const weeks: { date: string; count: number; level: number }[][] = [];
		const today = new Date();
		const start = new Date(today);
		const daysSinceMonday = (start.getDay() + 6) % 7;
		start.setDate(start.getDate() - 52 * 7 - daysSinceMonday);

		let currentWeek: { date: string; count: number; level: number }[] = [];
		for (let d = new Date(start); d <= today; d.setDate(d.getDate() + 1)) {
			const ds = localDateStr(d);
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

	let monthLabels = $derived.by(() => {
		if (contributionWeeks.length === 0) return [];
		const labels: { label: string; col: number }[] = [];
		let lastMonth = -1;
		for (let i = 0; i < contributionWeeks.length; i++) {
			const firstDay = contributionWeeks[i][0];
			if (!firstDay) continue;
			const month = new Date(firstDay.date + "T00:00:00").getMonth();
			if (month !== lastMonth) {
				labels.push({ label: MONTH_LABELS[month], col: i });
				lastMonth = month;
			}
		}
		return labels;
	});

	const moodColors: Record<string, string> = {
		calm: "oklch(0.68 0.12 220)", curious: "oklch(0.65 0.14 200)",
		excited: "oklch(0.75 0.14 75)", warm: "oklch(0.75 0.14 75)",
		happy: "oklch(0.72 0.16 145)", playful: "oklch(0.68 0.14 170)",
		thoughtful: "oklch(0.60 0.10 270)", focused: "oklch(0.62 0.12 240)",
		tender: "oklch(0.65 0.14 350)", loving: "oklch(0.62 0.16 340)",
		creative: "oklch(0.65 0.16 300)", energetic: "oklch(0.75 0.14 90)",
		melancholic: "oklch(0.55 0.06 260)", anxious: "oklch(0.65 0.10 50)",
		grateful: "oklch(0.68 0.14 155)", nostalgic: "oklch(0.58 0.08 320)",
	};

	function getMoodColor(mood: string): string {
		return moodColors[mood] ?? "oklch(0.55 0.06 240)";
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
				<div class="hero-card" style="animation-delay: 0ms">
					<span class="hero-value">{stats.total_messages.toLocaleString()}</span>
					<span class="hero-label">messages</span>
				</div>
				<div class="hero-card" style="animation-delay: 60ms">
					<span class="hero-value">{daysSinceFirst}</span>
					<span class="hero-label">days together</span>
				</div>
				<div class="hero-card" style="animation-delay: 120ms">
					<span class="hero-value">{stats.streak_days}</span>
					<span class="hero-label">day streak</span>
					{#if stats.streak_days > 0}
						<div class="streak-glow"></div>
					{/if}
				</div>
			</div>

			<!-- Contribution heatmap -->
			<section class="glass-card" style="animation-delay: 150ms">
				<h3 class="card-title">activity</h3>
				<div class="heatmap-container">
					<div class="heatmap-months">
						{#each monthLabels as { label, col }, i}
							{@const nextCol = i + 1 < monthLabels.length ? monthLabels[i + 1].col : contributionWeeks.length}
							<span
								class="heatmap-month-label"
								style="width: {(nextCol - col) * 11}px"
							>{label}</span>
						{/each}
					</div>
					<div class="heatmap-with-days">
						<div class="heatmap-day-labels">
							<span class="heatmap-day-label"></span>
							<span class="heatmap-day-label">Mon</span>
							<span class="heatmap-day-label"></span>
							<span class="heatmap-day-label">Wed</span>
							<span class="heatmap-day-label"></span>
							<span class="heatmap-day-label">Fri</span>
							<span class="heatmap-day-label"></span>
						</div>
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
			</section>

			<!-- Hourly activity -->
			<section class="glass-card" style="animation-delay: 200ms">
				<h3 class="card-title">
					peak hours
					<span class="card-hint">most active at {HOUR_LABELS[peakHour]}</span>
				</h3>
				<div class="bar-chart">
					{#each stats.hourly_activity as count, i}
						{@const pct = count / heatmapMax}
						<div class="bar-col" title="{HOUR_LABELS[i]}: {count} messages">
							<div
								class="bar-fill"
								class:bar-fill-peak={i === peakHour}
								style="height: {Math.max(pct * 100, 2)}%"
							></div>
							{#if i % 3 === 0}
								<span class="bar-label">{HOUR_LABELS[i]}</span>
							{/if}
						</div>
					{/each}
				</div>
			</section>

			<!-- Day of week -->
			<section class="glass-card" style="animation-delay: 250ms">
				<h3 class="card-title">
					day of week
					<span class="card-hint">{DAY_LABELS[peakDay]} is your day</span>
				</h3>
				<div class="day-bars">
					{#each stats.daily_activity as count, i}
						{@const max = Math.max(...stats.daily_activity, 1)}
						{@const pct = count / max}
						<div class="day-row">
							<span class="day-label" class:day-label-peak={i === peakDay}>{DAY_LABELS[i]}</span>
							<div class="day-track">
								<div class="day-fill" class:day-fill-peak={i === peakDay} style="width: {pct * 100}%"></div>
							</div>
							<span class="day-count">{count}</span>
						</div>
					{/each}
				</div>
			</section>

			<!-- Mood distribution -->
			{#if topMoods.length > 0}
				<section class="glass-card" style="animation-delay: 300ms">
					<h3 class="card-title">mood palette</h3>
					<div class="mood-chart">
						{#each topMoods as [mood, count]}
							{@const pct = (count / totalMoodCount) * 100}
							<div class="mood-row">
								<div class="mood-dot" style="background: {getMoodColor(mood)}; box-shadow: 0 0 8px {getMoodColor(mood)}"></div>
								<span class="mood-name">{mood}</span>
								<div class="mood-track">
									<div class="mood-fill" style="width: {pct}%; background: {getMoodColor(mood)}"></div>
								</div>
								<span class="mood-pct">{pct.toFixed(0)}%</span>
							</div>
						{/each}
					</div>
				</section>
			{/if}

			<!-- Quick stats -->
			<section class="glass-card" style="animation-delay: 350ms">
				<h3 class="card-title">details</h3>
				<div class="detail-grid">
					<div class="detail-item">
						<span class="detail-value">{Math.round(stats.avg_message_length)}</span>
						<span class="detail-label">avg chars/msg</span>
					</div>
					<div class="detail-item">
						<span class="detail-value">{formatInterval(stats.avg_response_interval_secs)}</span>
						<span class="detail-label">avg interval</span>
					</div>
					<div class="detail-item">
						<span class="detail-value">{stats.daily_history.length}</span>
						<span class="detail-label">active days</span>
					</div>
					<div class="detail-item">
						<span class="detail-value">{stats.total_messages > 0 && stats.daily_history.length > 0 ? (stats.total_messages / stats.daily_history.length).toFixed(1) : "0"}</span>
						<span class="detail-label">msgs/day</span>
					</div>
				</div>
			</section>
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
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
	}

	.stats-loading-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(0.55 0.12 220 / 40%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	.stats-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		font-family: var(--font-mono);
		font-size: 0.72rem;
		color: oklch(0.55 0.08 220 / 40%);
	}

	.stats-scroll {
		height: 100%;
		overflow-y: auto;
		padding: 1.5rem;
		max-width: 560px;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		scrollbar-width: none;
	}
	.stats-scroll::-webkit-scrollbar { display: none; }

	/* ═══ Glass card ═══ */

	.glass-card {
		position: relative;
		padding: 1.125rem 1.25rem;
		border-radius: 1rem;
		border: 1px solid oklch(0.5 0.08 220 / 8%);
		border-top-color: oklch(0.6 0.10 220 / 14%);
		background: linear-gradient(
			165deg,
			oklch(0.5 0.06 220 / 6%) 0%,
			oklch(0.4 0.04 230 / 4%) 50%,
			oklch(0.5 0.06 220 / 5%) 100%
		);
		backdrop-filter: blur(16px) saturate(140%) brightness(1.04);
		-webkit-backdrop-filter: blur(16px) saturate(140%) brightness(1.04);
		box-shadow:
			0 2px 16px oklch(0 0 0 / 20%),
			0 8px 32px oklch(0.3 0.06 220 / 6%),
			inset 0 1px 0 oklch(1 0 0 / 5%);
		display: flex;
		flex-direction: column;
		gap: 0.875rem;
		animation: card-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	/* Specular top highlight */
	.glass-card::before {
		content: "";
		position: absolute;
		top: 0;
		left: 15%;
		right: 15%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(0.6 0.10 220 / 20%), transparent);
		pointer-events: none;
	}

	@keyframes card-enter {
		from { opacity: 0; transform: translateY(12px); filter: blur(4px); }
		to { opacity: 1; transform: translateY(0); filter: blur(0px); }
	}

	.card-title {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		font-weight: 400;
		color: oklch(0.70 0.08 220 / 50%);
		letter-spacing: 0.06em;
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
	}

	.card-hint {
		font-family: var(--font-body);
		font-size: 0.68rem;
		color: oklch(0.65 0.06 220 / 35%);
		font-style: italic;
	}

	/* ═══ Hero ═══ */

	.stats-hero {
		display: flex;
		justify-content: center;
		gap: 0.75rem;
		padding: 0.5rem 0;
	}

	.hero-card {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.375rem;
		padding: 1.25rem 0.75rem;
		border-radius: 1rem;
		border: 1px solid oklch(0.5 0.08 220 / 8%);
		border-top-color: oklch(0.6 0.10 220 / 14%);
		background: linear-gradient(
			170deg,
			oklch(0.5 0.06 220 / 6%) 0%,
			oklch(0.4 0.04 230 / 3%) 100%
		);
		backdrop-filter: blur(16px) saturate(140%) brightness(1.04);
		-webkit-backdrop-filter: blur(16px) saturate(140%) brightness(1.04);
		box-shadow:
			0 2px 16px oklch(0 0 0 / 20%),
			0 8px 32px oklch(0.3 0.06 220 / 6%),
			inset 0 1px 0 oklch(1 0 0 / 5%);
		animation: card-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
		overflow: hidden;
	}

	.hero-card::before {
		content: "";
		position: absolute;
		top: 0;
		left: 20%;
		right: 20%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(0.6 0.10 220 / 20%), transparent);
		pointer-events: none;
	}

	.hero-value {
		font-family: var(--font-display);
		font-size: 1.75rem;
		font-weight: 300;
		color: oklch(0.82 0.08 220 / 80%);
		line-height: 1;
	}

	.hero-label {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.65 0.06 220 / 40%);
		letter-spacing: 0.06em;
	}

	.streak-glow {
		position: absolute;
		inset: 0;
		border-radius: inherit;
		background: radial-gradient(
			ellipse at 50% 30%,
			oklch(0.60 0.14 220 / 8%) 0%,
			transparent 70%
		);
		pointer-events: none;
		animation: streak-pulse 3s ease-in-out infinite;
	}

	@keyframes streak-pulse {
		0%, 100% { opacity: 0.5; }
		50% { opacity: 1; }
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

	.heatmap-months {
		display: flex;
		padding-left: 28px;
		overflow: hidden;
	}

	.heatmap-month-label {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.65 0.06 220 / 35%);
		flex-shrink: 0;
	}

	.heatmap-with-days {
		display: flex;
		gap: 3px;
	}

	.heatmap-day-labels {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex-shrink: 0;
		width: 24px;
	}

	.heatmap-day-label {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.06 220 / 35%);
		height: 9px;
		line-height: 9px;
		text-align: right;
	}

	.heatmap-col {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.heatmap-cell {
		width: 9px;
		height: 9px;
		border-radius: 2.5px;
		transition: background 0.2s ease;
	}

	.heatmap-0 { background: oklch(0.4 0.04 220 / 8%); }
	.heatmap-1 { background: oklch(0.55 0.12 220 / 20%); }
	.heatmap-2 { background: oklch(0.58 0.14 220 / 38%); }
	.heatmap-3 { background: oklch(0.62 0.16 220 / 55%); }
	.heatmap-4 { background: oklch(0.68 0.18 220 / 75%); box-shadow: 0 0 4px oklch(0.60 0.14 220 / 25%); }

	.heatmap-legend {
		display: flex;
		align-items: center;
		gap: 3px;
		justify-content: flex-end;
	}

	.heatmap-legend-label {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.06 220 / 30%);
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
		height: 88px;
		padding-bottom: 16px;
		position: relative;
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
		background: oklch(0.58 0.14 220 / 35%);
		min-height: 2px;
		transition: height 0.4s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.bar-fill-peak {
		background: oklch(0.65 0.18 220 / 65%);
		box-shadow: 0 0 8px oklch(0.60 0.14 220 / 20%);
	}

	.bar-label {
		position: absolute;
		bottom: -14px;
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.06 220 / 30%);
	}

	/* ═══ Day bars ═══ */

	.day-bars {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.day-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.day-label {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.65 0.06 220 / 35%);
		width: 28px;
		text-align: right;
		transition: color 0.2s ease;
	}

	.day-label-peak {
		color: oklch(0.72 0.10 220 / 65%);
	}

	.day-track {
		flex: 1;
		height: 8px;
		border-radius: 4px;
		background: oklch(0.4 0.04 220 / 8%);
		overflow: hidden;
	}

	.day-fill {
		height: 100%;
		border-radius: 4px;
		background: oklch(0.58 0.14 220 / 40%);
		transition: width 0.5s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.day-fill-peak {
		background: oklch(0.65 0.18 220 / 60%);
		box-shadow: 0 0 6px oklch(0.60 0.14 220 / 15%);
	}

	.day-count {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.06 220 / 30%);
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
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.mood-name {
		font-family: var(--font-mono);
		font-size: 0.68rem;
		color: oklch(0.80 0.02 220 / 50%);
		width: 72px;
	}

	.mood-track {
		flex: 1;
		height: 6px;
		border-radius: 3px;
		background: oklch(0.4 0.04 220 / 8%);
		overflow: hidden;
	}

	.mood-fill {
		height: 100%;
		border-radius: 3px;
		opacity: 0.55;
		transition: width 0.5s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.mood-pct {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.06 220 / 35%);
		width: 28px;
		text-align: right;
	}

	/* ═══ Details grid ═══ */

	.detail-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.625rem;
	}

	.detail-item {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		padding: 0.75rem;
		border-radius: 0.625rem;
		background: oklch(0.4 0.04 220 / 6%);
		border: 1px solid oklch(0.5 0.06 220 / 6%);
	}

	.detail-value {
		font-family: var(--font-display);
		font-size: 1.1rem;
		font-weight: 300;
		color: oklch(0.75 0.10 220 / 65%);
	}

	.detail-label {
		font-family: var(--font-mono);
		font-size: 0.62rem;
		color: oklch(0.65 0.06 220 / 30%);
		letter-spacing: 0.04em;
	}

	/* ═══ Responsive ═══ */

	@media (max-width: 640px) {
		.stats-scroll { padding: 1rem; }
		.stats-hero { gap: 0.5rem; }
		.hero-value { font-size: 1.4rem; }
		.hero-card { padding: 1rem 0.5rem; }
		.glass-card { padding: 1rem; }
	}
</style>
