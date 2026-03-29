use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::EXECUTION_SCHEMA_VERSION;
use crate::runtime::{
    Assignment, AuditRecord, EventRecord, ResidentHive, TaskRecord, TaskRuntime, TaskSpec,
    TaskStatus, TraceRecord, TransitionOutcome, Trigger,
};

use super::{append_jsonl, from_json, sanitize_filename, to_pretty_json, write_atomic};

pub fn persist_task_submission(
    root: impl AsRef<Path>,
    spec: &TaskSpec,
    runtime: &TaskRuntime,
) -> io::Result<PathBuf> {
    let root = root.as_ref();
    let task_dir = root.join("runtime").join("tasks").join(&spec.task_id);
    fs::create_dir_all(&task_dir)?;

    let path = task_dir.join("task.json");
    let record = TaskRecord {
        schema_version: EXECUTION_SCHEMA_VERSION.to_owned(),
        task_spec: spec.clone(),
        task_runtime: runtime.clone(),
    };
    let body = to_pretty_json(&record)?;

    write_atomic(&path, &body)?;
    append_task_event(
        root,
        &spec.task_id,
        &EventRecord::now(
            format!("event-{}-task-submitted", spec.task_id),
            "task_submitted".to_owned(),
            spec.task_id.clone(),
            format!("goal={}", spec.goal),
        ),
    )?;
    append_task_audit(
        root,
        &spec.task_id,
        &AuditRecord::now(
            format!("audit-{}-task-submit", spec.task_id),
            "user".to_owned(),
            "local-cli".to_owned(),
            "task_submit".to_owned(),
            "task".to_owned(),
            spec.task_id.clone(),
            spec.task_id.clone(),
            "accepted".to_owned(),
            format!("tenant={} namespace={}", spec.tenant_id, spec.namespace),
        ),
    )?;
    append_task_trace(
        root,
        &spec.task_id,
        &TraceRecord::now(
            format!("trace-{}", spec.task_id),
            format!("span-{}-submit", spec.task_id),
            None,
            "task_submit".to_owned(),
            spec.task_id.clone(),
            "accepted".to_owned(),
            format!("goal={}", spec.goal),
        ),
    )?;
    Ok(path)
}

pub fn persist_assignment(root: impl AsRef<Path>, assignment: &Assignment) -> io::Result<PathBuf> {
    let task_dir = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(&assignment.task_id)
        .join("assignments");
    fs::create_dir_all(&task_dir)?;

    let path = task_dir.join(format!(
        "{}.json",
        sanitize_filename(&assignment.assignment_id)
    ));
    let body = to_pretty_json(assignment)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn persist_trigger(root: impl AsRef<Path>, trigger: &Trigger) -> io::Result<PathBuf> {
    let trigger_dir = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(&trigger.task_id)
        .join("triggers");
    fs::create_dir_all(&trigger_dir)?;

    let path = trigger_dir.join(format!("{}.json", sanitize_filename(&trigger.trigger_id)));
    let body = to_pretty_json(trigger)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn persist_resident(root: impl AsRef<Path>, resident: &ResidentHive) -> io::Result<PathBuf> {
    let resident_dir = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(&resident.task_id)
        .join("residents");
    fs::create_dir_all(&resident_dir)?;

    let path = resident_dir.join(format!("{}.json", sanitize_filename(&resident.resident_id)));
    let body = to_pretty_json(resident)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn update_task_runtime(
    root: impl AsRef<Path>,
    task_id: &str,
    next: TaskStatus,
) -> io::Result<(PathBuf, TransitionOutcome)> {
    let (path, mut record) = load_task_submission(root.as_ref(), task_id)?;
    let outcome = record
        .task_runtime
        .transition_to(next)
        .map_err(io::Error::other)?;
    if outcome == TransitionOutcome::Applied {
        let body = to_pretty_json(&record)?;
        write_atomic(&path, &body)?;
    }
    Ok((path, outcome))
}

pub fn update_task_submission<F>(
    root: impl AsRef<Path>,
    task_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, TaskRecord)>
where
    F: FnOnce(&mut TaskRecord) -> io::Result<()>,
{
    let (path, mut record) = load_task_submission(root.as_ref(), task_id)?;
    mutate(&mut record)?;
    let body = to_pretty_json(&record)?;
    write_atomic(&path, &body)?;
    Ok((path, record))
}

pub fn load_assignment(
    root: impl AsRef<Path>,
    task_id: &str,
    assignment_id: &str,
) -> io::Result<(PathBuf, Assignment)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("assignments")
        .join(format!("{}.json", sanitize_filename(assignment_id)));
    let body = fs::read_to_string(&path)?;
    let assignment = from_json::<Assignment>(&body)?;
    Ok((path, assignment))
}

pub fn load_trigger(
    root: impl AsRef<Path>,
    task_id: &str,
    trigger_id: &str,
) -> io::Result<(PathBuf, Trigger)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("triggers")
        .join(format!("{}.json", sanitize_filename(trigger_id)));
    let body = fs::read_to_string(&path)?;
    let trigger = from_json::<Trigger>(&body)?;
    Ok((path, trigger))
}

pub fn load_resident(
    root: impl AsRef<Path>,
    task_id: &str,
    resident_id: &str,
) -> io::Result<(PathBuf, ResidentHive)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("residents")
        .join(format!("{}.json", sanitize_filename(resident_id)));
    let body = fs::read_to_string(&path)?;
    let resident = from_json::<ResidentHive>(&body)?;
    Ok((path, resident))
}

