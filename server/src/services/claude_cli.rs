use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio::io::AsyncBufReadExt;

// ── OAuth constants (from grll/claude-code-login, public PKCE client) ──

const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";
const OAUTH_SCOPE: &str = "org:create_api_key user:profile user:inference";

const CLI_TIMEOUT: Duration = Duration::from_secs(300);

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

// ── CLI availability ──

/// Resolve the path to the `claude` binary.
/// Checks PATH first, then falls back to ~/.local/bin/claude (native install location).
fn resolve_binary() -> String {
    // Try PATH first
    if std::process::Command::new("claude")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return "claude".to_string();
    }
    // Fallback: native install location
    if let Some(home) = dirs::home_dir() {
        let local = home.join(".local/bin/claude");
        if local.exists() {
            return local.to_string_lossy().to_string();
        }
    }
    "claude".to_string()
}

/// Check if the `claude` CLI binary is available.
pub fn is_available() -> bool {
    let bin = resolve_binary();
    std::process::Command::new(&bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Get claude CLI version string, if available.
pub fn version() -> Option<String> {
    let bin = resolve_binary();
    let output = std::process::Command::new(&bin)
        .arg("--version")
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

// ── Auto-install ──

/// Install Claude CLI using the official install script.
pub async fn ensure_installed() -> anyhow::Result<()> {
    if is_available() {
        return Ok(());
    }

    log::info!("Claude CLI not found, installing...");
    let status = tokio::process::Command::new("bash")
        .arg("-c")
        .arg("curl -fsSL https://claude.ai/install.sh | bash")
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("failed to install claude CLI (exit {})", status);
    }

    // Skip Claude Code's own onboarding wizard
    if let Some(home) = dirs::home_dir() {
        let claude_json = home.join(".claude.json");
        if !claude_json.exists() {
            let _ = std::fs::write(&claude_json, r#"{"hasCompletedOnboarding":true}"#);
        }
    }

    log::info!("Claude CLI installed successfully");
    Ok(())
}

// ── OAuth PKCE flow ──

/// Generate a PKCE code verifier (43-128 chars, base64url).
fn generate_code_verifier() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Generate an OAuth authorization URL + state for the PKCE flow.
/// The user must be redirected to the returned URL in their browser.
/// After authorizing, Anthropic shows them a code to paste back.
pub fn initiate_oauth() -> OAuthState {
    let state_bytes: [u8; 32] = rand::rng().random();
    let state = hex::encode(state_bytes);
    let code_verifier = generate_code_verifier();

    let _code_challenge = {
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(hasher.finalize())
    };

    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 600_000; // 10 minutes

    OAuthState {
        state,
        code_verifier,
        expires_at,
    }
}

/// Build the full OAuth authorization URL from an OAuthState.
pub fn build_auth_url(oauth_state: &OAuthState) -> String {
    let code_challenge = {
        let mut hasher = Sha256::new();
        hasher.update(oauth_state.code_verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(hasher.finalize())
    };

    format!(
        "{}?code=true&client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        OAUTH_AUTHORIZE_URL,
        CLIENT_ID,
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(OAUTH_SCOPE),
        code_challenge,
        &oauth_state.state,
    )
}

/// Exchange an authorization code for access + refresh tokens.
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

    // Clean up code (strip any URL fragments)
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

// ── CLI model mapping ──

fn cli_model_name(model: &str) -> &str {
    let lower = model.to_lowercase();
    if lower.contains("opus") {
        "opus"
    } else if lower.contains("haiku") {
        "haiku"
    } else {
        "sonnet"
    }
}

// ── Run a prompt through Claude CLI ──

pub struct McpConfig {
    pub server_url: String,
    pub auth_token: String,
    pub instance_slug: String,
    pub chat_id: String,
}

pub async fn run_prompt(
    model: &str,
    system_prompt: &str,
    user_message: &str,
    oauth_token: &str,
    mcp: Option<&McpConfig>,
) -> anyhow::Result<(String, u64)> {
    // Create temp dir for all temp files (system prompt, MCP config, etc.)
    let temp_dir = std::env::temp_dir().join(format!("bolly-cli-{}", &uuid::Uuid::new_v4().to_string()[..8]));
    std::fs::create_dir_all(&temp_dir)?;

    let bin = resolve_binary();
    let mut cmd = tokio::process::Command::new(&bin);
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--model")
        .arg(cli_model_name(model));

    if !system_prompt.is_empty() {
        cmd.arg("--append-system-prompt").arg(system_prompt);
    }

    // Write MCP config pointing to ourselves as stdio bridge
    if let Some(mcp) = mcp {
        let mcp_url = format!(
            "{}/mcp/{}/{}",
            mcp.server_url, mcp.instance_slug, mcp.chat_id,
        );

        let self_bin = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("server"));

        let mcp_config_path = temp_dir.join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "personality": {
                    "command": self_bin.to_string_lossy(),
                    "args": ["--mcp-bridge", &mcp_url, &mcp.auth_token]
                }
            }
        });
        std::fs::write(&mcp_config_path, serde_json::to_string_pretty(&config)?)?;
        cmd.arg("--mcp-config").arg(&mcp_config_path);
        log::info!("claude CLI: MCP bridge via {} → {}", self_bin.display(), mcp_url);
    }

    // Write prompt to temp file (stdin pipe doesn't close reliably in async)
    let prompt_path = temp_dir.join("prompt.txt");
    std::fs::write(&prompt_path, user_message)?;
    cmd.stdin(std::process::Stdio::from(std::fs::File::open(&prompt_path)?));

    // Pass OAuth token via env var
    cmd.env("CLAUDE_CODE_OAUTH_TOKEN", oauth_token);

    // Clear any system Anthropic key to ensure CLI uses OAuth
    cmd.env_remove("ANTHROPIC_API_KEY");

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!("failed to spawn claude CLI: {e} — is it installed?")
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture claude stdout"))?;

    // Read JSONL output with timeout
    let reader = tokio::io::BufReader::new(stdout);
    let mut lines = reader.lines();

    let mut result_text = String::new();
    let mut tokens_used: u64 = 0;

    let read_result = tokio::time::timeout(CLI_TIMEOUT, async {
        while let Some(line) = lines.next_line().await? {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let event_type = json["type"].as_str().unwrap_or("unknown");
                log::debug!("claude CLI event: type={event_type}");
                match json["type"].as_str() {
                    Some("result") => {
                        log::info!("claude CLI: got result ({} chars)", json["result"].as_str().map(|s| s.len()).unwrap_or(0));
                        if let Some(text) = json["result"].as_str() {
                            result_text = text.to_string();
                        }
                        if let Some(usage) = json.get("usage") {
                            let input = usage["input_tokens"].as_u64().unwrap_or(0);
                            let output = usage["output_tokens"].as_u64().unwrap_or(0);
                            tokens_used = input + output;
                        }
                    }
                    _ => {} // init, etc.
                }
            }
        }
        log::info!("claude CLI: stream ended, result_text={} chars, tokens={}", result_text.len(), tokens_used);
        Ok::<(), anyhow::Error>(())
    })
    .await;

    match read_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let _ = child.kill().await;
            return Err(e);
        }
        Err(_) => {
            let _ = child.kill().await;
            anyhow::bail!(
                "claude CLI timed out after {}s",
                CLI_TIMEOUT.as_secs()
            );
        }
    }

    let output = child.wait_with_output().await?;
    if !output.status.success() && result_text.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_trimmed = stderr.trim();
        if stderr_trimmed.is_empty() {
            anyhow::bail!("claude CLI exited with {}", output.status);
        } else {
            log::error!("claude CLI stderr: {stderr_trimmed}");
            anyhow::bail!("claude CLI error: {stderr_trimmed}");
        }
    }

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok((result_text, tokens_used))
}

