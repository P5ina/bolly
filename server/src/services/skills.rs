use std::{fs, io, path::Path};

use crate::domain::skill::Skill;

/// The built-in "skill_creator" skill that is always present.
fn builtin_skill_creator() -> Skill {
    Skill {
        id: "skill_creator".into(),
        name: "Skill Creator".into(),
        description: "Create and manage new skills for your companion. Teach it new abilities by defining instructions, triggers, and behaviors.".into(),
        icon: "+".into(),
        builtin: true,
        enabled: true,
        instructions: String::new(),
    }
}

/// Read all skills: builtins + user-created ones from the skills directory.
pub fn list_skills(workspace_dir: &Path) -> Vec<Skill> {
    let mut skills = vec![builtin_skill_creator()];

    let skills_dir = workspace_dir.join("skills");
    if skills_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&skills_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(skill) = read_skill_dir(&path) {
                        skills.push(skill);
                    }
                }
            }
        }
    }

    skills.sort_by(|a, b| {
        // Builtins first, then alphabetical
        b.builtin.cmp(&a.builtin).then(a.name.cmp(&b.name))
    });

    skills
}

/// Read a single skill from its directory (expects skill.json).
fn read_skill_dir(path: &Path) -> Option<Skill> {
    let manifest = path.join("skill.json");
    let raw = fs::read_to_string(&manifest).ok()?;
    let mut skill: Skill = serde_json::from_str(&raw).ok()?;

    // Ensure ID matches directory name
    if skill.id.is_empty() {
        skill.id = path.file_name()?.to_string_lossy().to_string();
    }

    // Read instructions from instructions.md if present and not inline
    if skill.instructions.is_empty() {
        let instructions_file = path.join("instructions.md");
        if let Ok(content) = fs::read_to_string(&instructions_file) {
            skill.instructions = content;
        }
    }

    Some(skill)
}

/// Get a single skill by ID.
pub fn get_skill(workspace_dir: &Path, skill_id: &str) -> Option<Skill> {
    list_skills(workspace_dir)
        .into_iter()
        .find(|s| s.id == skill_id)
}

/// Create a new skill directory with manifest.
pub fn create_skill(workspace_dir: &Path, skill: &Skill) -> io::Result<()> {
    let skill_dir = workspace_dir.join("skills").join(&skill.id);
    fs::create_dir_all(&skill_dir)?;

    let manifest = serde_json::to_string_pretty(skill)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(skill_dir.join("skill.json"), manifest)?;

    if !skill.instructions.is_empty() {
        fs::write(skill_dir.join("instructions.md"), &skill.instructions)?;
    }

    Ok(())
}

/// Delete a user-created skill (cannot delete builtins).
pub fn delete_skill(workspace_dir: &Path, skill_id: &str) -> io::Result<bool> {
    // Prevent deleting builtins
    if skill_id == "skill_creator" {
        return Ok(false);
    }

    let skill_dir = workspace_dir.join("skills").join(skill_id);
    if skill_dir.is_dir() {
        fs::remove_dir_all(&skill_dir)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
