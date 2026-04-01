use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};

// ── OAuth constants (from grll/claude-code-login, public PKCE client) ──

const OAUTH_AUTHORIZE_URL: &str = "https://claude.com/cai/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const REDIRECT_URI: &str = "https://platform.claude.com/oauth/code/callback";
const OAUTH_SCOPE: &str = "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

/// BYOKEY default port.
pub const BYOKEY_PORT: u16 = 8018;

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

// ── BYOKEY proxy ──

/// Install BYOKEY (if not already installed).
/// Downloads pre-built binary from GitHub releases.
pub async fn ensure_proxy_installed() -> anyhow::Result<()> {
    let bin = resolve_byokey_binary();
    let check = std::process::Command::new(&bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if check.map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }

    log::info!("Installing BYOKEY proxy...");

    // Try cargo install first (works on macOS with Rust toolchain)
    let cargo_result = tokio::process::Command::new("cargo")
        .args(["install", "byokey"])
        .status()
        .await;

    if cargo_result.map(|s| s.success()).unwrap_or(false) {
        log::info!("BYOKEY installed via cargo");
        return Ok(());
    }

    // Fallback: download pre-built binary from GitHub releases
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        anyhow::bail!("unsupported architecture for BYOKEY binary download");
    };

    let url = format!(
        "https://github.com/AprilNEA/BYOKEY/releases/latest/download/byokey-v0.9.2-{arch}.tar.gz"
    );
    let install_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/usr/local"))
        .join(".local/bin");
    let _ = std::fs::create_dir_all(&install_dir);

    log::info!("Downloading BYOKEY from {url}...");
    let status = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "curl -fsSL '{url}' | tar xz -C '{dir}' byokey",
            dir = install_dir.display(),
        ))
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("failed to download BYOKEY binary");
    }

    log::info!("BYOKEY installed to {}", install_dir.display());
    Ok(())
}

/// Import an OAuth token into BYOKEY's SQLite store.
pub fn import_token_to_byokey(tokens: &OAuthTokens) {
    let db_path = byokey_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let token_json = serde_json::json!({
        "access_token": tokens.access_token,
        "refresh_token": tokens.refresh_token,
        "expires_at": tokens.expires_at / 1000, // ms → seconds (BYOKEY uses seconds)
        "token_type": "Bearer",
    });

    // Use sqlite3 CLI to avoid adding rusqlite as dependency
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS accounts (provider TEXT, account_id TEXT, label TEXT, is_active INTEGER, token_json TEXT, created_at INTEGER, updated_at INTEGER, PRIMARY KEY (provider, account_id));\
         INSERT OR REPLACE INTO accounts VALUES ('claude', 'default', NULL, 1, '{}', strftime('%s','now'), strftime('%s','now'));",
        token_json.to_string().replace('\'', "''")
    );

    let result = std::process::Command::new("sqlite3")
        .arg(&db_path)
        .arg(&sql)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            log::info!("Imported OAuth token into BYOKEY store");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("Failed to import token to BYOKEY: {stderr}");
        }
        Err(e) => {
            log::warn!("sqlite3 not found, writing BYOKEY DB manually: {e}");
            // Fallback: write a Python one-liner if sqlite3 binary not available
            let py = format!(
                "import sqlite3,time; db=sqlite3.connect('{}'); db.execute('CREATE TABLE IF NOT EXISTS accounts (provider TEXT, account_id TEXT, label TEXT, is_active INTEGER, token_json TEXT, created_at INTEGER, updated_at INTEGER, PRIMARY KEY (provider, account_id))'); db.execute('INSERT OR REPLACE INTO accounts VALUES (?,?,?,?,?,?,?)', ('claude','default',None,1,'{}',int(time.time()),int(time.time()))); db.commit()",
                db_path.display(),
                token_json.to_string().replace('\'', "\\'")
            );
            let _ = std::process::Command::new("python3")
                .args(["-c", &py])
                .status();
        }
    }
}

fn byokey_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".byokey/tokens.db")
}

/// Start BYOKEY as a child process. Returns the handle.
pub async fn start_proxy(workspace_dir: &Path) -> anyhow::Result<tokio::process::Child> {
    // Find OAuth token from any instance and import into BYOKEY
    let instances_dir = workspace_dir.join("instances");
    if let Ok(entries) = std::fs::read_dir(&instances_dir) {
        for entry in entries.flatten() {
            let slug = entry.file_name().to_string_lossy().to_string();
            if let Some(token) = load_token(workspace_dir, &slug) {
                import_token_to_byokey(&token);
                break;
            }
        }
    }

    log::info!("Starting BYOKEY proxy on port {BYOKEY_PORT}...");

    // Resolve byokey binary (may be in ~/.cargo/bin)
    let byokey_bin = resolve_byokey_binary();

    let child = tokio::process::Command::new(&byokey_bin)
        .args(["serve", "--port", &BYOKEY_PORT.to_string(), "--host", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to start byokey: {e}"))?;

    log::info!("BYOKEY launched (pid={})", child.id().unwrap_or(0));

    // Wait until ready
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{BYOKEY_PORT}/health");
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send().await.is_ok()
        {
            log::info!("BYOKEY is ready on port {BYOKEY_PORT}");
            return Ok(child);
        }
    }

    log::warn!("BYOKEY launched but not responding after 15s");
    Ok(child)
}

/// Start proxy and leak handle (for request handlers).
pub async fn start_proxy_detached(workspace_dir: &Path) -> anyhow::Result<()> {
    let child = start_proxy(workspace_dir).await?;
    std::mem::forget(child);
    Ok(())
}

/// Kill any running BYOKEY/Meridian proxy process.
pub fn kill_proxy() {
    for name in ["byokey", "meridian"] {
        let _ = std::process::Command::new("pkill")
            .args(["-f", name])
            .output();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
}

/// Check if proxy is running.
pub async fn is_proxy_running() -> bool {
    let client = reqwest::Client::new();
    client.get(&format!("http://127.0.0.1:{BYOKEY_PORT}/health"))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

pub fn resolve_byokey_binary_pub() -> String {
    resolve_byokey_binary()
}

fn resolve_byokey_binary() -> String {
    if let Some(home) = dirs::home_dir() {
        for dir in [".cargo/bin", ".local/bin"] {
            let bin = home.join(dir).join("byokey");
            if bin.exists() {
                return bin.to_string_lossy().to_string();
            }
        }
    }
    "byokey".to_string()
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
