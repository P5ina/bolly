import { Hono } from "hono";
import { describe, expect, it } from "vitest";
import { requireAuth } from "./auth.js";

function app(authToken: string | undefined) {
  const h = new Hono();
  h.use("*", requireAuth(authToken));
  h.get("/ok", (c) => c.text("ok"));
  return h;
}

describe("requireAuth", () => {
  it("passes through when authToken is undefined (no auth configured)", async () => {
    const res = await app(undefined).request("/ok");
    expect(res.status).toBe(200);
  });

  it("accepts a matching Bearer token", async () => {
    const res = await app("secret").request("/ok", {
      headers: { Authorization: "Bearer secret" },
    });
    expect(res.status).toBe(200);
  });

  it("accepts a matching ?token= query param (for WebSocket)", async () => {
    const res = await app("secret").request("/ok?token=secret");
    expect(res.status).toBe(200);
  });

  it("returns 401 with no token", async () => {
    const res = await app("secret").request("/ok");
    expect(res.status).toBe(401);
  });

  it("returns 401 with a wrong token", async () => {
    const res = await app("secret").request("/ok", {
      headers: { Authorization: "Bearer wrong" },
    });
    expect(res.status).toBe(401);
  });
});
