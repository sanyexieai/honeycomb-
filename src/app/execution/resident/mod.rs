use super::*;

pub(crate) fn handle_resident_run(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let resident_id = option_value(args, "--resident-id").unwrap_or("resident-demo");
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-local");
    let purpose = option_value(args, "--purpose").unwrap_or("resident_watch");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();

    let resident = ResidentHive::running(
        resident_id.to_owned(),
        task_id.to_owned(),
        worker_node_id.to_owned(),
        purpose.to_owned(),
        timestamp.clone(),
    );

    let path = match persist_resident(root, &resident) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist resident: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{resident_id}-resident-run"),
            "resident_started".to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "resident_id={} worker={}",
                resident.resident_id, resident.worker_node_id
            ),
        ),
    ) {
        eprintln!("failed to append resident event: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{resident_id}-resident-run"),
            timestamp.clone(),
            "user".to_owned(),
            "local-cli".to_owned(),
            "resident_run".to_owned(),
            "resident".to_owned(),
            resident.resident_id.clone(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "purpose={} worker={}",
                resident.purpose, resident.worker_node_id
            ),
        ),
    ) {
        eprintln!("failed to append resident audit: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{resident_id}-resident-run"),
            Some(format!("span-{task_id}-submit")),
            timestamp,
            "resident_run".to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "resident_id={} purpose={}",
                resident.resident_id, resident.purpose
            ),
        ),
    ) {
        eprintln!("failed to append resident trace: {error}");
        return ExitCode::from(1);
    }

    println!("resident run recorded");
    println!("  task_id: {}", resident.task_id);
    println!("  resident_id: {}", resident.resident_id);
    println!("  worker_node_id: {}", resident.worker_node_id);
    println!("  purpose: {}", resident.purpose);
    println!("  status: {}", resident.status.as_str());
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_resident_inspect(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let resident_id = option_value(args, "--resident-id").unwrap_or("resident-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, resident) = match load_resident(root, task_id, resident_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect resident: {error}");
            return ExitCode::from(1);
        }
    };

    println!("resident inspect loaded");
    println!("  task_id: {}", resident.task_id);
    println!("  resident_id: {}", resident.resident_id);
    println!("  worker_node_id: {}", resident.worker_node_id);
    println!("  purpose: {}", resident.purpose);
    println!("  status: {}", resident.status.as_str());
    println!("  started_at: {}", resident.started_at);
    println!("  last_seen_at: {}", resident.last_seen_at);
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_resident_heartbeat(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident heartbeat recorded",
        "resident_heartbeat",
        |resident, timestamp| {
            resident.refresh(timestamp);
        },
    )
}

pub(crate) fn handle_resident_pause(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident pause recorded",
        "resident_pause",
        |resident, timestamp| {
            resident.pause(timestamp);
        },
    )
}

pub(crate) fn handle_resident_resume(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident resume recorded",
        "resident_resume",
        |resident, timestamp| {
            resident.refresh(timestamp);
        },
    )
}

pub(crate) fn handle_resident_stop(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident stop recorded",
        "resident_stop",
        |resident, timestamp| {
            resident.stop(timestamp);
        },
    )
}

fn handle_resident_update<F>(args: &[String], title: &str, action: &str, mutate: F) -> ExitCode
where
    F: FnOnce(&mut ResidentHive, String),
{
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let resident_id = option_value(args, "--resident-id").unwrap_or("resident-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();

    let (path, resident) = match update_resident(root, task_id, resident_id, |resident| {
        mutate(resident, timestamp.clone());
        Ok(())
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to update resident: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{resident_id}-{action}"),
            action.to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "resident_id={} status={} last_seen_at={}",
                resident.resident_id,
                resident.status.as_str(),
                resident.last_seen_at
            ),
        ),
    ) {
        eprintln!("failed to append resident event: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{resident_id}-{action}"),
            timestamp.clone(),
            "system".to_owned(),
            "honeycomb".to_owned(),
            action.to_owned(),
            "resident".to_owned(),
            resident.resident_id.clone(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!("status={}", resident.status.as_str()),
        ),
    ) {
        eprintln!("failed to append resident audit: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{resident_id}-{action}"),
            Some(format!("span-{task_id}-submit")),
            timestamp,
            action.to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "resident_id={} status={}",
                resident.resident_id,
                resident.status.as_str()
            ),
        ),
    ) {
        eprintln!("failed to append resident trace: {error}");
        return ExitCode::from(1);
    }

    println!("{title}");
    println!("  task_id: {}", resident.task_id);
    println!("  resident_id: {}", resident.resident_id);
    println!("  status: {}", resident.status.as_str());
    println!("  last_seen_at: {}", resident.last_seen_at);
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}