// We need hex encoding for the state parameter
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

// ── MCP stdio bridge (runs in same binary) ──

/// Run as MCP stdio bridge: read JSON-RPC from stdin, POST to server, write to stdout.
/// Called via `server --mcp-bridge <url> <token>`.
pub fn run_mcp_bridge(server_url: &str, auth_token: &str) {
    use std::io::{BufRead, Write};

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .expect("failed to create HTTP client");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Validate JSON
        if serde_json::from_str::<serde_json::Value>(trimmed).is_err() {
            continue;
        }

        let resp = client
            .post(server_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {auth_token}"))
            .body(trimmed.to_string())
            .send();

        match resp {
            Ok(r) => {
                if let Ok(body) = r.text() {
                    let mut out = stdout.lock();
                    let _ = out.write_all(body.as_bytes());
                    let _ = out.write_all(b"\n");
                    let _ = out.flush();
                }
            }
            Err(e) => {
                // Return JSON-RPC error so Claude CLI doesn't hang
                if let Ok(req) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
                    let err = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("bridge error: {e}") }
                    });
                    let mut out = stdout.lock();
                    let _ = serde_json::to_writer(&mut out, &err);
                    let _ = out.write_all(b"\n");
                    let _ = out.flush();
                }
            }
        }
    }
}

// We need urlencoding for OAuth URL params
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
