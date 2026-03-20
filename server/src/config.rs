use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub auth_token: String,
    #[serde(default)]
    pub static_dir: String,
    #[serde(default)]
    pub landing_url: String,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default = "default_registry_url")]
    pub registry_url: String,
    #[serde(default)]
    pub plan: String,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub github: GithubConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GithubConfig {
    #[serde(default)]
    pub token: String,
}

/// A single SMTP/IMAP email account.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EmailConfig {
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_user: String,
    #[serde(default)]
    pub smtp_password: String,
    #[serde(default)]
    pub smtp_from: String,
    #[serde(default)]
    pub imap_host: String,
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub imap_user: String,
    #[serde(default)]
    pub imap_password: String,
}

fn default_smtp_port() -> u16 { 587 }
fn default_imap_port() -> u16 { 993 }

/// Per-instance configuration stored at `instances/{slug}/instance.toml`.
/// Holds settings that are specific to one user/instance, such as GitHub token.
/// Takes precedence over global `config.toml` for the same fields.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    #[serde(default)]
    pub github: GithubConfig,
    /// ElevenLabs voice ID override for this instance.
    #[serde(default)]
    pub elevenlabs_voice_id: String,
    /// Whether background music/ambient plays automatically. Default: true.
    #[serde(default = "default_true")]
    pub music_enabled: bool,
    /// Whether voice mode (TTS) is enabled. Default: false.
    #[serde(default)]
    pub voice_enabled: bool,
}

fn default_true() -> bool { true }

impl InstanceConfig {
    /// Load per-instance config from `instances/{slug}/instance.toml`.
    /// Returns default (empty) config if the file doesn't exist.
    pub fn load(workspace_dir: &Path, instance_slug: &str) -> Self {
        let path = workspace_dir
            .join("instances")
            .join(instance_slug)
            .join("instance.toml");
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };
        toml::from_str::<InstanceConfig>(&raw).unwrap_or_default()
    }

    /// Save per-instance config to `instances/{slug}/instance.toml`.
    pub fn save(&self, workspace_dir: &Path, instance_slug: &str) -> Result<(), Box<dyn std::error::Error>> {
        let dir = workspace_dir.join("instances").join(instance_slug);
        fs::create_dir_all(&dir)?;
        let raw = toml::to_string_pretty(self)?;
        fs::write(dir.join("instance.toml"), raw)?;
        Ok(())
    }

    /// Return the effective GitHub token: instance-level if set, otherwise fall back to global.
    #[allow(dead_code)]
    pub fn effective_github_token<'a>(&'a self, global: &'a Config) -> Option<&'a str> {
        if !self.github.token.is_empty() {
            return Some(&self.github.token);
        }
        if !global.github.token.is_empty() {
            return Some(&global.github.token);
        }
        None
    }
}

/// Wrapper for `instances/{slug}/email.toml` — supports multiple accounts.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EmailAccounts {
    #[serde(default)]
    pub accounts: Vec<EmailConfig>,
}

impl EmailAccounts {
    /// Load email accounts from `instances/{slug}/email.toml`.
    /// Supports both legacy flat format (single account) and `[[accounts]]` array.
    pub fn load(workspace_dir: &Path, instance_slug: &str) -> Vec<EmailConfig> {
        let path = workspace_dir
            .join("instances")
            .join(instance_slug)
            .join("email.toml");
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        // Try new format first: [[accounts]]
        if let Ok(wrapper) = toml::from_str::<EmailAccounts>(&raw) {
            if !wrapper.accounts.is_empty() {
                return wrapper.accounts.into_iter()
                    .filter(|c| !c.smtp_host.is_empty() || !c.imap_host.is_empty())
                    .collect();
            }
        }

        // Fall back to legacy flat format (single account)
        if let Ok(cfg) = toml::from_str::<EmailConfig>(&raw) {
            if !cfg.smtp_host.is_empty() || !cfg.imap_host.is_empty() {
                return vec![cfg];
            }
        }

        vec![]
    }

