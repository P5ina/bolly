import { describe, expect, it, vi } from "vitest";
import { Broadcaster } from "./broadcaster.js";
import type { ServerEvent } from "./server-event.js";

describe("Broadcaster", () => {
  it("delivers an event to every subscriber", () => {
    const b = new Broadcaster();
    const a = vi.fn();
    const c = vi.fn();
    b.subscribe(a);
    b.subscribe(c);

    const event: ServerEvent = {
      type: "agent_running",
      instance_slug: "alice",
      chat_id: "default",
    };
    b.emit(event);

    expect(a).toHaveBeenCalledWith(event);
    expect(c).toHaveBeenCalledWith(event);
  });

  it("unsubscribe stops delivery", () => {
    const b = new Broadcaster();
    const a = vi.fn();
    const unsub = b.subscribe(a);
    unsub();
    b.emit({ type: "agent_running", instance_slug: "x", chat_id: "y" });
    expect(a).not.toHaveBeenCalled();
  });

  it("isolates subscriber exceptions from other subscribers", () => {
    const b = new Broadcaster();
    b.subscribe(() => {
      throw new Error("boom");
    });
    const ok = vi.fn();
    b.subscribe(ok);
    b.emit({ type: "agent_running", instance_slug: "x", chat_id: "y" });
    expect(ok).toHaveBeenCalled();
  });
});
