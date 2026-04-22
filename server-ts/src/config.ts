export type RuntimeConfig = {
  anthropicApiKey: string;
  bollyHome: string;
  httpPort: number;
  authToken: string | undefined;
};

type Env = Record<string, string | undefined>;

/**
 * Load runtime config from a plain env-like object.
 * Accepts `process.env` at boot; accepts a mock at test time.
 */
export function loadConfig(env: Env): RuntimeConfig {
  const anthropicApiKey = env.ANTHROPIC_API_KEY;
  if (!anthropicApiKey) throw new Error("ANTHROPIC_API_KEY is required");

  const bollyHome = env.BOLLY_HOME;
  if (!bollyHome) throw new Error("BOLLY_HOME is required");

  const port = env.BOLLY_HTTP_PORT ? Number(env.BOLLY_HTTP_PORT) : 4242;
  if (!Number.isFinite(port) || port <= 0) {
    throw new Error(`BOLLY_HTTP_PORT is not a valid port: ${env.BOLLY_HTTP_PORT}`);
  }

  return {
    anthropicApiKey,
    bollyHome,
    httpPort: port,
    authToken: env.BOLLY_AUTH_TOKEN,
  };
}