    /// Save email accounts to `instances/{slug}/email.toml`.
    pub fn save(accounts: &[EmailConfig], workspace_dir: &Path, instance_slug: &str) -> Result<(), Box<dyn std::error::Error>> {
        let dir = workspace_dir.join("instances").join(instance_slug);
        fs::create_dir_all(&dir)?;
        let wrapper = EmailAccounts { accounts: accounts.to_vec() };
        let raw = toml::to_string_pretty(&wrapper)?;
        fs::write(dir.join("email.toml"), raw)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Human-readable name for this MCP server.
    pub name: String,
    /// URL for HTTP/SSE transport (e.g. "https://mcp.excalidraw.com/mcp").
    pub url: Option<String>,
    /// Command for stdio transport (e.g. "node /path/to/server.js --stdio").
    pub command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    OpenRouter,
}

impl LlmProvider {
    /// Default model for this provider (used when no model is explicitly configured).
    pub fn default_model(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "claude-sonnet-4-6",
            LlmProvider::OpenAI => "gpt-5.2",
            LlmProvider::OpenRouter => "anthropic/claude-sonnet-4-6",
        }
    }

    /// Fast/cheap model for this provider (used for triage, sentiment, etc.).
    pub fn fast_model(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "claude-haiku-4-5-20251001",
            LlmProvider::OpenAI => "gpt-5.4-nano",
            LlmProvider::OpenRouter => "google/gemini-3.1-flash-lite-preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelMode {
    Auto,
    Fast,
    Heavy,
}

impl Default for ModelMode {
    fn default() -> Self { ModelMode::Auto }
}

fn default_heavy_multiplier() -> f32 { 10.0 }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub provider: Option<LlmProvider>,
    #[serde(default)]
    pub model: Option<String>,
    /// Fast/cheap model override. Falls back to provider default (e.g. Haiku) if empty.
    #[serde(default)]
    pub fast_model: Option<String>,
    #[serde(default)]
    pub tokens: LlmTokens,
    #[serde(default)]
    pub model_mode: ModelMode,
    #[serde(default = "default_heavy_multiplier")]
    pub heavy_multiplier: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmTokens {
    #[serde(default, rename = "OPEN_AI", alias = "open_ai", alias = "openai")]
    pub open_ai: String,
    #[serde(default, rename = "ANTHROPIC", alias = "anthropic")]
    pub anthropic: String,
    #[serde(default, rename = "BRAVE_SEARCH", alias = "brave_search", alias = "brave")]
    pub brave_search: String,
    #[serde(default, rename = "OPENROUTER", alias = "open_router", alias = "openrouter")]
    pub open_router: String,
    #[serde(default, rename = "ELEVENLABS", alias = "elevenlabs")]
    pub elevenlabs: String,
}


fn default_host() -> String {
    "0.0.0.0".into()
}

fn default_port() -> u16 {
    8080
}

fn default_registry_url() -> String {
    "https://raw.githubusercontent.com/triangle-int/bolly-skills/main/registry.json".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 8080,
            auth_token: String::new(),
            static_dir: String::new(),
            landing_url: String::new(),
            llm: LlmConfig::default(),
            registry_url: default_registry_url(),
            plan: String::new(),
            mcp_servers: Vec::new(),
            github: GithubConfig::default(),
        }
    }
}

impl LlmConfig {
    /// The active model name, falling back to the provider's default.
    pub fn model_name(&self) -> &str {
        if let Some(ref m) = self.model {
            if !m.is_empty() { return m; }
        }
        self.provider.as_ref().map_or("claude-sonnet-4-6", |p| p.default_model())
    }

    /// The fast/cheap model name, falling back to the provider's default fast model.
    pub fn fast_model_name(&self) -> &str {
        if let Some(ref m) = self.fast_model {
            if !m.is_empty() { return m; }
        }
        self.provider.as_ref().map_or("claude-haiku-4-5-20251001", |p| p.fast_model())
    }

    /// The API key for the active provider, or None if not configured.
    pub fn api_key(&self) -> Option<&str> {
        let key = match self.provider? {
            LlmProvider::Anthropic => &self.tokens.anthropic,
            LlmProvider::OpenAI => &self.tokens.open_ai,
            LlmProvider::OpenRouter => &self.tokens.open_router,
        };
        if key.is_empty() { None } else { Some(key) }
    }

    /// Whether the LLM is fully configured (provider + key).
    pub fn is_configured(&self) -> bool {
        self.api_key().is_some()
    }

    /// List of provider names that have API keys set.
    pub fn configured_providers(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.tokens.anthropic.is_empty() { out.push("anthropic"); }
        if !self.tokens.open_ai.is_empty() { out.push("openai"); }
        if !self.tokens.open_router.is_empty() { out.push("openrouter"); }
        if !self.tokens.brave_search.is_empty() { out.push("brave_search"); }
        out
    }

