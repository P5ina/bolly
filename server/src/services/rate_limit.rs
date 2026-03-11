use sqlx::PgPool;

/// Plan limits derived from the tenants table.
struct PlanLimits {
    messages_per_day: i32,
    tokens_per_month: i32,
}

/// Map plan name to limits (must match landing PLANS config).
fn limits_for_plan(plan: &str) -> PlanLimits {
    match plan {
        "starter" => PlanLimits {
            messages_per_day: 100,
            tokens_per_month: 500_000,
        },
        "companion" => PlanLimits {
            messages_per_day: 300,
            tokens_per_month: 1_000_000,
        },
        "unlimited" => PlanLimits {
            messages_per_day: -1, // unlimited
            tokens_per_month: 5_000_000,
        },
        _ => PlanLimits {
            messages_per_day: 100,
            tokens_per_month: 500_000,
        },
    }
}

/// Fetch plan limits from the tenants table. Falls back to starter defaults.
async fn fetch_limits(pool: &PgPool, instance_id: &str) -> PlanLimits {
    let plan: Option<String> = sqlx::query_scalar("SELECT plan FROM tenants WHERE id = $1")
        .bind(instance_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    limits_for_plan(plan.as_deref().unwrap_or("starter"))
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

    let limits = fetch_limits(pool, instance_id).await;

    // -1 means unlimited
    if limits.messages_per_day > 0 && messages_today >= limits.messages_per_day {
        return Err(format!(
            "daily message limit reached ({})",
            limits.messages_per_day
        ));
    }
    if limits.tokens_per_month > 0 && tokens_this_month >= limits.tokens_per_month {
        return Err(format!(
            "monthly token limit reached ({})",
            limits.tokens_per_month
        ));
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

/// Return current usage and limits.
pub async fn get_usage(pool: &PgPool, instance_id: &str) -> Option<Usage> {
    let row = sqlx::query_as::<_, (i32, i32)>(
        r#"
        SELECT
            CASE WHEN last_reset_daily::date < CURRENT_DATE THEN 0 ELSE messages_today END,
            CASE WHEN last_reset_monthly < date_trunc('month', CURRENT_DATE) THEN 0 ELSE tokens_this_month END
        FROM rate_limits
        WHERE instance_id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (messages_today, tokens_this_month) = row.unwrap_or((0, 0));
    let limits = fetch_limits(pool, instance_id).await;

    Some(Usage {
        messages_today,
        messages_limit: limits.messages_per_day,
        tokens_this_month,
        tokens_limit: limits.tokens_per_month,
    })
}

#[derive(serde::Serialize)]
pub struct Usage {
    pub messages_today: i32,
    pub messages_limit: i32,
    pub tokens_this_month: i32,
    pub tokens_limit: i32,
}

/// Estimate token count from text (rough heuristic: ~3.2 chars per token).
pub fn estimate_tokens(text: &str) -> i32 {
    (text.len() as f64 / 3.2) as i32
}
