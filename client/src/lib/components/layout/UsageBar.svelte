<script lang="ts">
	import { fetchUsage } from "$lib/api/client.js";
	import type { Usage } from "$lib/api/types.js";
	import MessageSquare from "@lucide/svelte/icons/message-square";
	import Zap from "@lucide/svelte/icons/zap";

	let usage: Usage | null = $state(null);

	async function load() {
		try {
			usage = await fetchUsage();
		} catch {
			usage = null;
		}
	}

	$effect(() => {
		load();
		const interval = setInterval(load, 60_000);
		return () => clearInterval(interval);
	});

	function pct(used: number, limit: number): number {
		if (limit <= 0) return 0;
		return Math.min(100, Math.round((used / limit) * 100));
	}

	function barColor(p: number): string {
		if (p >= 100) return "bg-red-400/70";
		if (p >= 80) return "bg-amber-400/70";
		return "bg-warm/60";
	}

	function formatTokens(n: number): string {
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
		return String(n);
	}
</script>

{#if usage}
	{@const msgPct = pct(usage.messages_today, usage.messages_limit)}
	{@const tokPct = pct(usage.tokens_this_month, usage.tokens_limit)}
	{@const msgUnlimited = usage.messages_limit < 0}
	{@const tokUnlimited = usage.tokens_limit < 0}

	<div class="space-y-2.5 px-5 py-3.5">
		{#if !msgUnlimited}
			<div class="space-y-1">
				<div class="flex items-center justify-between text-[10px] text-muted-foreground/60">
					<span class="flex items-center gap-1">
						<MessageSquare class="h-2.5 w-2.5" />
						messages
					</span>
					<span>{usage.messages_today} / {usage.messages_limit}</span>
				</div>
				<div class="h-1 rounded-full bg-muted/30 overflow-hidden">
					<div
						class="h-full rounded-full transition-all duration-500 {barColor(msgPct)}"
						style="width: {msgPct}%"
					></div>
				</div>
			</div>
		{/if}

		{#if !tokUnlimited}
			<div class="space-y-1">
				<div class="flex items-center justify-between text-[10px] text-muted-foreground/60">
					<span class="flex items-center gap-1">
						<Zap class="h-2.5 w-2.5" />
						tokens
					</span>
					<span>{formatTokens(usage.tokens_this_month)} / {formatTokens(usage.tokens_limit)}</span>
				</div>
				<div class="h-1 rounded-full bg-muted/30 overflow-hidden">
					<div
						class="h-full rounded-full transition-all duration-500 {barColor(tokPct)}"
						style="width: {tokPct}%"
					></div>
				</div>
			</div>
		{/if}

		{#if msgUnlimited && tokUnlimited}
			<div class="flex items-center gap-1.5 text-[10px] text-muted-foreground/50">
				<Zap class="h-2.5 w-2.5" />
				<span>unlimited</span>
			</div>
		{/if}
	</div>
{/if}
