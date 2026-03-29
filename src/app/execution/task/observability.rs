use super::super::*;

pub(crate) fn handle_task_audit_tail(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id");
    let implementation_ref = option_value(args, "--implementation-ref");
    let root = option_value(args, "--root").unwrap_or(".");
    let mut audits = Vec::new();
    let mut sources = Vec::new();

    if let Some(task_id) = task_id {
        let (path, task_audits) = match load_task_audits(root, task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to read task audit: {error}");
                return ExitCode::from(1);
            }
        };
        sources.push(path.display().to_string());
        audits.extend(task_audits);
    } else if let Some(implementation_ref) = implementation_ref {
        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to list tasks for audit query: {error}");
                return ExitCode::from(1);
            }
        };
        for task in tasks
            .into_iter()
            .filter(|task| task.task_spec.implementation_ref.as_deref() == Some(implementation_ref))
        {
            let (path, task_audits) = match load_task_audits(root, &task.task_spec.task_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "failed to read task audit for {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            sources.push(path.display().to_string());
            audits.extend(task_audits);
        }
    } else {
        let task_id = "task-demo";
        let (path, task_audits) = match load_task_audits(root, task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to read task audit: {error}");
                return ExitCode::from(1);
            }
        };
        sources.push(path.display().to_string());
        audits.extend(task_audits);
    }

    audits.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then_with(|| a.audit_id.cmp(&b.audit_id))
    });

    println!("task audit loaded");
    println!("  task_id: {}", task_id.unwrap_or("<aggregated>"));
    println!(
        "  implementation_ref: {}",
        implementation_ref.unwrap_or("<none>")
    );
    println!("  source_count: {}", sources.len());
    for source in &sources {
        println!("  source: {source}");
    }
    println!("  audit_count: {}", audits.len());
    for audit in audits {
        println!(
            "  - [{}] {} {} {} -> {} ({}) detail={}",
            audit.timestamp,
            audit.actor_type,
            audit.actor_id,
            audit.action,
            audit.target_id,
            audit.result,
            audit.payload
        );
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_task_replay(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, events) = match load_task_events(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to replay task: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task replay loaded");
    println!("  task_id: {task_id}");
    println!("  read_from: {}", path.display());
    println!("  event_count: {}", events.len());
    for event in events {
        println!(
            "  - [{}] {} {} {}",
            event.timestamp, event.event_type, event.event_id, event.payload
        );
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_task_trace_tail(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id");
    let implementation_ref = option_value(args, "--implementation-ref");
    let root = option_value(args, "--root").unwrap_or(".");
    let mut traces = Vec::new();
    let mut sources = Vec::new();

    if let Some(task_id) = task_id {
        let (path, task_traces) = match load_task_traces(root, task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to read task trace: {error}");
                return ExitCode::from(1);
            }
        };
        sources.push(path.display().to_string());
        traces.extend(task_traces);
    } else if let Some(implementation_ref) = implementation_ref {
        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to list tasks for trace query: {error}");
                return ExitCode::from(1);
            }
        };
        for task in tasks
            .into_iter()
            .filter(|task| task.task_spec.implementation_ref.as_deref() == Some(implementation_ref))
        {
            let (path, task_traces) = match load_task_traces(root, &task.task_spec.task_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "failed to read task trace for {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            sources.push(path.display().to_string());
            traces.extend(task_traces);
        }
    } else {
        let task_id = "task-demo";
        let (path, task_traces) = match load_task_traces(root, task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to read task trace: {error}");
                return ExitCode::from(1);
            }
        };
        sources.push(path.display().to_string());
        traces.extend(task_traces);
    }

    traces.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then_with(|| a.span_id.cmp(&b.span_id))
    });

    println!("task trace loaded");
    println!("  task_id: {}", task_id.unwrap_or("<aggregated>"));
    println!(
        "  implementation_ref: {}",
        implementation_ref.unwrap_or("<none>")
    );
    println!("  source_count: {}", sources.len());
    for source in &sources {
        println!("  source: {source}");
    }
    println!("  trace_count: {}", traces.len());
    for trace in traces {
        let parent_span_id = trace.parent_span_id.as_deref().unwrap_or("-");
        println!(
            "  - [{}] {} {} parent={} status={} {}",
            trace.timestamp,
            trace.event_type,
            trace.span_id,
            parent_span_id,
            trace.status,
            trace.payload
        );
    }

    ExitCode::SUCCESS
}
