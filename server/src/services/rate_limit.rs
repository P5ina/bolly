use reqwest::Client;

/// Check rate limits via landing API. Returns Ok(()) if allowed, Err(message) if exceeded.
/// Fails open on network errors.
pub async fn check(http: &Client, landing_url: &str, auth_token: &str) -> Result<(), String> {
    let res = http
        .post(format!("{landing_url}/api/internal/check-rate-limit"))
        .bearer_auth(auth_token)
        .send()
        .await;

    let res = match res {
        Ok(r) => r,
        Err(e) => {
            log::warn!("rate_limit check failed (network): {e}");
            return Ok(()); // fail open
        }
    };

    let body: serde_json::Value = match res.json().await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("rate_limit check failed (parse): {e}");
            return Ok(()); // fail open
        }
    };

    if body["allowed"].as_bool() == Some(true) {
        Ok(())
    } else {
        let reason = body["reason"].as_str().unwrap_or("rate limit exceeded").to_string();
        Err(reason)
    }
}

/// Record token usage via landing API.
pub async fn record_usage(http: &Client, landing_url: &str, auth_token: &str, tokens: i32) {
    let result = http
        .post(format!("{landing_url}/api/internal/record-usage"))
        .bearer_auth(auth_token)
        .json(&serde_json::json!({ "tokens": tokens }))
        .send()
        .await;

    if let Err(e) = result {
        log::warn!("rate_limit record_usage failed: {e}");
    }
}

/// Fetch current usage and limits via landing API.
pub async fn get_usage(http: &Client, landing_url: &str, auth_token: &str) -> Option<Usage> {
    let res = http
        .get(format!("{landing_url}/api/internal/usage"))
        .bearer_auth(auth_token)
        .send()
        .await
        .ok()?;

    let body: serde_json::Value = res.json().await.ok()?;

    Some(Usage {
        tokens_this_month: body["tokens_this_month"].as_i64().unwrap_or(0) as i32,
        tokens_limit: body["tokens_limit"].as_i64().unwrap_or(0) as i32,
    })
}

#[derive(serde::Serialize)]
pub struct Usage {
    pub tokens_this_month: i32,
    pub tokens_limit: i32,
}
