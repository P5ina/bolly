#!/usr/bin/env python3
"""Calculate cost for a user conversation to reach 1M tokens."""

# ── Real data from your instance ──
CURRENT_MESSAGES = 700        # rig_history messages (user + assistant)
CURRENT_TOTAL_TOKENS = 148_253  # real input tokens from API
SYSTEM_TOOLS_TOKENS = 38_000    # system prompt + tools (stable, cached)
TARGET_TOKENS = 1_000_000

# ── Pricing ($/MTok) ──
PRICING = {
    "opus_4.6": {
        "cache_read": 0.50,
        "cache_write": 6.25,
        "base_input": 5.00,
        "output": 25.00,
    },
    "sonnet_4.6": {
        "cache_read": 0.30,
        "cache_write": 3.75,
        "base_input": 3.00,
        "output": 15.00,
    },
    "haiku_4.5": {
        "cache_read": 0.10,
        "cache_write": 1.25,
        "base_input": 1.00,
        "output": 5.00,
    },
}

# ── Auto mode split ──
SONNET_RATIO = 0.70
OPUS_RATIO = 0.30

def weighted(key: str) -> float:
    return SONNET_RATIO * PRICING["sonnet_4.6"][key] + OPUS_RATIO * PRICING["opus_4.6"][key]

def mtok(tokens: int) -> float:
    return tokens / 1_000_000

# ── Derived ──
message_tokens = CURRENT_TOTAL_TOKENS - SYSTEM_TOOLS_TOKENS
tokens_per_msg = message_tokens / CURRENT_MESSAGES
msgs_per_turn = 2  # user + assistant per turn
tokens_per_turn = tokens_per_msg * msgs_per_turn
current_turns = CURRENT_MESSAGES // msgs_per_turn

target_msg_tokens = TARGET_TOKENS - SYSTEM_TOOLS_TOKENS
total_messages = int(target_msg_tokens / tokens_per_msg)
total_turns = total_messages // msgs_per_turn

print(f"=== Cost to reach {TARGET_TOKENS // 1000}K tokens ===\n")
print(f"Current state:")
print(f"  Messages:        {CURRENT_MESSAGES}")
print(f"  Total tokens:    {CURRENT_TOTAL_TOKENS:,}")
print(f"  System+tools:    {SYSTEM_TOOLS_TOKENS:,}")
print(f"  Message tokens:  {message_tokens:,}")
print(f"  Avg per message: {tokens_per_msg:.0f}")
print(f"  Avg per turn:    {tokens_per_turn:.0f}")
print(f"  Current turns:   {current_turns}")
print()
print(f"To reach {TARGET_TOKENS // 1000}K:")
print(f"  Total messages:  {total_messages:,}")
print(f"  Total turns:     {total_turns:,}")
print(f"  Remaining turns: {total_turns - current_turns:,}")
print()

# ── Simulate turn by turn ──
avg_output_per_turn = 150  # short replies

for label, rate_fn in [
    ("Opus 4.6 only", lambda k: PRICING["opus_4.6"][k]),
    ("Sonnet 4.6 only", lambda k: PRICING["sonnet_4.6"][k]),
    ("Auto (70% Sonnet / 30% Opus)", weighted),
]:
    total_cache_read_tokens = 0
    total_cache_write_tokens = 0
    total_output_tokens = 0
    total_classifier_tokens = 0

    for turn in range(1, total_turns + 1):
        context = SYSTEM_TOOLS_TOKENS + tokens_per_turn * turn
        new_tokens = tokens_per_turn  # new user+assistant pair
        cached_tokens = context - new_tokens

        total_cache_read_tokens += max(0, cached_tokens)
        total_cache_write_tokens += new_tokens
        total_output_tokens += avg_output_per_turn

        # Haiku classifier call (~100 tokens per classification)
        if "Auto" in label:
            total_classifier_tokens += 100

    cost_read = mtok(total_cache_read_tokens) * rate_fn("cache_read")
    cost_write = mtok(total_cache_write_tokens) * rate_fn("cache_write")
    cost_output = mtok(total_output_tokens) * rate_fn("output")
    cost_classifier = mtok(total_classifier_tokens) * PRICING["haiku_4.5"]["base_input"]
    cost_total = cost_read + cost_write + cost_output + cost_classifier

    # No-cache comparison
    total_input_no_cache = total_cache_read_tokens + total_cache_write_tokens
    cost_no_cache = mtok(total_input_no_cache) * rate_fn("base_input") + mtok(total_output_tokens) * rate_fn("output")

    print(f"--- {label} ---")
    print(f"  Cache reads:    {total_cache_read_tokens / 1e9:.2f}B tokens  ${cost_read:>8.2f}")
    print(f"  Cache writes:   {total_cache_write_tokens / 1e6:.1f}M tokens  ${cost_write:>8.2f}")
    print(f"  Output:         {total_output_tokens / 1e6:.1f}M tokens  ${cost_output:>8.2f}")
    if total_classifier_tokens > 0:
        print(f"  Classifier:     {total_classifier_tokens / 1e6:.1f}M tokens  ${cost_classifier:>8.2f}")
    print(f"  ────────────────────────────────")
    print(f"  TOTAL with cache:              ${cost_total:>8.2f}")
    print(f"  WITHOUT cache:                 ${cost_no_cache:>8.2f}")
    print(f"  Cache savings:                  {(1 - cost_total / cost_no_cache) * 100:.0f}%")
    print()
