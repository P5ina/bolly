use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};

// ── OAuth constants (from grll/claude-code-login, public PKCE client) ──

const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";
const OAUTH_SCOPE: &str = "org:create_api_key user:profile user:inference";

// ── OAuth types ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64, // millis since epoch
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthState {
    pub state: String,
    pub code_verifier: String,
    pub expires_at: u64,
}

// ── Meridian proxy (turns Claude subscription into standard Anthropic API) ──

/// Install Meridian globally via npm.
pub async fn ensure_meridian_installed() -> anyhow::Result<()> {
    // Check if already available
    let check = std::process::Command::new("meridian")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if check.map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }
    log::info!("Installing Meridian proxy...");
    let status = tokio::process::Command::new("npm")
        .args(["install", "-g", "@rynfar/meridian"])
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("failed to install @rynfar/meridian");
    }
    log::info!("Meridian installed successfully");
    Ok(())
}

/// Start Meridian as a detached background process.
/// Listens on http://127.0.0.1:3456, proxying to Claude subscription.
/// Writes OAuth credentials to ~/.claude/.credentials.json so the
/// Claude Code SDK (used by Meridian) can authenticate.
pub async fn start_meridian(workspace_dir: &Path) -> anyhow::Result<()> {
    // Find OAuth token from any instance and write to Claude credentials file
    let instances_dir = workspace_dir.join("instances");
    if let Ok(entries) = std::fs::read_dir(&instances_dir) {
        for entry in entries.flatten() {
            let slug = entry.file_name().to_string_lossy().to_string();
            if let Some(token) = load_token(workspace_dir, &slug) {
                write_claude_credentials(&token);
                log::info!("Wrote Claude credentials from instance '{slug}' for Meridian");
                break;
            }
        }
    }

    log::info!("Starting Meridian proxy on port 3456...");

    let child = std::process::Command::new("meridian")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to start meridian: {e}"))?;

    log::info!("Meridian launched (pid={})", child.id());

    // Wait until it's ready
    let client = reqwest::Client::new();
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.get("http://127.0.0.1:3456/")
            .timeout(std::time::Duration::from_secs(2))
            .send().await.is_ok()
        {
            log::info!("Meridian is ready on port 3456");
            return Ok(());
        }
    }

    log::warn!("Meridian launched but not responding after 15s");
    Ok(())
}

/// Check if Meridian is running.
pub async fn is_meridian_running() -> bool {
    let client = reqwest::Client::new();
    client.get("http://127.0.0.1:3456/")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

// ── OAuth PKCE flow ──

fn generate_code_verifier() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn initiate_oauth() -> OAuthState {
    let state_bytes: [u8; 32] = rand::rng().random();
    let state = hex::encode(state_bytes);
    let code_verifier = generate_code_verifier();

    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 600_000; // 10 minutes

    OAuthState { state, code_verifier, expires_at }
}

pub fn build_auth_url(oauth_state: &OAuthState) -> String {
    let code_challenge = {
        let mut hasher = Sha256::new();
        hasher.update(oauth_state.code_verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(hasher.finalize())
    };

    format!(
        "{}?code=true&client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        OAUTH_AUTHORIZE_URL, CLIENT_ID,
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(OAUTH_SCOPE),
        code_challenge, &oauth_state.state,
    )
}

pub async fn exchange_code(
    http: &reqwest::Client,
    code: &str,
    oauth_state: &OAuthState,
) -> anyhow::Result<OAuthTokens> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    if now_ms > oauth_state.expires_at {
        anyhow::bail!("OAuth state expired — please start the flow again");
    }

    let clean_code = code.split('#').next().unwrap_or(code);
    let clean_code = clean_code.split('&').next().unwrap_or(clean_code);

    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": CLIENT_ID,
        "code": clean_code,
        "redirect_uri": REDIRECT_URI,
        "code_verifier": &oauth_state.code_verifier,
        "state": &oauth_state.state,
    });

    let resp = http
        .post(OAUTH_TOKEN_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Referer", "https://claude.ai/")
        .header("Origin", "https://claude.ai")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OAuth token exchange failed: {status} — {text}");
    }

    #[derive(serde::Deserialize)]
    struct TokenResp {
        access_token: String,
        refresh_token: String,
        expires_in: u64,
    }

    let data: TokenResp = resp.json().await?;
    let expires_at = now_ms + data.expires_in * 1000;

    Ok(OAuthTokens {
        access_token: data.access_token,
        refresh_token: data.refresh_token,
        expires_at,
    })
}

// ── Per-instance token storage ──

fn token_path(workspace_dir: &Path, instance_slug: &str) -> PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("claude_oauth_token.json")
}

pub fn load_token(workspace_dir: &Path, instance_slug: &str) -> Option<OAuthTokens> {
    let path = token_path(workspace_dir, instance_slug);
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_token(
    workspace_dir: &Path,
    instance_slug: &str,
    tokens: &OAuthTokens,
) -> anyhow::Result<()> {
    let path = token_path(workspace_dir, instance_slug);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(tokens)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn has_valid_token(workspace_dir: &Path, instance_slug: &str) -> bool {
    match load_token(workspace_dir, instance_slug) {
        Some(t) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            t.expires_at > now
        }
        None => false,
    }
}

/// Write OAuth tokens to ~/.claude/.credentials.json so Claude Code SDK
/// (used by Meridian) can authenticate on headless/managed servers.
fn write_claude_credentials(tokens: &OAuthTokens) {
    let Some(home) = dirs::home_dir() else { return };
    let claude_dir = home.join(".claude");
    let _ = std::fs::create_dir_all(&claude_dir);

    let creds = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "expiresAt": tokens.expires_at,
        }
    });

    let path = claude_dir.join(".credentials.json");
    if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&creds).unwrap_or_default()) {
        log::warn!("Failed to write Claude credentials: {e}");
    }

    // Also ensure onboarding is skipped
    let claude_json = claude_dir.join("claude.json");
    if !claude_json.exists() {
        let _ = std::fs::write(&claude_json, r#"{"hasCompletedOnboarding":true}"#);
    }
}

// ── Internal helpers ──

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char);
                }
                _ => {
                    out.push('%');
                    out.push_str(&format!("{b:02X}"));
                }
            }
        }
        out
    }
}