pub fn update_assignment<F>(
    root: impl AsRef<Path>,
    task_id: &str,
    assignment_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, Assignment, TransitionOutcome)>
where
    F: FnOnce(&mut Assignment) -> io::Result<TransitionOutcome>,
{
    let (path, mut assignment) = load_assignment(root.as_ref(), task_id, assignment_id)?;
    let outcome = mutate(&mut assignment)?;
    if outcome == TransitionOutcome::Applied {
        let body = to_pretty_json(&assignment)?;
        write_atomic(&path, &body)?;
    }
    Ok((path, assignment, outcome))
}

pub fn update_trigger<F>(
    root: impl AsRef<Path>,
    task_id: &str,
    trigger_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, Trigger)>
where
    F: FnOnce(&mut Trigger) -> io::Result<()>,
{
    let (path, mut trigger) = load_trigger(root.as_ref(), task_id, trigger_id)?;
    mutate(&mut trigger)?;
    let body = to_pretty_json(&trigger)?;
    write_atomic(&path, &body)?;
    Ok((path, trigger))
}

pub fn update_resident<F>(
    root: impl AsRef<Path>,
    task_id: &str,
    resident_id: &str,
    mutate: F,
) -> io::Result<(PathBuf, ResidentHive)>
where
    F: FnOnce(&mut ResidentHive) -> io::Result<()>,
{
    let (path, mut resident) = load_resident(root.as_ref(), task_id, resident_id)?;
    mutate(&mut resident)?;
    let body = to_pretty_json(&resident)?;
    write_atomic(&path, &body)?;
    Ok((path, resident))
}

pub fn load_task_assignments(
    root: impl AsRef<Path>,
    task_id: &str,
) -> io::Result<(PathBuf, Vec<Assignment>)> {
    let dir = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("assignments");
    let mut assignments = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            assignments.push(from_json::<Assignment>(&body)?);
        }
        assignments.sort_by(|a, b| a.assignment_id.cmp(&b.assignment_id));
    }

    Ok((dir, assignments))
}

pub fn list_triggers(root: impl AsRef<Path>, task_id: &str) -> io::Result<(PathBuf, Vec<Trigger>)> {
    let dir = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("triggers");
    let mut triggers = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            triggers.push(from_json::<Trigger>(&body)?);
        }
        triggers.sort_by(|a, b| a.trigger_id.cmp(&b.trigger_id));
    }

    Ok((dir, triggers))
}

pub fn list_residents(
    root: impl AsRef<Path>,
    task_id: &str,
) -> io::Result<(PathBuf, Vec<ResidentHive>)> {
    let dir = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("residents");
    let mut residents = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            residents.push(from_json::<ResidentHive>(&body)?);
        }
        residents.sort_by(|a, b| a.resident_id.cmp(&b.resident_id));
    }

    Ok((dir, residents))
}

pub fn load_task_submission(
    root: impl AsRef<Path>,
    task_id: &str,
) -> io::Result<(PathBuf, TaskRecord)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("task.json");
    let body = fs::read_to_string(&path)?;
    let record = from_json::<TaskRecord>(&body)?;
    Ok((path, record))
}

pub fn list_task_submissions(root: impl AsRef<Path>) -> io::Result<(PathBuf, Vec<TaskRecord>)> {
    let dir = root.as_ref().join("runtime").join("tasks");
    let mut tasks = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path().join("task.json");
            if !path.exists() {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            tasks.push(from_json::<TaskRecord>(&body)?);
        }
        tasks.sort_by(|a, b| a.task_spec.task_id.cmp(&b.task_spec.task_id));
    }

    Ok((dir, tasks))
}

