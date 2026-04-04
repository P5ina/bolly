<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let action = $state("");
  let recording = $state(false);
  let visible = $state(false);
  let skin = $state("orb");
  let actionQueue = $state<{ id: number; text: string; icon: string }[]>([]);
  let idCounter = 0;

  const skinAvatars: Record<string, string> = {
    orb: "/bolly-avatar-orb.webp",
    mint: "/bolly-avatar-mint.png",
  };

  const avatarSrc = $derived(skinAvatars[skin] ?? skinAvatars.orb);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function resetHideTimer() {
    if (hideTimer) clearTimeout(hideTimer);
    // Hide overlay 10s after last action (unless recording)
    hideTimer = setTimeout(() => {
      if (!recording) visible = false;
    }, 10000);
  }

  const actionIcons: Record<string, string> = {
    screenshot: "\u{1F4F7}",
    left_click: "\u{1F5B1}\uFE0F",
    right_click: "\u{1F5B1}\uFE0F",
    middle_click: "\u{1F5B1}\uFE0F",
    double_click: "\u{1F5B1}\uFE0F\u{1F5B1}\uFE0F",
    mouse_move: "\u2197\uFE0F",
    scroll: "\u2195\uFE0F",
    type: "\u2328\uFE0F",
    key: "\u2318",
    bash: "\u{1F4BB}",
    switch_desktop: "\u{1F5A5}\uFE0F",
    file_read: "\u{1F4C4}",
    file_write: "\u{1F4DD}",
    file_list: "\u{1F4C2}",
  };

  const actionLabels: Record<string, string> = {
    screenshot: "screenshot",
    left_click: "click",
    right_click: "right click",
    middle_click: "middle click",
    double_click: "double click",
    mouse_move: "move",
    scroll: "scroll",
    type: "typing",
    key: "key",
    bash: "command",
    switch_desktop: "switch space",
    file_read: "read file",
    file_write: "write file",
    file_list: "list files",
  };

  function flashAction(name: string, detail: string) {
    const id = ++idCounter;
    const icon = actionIcons[name] ?? "\u26A1";
    const label = actionLabels[name] ?? name;
    const text = detail ? `${label}: ${detail}` : label;
    actionQueue = [...actionQueue, { id, text, icon }];
    setTimeout(() => {
      actionQueue = actionQueue.filter(a => a.id !== id);
    }, 3000);
  }

  onMount(async () => {
    try {
      const isRec = await invoke<boolean>("get_screen_recording_allowed");
      if (isRec) recording = true;
    } catch {}

    const unlisten = listen<string>("computer-use-action", (e) => {
      try {
        const data = JSON.parse(e.payload);
        action = data.action ?? "";
        visible = true;
        flashAction(data.action ?? "", data.detail ?? "");
      } catch {
        action = e.payload;
        visible = true;
        flashAction(e.payload, "");
      }
      resetHideTimer();
    });

    const unlistenDone = listen("computer-use-idle", () => {
      if (!recording) visible = false;
    });

    const unlistenRec = listen<boolean>("screen-recording-state", (e) => {
      recording = e.payload;
      if (e.payload) visible = true;
    });

    const unlistenSkin = listen<string>("overlay-skin", (e) => {
      skin = e.payload;
    });

    return () => {
      unlisten.then(fn => fn());
      unlistenDone.then(fn => fn());
      unlistenRec.then(fn => fn());
    };
  });
</script>

