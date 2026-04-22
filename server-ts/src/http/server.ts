import { Hono } from "hono";
import type { Broadcaster } from "../events/broadcaster.js";
import type { WorkerPool } from "../mind/pool.js";
import { requireAuth } from "./auth.js";

export type AppOptions = {
  authToken: string | undefined;
  pool: WorkerPool;
  broadcaster: Broadcaster;
};

export function createApp(opts: AppOptions) {
  const app = new Hono();

  app.get("/api/health", (c) => c.json({ ok: true }));

  app.use("/api/*", requireAuth(opts.authToken));

  app.post("/api/chat", async (c) => {
    const body = await c.req.json<{
      instance_slug: string;
      chat_id: string;
      content: string;
    }>();

    const worker = opts.pool.get(body.instance_slug, body.chat_id);
    // Fire-and-forget: the mind turn runs asynchronously and broadcasts
    // progress over WebSocket. The HTTP caller just gets a 202.
    void worker.handleUserMessage(body.content).catch((err) => {
      console.error("mind worker error", err);
    });
    return c.body(null, 202);
  });

  return app;
}
