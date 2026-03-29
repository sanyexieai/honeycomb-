use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::registry::{
    ImplementationCompatibility, ImplementationEntry, ImplementationRecord, SkillRecord,
};
use crate::runtime::{Assignment, TaskRuntime, TaskSpec, TaskStatus, Trigger, TriggerStatus};
use crate::storage::{
    load_assignment, load_task_assignments, load_task_submission, load_trigger, persist_assignment,
    persist_implementation, persist_skill, persist_task_submission, persist_trigger,
    update_task_submission, update_trigger,
};

use super::common_support::{entrypoint_scheme, owner_trust_tier};
use super::overview::support::{
    AlertOwnerSummary, RuntimeOverviewPolicyRecentChangeJson, SystemAlertJson,
    collect_system_alerts, sort_alert_owner_summaries, sort_policy_recent_changes,
    system_alert_summary_key,
};
use super::scheduler::{handle_scheduler_loop, handle_scheduler_run_once};
use super::task::handle_task_rerun;
use super::task::{handle_task_reopen, handle_task_submit, resolve_skill_submission_preset};
use super::trigger::handle_trigger_clear_ready;

fn unique_test_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("honeycomb-execution-test-{nanos}"))
}

fn persist_test_implementation(root: &PathBuf, implementation_id: &str, skill_id: &str) {
    let implementation = ImplementationRecord::new(
        implementation_id.to_owned(),
        skill_id.to_owned(),
        "worker_process".to_owned(),
        ImplementationEntry::new(
            "script".to_owned(),
            format!("scripts/{implementation_id}.sh"),
        ),
        ImplementationCompatibility::new(
            skill_id.to_owned(),
            "1.0.0".to_owned(),
            "1.0.0".to_owned(),
        ),
    );
    persist_implementation(root, &implementation).expect("implementation should persist");
}

