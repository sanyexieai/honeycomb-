use super::super::common_support::joined_or_none;
use super::super::*;
use crate::runtime::ImplementationSnapshot;

fn load_implementation_snapshot_from_ref(
    root: &str,
    implementation_ref: Option<&str>,
) -> std::io::Result<Option<ImplementationSnapshot>> {
    let Some(implementation_id) = implementation_ref else {
        return Ok(None);
    };
    let (_, implementation) = load_implementation(root, implementation_id)?;
    Ok(Some(ImplementationSnapshot {
        implementation_id: implementation.implementation_id,
        skill_id: implementation.skill_id,
        executor: implementation.executor,
        entry_kind: implementation.entry.kind,
        entry_path: implementation.entry.path,
        strategy_mode: implementation.strategy.get("mode").cloned(),
        prompt_component: implementation.components.get("prompt").cloned(),
        config_component: implementation.components.get("config").cloned(),
        max_cost: implementation.constraints.get("max_cost").cloned(),
        max_latency_ms: implementation.constraints.get("max_latency_ms").cloned(),
    }))
}

pub(crate) fn resolve_execution_implementation_snapshot(
    root: &str,
    task_id: Option<&str>,
    assignment_id: Option<&str>,
    implementation_ref: Option<&str>,
) -> std::io::Result<Option<ImplementationSnapshot>> {
    if let Some(assignment_id) = assignment_id
        && let Ok((_, assignment)) = load_assignment(root, assignment_id)
    {
        if assignment.implementation_snapshot.is_some() {
            return Ok(assignment.implementation_snapshot);
        }
    }
    if let Some(task_id) = task_id
        && let Ok((_, task_record)) = load_task_submission(root, task_id)
    {
        if task_record.task_spec.implementation_snapshot.is_some() {
            return Ok(task_record.task_spec.implementation_snapshot);
        }
    }
    load_implementation_snapshot_from_ref(root, implementation_ref)
}

pub(crate) fn persist_execution_task_records(
    root: &str,
    record: &ExecutionRecord,
) -> std::io::Result<()> {
    let Some(task_id) = record.task_id.as_deref() else {
        return Ok(());
    };
    let timestamp = record.recorded_at.clone();
    let kind = record.kind.as_str();

    append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{}-{}", record.execution_id, kind),
            format!("{}_execution_recorded", kind),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "target={} assignment={} implementation={} status={}",
                record.target_id,
                record.assignment_id.as_deref().unwrap_or("<none>"),
                record.implementation_ref.as_deref().unwrap_or("<none>"),
                record.status.as_str()
            ),
        ),
    )?;
    append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{}-{}", record.execution_id, kind),
            timestamp.clone(),
            "executor".to_owned(),
            record.runner.clone(),
            format!("{}_execute", kind),
            kind.to_owned(),
            record.target_id.clone(),
            task_id.to_owned(),
            record.status.as_str().to_owned(),
            format!(
                "runner={} assignment={} skills={} tools={} output={}",
                record.runner,
                record.assignment_id.as_deref().unwrap_or("<none>"),
                joined_or_none(&record.skill_refs),
                joined_or_none(&record.tool_refs),
                record.output
            ),
        ),
    )?;
    append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{}-{}", record.execution_id, kind),
            record
                .assignment_id
                .as_ref()
                .map(|assignment_id| format!("span-{assignment_id}-assign")),
            timestamp,
            format!("{}_execute", kind),
            task_id.to_owned(),
            record.status.as_str().to_owned(),
            format!(
                "runner={} target={} implementation={} steps={}",
                record.runner,
                record.target_id,
                record.implementation_ref.as_deref().unwrap_or("<none>"),
                record.plan_steps.join(" | ")
            ),
        ),
    )?;
    Ok(())
}

pub(crate) fn handle_execution_inspect(args: &[String]) -> ExitCode {
    let execution_id = option_value(args, "--execution-id").unwrap_or("exec-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, record) = match load_execution_record(root, execution_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect execution record: {error}");
            return ExitCode::from(1);
        }
    };

    println!("execution inspect loaded");
    println!("  execution_id: {}", record.execution_id);
    println!("  kind: {}", record.kind.as_str());
    println!("  target_id: {}", record.target_id);
    println!(
        "  task_id: {}",
        record.task_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "  assignment_id: {}",
        record.assignment_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "  implementation_ref: {}",
        record.implementation_ref.as_deref().unwrap_or("<none>")
    );
    if let Some(snapshot) = &record.implementation_snapshot {
        println!("  implementation_skill: {}", snapshot.skill_id);
        println!("  implementation_executor: {}", snapshot.executor);
        println!("  implementation_entry: {}", snapshot.entry_path);
    }
    println!("  skill_refs: {}", joined_or_none(&record.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&record.tool_refs));
    println!("  input: {}", record.input);
    println!("  runner: {}", record.runner);
    println!("  status: {}", record.status.as_str());
    println!(
        "  exit_code: {}",
        record
            .exit_code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_owned())
    );
    println!("  output: {}", record.output);
    println!("  plan_step_count: {}", record.plan_steps.len());
    for step in record.plan_steps {
        println!("  plan_step: {step}");
    }
    println!("  recorded_at: {}", record.recorded_at);
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_execution_list(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, records) = match list_execution_records(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list execution records: {error}");
            return ExitCode::from(1);
        }
    };

    let filtered = records
        .into_iter()
        .filter(|record| {
            let task_match = task_id.is_none_or(|value| record.task_id.as_deref() == Some(value));
            let skill_match =
                skill_ref.is_none_or(|value| record.skill_refs.iter().any(|skill| skill == value));
            let tool_match =
                tool_ref.is_none_or(|value| record.tool_refs.iter().any(|tool| tool == value));
            task_match && skill_match && tool_match
        })
        .collect::<Vec<_>>();

    println!("execution list loaded");
    println!("  read_from: {}", dir.display());
    println!("  task_id: {}", task_id.unwrap_or("<none>"));
    println!("  skill_ref: {}", skill_ref.unwrap_or("<none>"));
    println!("  tool_ref: {}", tool_ref.unwrap_or("<none>"));
    println!("  execution_count: {}", filtered.len());
    for record in filtered {
        println!(
            "  - {} kind={} target={} task={} assignment={} implementation={} implementation_skill={} implementation_executor={} runner={} status={}",
            record.execution_id,
            record.kind.as_str(),
            record.target_id,
            record.task_id.as_deref().unwrap_or("<none>"),
            record.assignment_id.as_deref().unwrap_or("<none>"),
            record.implementation_ref.as_deref().unwrap_or("<none>"),
            record
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.skill_id.as_str())
                .unwrap_or("<none>"),
            record
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.executor.as_str())
                .unwrap_or("<none>"),
            record.runner,
            record.status.as_str()
        );
    }

    ExitCode::SUCCESS
}