pub fn append_task_event(
    root: impl AsRef<Path>,
    task_id: &str,
    event: &EventRecord,
) -> io::Result<PathBuf> {
    let task_dir = root.as_ref().join("runtime").join("tasks").join(task_id);
    fs::create_dir_all(&task_dir)?;

    let path = task_dir.join("events.jsonl");
    append_jsonl(&path, event)?;
    Ok(path)
}

pub fn load_task_events(
    root: impl AsRef<Path>,
    task_id: &str,
) -> io::Result<(PathBuf, Vec<EventRecord>)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("events.jsonl");
    let body = fs::read_to_string(&path)?;
    let events = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(from_json::<EventRecord>)
        .collect::<io::Result<Vec<_>>>()?;
    Ok((path, events))
}

pub fn append_task_audit(
    root: impl AsRef<Path>,
    task_id: &str,
    audit: &AuditRecord,
) -> io::Result<PathBuf> {
    let task_dir = root.as_ref().join("runtime").join("tasks").join(task_id);
    fs::create_dir_all(&task_dir)?;

    let path = task_dir.join("audit.jsonl");
    append_jsonl(&path, audit)?;
    Ok(path)
}

pub fn load_task_audits(
    root: impl AsRef<Path>,
    task_id: &str,
) -> io::Result<(PathBuf, Vec<AuditRecord>)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("audit.jsonl");
    let body = fs::read_to_string(&path)?;
    let audits = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(from_json::<AuditRecord>)
        .collect::<io::Result<Vec<_>>>()?;
    Ok((path, audits))
}

pub fn append_task_trace(
    root: impl AsRef<Path>,
    task_id: &str,
    trace: &TraceRecord,
) -> io::Result<PathBuf> {
    let task_dir = root.as_ref().join("runtime").join("tasks").join(task_id);
    fs::create_dir_all(&task_dir)?;

    let path = task_dir.join("trace.jsonl");
    append_jsonl(&path, trace)?;
    Ok(path)
}

