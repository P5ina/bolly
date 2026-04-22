import { describe, expect, it } from "vitest";
import { type RuntimeConfig, loadConfig } from "./config.js";

describe("loadConfig", () => {
  it("reads required fields from env", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
    });
    expect(cfg.anthropicApiKey).toBe("sk-ant-test");
    expect(cfg.bollyHome).toBe("/tmp/bolly");
  });

  it("defaults http port to 4242", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
    });
    expect(cfg.httpPort).toBe(4242);
  });

  it("reads BOLLY_HTTP_PORT override", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
      BOLLY_HTTP_PORT: "8080",
    });
    expect(cfg.httpPort).toBe(8080);
  });

  it("reads optional BOLLY_AUTH_TOKEN", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
      BOLLY_AUTH_TOKEN: "secret",
    });
    expect(cfg.authToken).toBe("secret");
  });

  it("authToken is undefined when BOLLY_AUTH_TOKEN is unset", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
    });
    expect(cfg.authToken).toBeUndefined();
  });

  it("throws when ANTHROPIC_API_KEY is missing", () => {
    expect(() => loadConfig({ BOLLY_HOME: "/tmp/bolly" })).toThrow(/ANTHROPIC_API_KEY/);
  });

  it("throws when BOLLY_HOME is missing", () => {
    expect(() => loadConfig({ ANTHROPIC_API_KEY: "sk-ant-test" })).toThrow(/BOLLY_HOME/);
  });
});
