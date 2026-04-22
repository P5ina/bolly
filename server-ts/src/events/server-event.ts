/**
 * ServerEvent — the WebSocket wire format. Byte-compatible with the Rust
 * backend's existing serde(rename_all = "snake_case") tagged enum, so the
 * SvelteKit client runs unmodified.
 *
 * This file intentionally defines only the variants Plan 2 emits. Plans 3
 * (outreach), 4 (cross-instance), and 5 (polish) add the remaining variants
 * (MoodUpdated, DropCreated, HeartbeatThought, McpAppStart, etc.) as their
 * features land.
 */
export type ChatMessage = {
  id: string;
  role: "User" | "Assistant";
  content: string;
  created_at: string;
  kind: "Message" | "ToolCall" | "ToolOutput" | "McpApp" | "Compaction";
  tool_name?: string;
  model?: string;
};

export type ServerEvent =
  | {
      type: "chat_message_created";
      instance_slug: string;
      chat_id: string;
      message: ChatMessage;
    }
  | {
      type: "chat_stream_delta";
      instance_slug: string;
      chat_id: string;
      message_id: string;
      delta: string;
    }
  | {
      type: "agent_running";
      instance_slug: string;
      chat_id: string;
    }
  | {
      type: "agent_stopped";
      instance_slug: string;
      chat_id: string;
    }
  | {
      type: "context_compacting";
      instance_slug: string;
      chat_id: string;
      messages_compacted: number;
    }
  | {
      type: "chat_snapshot";
      instance_slug: string;
      chat_id: string;
      messages: ChatMessage[];
      agent_running: boolean;
    };

export function serializeServerEvent(event: ServerEvent): string {
  return JSON.stringify(event);
}
