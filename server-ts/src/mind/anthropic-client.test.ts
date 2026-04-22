import { describe, expect, it } from "vitest";
import { type MindClient, createAnthropicClient } from "./anthropic-client.js";

describe("createAnthropicClient", () => {
  it("returns an object with messages.create and messages.stream methods", () => {
    const client: MindClient = createAnthropicClient("sk-ant-test");
    expect(typeof client.messages.create).toBe("function");
    expect(typeof client.messages.stream).toBe("function");
  });
});
