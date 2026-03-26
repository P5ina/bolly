use crate::domain::skill::{Skill, SkillKind};

/// Response from `GET /v1/skills`.
#[derive(serde::Deserialize)]
struct ListSkillsResponse {
    data: Vec<ApiSkill>,
}

#[derive(serde::Deserialize)]
struct ApiSkill {
    id: String,
    display_title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    latest_version: String,
}

/// Fetch all available Anthropic skills (built-in + custom uploaded).
pub async fn fetch_available_skills(api_key: &str) -> Result<Vec<Skill>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .get("https://api.anthropic.com/v1/skills")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "skills-2025-10-02")
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API returned {status}: {body}"));
    }

    let list: ListSkillsResponse = resp
        .json()
        .await
        .map_err(|e| format!("parse error: {e}"))?;

    let skills = list.data.into_iter().map(|s| {
        let builtin = s.source == "anthropic";
        Skill {
            id: s.id.clone(),
            name: s.display_title,
            description: s.description,
            icon: String::new(),
            builtin,
            enabled: true,
            kind: SkillKind::Anthropic,
            anthropic_skill_id: Some(s.id),
            anthropic_version: Some(if s.latest_version.is_empty() { "latest".into() } else { s.latest_version }),
            instructions: String::new(),
            source: None,
            resources: Vec::new(),
        }
    }).collect();

    Ok(skills)
}