#[test]
fn resolve_skill_submission_preset_uses_skill_defaults() {
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
    persist_test_implementation(&root, "impl://xhs/publish/v1", "xhs_publish");
    let (goal, implementation_ref, skill_refs, tool_refs, source_skill) =
        resolve_skill_submission_preset(
            root.to_str().expect("temp dir should be valid utf-8"),
            Some("xhs_publish"),
            None,
            false,
            vec![],
            vec![],
        )
        .expect("preset should resolve");

    assert_eq!(goal, "publish xhs draft");
    assert_eq!(implementation_ref.as_deref(), Some("impl://xhs/publish/v1"));
    assert_eq!(skill_refs, vec!["xhs_publish"]);
    assert_eq!(tool_refs, vec!["xhs_browser_login"]);
    assert_eq!(
        source_skill.expect("source skill should exist").skill_id,
        "xhs_publish"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_submit_persists_implementation_snapshot() {
    let root = unique_test_root();
    let mut implementation = ImplementationRecord::new(
        "impl://xhs/publish/v1".to_owned(),
        "xhs_publish".to_owned(),
        "worker_process".to_owned(),
        ImplementationEntry::new(
            "script".to_owned(),
            "scripts/impl-xhs-publish-v1.sh".to_owned(),
        ),
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
    implementation
        .constraints
        .insert("max_latency_ms".to_owned(), "5000".to_owned());
    persist_implementation(&root, &implementation).expect("implementation should persist");

    let args = vec![
        "task".to_owned(),
        "submit".to_owned(),
        "--task-id".to_owned(),
        "task-submit-snapshot".to_owned(),
        "--tenant".to_owned(),
        "tenant-local".to_owned(),
        "--namespace".to_owned(),
        "user/demo".to_owned(),
        "--goal".to_owned(),
        "publish".to_owned(),
        "--implementation-ref".to_owned(),
        "impl://xhs/publish/v1".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];

    let exit = handle_task_submit(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, record) = load_task_submission(&root, "task-submit-snapshot")
        .expect("submitted task should be readable");
    let snapshot = record
        .task_spec
        .implementation_snapshot
        .expect("task submit should persist implementation snapshot");
    assert_eq!(snapshot.implementation_id, "impl://xhs/publish/v1");
    assert_eq!(snapshot.skill_id, "xhs_publish");
    assert_eq!(snapshot.executor, "worker_process");
    assert_eq!(snapshot.entry_kind, "script");
    assert_eq!(snapshot.entry_path, "scripts/impl-xhs-publish-v1.sh");
    assert_eq!(
        snapshot.strategy_mode.as_deref(),
        Some("draft_then_publish")
    );
    assert_eq!(snapshot.prompt_component.as_deref(), Some("prompts/xhs.md"));
    assert_eq!(snapshot.max_latency_ms.as_deref(), Some("5000"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn resolve_skill_submission_preset_keeps_explicit_values() {
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
    persist_test_implementation(&root, "impl://xhs/publish/v1", "xhs_publish");
    let (goal, implementation_ref, skill_refs, tool_refs, _) = resolve_skill_submission_preset(
        root.to_str().expect("temp dir should be valid utf-8"),
        Some("xhs_publish"),
        Some("manual goal"),
        false,
        vec!["custom_skill".to_owned()],
        vec!["custom_tool".to_owned(), "xhs_browser_login".to_owned()],
    )
    .expect("preset should resolve");

    assert_eq!(goal, "manual goal");
    assert_eq!(implementation_ref.as_deref(), Some("impl://xhs/publish/v1"));
    assert_eq!(
        skill_refs,
        vec!["xhs_publish".to_owned(), "custom_skill".to_owned()]
    );
    assert_eq!(
        tool_refs,
        vec!["xhs_browser_login".to_owned(), "custom_tool".to_owned()]
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn resolve_skill_submission_preset_uses_recommended_implementation_when_requested() {
    let root = unique_test_root();
    let mut skill = SkillRecord::new(
        "xhs_publish".to_owned(),
        "XHS Publish".to_owned(),
        "Publish a post to Xiaohongshu".to_owned(),
        "impl://xhs/publish/v1".to_owned(),
        "tenant-local".to_owned(),
        "1.0.0".to_owned(),
        vec!["xhs_browser_login".to_owned()],
        Some("publish xhs draft".to_owned()),
    );
    skill.recommended_implementation_id = Some("impl-xhs-v4".to_owned());

    persist_skill(&root, &skill).expect("skill should persist");
    persist_test_implementation(&root, "impl://xhs/publish/v1", "xhs_publish");
    persist_test_implementation(&root, "impl-xhs-v4", "xhs_publish");
    let (_, implementation_ref, _, _, _) = resolve_skill_submission_preset(
        root.to_str().expect("temp dir should be valid utf-8"),
        Some("xhs_publish"),
        None,
        true,
        vec![],
        vec![],
    )
    .expect("preset should resolve");

    assert_eq!(implementation_ref.as_deref(), Some("impl-xhs-v4"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn resolve_skill_submission_preset_rejects_missing_implementation_record() {
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
    let error = resolve_skill_submission_preset(
        root.to_str().expect("temp dir should be valid utf-8"),
        Some("xhs_publish"),
        None,
        false,
        vec![],
        vec![],
    )
    .expect_err("preset should reject missing implementation record");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn entrypoint_scheme_detects_known_prefixes() {
    assert_eq!(entrypoint_scheme("shell://printf ok"), "shell");
    assert_eq!(entrypoint_scheme("tool://browser/login"), "tool");
    assert_eq!(entrypoint_scheme("custom-runner"), "custom");
}

#[test]
fn owner_trust_tier_maps_known_owners() {
    assert_eq!(owner_trust_tier("system"), "system");
    assert_eq!(owner_trust_tier("tenant-local"), "trusted_local");
    assert_eq!(owner_trust_tier("tenant-remote"), "tenant");
}

#[test]
fn sort_alert_owner_summaries_supports_count_and_target() {
    let mut rows = vec![
        AlertOwnerSummary {
            owner: "tenant-b".to_owned(),
            count: 1,
        },
        AlertOwnerSummary {
            owner: "tenant-a".to_owned(),
            count: 3,
        },
    ];
    sort_alert_owner_summaries(&mut rows, "count");
    assert_eq!(rows[0].owner, "tenant-a");

    sort_alert_owner_summaries(&mut rows, "target");
    assert_eq!(rows[0].owner, "tenant-a");
    assert_eq!(rows[1].owner, "tenant-b");
}

#[test]
fn sort_policy_recent_changes_supports_count_and_target() {
    let mut rows = vec![
        RuntimeOverviewPolicyRecentChangeJson {
            timestamp: "unix_ms_2".to_owned(),
            action: "tool_revoke_shell".to_owned(),
            tool_id: "tool-b".to_owned(),
            result: "blocked".to_owned(),
            detail: String::new(),
        },
        RuntimeOverviewPolicyRecentChangeJson {
            timestamp: "unix_ms_1".to_owned(),
            action: "tool_authorize_shell".to_owned(),
            tool_id: "tool-a".to_owned(),
            result: "allowed".to_owned(),
            detail: String::new(),
        },
    ];
    sort_policy_recent_changes(&mut rows, "count");
    assert_eq!(rows[0].timestamp, "unix_ms_2");

    sort_policy_recent_changes(&mut rows, "target");
    assert_eq!(rows[0].tool_id, "tool-a");
    assert_eq!(rows[1].tool_id, "tool-b");
}

#[test]
fn system_alert_summary_key_supports_owner_kind_and_severity() {
    let alert = SystemAlertJson {
        kind: "blocked_tool".to_owned(),
        severity: "warning".to_owned(),
        owner: Some("tenant-local".to_owned()),
        target: "shell_blocked".to_owned(),
        detail: String::new(),
    };

    assert_eq!(system_alert_summary_key(&alert, "owner"), "tenant-local");
    assert_eq!(system_alert_summary_key(&alert, "kind"), "blocked_tool");
    assert_eq!(system_alert_summary_key(&alert, "severity"), "warning");
}

#[test]
fn system_alert_summary_key_falls_back_for_missing_owner() {
    let alert = SystemAlertJson {
        kind: "active_task".to_owned(),
        severity: "attention".to_owned(),
        owner: None,
        target: "task-demo".to_owned(),
        detail: String::new(),
    };

    assert_eq!(system_alert_summary_key(&alert, "owner"), "<none>");
}

#[test]
fn collect_system_alerts_includes_trigger_waiting_consumption() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-trigger-alert".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "wait for fired trigger".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let mut trigger = Trigger::active(
        "trigger-alert".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    trigger
        .try_record_fire("unix_ms:1".to_owned())
        .expect("trigger should fire");
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let alerts = collect_system_alerts(
        root.to_str().expect("temp dir should be valid utf-8"),
        &[load_task_submission(&root, &spec.task_id)
            .expect("task should load")
            .1],
        &[],
        &[],
        &std::collections::BTreeSet::new(),
        &std::collections::BTreeMap::new(),
        &std::collections::BTreeMap::new(),
        None,
        Some("trigger_waiting_consumption"),
        None,
        false,
    )
    .expect("alerts should collect");

    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "trigger_waiting_consumption");
    assert_eq!(alerts[0].severity, "attention");
    assert_eq!(alerts[0].target, "task-trigger-alert");
    assert!(alerts[0].detail.contains("ready_trigger_count=1"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn collect_rerun_plan_alerts_includes_pending_plan_tasks() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-alert-plan".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "planned rerun task".to_owned(),
        None,
        vec![],
        vec![],
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let plans_dir = root.join("plans");
    fs::create_dir_all(&plans_dir).expect("plans dir should exist");
    let plan = crate::app::execution::TaskRerunBatchJson {
        mode: "all_completed".to_owned(),
        dry_run: true,
        summary_only: false,
        task_count: 1,
        tasks: vec![crate::app::execution::TaskRerunJson {
            task_id: spec.task_id.clone(),
            status: "completed".to_owned(),
            trigger_id: None,
            trigger_fired: false,
            schedule_now: false,
            scheduled: false,
            scheduled_assignment_id: None,
        }],
    };
    crate::app::execution::save_rerun_plan(
        plans_dir
            .join("alert-plan.json")
            .to_str()
            .expect("path should be utf-8"),
        &plan,
    )
    .expect("plan should save");

    let alerts = crate::app::execution::collect_rerun_plan_alerts(
        root.to_str().expect("root should be utf-8"),
        &[load_task_submission(&root, &spec.task_id)
            .expect("task should load")
            .1],
        None,
        Some("rerun_plan_pending"),
        Some("attention"),
    )
    .expect("alerts should collect");

    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "rerun_plan_pending");
    assert_eq!(alerts[0].target, spec.task_id);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_assigns_queued_task() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-queued".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        Some("impl-xhs-v4".to_owned()),
        vec!["xhs_publish".to_owned()],
        vec!["xhs_browser_login".to_owned()],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, "task-scheduler-queued").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Running);
    let (_, assignment) = load_assignment(
        &root,
        "task-scheduler-queued",
        "sched-task-scheduler-queued-1",
    )
    .expect("assignment should load");
    assert_eq!(assignment.worker_node_id, "worker-scheduler");
    assert_eq!(
        assignment.implementation_ref.as_deref(),
        Some("impl-xhs-v4")
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_skips_task_with_active_assignment() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        None,
        vec![],
        vec![],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let assignment = Assignment::assigned(
        "assign-existing".to_owned(),
        "task-scheduler-skip".to_owned(),
        "attempt-1".to_owned(),
        "worker-a".to_owned(),
        "publish post".to_owned(),
        None,
        None,
        vec![],
        vec![],
    );
    persist_assignment(&root, &assignment).expect("assignment should persist");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let result = load_assignment(&root, "task-scheduler-skip", "sched-task-scheduler-skip-2");
    assert!(
        result.is_err(),
        "scheduler should not create a new assignment"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_auto_completes_task() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-complete".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        Some("impl-xhs-v4".to_owned()),
        vec!["xhs_publish".to_owned()],
        vec!["xhs_browser_login".to_owned()],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--auto-complete".to_owned(),
        "--output-prefix".to_owned(),
        "done".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) =
        load_task_submission(&root, "task-scheduler-complete").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);
    let (_, assignment) = load_assignment(
        &root,
        "task-scheduler-complete",
        "sched-task-scheduler-complete-1",
    )
    .expect("assignment should load");
    assert_eq!(
        assignment.status,
        crate::runtime::AssignmentStatus::Completed
    );
    assert_eq!(
        assignment.output.as_deref(),
        Some("done:task-scheduler-complete")
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_skips_task_until_trigger_fires() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-trigger-wait".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        None,
        vec![],
        vec![],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let trigger = Trigger::active(
        "trigger-wait".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "hourly".to_owned(),
    );
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) =
        load_task_submission(&root, "task-scheduler-trigger-wait").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);
    let result = load_assignment(
        &root,
        "task-scheduler-trigger-wait",
        "sched-task-scheduler-trigger-wait-1",
    );
    assert!(result.is_err(), "scheduler should wait for trigger fire");

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_schedules_task_after_trigger_fire() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-trigger-ready".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        None,
        vec![],
        vec![],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let trigger = Trigger::active(
        "trigger-ready".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "hourly".to_owned(),
    );
    persist_trigger(&root, &trigger).expect("trigger should persist");
    update_trigger(&root, &spec.task_id, "trigger-ready", |trigger| {
        trigger
            .try_record_fire("unix_ms:1".to_owned())
            .map_err(std::io::Error::other)
    })
    .expect("trigger should fire");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) =
        load_task_submission(&root, "task-scheduler-trigger-ready").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Running);
    let (_, assignment) = load_assignment(
        &root,
        "task-scheduler-trigger-ready",
        "sched-task-scheduler-trigger-ready-1",
    )
    .expect("assignment should load");
    assert_eq!(assignment.worker_node_id, "worker-scheduler");
    let (_, trigger) = load_trigger(&root, "task-scheduler-trigger-ready", "trigger-ready")
        .expect("trigger should load");
    assert_eq!(trigger.consumed_fire_count, 1);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_triggered_only_skips_tasks_without_triggers() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-triggered-only-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "plain queued task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--triggered-only".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) =
        load_task_submission(&root, "task-triggered-only-skip").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);
    let (_, assignments) =
        load_task_assignments(&root, "task-triggered-only-skip").expect("assignments should load");
    assert!(assignments.is_empty());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_run_once_triggered_only_schedules_ready_trigger_task() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-triggered-only-ready".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "trigger ready task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let mut trigger = Trigger::active(
        "trigger-ready".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    trigger
        .try_record_fire("unix_ms:1".to_owned())
        .expect("trigger should fire");
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--triggered-only".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) =
        load_task_submission(&root, "task-triggered-only-ready").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Running);
    let (_, assignments) =
        load_task_assignments(&root, "task-triggered-only-ready").expect("assignments should load");
    assert_eq!(assignments.len(), 1);
    let (_, consumed_trigger) = load_trigger(&root, "task-triggered-only-ready", "trigger-ready")
        .expect("trigger should load");
    assert_eq!(consumed_trigger.consumed_fire_count, 1);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn trigger_clear_ready_consumes_unconsumed_fire() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-trigger-clear".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "clear ready trigger".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let mut trigger = Trigger::active(
        "trigger-clear".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    trigger
        .try_record_fire("unix_ms:1".to_owned())
        .expect("trigger should fire");
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let args = vec![
        "trigger".to_owned(),
        "clear-ready".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--trigger-id".to_owned(),
        "trigger-clear".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_trigger_clear_ready(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, updated_trigger) =
        load_trigger(&root, &spec.task_id, "trigger-clear").expect("trigger should load");
    assert_eq!(updated_trigger.fire_count, 1);
    assert_eq!(updated_trigger.consumed_fire_count, 1);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_reopen_moves_completed_task_to_queued() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-reopen-ok".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "reopen completed task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "reopen".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_reopen(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_reopen_rejects_task_with_active_assignment() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-reopen-busy".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "reopen busy task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let assignment = Assignment::assigned(
        "assignment-busy".to_owned(),
        spec.task_id.clone(),
        "attempt-1".to_owned(),
        "worker-a".to_owned(),
        "input".to_owned(),
        None,
        None,
        Vec::new(),
        Vec::new(),
    );
    persist_assignment(&root, &assignment).expect("assignment should persist");

    let args = vec![
        "task".to_owned(),
        "reopen".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_reopen(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_reopens_completed_task() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-ok".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "rerun completed task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_refire_trigger() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-trigger".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "rerun trigger task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let mut trigger = Trigger::active(
        "trigger-rerun".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    trigger.consumed_fire_count = 1;
    trigger.fire_count = 1;
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--trigger-id".to_owned(),
        "trigger-rerun".to_owned(),
        "--fire-trigger".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);
    let (_, updated_trigger) =
        load_trigger(&root, &spec.task_id, "trigger-rerun").expect("trigger should load");
    assert_eq!(updated_trigger.fire_count, 2);
    assert_eq!(updated_trigger.consumed_fire_count, 1);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_schedule_now_without_trigger() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-schedule".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "rerun and schedule".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--schedule-now".to_owned(),
        "--worker-node".to_owned(),
        "worker-rerun".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Running);
    let (_, assignments) =
        load_task_assignments(&root, &spec.task_id).expect("assignments should load");
    assert_eq!(assignments.len(), 1);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_refire_trigger_and_schedule_now() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-trigger-schedule".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "rerun trigger and schedule".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let mut trigger = Trigger::active(
        "trigger-rerun-schedule".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    trigger.fire_count = 1;
    trigger.consumed_fire_count = 1;
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--trigger-id".to_owned(),
        "trigger-rerun-schedule".to_owned(),
        "--fire-trigger".to_owned(),
        "--schedule-now".to_owned(),
        "--worker-node".to_owned(),
        "worker-rerun".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Running);
    let (_, assignments) =
        load_task_assignments(&root, &spec.task_id).expect("assignments should load");
    assert_eq!(assignments.len(), 1);
    let (_, updated_trigger) =
        load_trigger(&root, &spec.task_id, "trigger-rerun-schedule").expect("trigger should load");
    assert_eq!(updated_trigger.fire_count, 2);
    assert_eq!(updated_trigger.consumed_fire_count, 2);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_failed_reopens_multiple_failed_tasks() {
    let root = unique_test_root();
    for task_id in ["task-rerun-failed-a", "task-rerun-failed-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "rerun failed batch".to_owned(),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Failed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-failed".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-failed-a", "task-rerun-failed-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Queued);
    }

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_reopens_multiple_completed_tasks() {
    let root = unique_test_root();
    for task_id in ["task-rerun-completed-a", "task-rerun-completed-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "rerun completed batch".to_owned(),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-completed-a", "task-rerun-completed-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Queued);
    }

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_schedule_now_schedules_each_task() {
    let root = unique_test_root();
    for task_id in ["task-rerun-schedule-a", "task-rerun-schedule-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            format!("rerun completed batch {task_id}"),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--schedule-now".to_owned(),
        "--worker-node".to_owned(),
        "worker-batch".to_owned(),
        "--json".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-schedule-a", "task-rerun-schedule-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Running);
        let (_, assignments) =
            load_task_assignments(&root, task_id).expect("assignments should load");
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].task_id, task_id);
    }

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_schedule_now_auto_completes_each_task() {
    let root = unique_test_root();
    for task_id in ["task-rerun-auto-a", "task-rerun-auto-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            format!("rerun auto-complete batch {task_id}"),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--schedule-now".to_owned(),
        "--auto-complete".to_owned(),
        "--output-prefix".to_owned(),
        "rerun-auto".to_owned(),
        "--worker-node".to_owned(),
        "worker-batch".to_owned(),
        "--json".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-auto-a", "task-rerun-auto-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Completed);
        let (_, assignments) =
            load_task_assignments(&root, task_id).expect("assignments should load");
        assert_eq!(assignments.len(), 1);
        assert_eq!(
            assignments[0].status,
            crate::runtime::AssignmentStatus::Completed
        );
        let expected_output = format!("rerun-auto:{task_id}");
        assert_eq!(
            assignments[0].output.as_deref(),
            Some(expected_output.as_str())
        );
    }

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_by_tenant_skill_and_implementation() {
    let root = unique_test_root();
    let matching_spec = TaskSpec::new(
        "task-rerun-filter-match".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "matching rerun target".to_owned(),
        Some("impl-match".to_owned()),
        vec!["skill-match".to_owned()],
        Vec::new(),
    );
    let mut matching_runtime =
        TaskRuntime::queued(matching_spec.task_id.clone(), "queen-a".to_owned());
    matching_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &matching_spec, &matching_runtime)
        .expect("matching task should persist");

    let other_spec = TaskSpec::new(
        "task-rerun-filter-skip".to_owned(),
        "tenant-other".to_owned(),
        "user/demo".to_owned(),
        "non matching rerun target".to_owned(),
        Some("impl-other".to_owned()),
        vec!["skill-other".to_owned()],
        Vec::new(),
    );
    let mut other_runtime = TaskRuntime::queued(other_spec.task_id.clone(), "queen-a".to_owned());
    other_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &other_spec, &other_runtime).expect("other task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--tenant".to_owned(),
        "tenant-local".to_owned(),
        "--skill-ref".to_owned(),
        "skill-match".to_owned(),
        "--implementation-ref".to_owned(),
        "impl-match".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, matching_task) =
        load_task_submission(&root, "task-rerun-filter-match").expect("matching task should load");
    let (_, other_task) =
        load_task_submission(&root, "task-rerun-filter-skip").expect("other task should load");
    assert_eq!(matching_task.task_runtime.status, TaskStatus::Queued);
    assert_eq!(other_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_batch_filters_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-filter-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "invalid rerun filter usage".to_owned(),
        None,
        vec!["skill-match".to_owned()],
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--skill-ref".to_owned(),
        "skill-match".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_limit_batch_size() {
    let root = unique_test_root();
    for task_id in [
        "task-rerun-limit-a",
        "task-rerun-limit-b",
        "task-rerun-limit-c",
    ] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            format!("rerun completed batch {task_id}"),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--limit".to_owned(),
        "2".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-limit-a", "task-rerun-limit-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Queued);
    }
    let (_, untouched_task) =
        load_task_submission(&root, "task-rerun-limit-c").expect("task should load");
    assert_eq!(untouched_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_limit_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-limit-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "invalid rerun limit usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--limit".to_owned(),
        "1".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_by_goal_contains() {
    let root = unique_test_root();
    let matching_spec = TaskSpec::new(
        "task-rerun-goal-match".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish scheduled xhs post".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut matching_runtime =
        TaskRuntime::queued(matching_spec.task_id.clone(), "queen-a".to_owned());
    matching_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &matching_spec, &matching_runtime)
        .expect("matching task should persist");

    let other_spec = TaskSpec::new(
        "task-rerun-goal-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "refresh login session".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut other_runtime = TaskRuntime::queued(other_spec.task_id.clone(), "queen-a".to_owned());
    other_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &other_spec, &other_runtime).expect("other task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--goal-contains".to_owned(),
        "publish".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, matching_task) =
        load_task_submission(&root, "task-rerun-goal-match").expect("matching task should load");
    let (_, other_task) =
        load_task_submission(&root, "task-rerun-goal-skip").expect("other task should load");
    assert_eq!(matching_task.task_runtime.status, TaskStatus::Queued);
    assert_eq!(other_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_goal_contains_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-goal-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish scheduled xhs post".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--goal-contains".to_owned(),
        "publish".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_by_namespace() {
    let root = unique_test_root();
    let matching_spec = TaskSpec::new(
        "task-rerun-namespace-match".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish scheduled xhs post".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut matching_runtime =
        TaskRuntime::queued(matching_spec.task_id.clone(), "queen-a".to_owned());
    matching_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &matching_spec, &matching_runtime)
        .expect("matching task should persist");

    let other_spec = TaskSpec::new(
        "task-rerun-namespace-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/session".to_owned(),
        "refresh login session".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut other_runtime = TaskRuntime::queued(other_spec.task_id.clone(), "queen-a".to_owned());
    other_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &other_spec, &other_runtime).expect("other task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--namespace".to_owned(),
        "user/publish".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, matching_task) = load_task_submission(&root, "task-rerun-namespace-match")
        .expect("matching task should load");
    let (_, other_task) =
        load_task_submission(&root, "task-rerun-namespace-skip").expect("other task should load");
    assert_eq!(matching_task.task_runtime.status, TaskStatus::Queued);
    assert_eq!(other_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_namespace_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-namespace-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish scheduled xhs post".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--namespace".to_owned(),
        "user/publish".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_dry_run_does_not_mutate_tasks() {
    let root = unique_test_root();
    for task_id in ["task-rerun-dry-a", "task-rerun-dry-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/publish".to_owned(),
            format!("dry run batch {task_id}"),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--dry-run".to_owned(),
        "--json".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-dry-a", "task-rerun-dry-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Completed);
        let (_, assignments) =
            load_task_assignments(&root, task_id).expect("assignments should load");
        assert!(assignments.is_empty());
    }

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_dry_run_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-dry-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "dry run invalid usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--dry-run".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_dry_run_can_sort_preview_by_status() {
    let root = unique_test_root();
    let failed_spec = TaskSpec::new(
        "task-rerun-sort-b".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish failed task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut failed_runtime = TaskRuntime::queued(failed_spec.task_id.clone(), "queen-a".to_owned());
    failed_runtime.status = TaskStatus::Failed;
    persist_task_submission(&root, &failed_spec, &failed_runtime)
        .expect("failed task should persist");

    let completed_spec = TaskSpec::new(
        "task-rerun-sort-a".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish completed task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut completed_runtime =
        TaskRuntime::queued(completed_spec.task_id.clone(), "queen-a".to_owned());
    completed_runtime.status = TaskStatus::Failed;
    persist_task_submission(&root, &completed_spec, &completed_runtime)
        .expect("completed task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-failed".to_owned(),
        "--dry-run".to_owned(),
        "--sort".to_owned(),
        "status".to_owned(),
        "--json".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, first_task) =
        load_task_submission(&root, "task-rerun-sort-a").expect("task should load");
    let (_, second_task) =
        load_task_submission(&root, "task-rerun-sort-b").expect("task should load");
    assert_eq!(first_task.task_runtime.status, TaskStatus::Failed);
    assert_eq!(second_task.task_runtime.status, TaskStatus::Failed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_batch_summary_only_preserves_count_without_mutation() {
    let root = unique_test_root();
    for task_id in ["task-rerun-summary-a", "task-rerun-summary-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/publish".to_owned(),
            format!("summary batch {task_id}"),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--dry-run".to_owned(),
        "--summary-only".to_owned(),
        "--json".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    for task_id in ["task-rerun-summary-a", "task-rerun-summary-b"] {
        let (_, task) = load_task_submission(&root, task_id).expect("task should load");
        assert_eq!(task.task_runtime.status, TaskStatus::Completed);
    }

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_summary_only_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-summary-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "summary invalid usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--summary-only".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_to_tasks_with_triggers() {
    let root = unique_test_root();
    let triggered_spec = TaskSpec::new(
        "task-rerun-trigger-only-match".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish via trigger".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut triggered_runtime =
        TaskRuntime::queued(triggered_spec.task_id.clone(), "queen-a".to_owned());
    triggered_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &triggered_spec, &triggered_runtime)
        .expect("triggered task should persist");
    let trigger = Trigger::active(
        "trigger-rerun-filter".to_owned(),
        triggered_spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let plain_spec = TaskSpec::new(
        "task-rerun-trigger-only-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish without trigger".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut plain_runtime = TaskRuntime::queued(plain_spec.task_id.clone(), "queen-a".to_owned());
    plain_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &plain_spec, &plain_runtime).expect("plain task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--has-trigger".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, triggered_task) =
        load_task_submission(&root, &triggered_spec.task_id).expect("triggered task should load");
    let (_, plain_task) =
        load_task_submission(&root, &plain_spec.task_id).expect("plain task should load");
    assert_eq!(triggered_task.task_runtime.status, TaskStatus::Queued);
    assert_eq!(plain_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_to_tasks_without_triggers() {
    let root = unique_test_root();
    let triggered_spec = TaskSpec::new(
        "task-rerun-no-trigger-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish via trigger".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut triggered_runtime =
        TaskRuntime::queued(triggered_spec.task_id.clone(), "queen-a".to_owned());
    triggered_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &triggered_spec, &triggered_runtime)
        .expect("triggered task should persist");
    let trigger = Trigger::active(
        "trigger-rerun-no-trigger".to_owned(),
        triggered_spec.task_id.clone(),
        "schedule".to_owned(),
        "daily".to_owned(),
    );
    persist_trigger(&root, &trigger).expect("trigger should persist");

    let plain_spec = TaskSpec::new(
        "task-rerun-no-trigger-match".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "publish without trigger".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut plain_runtime = TaskRuntime::queued(plain_spec.task_id.clone(), "queen-a".to_owned());
    plain_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &plain_spec, &plain_runtime).expect("plain task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--without-trigger".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, triggered_task) =
        load_task_submission(&root, &triggered_spec.task_id).expect("triggered task should load");
    let (_, plain_task) =
        load_task_submission(&root, &plain_spec.task_id).expect("plain task should load");
    assert_eq!(triggered_task.task_runtime.status, TaskStatus::Completed);
    assert_eq!(plain_task.task_runtime.status, TaskStatus::Queued);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_conflicting_trigger_filters() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-trigger-filter-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "invalid trigger filter usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--has-trigger".to_owned(),
        "--without-trigger".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_to_tasks_with_active_residents() {
    let root = unique_test_root();
    let resident_task_spec = TaskSpec::new(
        "task-rerun-resident-match".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "resident-backed publish".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut resident_task_runtime =
        TaskRuntime::queued(resident_task_spec.task_id.clone(), "queen-a".to_owned());
    resident_task_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &resident_task_spec, &resident_task_runtime)
        .expect("resident task should persist");
    let resident = crate::runtime::ResidentHive::running(
        "resident-match".to_owned(),
        resident_task_spec.task_id.clone(),
        "worker-a".to_owned(),
        "session keeper".to_owned(),
        "unix_ms:1".to_owned(),
    );
    crate::storage::persist_resident(&root, &resident).expect("resident should persist");

    let plain_spec = TaskSpec::new(
        "task-rerun-resident-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "plain publish".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut plain_runtime = TaskRuntime::queued(plain_spec.task_id.clone(), "queen-a".to_owned());
    plain_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &plain_spec, &plain_runtime).expect("plain task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--with-active-resident".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, resident_task) = load_task_submission(&root, &resident_task_spec.task_id)
        .expect("resident task should load");
    let (_, plain_task) =
        load_task_submission(&root, &plain_spec.task_id).expect("plain task should load");
    assert_eq!(resident_task.task_runtime.status, TaskStatus::Queued);
    assert_eq!(plain_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_to_tasks_without_residents() {
    let root = unique_test_root();
    let resident_task_spec = TaskSpec::new(
        "task-rerun-without-resident-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "resident-backed publish".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut resident_task_runtime =
        TaskRuntime::queued(resident_task_spec.task_id.clone(), "queen-a".to_owned());
    resident_task_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &resident_task_spec, &resident_task_runtime)
        .expect("resident task should persist");
    let resident = crate::runtime::ResidentHive::running(
        "resident-skip".to_owned(),
        resident_task_spec.task_id.clone(),
        "worker-a".to_owned(),
        "session keeper".to_owned(),
        "unix_ms:1".to_owned(),
    );
    crate::storage::persist_resident(&root, &resident).expect("resident should persist");

    let plain_spec = TaskSpec::new(
        "task-rerun-without-resident-match".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "plain publish".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut plain_runtime = TaskRuntime::queued(plain_spec.task_id.clone(), "queen-a".to_owned());
    plain_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &plain_spec, &plain_runtime).expect("plain task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--without-resident".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, resident_task) = load_task_submission(&root, &resident_task_spec.task_id)
        .expect("resident task should load");
    let (_, plain_task) =
        load_task_submission(&root, &plain_spec.task_id).expect("plain task should load");
    assert_eq!(resident_task.task_runtime.status, TaskStatus::Completed);
    assert_eq!(plain_task.task_runtime.status, TaskStatus::Queued);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_conflicting_resident_filters() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-resident-filter-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "invalid resident filter usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--with-active-resident".to_owned(),
        "--without-resident".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_all_completed_can_filter_by_latest_assignment_status() {
    let root = unique_test_root();
    let failed_spec = TaskSpec::new(
        "task-rerun-assignment-match".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "rerun completed with failed assignment".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut failed_runtime = TaskRuntime::queued(failed_spec.task_id.clone(), "queen-a".to_owned());
    failed_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &failed_spec, &failed_runtime)
        .expect("failed-match task should persist");
    let failed_assignment = Assignment::assigned(
        "assignment-match".to_owned(),
        failed_spec.task_id.clone(),
        "attempt-1".to_owned(),
        "worker-a".to_owned(),
        "input".to_owned(),
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .with_result(
        "failure".to_owned(),
        crate::runtime::AssignmentStatus::Failed,
    );
    persist_assignment(&root, &failed_assignment).expect("failed assignment should persist");

    let completed_spec = TaskSpec::new(
        "task-rerun-assignment-skip".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "rerun completed with completed assignment".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut completed_runtime =
        TaskRuntime::queued(completed_spec.task_id.clone(), "queen-a".to_owned());
    completed_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &completed_spec, &completed_runtime)
        .expect("completed-skip task should persist");
    let completed_assignment = Assignment::assigned(
        "assignment-skip".to_owned(),
        completed_spec.task_id.clone(),
        "attempt-1".to_owned(),
        "worker-a".to_owned(),
        "input".to_owned(),
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .with_result(
        "success".to_owned(),
        crate::runtime::AssignmentStatus::Completed,
    );
    persist_assignment(&root, &completed_assignment).expect("completed assignment should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--assignment-status".to_owned(),
        "failed".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, matching_task) =
        load_task_submission(&root, &failed_spec.task_id).expect("matching task should load");
    let (_, skipped_task) =
        load_task_submission(&root, &completed_spec.task_id).expect("skipped task should load");
    assert_eq!(matching_task.task_runtime.status, TaskStatus::Queued);
    assert_eq!(skipped_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_assignment_status_without_batch_mode() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-assignment-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "invalid assignment filter usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--assignment-status".to_owned(),
        "failed".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_dry_run_can_save_plan_file() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-plan-dry".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "dry run save plan".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let plan_path = root.join("plans").join("rerun-plan.json");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--dry-run".to_owned(),
        "--save-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let plan_body = std::fs::read_to_string(&plan_path).expect("plan should be written");
    let plan_json: serde_json::Value =
        serde_json::from_str(&plan_body).expect("plan json should parse");
    assert_eq!(plan_json["mode"], "all_completed");
    assert_eq!(plan_json["dry_run"], true);
    assert_eq!(plan_json["task_count"], 1);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_single_can_save_plan_file() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-plan-single".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "single save plan".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let plan_path = root.join("plans").join("single-plan.json");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--task-id".to_owned(),
        spec.task_id.clone(),
        "--save-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let plan_body = std::fs::read_to_string(&plan_path).expect("plan should be written");
    let plan_json: serde_json::Value =
        serde_json::from_str(&plan_body).expect("plan json should parse");
    assert_eq!(plan_json["mode"], "single");
    assert_eq!(plan_json["dry_run"], false);
    assert_eq!(plan_json["task_count"], 1);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_execute_from_saved_plan() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-from-plan".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "rerun from saved plan".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let plan_path = root.join("plans").join("saved-plan.json");

    let save_args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--dry-run".to_owned(),
        "--save-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let save_exit = handle_task_rerun(&save_args);
    assert_eq!(save_exit, ExitCode::SUCCESS);

    let run_args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--from-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let run_exit = handle_task_rerun(&run_args);
    assert_eq!(run_exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Queued);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_summary_only_plan_execution() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-summary-plan".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "summary plan cannot execute".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let plan_path = root.join("plans").join("summary-plan.json");

    let save_args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--dry-run".to_owned(),
        "--summary-only".to_owned(),
        "--save-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let save_exit = handle_task_rerun(&save_args);
    assert_eq!(save_exit, ExitCode::SUCCESS);

    let run_args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--from-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let run_exit = handle_task_rerun(&run_args);
    assert_eq!(run_exit, ExitCode::from(1));

    let (_, task) = load_task_submission(&root, &spec.task_id).expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_append_plan_entries() {
    let root = unique_test_root();
    for (task_id, goal) in [
        ("task-rerun-append-a", "append plan a"),
        ("task-rerun-append-b", "append plan b"),
    ] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/publish".to_owned(),
            goal.to_owned(),
            None,
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        runtime.status = TaskStatus::Completed;
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }
    let plan_path = root.join("plans").join("append-plan.json");

    let first_args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--goal-contains".to_owned(),
        "a".to_owned(),
        "--dry-run".to_owned(),
        "--append-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let first_exit = handle_task_rerun(&first_args);
    assert_eq!(first_exit, ExitCode::SUCCESS);

    let second_args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--goal-contains".to_owned(),
        "b".to_owned(),
        "--dry-run".to_owned(),
        "--append-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let second_exit = handle_task_rerun(&second_args);
    assert_eq!(second_exit, ExitCode::SUCCESS);

    let plan_body = std::fs::read_to_string(&plan_path).expect("plan should be written");
    let plan_json: serde_json::Value =
        serde_json::from_str(&plan_body).expect("plan json should parse");
    assert_eq!(plan_json["mode"], "appended");
    assert_eq!(plan_json["task_count"], 2);
    assert_eq!(
        plan_json["tasks"].as_array().map(|items| items.len()),
        Some(2)
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_save_and_append_plan_together() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-rerun-append-invalid".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "append invalid usage".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let save_path = root.join("plans").join("save.json");
    let append_path = root.join("plans").join("append.json");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--all-completed".to_owned(),
        "--dry-run".to_owned(),
        "--save-plan".to_owned(),
        save_path.to_string_lossy().into_owned(),
        "--append-plan".to_owned(),
        append_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_prune_plan_by_current_task_status() {
    let root = unique_test_root();
    let completed_spec = TaskSpec::new(
        "task-rerun-prune-completed".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "completed task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut completed_runtime =
        TaskRuntime::queued(completed_spec.task_id.clone(), "queen-a".to_owned());
    completed_runtime.status = TaskStatus::Completed;
    persist_task_submission(&root, &completed_spec, &completed_runtime)
        .expect("completed task should persist");

    let failed_spec = TaskSpec::new(
        "task-rerun-prune-failed".to_owned(),
        "tenant-local".to_owned(),
        "user/publish".to_owned(),
        "failed task".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut failed_runtime = TaskRuntime::queued(failed_spec.task_id.clone(), "queen-a".to_owned());
    failed_runtime.status = TaskStatus::Failed;
    persist_task_submission(&root, &failed_spec, &failed_runtime)
        .expect("failed task should persist");

    let plan_path = root.join("plans").join("prune-plan.json");
    let plan = crate::app::execution::TaskRerunBatchJson {
        mode: "all_completed".to_owned(),
        dry_run: true,
        summary_only: false,
        task_count: 2,
        tasks: vec![
            crate::app::execution::TaskRerunJson {
                task_id: completed_spec.task_id.clone(),
                status: "completed".to_owned(),
                trigger_id: None,
                trigger_fired: false,
                schedule_now: false,
                scheduled: false,
                scheduled_assignment_id: None,
            },
            crate::app::execution::TaskRerunJson {
                task_id: failed_spec.task_id.clone(),
                status: "failed".to_owned(),
                trigger_id: None,
                trigger_fired: false,
                schedule_now: false,
                scheduled: false,
                scheduled_assignment_id: None,
            },
        ],
    };
    crate::app::execution::save_rerun_plan(
        plan_path.to_str().expect("plan path should be utf-8"),
        &plan,
    )
    .expect("plan should save");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--prune-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--result-status".to_owned(),
        "completed".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let pruned = crate::app::execution::load_rerun_plan(
        plan_path.to_str().expect("plan path should be utf-8"),
    )
    .expect("plan should load");
    assert_eq!(pruned.task_count, 1);
    assert_eq!(pruned.tasks.len(), 1);
    assert_eq!(pruned.tasks[0].task_id, failed_spec.task_id);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_from_plan_and_prune_plan_together() {
    let root = unique_test_root();
    let plan_path = root.join("plans").join("conflict-plan.json");
    let plan = crate::app::execution::TaskRerunBatchJson {
        mode: "all_completed".to_owned(),
        dry_run: true,
        summary_only: false,
        task_count: 0,
        tasks: Vec::new(),
    };
    crate::app::execution::save_rerun_plan(
        plan_path.to_str().expect("plan path should be utf-8"),
        &plan,
    )
    .expect("plan should save");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--from-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--prune-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_can_show_plan_summary() {
    let root = unique_test_root();
    let plan_path = root.join("plans").join("summary-view.json");
    let plan = crate::app::execution::TaskRerunBatchJson {
        mode: "all_completed".to_owned(),
        dry_run: true,
        summary_only: false,
        task_count: 2,
        tasks: vec![
            crate::app::execution::TaskRerunJson {
                task_id: "task-a".to_owned(),
                status: "completed".to_owned(),
                trigger_id: None,
                trigger_fired: false,
                schedule_now: false,
                scheduled: false,
                scheduled_assignment_id: None,
            },
            crate::app::execution::TaskRerunJson {
                task_id: "task-b".to_owned(),
                status: "completed".to_owned(),
                trigger_id: None,
                trigger_fired: false,
                schedule_now: false,
                scheduled: false,
                scheduled_assignment_id: None,
            },
        ],
    };
    crate::app::execution::save_rerun_plan(
        plan_path.to_str().expect("plan path should be utf-8"),
        &plan,
    )
    .expect("plan should save");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--plan-summary".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--json".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn task_rerun_rejects_from_plan_and_plan_summary_together() {
    let root = unique_test_root();
    let plan_path = root.join("plans").join("summary-conflict.json");
    let plan = crate::app::execution::TaskRerunBatchJson {
        mode: "all_completed".to_owned(),
        dry_run: true,
        summary_only: false,
        task_count: 0,
        tasks: Vec::new(),
    };
    crate::app::execution::save_rerun_plan(
        plan_path.to_str().expect("plan path should be utf-8"),
        &plan,
    )
    .expect("plan should save");

    let args = vec![
        "task".to_owned(),
        "rerun".to_owned(),
        "--from-plan".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--plan-summary".to_owned(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_task_rerun(&args);
    assert_eq!(exit, ExitCode::from(1));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_does_not_reuse_consumed_trigger_fire() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-trigger-consumed".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        None,
        vec![],
        vec![],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let trigger = Trigger::active(
        "trigger-consumed".to_owned(),
        spec.task_id.clone(),
        "schedule".to_owned(),
        "hourly".to_owned(),
    );
    persist_trigger(&root, &trigger).expect("trigger should persist");
    update_trigger(&root, &spec.task_id, "trigger-consumed", |trigger| {
        trigger
            .try_record_fire("unix_ms:1".to_owned())
            .map_err(std::io::Error::other)
    })
    .expect("trigger should fire");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let first_exit = handle_scheduler_run_once(&args);
    assert_eq!(first_exit, ExitCode::SUCCESS);

    update_task_submission(&root, &spec.task_id, |record| {
        record.task_runtime.status = TaskStatus::Queued;
        Ok(())
    })
    .expect("task should return to queued");

    let second_exit = handle_scheduler_run_once(&args);
    assert_eq!(second_exit, ExitCode::SUCCESS);

    let result = load_assignment(
        &root,
        "task-scheduler-trigger-consumed",
        "sched-task-scheduler-trigger-consumed-2",
    );
    assert!(
        result.is_err(),
        "consumed trigger fire should not be reused"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_pauses_oneshot_trigger_after_consumption() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-trigger-oneshot".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        None,
        vec![],
        vec![],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    let trigger = Trigger::active(
        "trigger-oneshot".to_owned(),
        spec.task_id.clone(),
        "oneshot".to_owned(),
        "once".to_owned(),
    );
    persist_trigger(&root, &trigger).expect("trigger should persist");
    update_trigger(&root, &spec.task_id, "trigger-oneshot", |trigger| {
        trigger
            .try_record_fire("unix_ms:1".to_owned())
            .map_err(std::io::Error::other)
    })
    .expect("trigger should fire");

    let args = vec![
        "scheduler".to_owned(),
        "run-once".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_run_once(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, trigger) = load_trigger(&root, "task-scheduler-trigger-oneshot", "trigger-oneshot")
        .expect("trigger should load");
    assert_eq!(trigger.status, TriggerStatus::Paused);
    assert_eq!(trigger.consumed_fire_count, 1);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_loop_stops_when_idle() {
    let root = unique_test_root();
    let spec = TaskSpec::new(
        "task-scheduler-loop".to_owned(),
        "tenant-local".to_owned(),
        "user/demo".to_owned(),
        "publish post".to_owned(),
        None,
        vec![],
        vec![],
    );
    let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
    persist_task_submission(&root, &spec, &runtime).expect("task should persist");

    let args = vec![
        "scheduler".to_owned(),
        "loop".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--iterations".to_owned(),
        "3".to_owned(),
        "--auto-complete".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_loop(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, task) = load_task_submission(&root, "task-scheduler-loop").expect("task should load");
    assert_eq!(task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn scheduler_loop_until_idle_processes_multiple_tasks() {
    let root = unique_test_root();
    for task_id in ["task-scheduler-loop-a", "task-scheduler-loop-b"] {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "publish post".to_owned(),
            None,
            vec![],
            vec![],
        );
        let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());
        persist_task_submission(&root, &spec, &runtime).expect("task should persist");
    }

    let args = vec![
        "scheduler".to_owned(),
        "loop".to_owned(),
        "--worker-node".to_owned(),
        "worker-scheduler".to_owned(),
        "--iterations".to_owned(),
        "1".to_owned(),
        "--until-idle".to_owned(),
        "--limit".to_owned(),
        "1".to_owned(),
        "--auto-complete".to_owned(),
        "--root".to_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let exit = handle_scheduler_loop(&args);
    assert_eq!(exit, ExitCode::SUCCESS);

    let (_, first_task) =
        load_task_submission(&root, "task-scheduler-loop-a").expect("first task should load");
    let (_, second_task) =
        load_task_submission(&root, "task-scheduler-loop-b").expect("second task should load");
    assert_eq!(first_task.task_runtime.status, TaskStatus::Completed);
    assert_eq!(second_task.task_runtime.status, TaskStatus::Completed);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}
