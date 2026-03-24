#!/usr/bin/env python3
"""
Simulate cache expiration costs based on user messaging patterns.
Compares 5-minute vs 1-hour TTL at different message frequencies.
"""

CONTEXT_TOKENS = 150_000  # typical context size
NEW_TOKENS_PER_TURN = 315  # new content per turn
OUTPUT_TOKENS = 150

# Pricing per MTok (Auto 70/30 weighted)
PRICE = {
    "5min": {"write": 4.50, "read": 0.36},   # 1.25x base write
    "1hour": {"write": 7.20, "read": 0.36},   # 2x base write
}
OUTPUT_PRICE = 18.0  # weighted

def mtok(t): return t / 1_000_000

def simulate_session(msgs_per_session: int, gap_minutes: float, ttl_minutes: float,
                     sessions_per_day: int, label: str):
    """Simulate a day of usage with bursts of messages separated by gaps."""
    total_read = 0
    total_write = 0
    total_output = 0
    cache_warm = False
    total_turns = msgs_per_session * sessions_per_day

    for session in range(sessions_per_day):
        for msg in range(msgs_per_session):
            ctx = CONTEXT_TOKENS + NEW_TOKENS_PER_TURN * (session * msgs_per_session + msg)

            if not cache_warm:
                # Cache miss — everything is a write
                total_write += ctx
                cache_warm = True
            else:
                # Cache hit — old content is read, new is write
                total_read += ctx - NEW_TOKENS_PER_TURN
                total_write += NEW_TOKENS_PER_TURN

            total_output += OUTPUT_TOKENS

        # Gap between sessions — does cache expire?
        if gap_minutes > ttl_minutes:
            cache_warm = False

    return total_read, total_write, total_output, total_turns


print("=" * 75)
print("Cache TTL cost comparison — daily cost per user")
print("=" * 75)
print(f"Context size: {CONTEXT_TOKENS:,} tokens | New per turn: {NEW_TOKENS_PER_TURN}")
print()

patterns = [
    # (label, msgs_per_session, gap_minutes, sessions_per_day)
    ("Active chatter (10 msgs, no gaps)",        10, 1,   1),
    ("Burst user (5 msgs, 3 min gaps, 3x/day)",  5, 3,   3),
    ("Burst user (5 msgs, 10 min gaps, 3x/day)", 5, 10,  3),
    ("Burst user (5 msgs, 30 min gaps, 3x/day)", 5, 30,  3),
    ("Casual (3 msgs, 1 hour gaps, 4x/day)",     3, 60,  4),
    ("Sporadic (2 msgs, 2 hour gaps, 3x/day)",   2, 120, 3),
    ("Once a day (1 msg)",                        1, 999, 1),
]

print(f"{'Pattern':<50} {'Turns':>5}  {'5min TTL':>10}  {'1hr TTL':>10}  {'Savings':>8}")
print("-" * 90)

for label, mps, gap, spd in patterns:
    costs = {}
    for ttl_label, ttl_min in [("5min", 5), ("1hour", 60)]:
        p = PRICE[ttl_label]
        read, write, output, turns = simulate_session(mps, gap, ttl_min, spd, label)
        cost = (mtok(read) * p["read"]
                + mtok(write) * p["write"]
                + mtok(output) * OUTPUT_PRICE)
        costs[ttl_label] = cost
        if ttl_label == "5min":
            total_turns = turns

    saving = (1 - costs["1hour"] / costs["5min"]) * 100 if costs["5min"] > 0 else 0
    better = "1hr wins" if saving > 0 else "5min wins"

    print(f"{label:<50} {total_turns:>5}  ${costs['5min']:>8.3f}  ${costs['1hour']:>8.3f}  {saving:>+6.1f}%")

print()
print("=" * 75)
print("Breakdown: Burst user (5 msgs, 10 min gaps, 3x/day)")
print("=" * 75)

for ttl_label, ttl_min in [("5min", 5), ("1hour", 60)]:
    p = PRICE[ttl_label]
    read, write, output, turns = simulate_session(5, 10, ttl_min, 3, "")

    cost_read = mtok(read) * p["read"]
    cost_write = mtok(write) * p["write"]
    cost_output = mtok(output) * OUTPUT_PRICE

    print(f"\n  {ttl_label} TTL:")
    print(f"    Cache reads:  {read/1e6:>6.1f}M tokens  ${cost_read:.3f}")
    print(f"    Cache writes: {write/1e6:>6.1f}M tokens  ${cost_write:.3f}")
    print(f"    Output:       {output/1e3:>6.1f}K tokens  ${cost_output:.3f}")
    print(f"    TOTAL:                       ${cost_read + cost_write + cost_output:.3f}/day")

