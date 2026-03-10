use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
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
}

fn default_port() -> u16 {
    8080
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            llm: LlmConfig::default(),
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
        }
    }
}

pub fn workspace_root() -> PathBuf {
    if let Some(path) = env::var_os("PERSONALITY_HOME") {
        return PathBuf::from(path);
    }

    dirs::home_dir()
        .expect("failed to resolve home directory")
        .join(".personality")
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
    let config: Config = toml::from_str(&raw)?;
    ensure_workspace_layout(&workspace_root())?;
    Ok(config)
}
