import { describe, expect, it } from "vitest";
import { ContentBlockSchema, type ConversationEntry, ConversationEntrySchema } from "./types.js";

describe("ContentBlockSchema", () => {
  it("accepts a text block", () => {
    const parsed = ContentBlockSchema.parse({ type: "text", text: "hi" });
    expect(parsed).toEqual({ type: "text", text: "hi" });
  });

  it("accepts a tool_use block", () => {
    const parsed = ContentBlockSchema.parse({
      type: "tool_use",
      id: "toolu_1",
      name: "send_push",
      input: { title: "hi" },
    });
    expect(parsed.type).toBe("tool_use");
  });

  it("accepts a tool_result block", () => {
    const parsed = ContentBlockSchema.parse({
      type: "tool_result",
      tool_use_id: "toolu_1",
      content: "done",
    });
    expect(parsed.type).toBe("tool_result");
  });

  it("rejects unknown block types", () => {
    expect(() => ContentBlockSchema.parse({ type: "wut", x: 1 })).toThrow();
  });
});

describe("ConversationEntrySchema", () => {
  it("parses a minimal user entry", () => {
    const entry = {
      id: "msg_1",
      role: "user" as const,
      content: [{ type: "text" as const, text: "hello" }],
      ts: 1714000000000,
    };
    const parsed: ConversationEntry = ConversationEntrySchema.parse(entry);
    expect(parsed.role).toBe("user");
  });

  it("parses an assistant entry with model field", () => {
    const entry = {
      id: "msg_2",
      role: "assistant" as const,
      content: [{ type: "text" as const, text: "hi back" }],
      ts: 1714000001000,
      model: "claude-sonnet-4-6",
    };
    const parsed = ConversationEntrySchema.parse(entry);
    expect(parsed.model).toBe("claude-sonnet-4-6");
  });

  it("rejects entries with invalid role", () => {
    expect(() =>
      ConversationEntrySchema.parse({
        id: "msg_3",
        role: "system",
        content: [],
        ts: 0,
      }),
    ).toThrow();
  });
});
