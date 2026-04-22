import type { WSContext, WSEvents } from "hono/ws";
import type { Broadcaster } from "../events/broadcaster.js";
import { serializeServerEvent } from "../events/server-event.js";

// Augment the raw socket type to hold the unsubscribe cleanup function.
type RawWithUnsub = { _unsub?: () => void };

/**
 * Wire a Hono WebSocket to the Broadcaster. Returns a factory usable with
 * @hono/node-ws's upgradeWebSocket.
 *
 * Auth is handled by the upgrade path (middleware + ?token= query param).
 *
 * Note: generic param is `unknown` because the concrete WebSocket type lives
 * in the `ws` package which is not a direct dependency. The call site in
 * server.ts casts to the correct type expected by createNodeWebSocket.
 */
export function wsHandler(broadcaster: Broadcaster): () => WSEvents<unknown> {
  return () => ({
    onOpen(_evt: unknown, ws: WSContext<unknown>) {
      const unsub = broadcaster.subscribe((event) => {
        try {
          ws.send(serializeServerEvent(event));
        } catch {
          unsub();
          ws.close();
        }
      });
      // Stash the unsubscribe fn on the raw socket so onClose/onError can call it.
      const raw = ws.raw as RawWithUnsub | undefined;
      if (raw) raw._unsub = unsub;
    },
    onClose(_evt: unknown, ws: WSContext<unknown>) {
      const raw = ws.raw as RawWithUnsub | undefined;
      raw?._unsub?.();
    },
    onError(_evt: unknown, ws: WSContext<unknown>) {
      const raw = ws.raw as RawWithUnsub | undefined;
      raw?._unsub?.();
    },
    onMessage: () => {
      // Clients don't need to send messages over WS — chat goes through POST /api/chat
    },
  });
}
