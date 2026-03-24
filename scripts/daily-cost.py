#!/usr/bin/env python3
"""
Daily cost for a user with ~200K token context sending 50 messages/day.
All scenarios: model modes, TTLs, messaging patterns.
"""

# ── Constants ──
CONTEXT_BASE = 200_000       # starting context (system + tools + history)
TOKENS_PER_TURN = 315        # new content per round trip (user + assistant)
TURNS_PER_DAY = 50
OUTPUT_PER_TURN = 150        # avg output tokens

# ── Pricing ($/MTok) ──
MODELS = {
    "Opus 4.6": {
        "base": 5.00, "output": 25.00,
        "5min": {"write": 6.25, "read": 0.50},
        "1hr":  {"write": 10.00, "read": 0.50},
    },
    "Sonnet 4.6": {
        "base": 3.00, "output": 15.00,
        "5min": {"write": 3.75, "read": 0.30},
        "1hr":  {"write": 6.00, "read": 0.30},
    },
    "Auto 70/30": {
        "base": 3.60, "output": 18.00,
        "5min": {"write": 4.50, "read": 0.36},
        "1hr":  {"write": 7.20, "read": 0.36},
    },
}

def mtok(t): return t / 1_000_000

def simulate_day(model: dict, ttl: str, pattern: list[int]) -> dict:
    """
    pattern: list of session sizes, e.g. [50] = one session of 50,
             [10,10,10,10,10] = five sessions of 10 with gaps between.
    Gap between sessions > TTL means cache expires.
    """
    p = model[ttl]
    total_read = 0
    total_write = 0
    total_output = 0
    expirations = 0
    turn_idx = 0

    for session_size in pattern:
        # Cache expires between sessions
        cache_warm = False
        for msg in range(session_size):
            ctx = CONTEXT_BASE + TOKENS_PER_TURN * turn_idx
            new = TOKENS_PER_TURN

            if not cache_warm:
                total_write += ctx
                cache_warm = True
                if turn_idx > 0:
                    expirations += 1
            else:
                total_read += ctx - new
                total_write += new

            total_output += OUTPUT_PER_TURN
            turn_idx += 1

    cost_read = mtok(total_read) * p["read"]
    cost_write = mtok(total_write) * p["write"]
    cost_output = mtok(total_output) * model["output"]
    cost_no_cache = mtok(total_read + total_write) * model["base"] + mtok(total_output) * model["output"]

    return {
        "read": total_read, "write": total_write, "output": total_output,
        "cost_read": cost_read, "cost_write": cost_write, "cost_output": cost_output,
        "cost_total": cost_read + cost_write + cost_output,
        "cost_no_cache": cost_no_cache,
        "expirations": expirations,
    }

# ── Messaging patterns ──
# Each is a list of session sizes that sum to 50
PATTERNS = {
    "Nonstop (50 msgs, no breaks)":        [50],
    "2 sessions (25+25, 1 gap)":           [25, 25],
    "5 sessions (10×5, 4 gaps)":           [10, 10, 10, 10, 10],
    "10 sessions (5×10, 9 gaps)":          [5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    "25 sessions (2×25, 24 gaps)":         [2] * 25,
    "All separate (1×50, 49 gaps)":        [1] * 50,
}

print("=" * 90)
print(f"DAILY COST: 50 msgs/day, ~200K context, {TOKENS_PER_TURN} tok/turn")
print("=" * 90)

for ttl in ["1hr", "5min"]:
    print(f"\n{'─' * 90}")
    print(f"  TTL: {ttl}")
    print(f"{'─' * 90}")

    header = f"{'Pattern':<42}"
    for model_name in MODELS:
        header += f" {model_name:>12}"
    print(header)
    print("-" * 90)

    for pat_name, pat in PATTERNS.items():
        row = f"{pat_name:<42}"
        for model_name, model in MODELS.items():
            r = simulate_day(model, ttl, pat)
            row += f" ${r['cost_total']:>10.2f}"
        print(row)

# ── Detailed breakdown for typical case ──
print()
print("=" * 90)
print("DETAILED: 5 sessions × 10 msgs, Auto 70/30")
print("=" * 90)

for ttl in ["1hr", "5min"]:
    model = MODELS["Auto 70/30"]
    r = simulate_day(model, ttl, [10, 10, 10, 10, 10])
    print(f"\n  TTL: {ttl} | Expirations: {r['expirations']}")
    print(f"    Cache reads:  {r['read']/1e6:>6.2f}M tok  ${r['cost_read']:.3f}")
    print(f"    Cache writes: {r['write']/1e6:>6.2f}M tok  ${r['cost_write']:.3f}")
    print(f"    Output:       {r['output']/1e3:>6.1f}K tok  ${r['cost_output']:.3f}")
    print(f"    ─────────────────────────────────")
    print(f"    TOTAL:                   ${r['cost_total']:.2f}/day  (${r['cost_total']*30:.0f}/mo)")
    print(f"    Without cache:           ${r['cost_no_cache']:.2f}/day  (${r['cost_no_cache']*30:.0f}/mo)")
    print(f"    Cache savings:           {(1 - r['cost_total'] / r['cost_no_cache']) * 100:.0f}%")

# ── Summary table: monthly ──
print()
print("=" * 90)
print("MONTHLY COST (×30 days) — Auto 70/30, 1hr TTL")
print("=" * 90)
print(f"\n{'Pattern':<42} {'$/day':>8} {'$/month':>8}")
print("-" * 60)
for pat_name, pat in PATTERNS.items():
    r = simulate_day(MODELS["Auto 70/30"], "1hr", pat)
    print(f"{pat_name:<42} ${r['cost_total']:>7.2f} ${r['cost_total']*30:>7.0f}")
