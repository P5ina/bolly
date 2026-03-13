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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    OpenRouter,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub provider: Option<LlmProvider>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tokens: LlmTokens,
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
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            tokens: LlmTokens::default(),
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
                config.llm.model = Some("claude-sonnet-4-6".to_string());
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
            // OpenRouter takes priority over other providers for chat
            config.llm.provider = Some(LlmProvider::OpenRouter);
            config.llm.model = Some("moonshotai/kimi-k2.5".to_string());
        }
    }
    if let Ok(key) = env::var("BRAVE_SEARCH_API_KEY") {
        if !key.is_empty() {
            config.llm.tokens.brave_search = key;
        }
    }

    Ok(config)
}
