import { describe, expect, it } from "vitest";
import { type ServerEvent, serializeServerEvent } from "./server-event.js";

describe("serializeServerEvent", () => {
  it("ChatMessageCreated matches the Rust wire format shape", () => {
    const event: ServerEvent = {
      type: "chat_message_created",
      instance_slug: "alice",
      chat_id: "default",
      message: {
        id: "msg_1",
        role: "Assistant",
        content: "hello",
        created_at: "1714000000000",
        kind: "Message",
      },
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.type).toBe("chat_message_created");
    expect(json.instance_slug).toBe("alice");
    expect(json.message.role).toBe("Assistant");
  });

  it("ChatStreamDelta carries message_id + delta", () => {
    const event: ServerEvent = {
      type: "chat_stream_delta",
      instance_slug: "alice",
      chat_id: "default",
      message_id: "msg_2",
      delta: "Hi ",
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.type).toBe("chat_stream_delta");
    expect(json.delta).toBe("Hi ");
  });

  it("AgentRunning / AgentStopped are bare signals", () => {
    const running: ServerEvent = {
      type: "agent_running",
      instance_slug: "alice",
      chat_id: "default",
    };
    const json = JSON.parse(serializeServerEvent(running));
    expect(json).toEqual({
      type: "agent_running",
      instance_slug: "alice",
      chat_id: "default",
    });
  });

  it("ContextCompacting reports how many messages were compacted", () => {
    const event: ServerEvent = {
      type: "context_compacting",
      instance_slug: "alice",
      chat_id: "default",
      messages_compacted: 42,
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.messages_compacted).toBe(42);
  });

  it("ChatSnapshot replaces the whole chat state", () => {
    const event: ServerEvent = {
      type: "chat_snapshot",
      instance_slug: "alice",
      chat_id: "default",
      messages: [],
      agent_running: false,
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.agent_running).toBe(false);
  });
});
