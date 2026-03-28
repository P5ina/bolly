import { invoke } from "@tauri-apps/api/core";

/**
 * Handle a computer_use_request from the server WebSocket.
 * Executes the action via Tauri commands and returns the result
 * to post back to the server.
 */
export async function handleComputerUse(event: {
  action: string;
  coordinate?: [number, number];
  text?: string;
  key?: string;
  scroll_delta?: [number, number];
}): Promise<
  | { type: "screenshot"; image: string; width: number; height: number; scale: number }
  | { type: "action"; success: boolean; error?: string }
> {
  try {
    switch (event.action) {
      case "screenshot": {
        const result = await invoke<{
          image: string;
          width: number;
          height: number;
          scale: number;
        }>("computer_screenshot");
        return { type: "screenshot", ...result };
      }

      case "left_click": {
        const [x, y] = event.coordinate ?? [0, 0];
        const scale = await getScale();
        await invoke("computer_click", { x, y, scale, button: "left" });
        return { type: "action", success: true };
      }

      case "right_click": {
        const [x, y] = event.coordinate ?? [0, 0];
        const scale = await getScale();
        await invoke("computer_click", { x, y, scale, button: "right" });
        return { type: "action", success: true };
      }

      case "middle_click": {
        const [x, y] = event.coordinate ?? [0, 0];
        const scale = await getScale();
        await invoke("computer_click", { x, y, scale, button: "middle" });
        return { type: "action", success: true };
      }

      case "double_click": {
        const [x, y] = event.coordinate ?? [0, 0];
        const scale = await getScale();
        await invoke("computer_double_click", { x, y, scale });
        return { type: "action", success: true };
      }

      case "mouse_move": {
        const [x, y] = event.coordinate ?? [0, 0];
        const scale = await getScale();
        await invoke("computer_mouse_move", { x, y, scale });
        return { type: "action", success: true };
      }

      case "type": {
        await invoke("computer_type", { text: event.text ?? "" });
        return { type: "action", success: true };
      }

      case "key": {
        await invoke("computer_key", { key: event.key ?? "" });
        return { type: "action", success: true };
      }

      case "scroll": {
        const [x, y] = event.coordinate ?? [0, 0];
        const [dx, dy] = event.scroll_delta ?? [0, -3];
        const scale = await getScale();
        await invoke("computer_scroll", {
          x,
          y,
          scale,
          deltaX: dx,
          deltaY: dy,
        });
        return { type: "action", success: true };
      }

      default:
        return { type: "action", success: false, error: `unknown action: ${event.action}` };
    }
  } catch (err) {
    return {
      type: "action",
      success: false,
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

// Cache the scale factor from the last screenshot.
// The server should always take a screenshot before sending coordinate actions,
// so this will be populated.
let cachedScale = 1.0;

async function getScale(): Promise<number> {
  return cachedScale;
}

/**
 * Call this after a screenshot to cache the scale factor for subsequent
 * coordinate-based actions.
 */
export function setScale(scale: number) {
  cachedScale = scale;
}
