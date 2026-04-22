import type { Broadcaster } from "../events/broadcaster.js";
import type { MindClient } from "./anthropic-client.js";
import { MindWorker } from "./worker.js";

export type WorkerPoolOptions = {
  clientFactory: () => MindClient;
  home: string;
  companyName: string;
  model: string;
  pricePerMTokIn: number;
  pricePerMTokOut: number;
  broadcaster: Broadcaster;
  warmTtlMs?: number;
};

export class WorkerPool {
  private readonly workers = new Map<string, MindWorker>();

  constructor(private readonly opts: WorkerPoolOptions) {}

  private key(slug: string, chatId: string): string {
    return `${slug}/${chatId}`;
  }

  get(slug: string, chatId: string): MindWorker {
    const k = this.key(slug, chatId);
    const existing = this.workers.get(k);
    if (existing) return existing;

    const worker = new MindWorker({
      client: this.opts.clientFactory(),
      home: this.opts.home,
      slug,
      chatId,
      companyName: this.opts.companyName,
      model: this.opts.model,
      pricePerMTokIn: this.opts.pricePerMTokIn,
      pricePerMTokOut: this.opts.pricePerMTokOut,
      broadcaster: this.opts.broadcaster,
      ...(this.opts.warmTtlMs !== undefined ? { warmTtlMs: this.opts.warmTtlMs } : {}),
    });
    this.workers.set(k, worker);
    return worker;
  }

  sweepStale(nowMs: number): void {
    for (const [k, w] of this.workers) {
      if (w.isStaleAt(nowMs)) this.workers.delete(k);
    }
  }

  size(): number {
    return this.workers.size;
  }
}
