use sqlx::PgPool;

/// Env-configurable limits with defaults.
fn messages_per_day() -> i32 {
    std::env::var("RATE_LIMIT_MESSAGES_PER_DAY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100)
}

fn tokens_per_month() -> i32 {
    std::env::var("RATE_LIMIT_TOKENS_PER_MONTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500_000)
}

/// Check rate limits. Returns Ok(()) if allowed, Err(message) if exceeded.
/// Automatically resets counters when a new day/month starts.
pub async fn check(pool: &PgPool, instance_id: &str) -> Result<(), String> {
    // Upsert to ensure row exists, then reset stale counters and return current values.
    let row = sqlx::query_as::<_, (i32, i32)>(
        r#"
        INSERT INTO rate_limits (instance_id, messages_today, tokens_this_month, last_reset_daily, last_reset_monthly)
        VALUES ($1, 0, 0, now(), now())
        ON CONFLICT (instance_id) DO UPDATE SET
            -- reset daily counter if last_reset_daily is before today
            messages_today = CASE
                WHEN rate_limits.last_reset_daily::date < CURRENT_DATE THEN 0
                ELSE rate_limits.messages_today
            END,
            last_reset_daily = CASE
                WHEN rate_limits.last_reset_daily::date < CURRENT_DATE THEN now()
                ELSE rate_limits.last_reset_daily
            END,
            -- reset monthly counter if last_reset_monthly is before start of current month
            tokens_this_month = CASE
                WHEN rate_limits.last_reset_monthly < date_trunc('month', CURRENT_DATE)
                THEN 0
                ELSE rate_limits.tokens_this_month
            END,
            last_reset_monthly = CASE
                WHEN rate_limits.last_reset_monthly < date_trunc('month', CURRENT_DATE)
                THEN now()
                ELSE rate_limits.last_reset_monthly
            END
        RETURNING messages_today, tokens_this_month
        "#,
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        log::warn!("rate_limit check failed: {e}");
        // Fail open — allow request if DB is unreachable
        return;
    });

    let (messages_today, tokens_this_month) = match row {
        Ok(r) => r,
        Err(_) => return Ok(()), // fail open
    };

    let msg_limit = messages_per_day();
    let tok_limit = tokens_per_month();

    // -1 means unlimited
    if msg_limit > 0 && messages_today >= msg_limit {
        return Err(format!("daily message limit reached ({msg_limit})"));
    }
    if tok_limit > 0 && tokens_this_month >= tok_limit {
        return Err(format!("monthly token limit reached ({tok_limit})"));
    }

    Ok(())
}

/// Increment usage counters after a successful response.
pub async fn record_usage(pool: &PgPool, instance_id: &str, tokens: i32) {
    let result = sqlx::query(
        r#"
        INSERT INTO rate_limits (instance_id, messages_today, tokens_this_month, last_reset_daily, last_reset_monthly)
        VALUES ($1, 1, $2, now(), now())
        ON CONFLICT (instance_id) DO UPDATE SET
            messages_today = rate_limits.messages_today + 1,
            tokens_this_month = rate_limits.tokens_this_month + $2
        "#,
    )
    .bind(instance_id)
    .bind(tokens)
    .execute(pool)
    .await;

    if let Err(e) = result {
        log::warn!("rate_limit record_usage failed: {e}");
    }
}

/// Estimate token count from text (rough heuristic: ~3.2 chars per token).
pub fn estimate_tokens(text: &str) -> i32 {
    (text.len() as f64 / 3.2) as i32
}