<div class="overlay" class:overlay-visible={visible || recording}>
  <!-- Avatar pip -->
  <div class="pip" class:pip-recording={recording}>
    {#if recording}
      <div class="pip-ring"></div>
      <div class="pip-ring pip-ring-2"></div>
    {/if}
    <img class="pip-avatar" src="/bolly-avatar.png" alt="Bolly" />
    {#if recording}
      <div class="pip-rec">
        <div class="pip-rec-dot"></div>
      </div>
    {/if}
  </div>

  <!-- Action flashes — stack above the pip -->
  <div class="flash-stack">
    {#each actionQueue as flash (flash.id)}
      <div class="flash">
        <span class="flash-icon">{flash.icon}</span>
        <span class="flash-text">{flash.text}</span>
      </div>
    {/each}
  </div>
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
    opacity: 0;
    transition: opacity 0.5s ease;
  }

  .overlay-visible {
    opacity: 1;
  }

  /* ─── Avatar pip ────────────────────────────────────────── */
  .pip {
    position: absolute;
    bottom: 20px;
    right: 20px;
    width: 52px;
    height: 52px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: pip-in 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  @keyframes pip-in {
    from { opacity: 0; transform: scale(0.5) translateY(10px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }

  .pip-avatar {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    object-fit: cover;
    object-position: center 15%;
    background: oklch(0.08 0.02 160 / 90%);
    border: 2px solid oklch(0.75 0.12 160 / 50%);
    box-shadow: 0 2px 12px oklch(0 0 0 / 50%),
                0 0 20px oklch(0.70 0.12 160 / 15%);
    animation: breathe 4s ease-in-out infinite;
  }

  @keyframes breathe {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.03); }
  }

  /* Recording ring pulse */
  .pip-ring {
    position: absolute;
    inset: -4px;
    border-radius: 50%;
    border: 2px solid oklch(0.65 0.22 25 / 60%);
    animation: ring-pulse 2s ease-in-out infinite;
  }

  .pip-ring-2 {
    inset: -8px;
    border-color: oklch(0.65 0.22 25 / 25%);
    animation-delay: 0.5s;
  }

  @keyframes ring-pulse {
    0%, 100% { transform: scale(1); opacity: 0.6; }
    50% { transform: scale(1.08); opacity: 0.2; }
  }

  .pip-recording .pip-avatar {
    border-color: oklch(0.65 0.22 25 / 60%);
    box-shadow: 0 2px 12px oklch(0 0 0 / 50%),
                0 0 24px oklch(0.65 0.22 25 / 20%);
  }

  /* REC dot */
  .pip-rec {
    position: absolute;
    top: -2px;
    right: -2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: oklch(0.10 0.02 25 / 90%);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px oklch(0 0 0 / 40%);
  }

  .pip-rec-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: oklch(0.62 0.25 25);
    box-shadow: 0 0 8px oklch(0.62 0.25 25 / 80%);
    animation: rec-pulse 1.5s ease-in-out infinite;
  }

  @keyframes rec-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.85); }
  }

  /* ─── Action flash stack ───────────────────────────────── */
  .flash-stack {
    position: absolute;
    bottom: 78px;
    right: 12px;
    display: flex;
    flex-direction: column-reverse;
    gap: 6px;
    align-items: flex-end;
  }

  .flash {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: 12px;
    background: oklch(0.10 0.02 260 / 88%);
    backdrop-filter: blur(16px) saturate(120%);
    border: 1px solid oklch(1 0 0 / 10%);
    box-shadow: 0 4px 16px oklch(0 0 0 / 40%),
                0 0 0 0.5px oklch(1 0 0 / 5%);
    animation: flash-in 3s ease both;
    white-space: nowrap;
  }

  @keyframes flash-in {
    0% {
      opacity: 0;
      transform: translateX(16px);
    }
    8% {
      opacity: 1;
      transform: translateX(0);
    }
    80% {
      opacity: 1;
      transform: translateX(0);
    }
    100% {
      opacity: 0;
      transform: translateX(8px);
    }
  }

  .flash-icon {
    font-size: 13px;
    line-height: 1;
  }

  .flash-text {
    font-family: "SF Mono", "JetBrains Mono", ui-monospace, monospace;
    font-size: 11px;
    font-weight: 500;
    color: oklch(0.85 0.03 75);
    letter-spacing: 0.02em;
  }
</style>
