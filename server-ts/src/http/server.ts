import { createNodeWebSocket } from "@hono/node-ws";
import { Hono } from "hono";
import type { WSEvents } from "hono/ws";
import type { Broadcaster } from "../events/broadcaster.js";
import type { WorkerPool } from "../mind/pool.js";
import { requireAuth } from "./auth.js";
import { wsHandler } from "./ws.js";

export type AppOptions = {
  authToken: string | undefined;
  pool: WorkerPool;
  broadcaster: Broadcaster;
};

export function createApp(opts: AppOptions) {
  const app = new Hono();
  const { injectWebSocket, upgradeWebSocket } = createNodeWebSocket({ app });

  app.get("/api/health", (c) => c.json({ ok: true }));

  app.use("/api/*", requireAuth(opts.authToken));

  app.post("/api/chat", async (c) => {
    const body = await c.req.json<{
      instance_slug: string;
      chat_id: string;
      content: string;
    }>();

    const worker = opts.pool.get(body.instance_slug, body.chat_id);
    void worker.handleUserMessage(body.content).catch((err) => {
      console.error("mind worker error", err);
    });
    return c.body(null, 202);
  });

  // Cast needed because wsHandler uses WSEvents<unknown> to avoid importing
  // the `ws` package type — upgradeWebSocket expects WSEvents<WebSocket>.
  // biome-ignore lint/suspicious/noExplicitAny: bridging wsHandler's unknown generic to node-ws WebSocket
  app.get("/api/ws", upgradeWebSocket(wsHandler(opts.broadcaster) as () => WSEvents<any>));

  return { app, injectWebSocket };
}
