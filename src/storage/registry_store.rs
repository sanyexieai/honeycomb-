use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::registry::{
    GovernanceDefaultsRecord, ImplementationRecord, PolicyAlertAck, ShellApprovalRequest,
    SkillRecord, ToolRecord,
};

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

pub fn persist_governance_defaults(
    root: impl AsRef<Path>,
    defaults: &GovernanceDefaultsRecord,
) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("registry");
    fs::create_dir_all(&dir)?;

    let path = dir.join("governance-defaults.json");
    let body = to_pretty_json(defaults)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn load_governance_defaults(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, GovernanceDefaultsRecord)> {
    let path = root
        .as_ref()
        .join("registry")
        .join("governance-defaults.json");
    let body = fs::read_to_string(&path)?;
    let defaults = from_json::<GovernanceDefaultsRecord>(&body)?;
    Ok((path, defaults))
}

pub fn persist_implementation(
    root: impl AsRef<Path>,
    implementation: &ImplementationRecord,
) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("registry").join("implementations");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!(
        "{}.json",
        sanitize_filename(&implementation.implementation_id)
    ));
    let body = to_pretty_json(implementation)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn load_implementation(
    root: impl AsRef<Path>,
    implementation_id: &str,
) -> io::Result<(PathBuf, ImplementationRecord)> {
    let path = root
        .as_ref()
        .join("registry")
        .join("implementations")
        .join(format!("{}.json", sanitize_filename(implementation_id)));
    let body = fs::read_to_string(&path)?;
    let implementation = from_json::<ImplementationRecord>(&body)?;
    Ok((path, implementation))
}

pub fn update_implementation<F>(
    root: impl AsRef<Path>,
    implementation_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, ImplementationRecord)>
where
    F: FnOnce(&mut ImplementationRecord) -> io::Result<()>,
{
    let (path, mut implementation) = load_implementation(root.as_ref(), implementation_id)?;
    mutate(&mut implementation)?;
    let body = to_pretty_json(&implementation)?;
    write_atomic(&path, &body)?;
    Ok((path, implementation))
}

pub fn list_implementations(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, Vec<ImplementationRecord>)> {
    let dir = root.as_ref().join("registry").join("implementations");
    let mut implementations = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            implementations.push(from_json::<ImplementationRecord>(&body)?);
        }
        implementations.sort_by(|a, b| a.implementation_id.cmp(&b.implementation_id));
    }

    Ok((dir, implementations))
}

fn ensure_skill_matches_implementation(
    skill: &SkillRecord,
    field_name: &str,
    implementation: &ImplementationRecord,
) -> io::Result<()> {
    if implementation.skill_id != skill.skill_id {
        return Err(io::Error::other(format!(
            "skill {} has invalid {} {}; implementation belongs to skill {}",
            skill.skill_id, field_name, implementation.implementation_id, implementation.skill_id
        )));
    }
    Ok(())
}

pub fn load_skill_implementations(
    root: impl AsRef<Path>,
    skill: &SkillRecord,
) -> io::Result<(
    (PathBuf, ImplementationRecord),
    Option<(PathBuf, ImplementationRecord)>,
)> {
    let primary = load_implementation(root.as_ref(), &skill.implementation_ref)?;
    ensure_skill_matches_implementation(skill, "implementation_ref", &primary.1)?;

    let recommended = match &skill.recommended_implementation_id {
        Some(implementation_id) => {
            let loaded = load_implementation(root.as_ref(), implementation_id)?;
            ensure_skill_matches_implementation(skill, "recommended_implementation_id", &loaded.1)?;
            Some(loaded)
        }
        None => None,
    };

    Ok((primary, recommended))
}

