import type { ServerEvent } from "./server-event.js";

export type Subscriber = (event: ServerEvent) => void;

/**
 * In-process pub-sub for ServerEvents. WebSocket handlers call subscribe()
 * on connect; the mind worker calls emit() as events fire.
 * Subscriber exceptions are caught so one bad client can't break others.
 */
export class Broadcaster {
  private readonly subs = new Set<Subscriber>();

  subscribe(fn: Subscriber): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }

  emit(event: ServerEvent): void {
    for (const fn of this.subs) {
      try {
        fn(event);
      } catch (err) {
        console.error("Broadcaster: subscriber threw", err);
      }
    }
  }
}
