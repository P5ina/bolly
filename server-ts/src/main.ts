import { serve } from "@hono/node-server";
import { loadConfig } from "./config.js";
import { Broadcaster } from "./events/broadcaster.js";
import { createApp } from "./http/server.js";
import { createAnthropicClient } from "./mind/anthropic-client.js";
import { WorkerPool } from "./mind/pool.js";

const DEFAULT_MODEL = "claude-sonnet-4-6";
const SONNET_PRICE_IN = 3.0; // $/M input tokens
const SONNET_PRICE_OUT = 15.0; // $/M output tokens
const STALE_SWEEP_MS = 60_000; // once a minute

function getCompanyName(env: NodeJS.ProcessEnv): string {
  return env.BOLLY_COMPANY_NAME ?? "your company";
}

async function main() {
  const config = loadConfig(process.env as Record<string, string | undefined>);

  const broadcaster = new Broadcaster();
  const pool = new WorkerPool({
    clientFactory: () => createAnthropicClient(config.anthropicApiKey),
    home: config.bollyHome,
    companyName: getCompanyName(process.env),
    model: DEFAULT_MODEL,
    pricePerMTokIn: SONNET_PRICE_IN,
    pricePerMTokOut: SONNET_PRICE_OUT,
    broadcaster,
  });

  const { app, injectWebSocket } = createApp({
    authToken: config.authToken,
    pool,
    broadcaster,
  });

  const server = serve({
    fetch: app.fetch,
    port: config.httpPort,
  });
  // biome-ignore lint/suspicious/noExplicitAny: @hono/node-server server type doesn't exactly match @hono/node-ws expected param
  injectWebSocket(server as any);

  const sweepInterval = setInterval(() => pool.sweepStale(Date.now()), STALE_SWEEP_MS);

  const shutdown = (signal: string) => {
    console.log(`received ${signal}, shutting down`);
    clearInterval(sweepInterval);
    server.close(() => {
      process.exit(0);
    });
    setTimeout(() => process.exit(1), 10_000).unref();
  };

  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("SIGINT", () => shutdown("SIGINT"));

  console.log(`Bolly v1.0 listening on port ${config.httpPort}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