pub fn validate_skill_implementation_refs(
    root: impl AsRef<Path>,
    skill: &SkillRecord,
) -> io::Result<()> {
    load_skill_implementations(root, skill).map(|_| ())
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

pub fn update_tool<F>(
    root: impl AsRef<Path>,
    tool_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, ToolRecord)>
where
    F: FnOnce(&mut ToolRecord) -> io::Result<()>,
{
    let (path, mut tool) = load_tool(root.as_ref(), tool_id)?;
    mutate(&mut tool)?;
    let body = to_pretty_json(&tool)?;
    write_atomic(&path, &body)?;
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

pub fn persist_shell_approval_request(
    root: impl AsRef<Path>,
    request: &ShellApprovalRequest,
) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("registry").join("approval_requests");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", sanitize_filename(&request.request_id)));
    let body = to_pretty_json(request)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn load_shell_approval_request(
    root: impl AsRef<Path>,
    request_id: &str,
) -> io::Result<(PathBuf, ShellApprovalRequest)> {
    let path = root
        .as_ref()
        .join("registry")
        .join("approval_requests")
        .join(format!("{}.json", sanitize_filename(request_id)));
    let body = fs::read_to_string(&path)?;
    let request = from_json::<ShellApprovalRequest>(&body)?;
    Ok((path, request))
}

pub fn update_shell_approval_request<F>(
    root: impl AsRef<Path>,
    request_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, ShellApprovalRequest)>
where
    F: FnOnce(&mut ShellApprovalRequest) -> io::Result<()>,
{
    let (path, mut request) = load_shell_approval_request(root.as_ref(), request_id)?;
    mutate(&mut request)?;
    let body = to_pretty_json(&request)?;
    write_atomic(&path, &body)?;
    Ok((path, request))
}

pub fn list_shell_approval_requests(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, Vec<ShellApprovalRequest>)> {
    let dir = root.as_ref().join("registry").join("approval_requests");
    let mut requests = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            requests.push(from_json::<ShellApprovalRequest>(&body)?);
        }
        requests.sort_by(|a, b| a.request_id.cmp(&b.request_id));
    }

    Ok((dir, requests))
}

pub fn persist_policy_alert_ack(
    root: impl AsRef<Path>,
    ack: &PolicyAlertAck,
) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("registry").join("alert_acks");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", sanitize_filename(&ack.alert_id)));
    let body = to_pretty_json(ack)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn list_policy_alert_acks(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, Vec<PolicyAlertAck>)> {
    let dir = root.as_ref().join("registry").join("alert_acks");
    let mut acks = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            acks.push(from_json::<PolicyAlertAck>(&body)?);
        }
        acks.sort_by(|a, b| a.alert_id.cmp(&b.alert_id));
    }

    Ok((dir, acks))
}