    /// Get API key + model for Anthropic specifically (for count_tokens API etc.).
    /// Returns None if Anthropic is not the active provider or key is missing.
    pub fn anthropic_credentials(&self) -> Option<(&str, &str)> {
        if self.provider != Some(LlmProvider::Anthropic) { return None; }
        let key = if self.tokens.anthropic.is_empty() { return None } else { &self.tokens.anthropic };
        Some((key, self.model_name()))
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            fast_model: None,
            tokens: LlmTokens::default(),
            model_mode: ModelMode::default(),
            heavy_multiplier: default_heavy_multiplier(),
        }
    }
}

impl Default for LlmTokens {
    fn default() -> Self {
        Self {
            open_ai: String::new(),
            anthropic: String::new(),
            brave_search: String::new(),
            open_router: String::new(),
            elevenlabs: String::new(),
        }
    }
}

pub fn workspace_root() -> PathBuf {
    if let Some(path) = env::var_os("BOLLY_HOME") {
        return PathBuf::from(path);
    }

    dirs::home_dir()
        .expect("failed to resolve home directory")
        .join(".bolly")
}

pub fn config_path() -> PathBuf {
    workspace_root().join("config.toml")
}

fn ensure_config_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let default_config =
        toml::to_string_pretty(&Config::default()).expect("default config should serialize");
    fs::write(path, default_config)
}

fn ensure_workspace_layout(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    fs::create_dir_all(path.join("instances"))?;
    fs::create_dir_all(path.join("skills"))?;
    Ok(())
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = config_path();
    ensure_config_exists(&path)?;
    let raw = fs::read_to_string(&path)?;
    let mut config: Config = toml::from_str(&raw)?;
    ensure_workspace_layout(&workspace_root())?;

    // Allow env var overrides for managed hosting
    if let Ok(token) = env::var("BOLLY_AUTH_TOKEN") {
        if !token.is_empty() {
            config.auth_token = token;
        }
    }

    if let Ok(url) = env::var("LANDING_URL") {
        if !url.is_empty() {
            config.landing_url = url;
        }
    }

    // API key overrides from env (for managed hosting)
    if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() {
            config.llm.tokens.anthropic = key;
            // Auto-set provider and model if not already configured
            if config.llm.provider.is_none() {
                config.llm.provider = Some(LlmProvider::Anthropic);
            }
            if config.llm.model.is_none() {
                config.llm.model = Some(LlmProvider::Anthropic.default_model().to_string());
            }
        }
    }
    if let Ok(key) = env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            config.llm.tokens.open_ai = key;
            if config.llm.provider.is_none() {
                config.llm.provider = Some(LlmProvider::OpenAI);
            }
        }
    }
    if let Ok(key) = env::var("OPENROUTER_API_KEY") {
        if !key.is_empty() {
            config.llm.tokens.open_router = key;
            if config.llm.provider.is_none() {
                config.llm.provider = Some(LlmProvider::OpenRouter);
            }
        }
    }
    if let Ok(key) = env::var("BRAVE_SEARCH_API_KEY") {
        if !key.is_empty() {
            config.llm.tokens.brave_search = key;
        }
    }
    if let Ok(key) = env::var("ELEVENLABS_API_KEY") {
        if !key.is_empty() {
            config.llm.tokens.elevenlabs = key;
        }
    }

    // GitHub token override
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            config.github.token = token;
        }
    }

    // Explicit provider/model override (set by admin panel via Fly env)
    if let Ok(provider) = env::var("BOLLY_LLM_PROVIDER") {
        match provider.as_str() {
            "anthropic" => config.llm.provider = Some(LlmProvider::Anthropic),
            "openai" => config.llm.provider = Some(LlmProvider::OpenAI),
            "openrouter" => config.llm.provider = Some(LlmProvider::OpenRouter),
            _ => {}
        }
    }
    if let Ok(model) = env::var("BOLLY_LLM_MODEL") {
        if !model.is_empty() {
            config.llm.model = Some(model);
        }
    }
    if let Ok(fast) = env::var("BOLLY_LLM_FAST_MODEL") {
        if !fast.is_empty() {
            config.llm.fast_model = Some(fast);
        }
    }
    if let Ok(mode) = env::var("BOLLY_MODEL_MODE") {
        match mode.to_lowercase().as_str() {
            "auto" => config.llm.model_mode = ModelMode::Auto,
            "fast" => config.llm.model_mode = ModelMode::Fast,
            "heavy" => config.llm.model_mode = ModelMode::Heavy,
            _ => {}
        }
    }

    Ok(config)
}
