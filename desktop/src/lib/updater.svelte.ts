import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

type UpdateState = {
  available: boolean;
  version: string;
  downloading: boolean;
  progress: number;
  error: string | null;
};

export const updater: UpdateState = $state({
  available: false,
  version: "",
  downloading: false,
  progress: 0,
  error: null,
});

let pending: Update | null = null;

export async function checkForUpdates() {
  try {
    const update = await check();
    if (update) {
      pending = update;
      updater.available = true;
      updater.version = update.version;
    }
  } catch (e) {
    console.error("Update check failed:", e);
  }
}

export async function installUpdate() {
  if (!pending) return;
  updater.downloading = true;
  updater.error = null;
  updater.progress = 0;

  try {
    let totalBytes = 0;
    let downloadedBytes = 0;

    await pending.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          totalBytes = event.data.contentLength ?? 0;
          break;
        case "Progress":
          downloadedBytes += event.data.chunkLength;
          updater.progress = totalBytes > 0 ? downloadedBytes / totalBytes : 0;
          break;
        case "Finished":
          updater.progress = 1;
          break;
      }
    });

    await relaunch();
  } catch (e) {
    updater.downloading = false;
    updater.error = e instanceof Error ? e.message : String(e);
  }
}

export function dismissUpdate() {
  updater.available = false;
  pending = null;
}