pub fn delete_policy_alert_ack(root: impl AsRef<Path>, alert_id: &str) -> io::Result<PathBuf> {
    let path = root
        .as_ref()
        .join("registry")
        .join("alert_acks")
        .join(format!("{}.json", sanitize_filename(alert_id)));
    fs::remove_file(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::registry::{
        AlertKind, ApprovalRequestStatus, GovernanceDefaultsRecord, ImplementationCompatibility,
        ImplementationEntry, ImplementationOrigin, ImplementationRecord, PolicyAlertAck,
        ShellApprovalRequest, SkillRecord, ToolRecord,
    };

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
    fn persist_and_load_governance_defaults() {
        let root = unique_test_root();
        let mut defaults = GovernanceDefaultsRecord::new();
        defaults.governance_policy.insert(
            "review_refresh_min_absolute_increase".to_owned(),
            "7".to_owned(),
        );
        defaults.governance_policy.insert(
            "review_severity_weight_active_tasks".to_owned(),
            "4".to_owned(),
        );
        defaults.updated_at = Some("unix_ms:321".to_owned());

        persist_governance_defaults(&root, &defaults).expect("governance defaults should persist");
        let (_, loaded) = load_governance_defaults(&root).expect("governance defaults should load");

        assert_eq!(
            loaded
                .governance_policy
                .get("review_refresh_min_absolute_increase")
                .map(String::as_str),
            Some("7")
        );
        assert_eq!(
            loaded
                .governance_policy
                .get("review_severity_weight_active_tasks")
                .map(String::as_str),
            Some("4")
        );
        assert_eq!(loaded.updated_at.as_deref(), Some("unix_ms:321"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_list_implementations() {
        let root = unique_test_root();
        let mut implementation = ImplementationRecord::new(
            "impl-xhs-v1".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            ImplementationEntry::new("script".to_owned(), "scripts/xhs_publish_v1.sh".to_owned()),
            ImplementationCompatibility::new(
                "xhs_publish".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );
        implementation
            .components
            .insert("prompt".to_owned(), "prompts/xhs.md".to_owned());
        implementation
            .strategy
            .insert("mode".to_owned(), "draft_then_publish".to_owned());
        implementation.origin = Some(ImplementationOrigin::new("manual".to_owned(), None));

        persist_implementation(&root, &implementation).expect("implementation should persist");

        let (_, implementations) =
            list_implementations(&root).expect("implementations should list");
        assert_eq!(implementations.len(), 1);
        assert_eq!(implementations[0].implementation_id, "impl-xhs-v1");
        assert_eq!(implementations[0].skill_id, "xhs_publish");
        assert_eq!(implementations[0].entry.kind, "script");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_implementation_persists_changes() {
        let root = unique_test_root();
        let implementation = ImplementationRecord::new(
            "impl-xhs-v1".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            ImplementationEntry::new("script".to_owned(), "scripts/xhs_publish_v1.sh".to_owned()),
            ImplementationCompatibility::new(
                "xhs_publish".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );

        persist_implementation(&root, &implementation).expect("implementation should persist");
        let (_, updated) = update_implementation(&root, "impl-xhs-v1", |record| {
            record.executor = "resident_worker".to_owned();
            record.entry.path = "scripts/xhs_publish_v2.sh".to_owned();
            record
                .constraints
                .insert("max_latency_ms".to_owned(), "5000".to_owned());
            Ok(())
        })
        .expect("implementation should update");

        assert_eq!(updated.executor, "resident_worker");
        assert_eq!(updated.entry.path, "scripts/xhs_publish_v2.sh");
        assert_eq!(
            updated
                .constraints
                .get("max_latency_ms")
                .map(String::as_str),
            Some("5000")
        );

        let (_, loaded) =
            load_implementation(&root, "impl-xhs-v1").expect("implementation should load");
        assert_eq!(loaded.executor, "resident_worker");
        assert_eq!(loaded.entry.path, "scripts/xhs_publish_v2.sh");

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
            false,
            false,
            None,
        );

        persist_tool(&root, &tool).expect("tool should persist");
        let (_, loaded) = load_tool(&root, &tool.tool_id).expect("tool should load");

        assert_eq!(loaded.tool_id, "xhs_browser_login");
        assert_eq!(loaded.entrypoint, "tool://browser/login");
        assert!(!loaded.allow_shell);
        assert!(!loaded.shell_approval_pending);

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

    #[test]
    fn validate_skill_implementation_refs_accepts_matching_records() {
        let root = unique_test_root();
        let mut skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl-xhs-v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec![],
            None,
        );
        skill.recommended_implementation_id = Some("impl-xhs-v2".to_owned());

        let primary = ImplementationRecord::new(
            "impl-xhs-v1".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            ImplementationEntry::new("script".to_owned(), "scripts/xhs_publish_v1.sh".to_owned()),
            ImplementationCompatibility::new(
                "xhs_publish".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );
        let recommended = ImplementationRecord::new(
            "impl-xhs-v2".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            ImplementationEntry::new("script".to_owned(), "scripts/xhs_publish_v2.sh".to_owned()),
            ImplementationCompatibility::new(
                "xhs_publish".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );

        persist_implementation(&root, &primary).expect("primary implementation should persist");
        persist_implementation(&root, &recommended)
            .expect("recommended implementation should persist");

        validate_skill_implementation_refs(&root, &skill)
            .expect("skill implementation refs should validate");
        let ((_, loaded_primary), Some((_, loaded_recommended))) =
            load_skill_implementations(&root, &skill).expect("implementations should load")
        else {
            panic!("recommended implementation should load");
        };

        assert_eq!(loaded_primary.implementation_id, "impl-xhs-v1");
        assert_eq!(loaded_recommended.implementation_id, "impl-xhs-v2");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn validate_skill_implementation_refs_rejects_cross_skill_binding() {
        let root = unique_test_root();
        let skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl-video-v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec![],
            None,
        );

        let implementation = ImplementationRecord::new(
            "impl-video-v1".to_owned(),
            "video_publish".to_owned(),
            "worker_process".to_owned(),
            ImplementationEntry::new(
                "script".to_owned(),
                "scripts/video_publish_v1.sh".to_owned(),
            ),
            ImplementationCompatibility::new(
                "video_publish".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );
        persist_implementation(&root, &implementation).expect("implementation should persist");

        let error = validate_skill_implementation_refs(&root, &skill)
            .expect_err("cross-skill implementation binding should fail");
        assert!(
            error
                .to_string()
                .contains("implementation_ref impl-video-v1")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_tool_persists_allow_shell_change() {
        let root = unique_test_root();
        let tool = ToolRecord::new(
            "shell_echo".to_owned(),
            "Shell Echo".to_owned(),
            "Shell tool".to_owned(),
            "shell://printf ok".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            false,
            false,
            None,
        );

        persist_tool(&root, &tool).expect("tool should persist");
        let (_, updated) = update_tool(&root, "shell_echo", |tool| {
            tool.allow_shell = true;
            Ok(())
        })
        .expect("tool should update");

        assert!(updated.allow_shell);
        assert!(!updated.shell_approval_pending);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_tool_persists_shell_pending_change() {
        let root = unique_test_root();
        let tool = ToolRecord::new(
            "shell_pending".to_owned(),
            "Shell Pending".to_owned(),
            "Shell tool pending approval".to_owned(),
            "shell://printf ok".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            false,
            false,
            None,
        );

        persist_tool(&root, &tool).expect("tool should persist");
        let (_, updated) = update_tool(&root, "shell_pending", |tool| {
            tool.shell_approval_pending = true;
            Ok(())
        })
        .expect("tool should update");

        assert!(updated.shell_approval_pending);
        assert!(!updated.allow_shell);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_update_shell_approval_request() {
        let root = unique_test_root();
        let request = ShellApprovalRequest::pending(
            "shell-approval-shell_echo-unix_ms_123".to_owned(),
            "shell_echo".to_owned(),
            "tenant-local".to_owned(),
            "shell://printf ok".to_owned(),
            "requester-local".to_owned(),
            "unix_ms:123".to_owned(),
        );

        persist_shell_approval_request(&root, &request).expect("request should persist");
        let (_, updated) = update_shell_approval_request(&root, &request.request_id, |request| {
            request.status = ApprovalRequestStatus::Approved;
            request.resolved_at = Some("unix_ms:456".to_owned());
            request.resolved_by = Some("approver-local".to_owned());
            request.resolution_note = Some("approved via cli".to_owned());
            Ok(())
        })
        .expect("request should update");

        assert_eq!(updated.status, ApprovalRequestStatus::Approved);
        assert_eq!(updated.requested_by, "requester-local");
        assert_eq!(updated.resolved_at.as_deref(), Some("unix_ms:456"));
        assert_eq!(updated.resolved_by.as_deref(), Some("approver-local"));
        assert_eq!(updated.resolution_note.as_deref(), Some("approved via cli"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_list_policy_alert_acks() {
        let root = unique_test_root();
        let ack = PolicyAlertAck::new(
            "blocked-tool-shell_echo".to_owned(),
            AlertKind::BlockedTool,
            "shell_echo".to_owned(),
            "reviewer-a".to_owned(),
            "unix_ms:789".to_owned(),
            Some("accepted risk".to_owned()),
        );

        persist_policy_alert_ack(&root, &ack).expect("ack should persist");
        let (_, acks) = list_policy_alert_acks(&root).expect("acks should list");

        assert_eq!(acks.len(), 1);
        assert_eq!(acks[0].alert_id, "blocked-tool-shell_echo");
        assert_eq!(acks[0].kind, AlertKind::BlockedTool);
        assert_eq!(acks[0].acked_by, "reviewer-a");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn delete_policy_alert_ack_removes_file() {
        let root = unique_test_root();
        let ack = PolicyAlertAck::new(
            "blocked-tool-shell_echo".to_owned(),
            AlertKind::BlockedTool,
            "shell_echo".to_owned(),
            "reviewer-a".to_owned(),
            "unix_ms:789".to_owned(),
            None,
        );

        persist_policy_alert_ack(&root, &ack).expect("ack should persist");
        let deleted = delete_policy_alert_ack(&root, &ack.alert_id).expect("ack should delete");

        assert!(!deleted.exists());

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