print()
print("=" * 75)
print("Monthly cost projection (30 days)")
print("=" * 75)
print(f"\n{'Pattern':<50} {'5min/mo':>10}  {'1hr/mo':>10}")
print("-" * 75)

for label, mps, gap, spd in patterns:
    costs = {}
    for ttl_label, ttl_min in [("5min", 5), ("1hour", 60)]:
        p = PRICE[ttl_label]
        read, write, output, _ = simulate_session(mps, gap, ttl_min, spd, label)
        cost = (mtok(read) * p["read"] + mtok(write) * p["write"] + mtok(output) * OUTPUT_PRICE)
        costs[ttl_label] = cost * 30

    print(f"{label:<50} ${costs['5min']:>8.2f}  ${costs['1hour']:>8.2f}")

# ═══════════════════════════════════════════════════════════════
# Full journey: cost to reach 1M tokens with cache expiration
# ═══════════════════════════════════════════════════════════════

print()
print("=" * 75)
print("FULL JOURNEY: 0 → 1M tokens with cache expiration")
print("=" * 75)

SYSTEM_TOOLS = 38_000
TARGET = 1_000_000
TOKENS_PER_MSG = 158  # from real data (700 msgs → 110K msg tokens)
MSGS_PER_TURN = 2
TOKENS_PER_TURN = TOKENS_PER_MSG * MSGS_PER_TURN
TARGET_MSG_TOKENS = TARGET - SYSTEM_TOOLS
TOTAL_TURNS = int(TARGET_MSG_TOKENS / TOKENS_PER_TURN)

# Messaging patterns: (label, avg_gap_minutes, msgs_per_session, sessions_per_day)
journey_patterns = [
    ("Always active (no gaps > TTL)",     2,  99, 1),
    ("Burst 5 msgs, 10 min gaps, 3x/day", 10, 5,  3),
    ("Burst 5 msgs, 30 min gaps, 3x/day", 30, 5,  3),
    ("Casual 3 msgs, 1hr gaps, 4x/day",  60,  3,  4),
    ("Sporadic 2 msgs, 2hr gaps, 3x/day", 120, 2, 3),
]

for ttl_label, ttl_min in [("5min", 5), ("1hour", 60)]:
    p = PRICE[ttl_label]
    print(f"\n--- TTL: {ttl_label} (write: ${p['write']}/MTok, read: ${p['read']}/MTok) ---")
    print(f"{'Pattern':<47} {'Turns':>5} {'Expirations':>11} {'Cost':>10} {'$/turn':>8}")
    print("-" * 85)

    for label, gap_min, mps, spd in journey_patterns:
        total_read = 0
        total_write = 0
        total_output = 0
        expirations = 0
        cache_warm = False
        msg_in_session = 0

        for turn in range(TOTAL_TURNS):
            ctx = SYSTEM_TOOLS + TOKENS_PER_TURN * (turn + 1)
            new = TOKENS_PER_TURN

            # Check if this is a new session (gap between sessions)
            msg_in_session += 1
            if msg_in_session > mps:
                msg_in_session = 1
                if gap_min > ttl_min:
                    cache_warm = False
                    expirations += 1

            if not cache_warm:
                total_write += ctx
                cache_warm = True
            else:
                total_read += ctx - new
                total_write += new

            total_output += OUTPUT_TOKENS

        cost = (mtok(total_read) * p["read"]
                + mtok(total_write) * p["write"]
                + mtok(total_output) * OUTPUT_PRICE)
        per_turn = cost / TOTAL_TURNS

        print(f"{label:<47} {TOTAL_TURNS:>5} {expirations:>11} ${cost:>8.1f} ${per_turn:>.4f}")

print(f"\n  Total turns to 1M: {TOTAL_TURNS:,}")
print(f"  At 15 msgs/day ≈ {TOTAL_TURNS * 2 / 15:.0f} days ≈ {TOTAL_TURNS * 2 / 15 / 30:.0f} months")
