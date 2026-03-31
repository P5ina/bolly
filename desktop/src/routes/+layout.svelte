<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let { children } = $props();

  onMount(() => {
    function handleClick(e: MouseEvent) {
      const anchor = (e.target as HTMLElement).closest("a");
      if (!anchor) return;

      const href = anchor.getAttribute("href");
      if (!href) return;

      // Open external links, target="_blank", and downloads in the system browser
      const isExternal = href.startsWith("http://") || href.startsWith("https://");
      const isBlank = anchor.getAttribute("target") === "_blank";
      const isDownload = anchor.hasAttribute("download");

      if (isExternal || isBlank || isDownload) {
        e.preventDefault();
        e.stopPropagation();
        openUrl(href);
      }
    }

    // Intercept "Open in New Tab" from context menu and middle-click
    const origOpen = window.open;
    window.open = function (url?: string | URL) {
      if (url) openUrl(String(url));
      return null;
    };

    document.addEventListener("click", handleClick, true);
    return () => {
      document.removeEventListener("click", handleClick, true);
      window.open = origOpen;
    };
  });
</script>

{@render children()}