pub fn load_task_traces(
    root: impl AsRef<Path>,
    task_id: &str,
) -> io::Result<(PathBuf, Vec<TraceRecord>)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("tasks")
        .join(task_id)
        .join("trace.jsonl");
    let body = fs::read_to_string(&path)?;
    let traces = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(from_json::<TraceRecord>)
        .collect::<io::Result<Vec<_>>>()?;
    Ok((path, traces))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::runtime::{AssignmentStatus, ImplementationSnapshot, TaskStatus};

    use super::*;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-store-test-{nanos}"))
    }

    #[test]
    fn update_task_runtime_applies_and_persists() {
        let root = unique_test_root();
        let spec = TaskSpec::new(
            "task-store-apply".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "test goal".to_owned(),
            None,
            Vec::new(),
            Vec::new(),
        );
        let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());

        persist_task_submission(&root, &spec, &runtime).expect("task submission should persist");
        let (_, outcome) = update_task_runtime(&root, &spec.task_id, TaskStatus::Running)
            .expect("task runtime update should succeed");
        let (_, record) =
            load_task_submission(&root, &spec.task_id).expect("task submission should be readable");

        assert_eq!(outcome, TransitionOutcome::Applied);
        assert_eq!(record.task_runtime.status, TaskStatus::Running);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_task_runtime_returns_noop_without_rewrite() {
        let root = unique_test_root();
        let spec = TaskSpec::new(
            "task-store-noop".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "test goal".to_owned(),
            None,
            Vec::new(),
            Vec::new(),
        );
        let runtime = TaskRuntime {
            task_id: spec.task_id.clone(),
            queen_node_id: "queen-a".to_owned(),
            status: TaskStatus::Completed,
        };

        persist_task_submission(&root, &spec, &runtime).expect("task submission should persist");
        let (path, before) =
            load_task_submission(&root, &spec.task_id).expect("task submission should be readable");
        let before_body = fs::read_to_string(&path).expect("task file should be readable");
        let (_, outcome) = update_task_runtime(&root, &spec.task_id, TaskStatus::Completed)
            .expect("noop task runtime update should succeed");
        let after_body = fs::read_to_string(&path).expect("task file should still be readable");
        let (_, after) = load_task_submission(&root, &spec.task_id)
            .expect("task submission should remain readable");

        assert_eq!(outcome, TransitionOutcome::NoOp);
        assert_eq!(before_body, after_body);
        assert_eq!(before.task_runtime.status, after.task_runtime.status);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_task_submission_updates_implementation_ref() {
        let root = unique_test_root();
        let spec = TaskSpec::new(
            "task-store-backfill".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "test goal".to_owned(),
            None,
            vec!["skill/xhs_publish".to_owned()],
            vec!["tool/xhs_browser_login".to_owned()],
        );
        let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());

        persist_task_submission(&root, &spec, &runtime).expect("task submission should persist");
        let (_, record) = update_task_submission(&root, &spec.task_id, |record| {
            record.task_spec.implementation_ref = Some("impl-xhs-v4".to_owned());
            Ok(())
        })
        .expect("task submission should update");

        assert_eq!(
            record.task_spec.implementation_ref.as_deref(),
            Some("impl-xhs-v4")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_assignment_applies_and_persists() {
        let root = unique_test_root();
        let assignment = Assignment::assigned(
            "assign-store-apply".to_owned(),
            "task-store-assign".to_owned(),
            "attempt-1".to_owned(),
            "worker-a".to_owned(),
            "draft-post".to_owned(),
            Some("impl://xhs/publish/v1".to_owned()),
            None,
            vec!["skill/xhs_publish".to_owned()],
            vec!["tool/xhs_browser_login".to_owned()],
        );

        persist_assignment(&root, &assignment).expect("assignment should persist");
        let (_, updated, outcome) = update_assignment(
            &root,
            &assignment.task_id,
            &assignment.assignment_id,
            |assignment| {
                assignment.mark_running().map_err(io::Error::other)?;
                assignment
                    .complete("posted".to_owned())
                    .map_err(io::Error::other)?;
                Ok(TransitionOutcome::Applied)
            },
        )
        .expect("assignment update should succeed");
        let (_, reloaded) = load_assignment(&root, &assignment.task_id, &assignment.assignment_id)
            .expect("updated assignment should be readable");

        assert_eq!(outcome, TransitionOutcome::Applied);
        assert_eq!(updated.status, AssignmentStatus::Completed);
        assert_eq!(updated.output.as_deref(), Some("posted"));
        assert_eq!(reloaded.status, AssignmentStatus::Completed);
        assert_eq!(reloaded.output.as_deref(), Some("posted"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_task_submission_preserves_skill_and_tool_refs() {
        let root = unique_test_root();
        let spec = TaskSpec::new(
            "task-store-refs".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "test refs".to_owned(),
            Some("impl://xhs/publish/v1".to_owned()),
            vec!["skill/xhs_publish".to_owned()],
            vec!["tool/xhs_browser_login".to_owned()],
        );
        let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());

        persist_task_submission(&root, &spec, &runtime).expect("task submission should persist");
        let (_, record) =
            load_task_submission(&root, &spec.task_id).expect("task submission should be readable");

        assert_eq!(
            record.task_spec.implementation_ref.as_deref(),
            Some("impl://xhs/publish/v1")
        );
        assert_eq!(record.task_spec.skill_refs, vec!["skill/xhs_publish"]);
        assert_eq!(record.task_spec.tool_refs, vec!["tool/xhs_browser_login"]);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_task_submission_preserves_implementation_snapshot() {
        let root = unique_test_root();
        let spec = TaskSpec::new(
            "task-store-impl-snapshot".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "test implementation snapshot".to_owned(),
            Some("impl://xhs/publish/v1".to_owned()),
            vec!["skill/xhs_publish".to_owned()],
            vec!["tool/xhs_browser_login".to_owned()],
        )
        .with_implementation_snapshot(Some(ImplementationSnapshot {
            implementation_id: "impl://xhs/publish/v1".to_owned(),
            skill_id: "skill/xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            entry_kind: "script".to_owned(),
            entry_path: "scripts/xhs_publish_v1.sh".to_owned(),
            strategy_mode: Some("draft_then_publish".to_owned()),
            prompt_component: Some("prompts/xhs.md".to_owned()),
            config_component: None,
            max_cost: Some("0.02".to_owned()),
            max_latency_ms: Some("5000".to_owned()),
        }));
        let runtime = TaskRuntime::queued(spec.task_id.clone(), "queen-a".to_owned());

        persist_task_submission(&root, &spec, &runtime).expect("task submission should persist");
        let (_, record) =
            load_task_submission(&root, &spec.task_id).expect("task submission should be readable");

        let snapshot = record
            .task_spec
            .implementation_snapshot
            .expect("implementation snapshot should persist");
        assert_eq!(snapshot.implementation_id, "impl://xhs/publish/v1");
        assert_eq!(snapshot.skill_id, "skill/xhs_publish");
        assert_eq!(snapshot.executor, "worker_process");
        assert_eq!(
            snapshot.strategy_mode.as_deref(),
            Some("draft_then_publish")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_assignment_preserves_implementation_snapshot() {
        let root = unique_test_root();
        let assignment = Assignment::assigned(
            "assign-store-impl-snapshot".to_owned(),
            "task-store-assign-snapshot".to_owned(),
            "attempt-1".to_owned(),
            "worker-a".to_owned(),
            "draft-post".to_owned(),
            Some("impl://xhs/publish/v1".to_owned()),
            Some(ImplementationSnapshot {
                implementation_id: "impl://xhs/publish/v1".to_owned(),
                skill_id: "skill/xhs_publish".to_owned(),
                executor: "worker_process".to_owned(),
                entry_kind: "script".to_owned(),
                entry_path: "scripts/xhs_publish_v1.sh".to_owned(),
                strategy_mode: Some("draft_then_publish".to_owned()),
                prompt_component: Some("prompts/xhs.md".to_owned()),
                config_component: None,
                max_cost: Some("0.02".to_owned()),
                max_latency_ms: Some("5000".to_owned()),
            }),
            vec!["skill/xhs_publish".to_owned()],
            vec!["tool/xhs_browser_login".to_owned()],
        );

        persist_assignment(&root, &assignment).expect("assignment should persist");
        let (_, loaded) = load_assignment(&root, &assignment.task_id, &assignment.assignment_id)
            .expect("assignment should load");

        let snapshot = loaded
            .implementation_snapshot
            .expect("assignment snapshot should persist");
        assert_eq!(snapshot.implementation_id, "impl://xhs/publish/v1");
        assert_eq!(snapshot.skill_id, "skill/xhs_publish");
        assert_eq!(snapshot.executor, "worker_process");
        assert_eq!(
            snapshot.strategy_mode.as_deref(),
            Some("draft_then_publish")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn list_task_submissions_reads_multiple_records() {
        let root = unique_test_root();
        let first = TaskSpec::new(
            "task-a".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "first".to_owned(),
            Some("impl-a".to_owned()),
            vec!["skill/a".to_owned()],
            vec![],
        );
        let second = TaskSpec::new(
            "task-b".to_owned(),
            "tenant-local".to_owned(),
            "user/demo".to_owned(),
            "second".to_owned(),
            Some("impl-b".to_owned()),
            vec!["skill/b".to_owned()],
            vec!["tool/b".to_owned()],
        );
        let runtime_a = TaskRuntime::queued(first.task_id.clone(), "queen-a".to_owned());
        let runtime_b = TaskRuntime::queued(second.task_id.clone(), "queen-b".to_owned());

        persist_task_submission(&root, &first, &runtime_a).expect("first task should persist");
        persist_task_submission(&root, &second, &runtime_b).expect("second task should persist");

        let (_, tasks) = list_task_submissions(&root).expect("tasks should list");

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].task_spec.task_id, "task-a");
        assert_eq!(tasks[1].task_spec.task_id, "task-b");
        assert_eq!(
            tasks[1].task_spec.implementation_ref.as_deref(),
            Some("impl-b")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_assignment_returns_noop_without_rewrite() {
        let root = unique_test_root();
        let assignment = Assignment {
            assignment_id: "assign-store-noop".to_owned(),
            task_id: "task-store-assign-noop".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            worker_node_id: "worker-a".to_owned(),
            status: AssignmentStatus::Completed,
            input: "draft-post".to_owned(),
            output: Some("posted".to_owned()),
            implementation_ref: Some("impl://xhs/publish/v1".to_owned()),
            implementation_snapshot: None,
            skill_refs: vec!["skill/xhs_publish".to_owned()],
            tool_refs: vec!["tool/xhs_browser_login".to_owned()],
        };

        let path = persist_assignment(&root, &assignment).expect("assignment should persist");
        let before_body = fs::read_to_string(&path).expect("assignment file should be readable");
        let (_, updated, outcome) = update_assignment(
            &root,
            &assignment.task_id,
            &assignment.assignment_id,
            |_assignment| Ok(TransitionOutcome::NoOp),
        )
        .expect("noop assignment update should succeed");
        let after_body =
            fs::read_to_string(&path).expect("assignment file should still be readable");

        assert_eq!(outcome, TransitionOutcome::NoOp);
        assert_eq!(before_body, after_body);
        assert_eq!(updated.status, AssignmentStatus::Completed);
        assert_eq!(updated.output.as_deref(), Some("posted"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_list_triggers() {
        let root = unique_test_root();
        let trigger = Trigger::active(
            "trigger-a".to_owned(),
            "task-trigger-list".to_owned(),
            "schedule".to_owned(),
            "daily-09-00".to_owned(),
        );

        persist_trigger(&root, &trigger).expect("trigger should persist");
        let (_, triggers) =
            list_triggers(&root, &trigger.task_id).expect("trigger list should be readable");

        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].trigger_id, "trigger-a");
        assert_eq!(triggers[0].trigger_type, "schedule");
        assert_eq!(triggers[0].status.as_str(), "active");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_trigger_persists_fire_count_and_timestamp() {
        let root = unique_test_root();
        let trigger = Trigger::active(
            "trigger-fire".to_owned(),
            "task-trigger-fire".to_owned(),
            "manual".to_owned(),
            "on_demand".to_owned(),
        );

        persist_trigger(&root, &trigger).expect("trigger should persist");
        let (_, updated) =
            update_trigger(&root, &trigger.task_id, &trigger.trigger_id, |trigger| {
                trigger.record_fire("unix_ms:123".to_owned());
                Ok(())
            })
            .expect("trigger update should succeed");
        let (_, reloaded) = load_trigger(&root, &trigger.task_id, &trigger.trigger_id)
            .expect("trigger should reload");

        assert_eq!(updated.fire_count, 1);
        assert_eq!(updated.last_fired_at.as_deref(), Some("unix_ms:123"));
        assert_eq!(reloaded.fire_count, 1);
        assert_eq!(reloaded.last_fired_at.as_deref(), Some("unix_ms:123"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn load_trigger_reads_persisted_trigger() {
        let root = unique_test_root();
        let trigger = Trigger::active(
            "trigger-inspect".to_owned(),
            "task-trigger-inspect".to_owned(),
            "schedule".to_owned(),
            "hourly".to_owned(),
        );

        persist_trigger(&root, &trigger).expect("trigger should persist");
        let (_, loaded) = load_trigger(&root, &trigger.task_id, &trigger.trigger_id)
            .expect("trigger should load");

        assert_eq!(loaded.trigger_id, "trigger-inspect");
        assert_eq!(loaded.task_id, "task-trigger-inspect");
        assert_eq!(loaded.trigger_type, "schedule");
        assert_eq!(loaded.schedule, "hourly");
        assert_eq!(loaded.fire_count, 0);
        assert_eq!(loaded.last_fired_at, None);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_list_residents() {
        let root = unique_test_root();
        let resident = ResidentHive::running(
            "resident-a".to_owned(),
            "task-resident-list".to_owned(),
            "worker-a".to_owned(),
            "session_watch".to_owned(),
            "unix_ms:123".to_owned(),
        );

        persist_resident(&root, &resident).expect("resident should persist");
        let (_, residents) =
            list_residents(&root, &resident.task_id).expect("resident list should be readable");

        assert_eq!(residents.len(), 1);
        assert_eq!(residents[0].resident_id, "resident-a");
        assert_eq!(residents[0].worker_node_id, "worker-a");
        assert_eq!(residents[0].purpose, "session_watch");
        assert_eq!(residents[0].status.as_str(), "running");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn update_resident_persists_status_and_last_seen() {
        let root = unique_test_root();
        let resident = ResidentHive::running(
            "resident-update".to_owned(),
            "task-resident-update".to_owned(),
            "worker-a".to_owned(),
            "session_watch".to_owned(),
            "unix_ms:100".to_owned(),
        );

        persist_resident(&root, &resident).expect("resident should persist");
        let (_, updated) = update_resident(
            &root,
            &resident.task_id,
            &resident.resident_id,
            |resident| {
                resident.stop("unix_ms:300".to_owned());
                Ok(())
            },
        )
        .expect("resident update should succeed");
        let (_, reloaded) = load_resident(&root, &resident.task_id, &resident.resident_id)
            .expect("resident should reload");

        assert_eq!(updated.status.as_str(), "stopped");
        assert_eq!(updated.last_seen_at, "unix_ms:300");
        assert_eq!(reloaded.status.as_str(), "stopped");
        assert_eq!(reloaded.last_seen_at, "unix_ms:300");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
