export type ThrottleConfig = {
  maxCalls: number;
  windowMs: number;
};

/**
 * Rolling-window per-user throttle. In-memory only — suitable for the
 * single-mind-per-user model where there's one process per user.
 */
export class Throttle {
  private readonly buckets = new Map<string, number[]>();

  constructor(private readonly config: ThrottleConfig) {}

  /**
   * Returns true if the call is allowed (and records it).
   * Returns false if the call would exceed the window's max.
   */
  check(userId: string, nowMs: number): boolean {
    const cutoff = nowMs - this.config.windowMs;
    const existing = this.buckets.get(userId) ?? [];
    const pruned = existing.filter((ts) => ts > cutoff);

    if (pruned.length >= this.config.maxCalls) {
      this.buckets.set(userId, pruned);
      return false;
    }

    pruned.push(nowMs);
    this.buckets.set(userId, pruned);
    return true;
  }
}
