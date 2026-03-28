<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let action = $state("");
  let active = $state(true);
  let fadeTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    const unlisten = listen<string>("computer-use-action", (e) => {
      action = e.payload;
      active = true;
      if (fadeTimer) clearTimeout(fadeTimer);
      fadeTimer = setTimeout(() => { action = ""; }, 2000);
    });

    const unlistenDone = listen("computer-use-idle", () => {
      active = false;
    });

    return () => {
      unlisten.then(fn => fn());
      unlistenDone.then(fn => fn());
    };
  });
</script>

<div class="overlay" class:overlay-active={active}>
  <div class="border-top"></div>
  <div class="border-right"></div>
  <div class="border-bottom"></div>
  <div class="border-left"></div>

  {#if action}
    <div class="action-badge">
      <div class="action-dot"></div>
      <span class="action-text">{action}</span>
    </div>
  {/if}

  <div class="corner corner-tl"></div>
  <div class="corner corner-tr"></div>
  <div class="corner corner-bl"></div>
  <div class="corner corner-br"></div>
</div>

<style>
  :global(html), :global(body) {
    background: transparent !important;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  .overlay {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 99999;
    transition: opacity 0.4s ease;
  }

  .overlay:not(.overlay-active) {
    opacity: 0;
  }

  /* ─── Glowing borders ───────────────────────────────────── */
  .border-top, .border-bottom, .border-left, .border-right {
    position: absolute;
    background: linear-gradient(90deg,
      transparent,
      oklch(0.78 0.12 75 / 50%),
      oklch(0.65 0.15 45 / 60%),
      oklch(0.78 0.12 75 / 50%),
      transparent
    );
    background-size: 200% 100%;
    animation: shimmer 3s ease-in-out infinite;
  }

  .border-top {
    top: 0; left: 0; right: 0;
    height: 2px;
    box-shadow: 0 0 12px 2px oklch(0.78 0.12 75 / 30%),
                0 0 30px 4px oklch(0.78 0.12 75 / 12%);
  }

  .border-bottom {
    bottom: 0; left: 0; right: 0;
    height: 2px;
    box-shadow: 0 0 12px 2px oklch(0.78 0.12 75 / 30%),
                0 0 30px 4px oklch(0.78 0.12 75 / 12%);
  }

  .border-left {
    top: 0; bottom: 0; left: 0;
    width: 2px;
    background: linear-gradient(180deg,
      transparent,
      oklch(0.78 0.12 75 / 50%),
      oklch(0.65 0.15 45 / 60%),
      oklch(0.78 0.12 75 / 50%),
      transparent
    );
    background-size: 100% 200%;
    animation: shimmer-v 3s ease-in-out infinite;
    box-shadow: 0 0 12px 2px oklch(0.78 0.12 75 / 30%),
                0 0 30px 4px oklch(0.78 0.12 75 / 12%);
  }

  .border-right {
    top: 0; bottom: 0; right: 0;
    width: 2px;
    background: linear-gradient(180deg,
      transparent,
      oklch(0.78 0.12 75 / 50%),
      oklch(0.65 0.15 45 / 60%),
      oklch(0.78 0.12 75 / 50%),
      transparent
    );
    background-size: 100% 200%;
    animation: shimmer-v 3s ease-in-out infinite;
    box-shadow: 0 0 12px 2px oklch(0.78 0.12 75 / 30%),
                0 0 30px 4px oklch(0.78 0.12 75 / 12%);
  }

  @keyframes shimmer {
    0%, 100% { background-position: 0% 0; }
    50% { background-position: 100% 0; }
  }

  @keyframes shimmer-v {
    0%, 100% { background-position: 0 0%; }
    50% { background-position: 0 100%; }
  }

  /* ─── Corner accents ────────────────────────────────────── */
  .corner {
    position: absolute;
    width: 16px;
    height: 16px;
    border-color: oklch(0.78 0.12 75 / 70%);
    border-style: solid;
    border-width: 0;
  }

  .corner-tl { top: 0; left: 0; border-top-width: 3px; border-left-width: 3px; border-radius: 2px 0 0 0; }
  .corner-tr { top: 0; right: 0; border-top-width: 3px; border-right-width: 3px; border-radius: 0 2px 0 0; }
  .corner-bl { bottom: 0; left: 0; border-bottom-width: 3px; border-left-width: 3px; border-radius: 0 0 0 2px; }
  .corner-br { bottom: 0; right: 0; border-bottom-width: 3px; border-right-width: 3px; border-radius: 0 0 2px 0; }

  /* ─── Action badge ──────────────────────────────────────── */
  .action-badge {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border-radius: 20px;
    background: oklch(0.06 0.015 260 / 85%);
    backdrop-filter: blur(12px);
    border: 1px solid oklch(0.78 0.12 75 / 25%);
    animation: badge-in 0.3s ease both;
  }

  .action-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: oklch(0.78 0.12 75);
    box-shadow: 0 0 8px oklch(0.78 0.12 75 / 60%);
    animation: pulse 1.5s ease-in-out infinite;
  }

  .action-text {
    font-family: system-ui, sans-serif;
    font-size: 11px;
    color: oklch(0.78 0.12 75 / 80%);
    letter-spacing: 0.03em;
  }

  @keyframes badge-in {
    from { opacity: 0; transform: translateX(-50%) translateY(-6px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }
</style>
