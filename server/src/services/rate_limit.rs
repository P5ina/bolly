use sqlx::PgPool;

/// Plan limits read from the tenants table.
struct PlanLimits {
    tokens_per_month: i32,
}

/// Fetch plan limits from the tenants table. Falls back to starter defaults.
async fn fetch_limits(pool: &PgPool, instance_id: &str) -> PlanLimits {
    let row = sqlx::query_scalar::<_, i32>(
        "SELECT tokens_per_month FROM tenants WHERE id = $1",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    PlanLimits {
        tokens_per_month: row.unwrap_or(500_000),
    }
}

/// Check rate limits. Returns Ok(()) if allowed, Err(message) if exceeded.
pub async fn check(pool: &PgPool, instance_id: &str) -> Result<(), String> {
    let row = sqlx::query_scalar::<_, i32>(
        r#"
        INSERT INTO rate_limits (instance_id, messages_today, tokens_this_month, last_reset_daily, last_reset_monthly)
        VALUES ($1, 0, 0, now(), now())
        ON CONFLICT (instance_id) DO UPDATE SET
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
        RETURNING tokens_this_month
        "#,
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        log::warn!("rate_limit check failed: {e}");
        return;
    });

    let tokens_this_month = match row {
        Ok(r) => r,
        Err(_) => return Ok(()), // fail open
    };

    let limits = fetch_limits(pool, instance_id).await;

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
    let row = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT
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

    let tokens_this_month = row.unwrap_or(0);
    let limits = fetch_limits(pool, instance_id).await;

    Some(Usage {
        tokens_this_month,
        tokens_limit: limits.tokens_per_month,
    })
}

#[derive(serde::Serialize)]
pub struct Usage {
    pub tokens_this_month: i32,
    pub tokens_limit: i32,
}
