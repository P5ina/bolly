use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Cached Google access token with TTL.
#[derive(Clone)]
struct CachedToken {
    access_token: String,
    email: String,
    expires_at: std::time::Instant,
}

/// A single account entry returned by the landing API.
#[derive(Clone, Debug)]
pub struct GoogleAccountInfo {
    pub access_token: String,
    pub email: String,
}

/// Client for fetching Google OAuth tokens from the landing API.
/// Supports multiple accounts per instance.
#[derive(Clone)]
pub struct GoogleClient {
    landing_url: String,
    auth_token: String,
    http: reqwest::Client,
    // Cache keyed by (instance_slug, email)
    cache: Arc<Mutex<HashMap<(String, String), CachedToken>>>,
}

impl GoogleClient {
    /// Build a GoogleClient. Returns None if landing_url or auth_token is empty.
    pub fn new(landing_url: &str, auth_token: &str) -> Option<Self> {
        if landing_url.is_empty() || auth_token.is_empty() {
            return None;
        }
        let landing_url = landing_url.to_string();
        Some(Self {
            landing_url,
            auth_token: auth_token.to_string(),
            http: reqwest::Client::new(),
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Fetch all connected Google accounts for an instance.
    /// Returns a list of (access_token, email) pairs.
    pub async fn accounts(&self, instance_slug: &str) -> Result<Vec<GoogleAccountInfo>, String> {
        let url = format!(
            "{}/api/google-token",
            self.landing_url.trim_end_matches('/')
        );
        let res = self
            .http
            .get(&url)
            .query(&[("instance", instance_slug)])
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await
            .map_err(|e| format!("failed to reach landing for Google accounts: {e}"))?;

        if res.status().as_u16() == 404 {
            return Ok(vec![]);
        }

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("landing returned {status}: {body}"));
        }

        #[derive(serde::Deserialize)]
        struct AccountEntry {
            access_token: String,
            email: String,
        }

        #[derive(serde::Deserialize)]
        struct TokenResponse {
            accounts: Vec<AccountEntry>,
        }

        let data: TokenResponse = res
            .json()
            .await
            .map_err(|e| format!("failed to parse token response: {e}"))?;

        // Update cache for all returned accounts
        {
            let mut cache = self.cache.lock().await;
            for account in &data.accounts {
                let key = (instance_slug.to_string(), account.email.clone());
                cache.insert(
                    key,
                    CachedToken {
                        access_token: account.access_token.clone(),
                        email: account.email.clone(),
                        expires_at: std::time::Instant::now()
                            + std::time::Duration::from_secs(3000),
                    },
                );
            }
        }

        Ok(data
            .accounts
            .into_iter()
            .map(|a| GoogleAccountInfo {
                access_token: a.access_token,
                email: a.email,
            })
            .collect())
    }

    /// Get a fresh Google access token and associated email for an instance.
    /// If `account` is Some, returns that specific account's token.
    /// If `account` is None, returns the first account.
    /// Returns error if no accounts are connected.
    pub async fn access_token(
        &self,
        instance_slug: &str,
        account: Option<&str>,
    ) -> Result<(String, String), String> {
        // Check cache first
        {
            let cache = self.cache.lock().await;
            if let Some(email) = account {
                let key = (instance_slug.to_string(), email.to_string());
                if let Some(cached) = cache.get(&key) {
                    if cached.expires_at > std::time::Instant::now() {
                        return Ok((cached.access_token.clone(), cached.email.clone()));
                    }
                }
            } else {
                // Find any cached token for this instance
                for ((slug, _), cached) in cache.iter() {
                    if slug == instance_slug && cached.expires_at > std::time::Instant::now() {
                        return Ok((cached.access_token.clone(), cached.email.clone()));
                    }
                }
            }
        }

        // Fetch from landing (refreshes cache)
        let accounts = self.accounts(instance_slug).await?;

        if accounts.is_empty() {
            return Err(
                "no Google accounts connected. ask the user to connect Google from settings."
                    .into(),
            );
        }

        if let Some(email) = account {
            accounts
                .into_iter()
                .find(|a| a.email == email)
                .map(|a| (a.access_token, a.email))
                .ok_or_else(|| {
                    format!("Google account '{email}' is not connected to this instance.")
                })
        } else {
            let first = accounts.into_iter().next().unwrap();
            Ok((first.access_token, first.email))
        }
    }
}
