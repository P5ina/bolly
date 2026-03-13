use std::sync::Arc;
use tokio::sync::Mutex;

/// Cached Google access token with TTL.
#[derive(Clone)]
struct CachedToken {
    access_token: String,
    email: String,
    expires_at: std::time::Instant,
}

/// Client for fetching Google OAuth tokens from the landing API.
#[derive(Clone)]
pub struct GoogleClient {
    landing_url: String,
    auth_token: String,
    http: reqwest::Client,
    cache: Arc<Mutex<Option<CachedToken>>>,
}

impl GoogleClient {
    /// Try to build a GoogleClient from environment variables.
    /// Returns None if LANDING_URL is not set or auth_token is empty.
    pub fn from_env(auth_token: &str) -> Option<Self> {
        let landing_url = std::env::var("LANDING_URL").ok()?;
        if landing_url.is_empty() || auth_token.is_empty() {
            return None;
        }
        Some(Self {
            landing_url,
            auth_token: auth_token.to_string(),
            http: reqwest::Client::new(),
            cache: Arc::new(Mutex::new(None)),
        })
    }

    /// Get a fresh Google access token and associated email.
    /// Caches the token and only refreshes when near expiry.
    pub async fn access_token(&self) -> Result<(String, String), String> {
        // Check cache
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if cached.expires_at > std::time::Instant::now() {
                    return Ok((cached.access_token.clone(), cached.email.clone()));
                }
            }
        }

        // Fetch from landing
        let url = format!("{}/api/google-token", self.landing_url.trim_end_matches('/'));
        let res = self.http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await
            .map_err(|e| format!("failed to reach landing for Google token: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            if status.as_u16() == 404 {
                return Err("Google account not connected. Ask the user to connect Google from the dashboard.".into());
            }
            return Err(format!("landing returned {status}: {body}"));
        }

        #[derive(serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
            email: String,
        }

        let data: TokenResponse = res.json().await
            .map_err(|e| format!("failed to parse token response: {e}"))?;

        // Cache for 50 minutes (tokens usually last 60 min)
        let cached = CachedToken {
            access_token: data.access_token.clone(),
            email: data.email.clone(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(3000),
        };

        {
            let mut cache = self.cache.lock().await;
            *cache = Some(cached);
        }

        Ok((data.access_token, data.email))
    }
}
