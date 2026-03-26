use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::EXECUTION_SCHEMA_VERSION;
use crate::runtime::{
    Assignment, AuditRecord, EventRecord, TaskRecord, TaskRuntime, TaskSpec, TaskStatus,
    TraceRecord, TransitionOutcome,
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

    let path = task_dir.join(format!("{}.json", sanitize_filename(&assignment.assignment_id)));
    let body = to_pretty_json(assignment)?;
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
