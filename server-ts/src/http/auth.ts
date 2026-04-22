import type { MiddlewareHandler } from "hono";

/**
 * Bearer-token or ?token= query-param auth.
 * - If authToken is undefined, auth is disabled (dev).
 * - Otherwise, requests must supply the token via either channel.
 */
export function requireAuth(authToken: string | undefined): MiddlewareHandler {
  return async (c, next) => {
    if (!authToken) return next();

    const header = c.req.header("authorization") ?? "";
    const bearer = header.startsWith("Bearer ") ? header.slice(7) : "";
    const query = c.req.query("token") ?? "";

    if (bearer === authToken || query === authToken) {
      return next();
    }
    return c.text("Unauthorized", 401);
  };
}
