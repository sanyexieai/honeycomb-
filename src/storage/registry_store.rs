use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::registry::{SkillRecord, ToolRecord};

use super::{from_json, sanitize_filename, to_pretty_json, write_atomic};

pub fn persist_skill(root: impl AsRef<Path>, skill: &SkillRecord) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("registry").join("skills");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", sanitize_filename(&skill.skill_id)));
    let body = to_pretty_json(skill)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn load_skill(root: impl AsRef<Path>, skill_id: &str) -> io::Result<(PathBuf, SkillRecord)> {
    let path = root
        .as_ref()
        .join("registry")
        .join("skills")
        .join(format!("{}.json", sanitize_filename(skill_id)));
    let body = fs::read_to_string(&path)?;
    let skill = from_json::<SkillRecord>(&body)?;
    Ok((path, skill))
}

pub fn update_skill<F>(
    root: impl AsRef<Path>,
    skill_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, SkillRecord)>
where
    F: FnOnce(&mut SkillRecord) -> io::Result<()>,
{
    let (path, mut skill) = load_skill(root.as_ref(), skill_id)?;
    mutate(&mut skill)?;
    let body = to_pretty_json(&skill)?;
    write_atomic(&path, &body)?;
    Ok((path, skill))
}

pub fn list_skills(root: impl AsRef<Path>) -> io::Result<(PathBuf, Vec<SkillRecord>)> {
    let dir = root.as_ref().join("registry").join("skills");
    let mut skills = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            skills.push(from_json::<SkillRecord>(&body)?);
        }
        skills.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
    }

    Ok((dir, skills))
}

pub fn persist_tool(root: impl AsRef<Path>, tool: &ToolRecord) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("registry").join("tools");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", sanitize_filename(&tool.tool_id)));
    let body = to_pretty_json(tool)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn load_tool(root: impl AsRef<Path>, tool_id: &str) -> io::Result<(PathBuf, ToolRecord)> {
    let path = root
        .as_ref()
        .join("registry")
        .join("tools")
        .join(format!("{}.json", sanitize_filename(tool_id)));
    let body = fs::read_to_string(&path)?;
    let tool = from_json::<ToolRecord>(&body)?;
    Ok((path, tool))
}

pub fn list_tools(root: impl AsRef<Path>) -> io::Result<(PathBuf, Vec<ToolRecord>)> {
    let dir = root.as_ref().join("registry").join("tools");
    let mut tools = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            tools.push(from_json::<ToolRecord>(&body)?);
        }
        tools.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
    }

    Ok((dir, tools))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::registry::{SkillRecord, ToolRecord};

    use super::*;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-registry-test-{nanos}"))
    }

    #[test]
    fn persist_and_list_skills() {
        let root = unique_test_root();
        let skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl://xhs/publish/v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec!["xhs_browser_login".to_owned()],
            Some("publish xhs draft".to_owned()),
        );

        persist_skill(&root, &skill).expect("skill should persist");
        let (_, skills) = list_skills(&root).expect("skills should list");

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_id, "xhs_publish");
        assert_eq!(skills[0].default_tool_refs, vec!["xhs_browser_login"]);
        assert_eq!(
            skills[0].goal_template.as_deref(),
            Some("publish xhs draft")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_load_tool() {
        let root = unique_test_root();
        let tool = ToolRecord::new(
            "xhs_browser_login".to_owned(),
            "XHS Browser Login".to_owned(),
            "Browser login helper".to_owned(),
            "tool://browser/login".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
        );

        persist_tool(&root, &tool).expect("tool should persist");
        let (_, loaded) = load_tool(&root, &tool.tool_id).expect("tool should load");

        assert_eq!(loaded.tool_id, "xhs_browser_login");
        assert_eq!(loaded.entrypoint, "tool://browser/login");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_skill_persists_sync_metadata() {
        let root = unique_test_root();
        let skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl://xhs/publish/v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec![],
            None,
        );

        persist_skill(&root, &skill).expect("skill should persist");
        let (_, updated) = update_skill(&root, "xhs_publish", |skill| {
            skill.recommended_implementation_id = Some("impl-xhs-v4".to_owned());
            skill.governance_decision = Some(crate::governance::GovernanceDecision::Hold);
            skill.last_synced_at = Some("unix_ms:123".to_owned());
            Ok(())
        })
        .expect("skill should update");

        assert_eq!(
            updated.recommended_implementation_id.as_deref(),
            Some("impl-xhs-v4")
        );
        assert_eq!(
            updated.governance_decision,
            Some(crate::governance::GovernanceDecision::Hold)
        );
        assert_eq!(updated.last_synced_at.as_deref(), Some("unix_ms:123"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
