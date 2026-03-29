use super::*;

pub(crate) fn fire_trigger_internal(
    root: &str,
    task_id: &str,
    trigger_id: &str,
) -> std::io::Result<(PathBuf, Trigger)> {
    let timestamp = crate::core::current_timestamp();

    let (path, trigger) = update_trigger(root, task_id, trigger_id, |trigger| {
        trigger
            .try_record_fire(timestamp.clone())
            .map_err(std::io::Error::other)
    })?;

    append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{trigger_id}-fired-{}", trigger.fire_count),
            "trigger_fired".to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "trigger_id={} fire_count={}",
                trigger.trigger_id, trigger.fire_count
            ),
        ),
    )?;

    append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{trigger_id}-fired-{}", trigger.fire_count),
            timestamp.clone(),
            "system".to_owned(),
            "honeycomb".to_owned(),
            "trigger_fire".to_owned(),
            "trigger".to_owned(),
            trigger.trigger_id.clone(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!("fire_count={}", trigger.fire_count),
        ),
    )?;

    append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{trigger_id}-fire-{}", trigger.fire_count),
            Some(format!("span-{task_id}-submit")),
            timestamp,
            "trigger_fire".to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "trigger_id={} fire_count={}",
                trigger.trigger_id, trigger.fire_count
            ),
        ),
    )?;

    Ok((path, trigger))
}

pub(crate) fn handle_trigger_create(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let trigger_id = option_value(args, "--trigger-id").unwrap_or("trigger-demo");
    let trigger_type = option_value(args, "--trigger-type").unwrap_or("manual");
    let schedule = option_value(args, "--schedule").unwrap_or("on_demand");
    let root = option_value(args, "--root").unwrap_or(".");

    let trigger = Trigger::active(
        trigger_id.to_owned(),
        task_id.to_owned(),
        trigger_type.to_owned(),
        schedule.to_owned(),
    );
    let output_path = match persist_trigger(root, &trigger) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist trigger: {error}");
            return ExitCode::from(1);
        }
    };

    println!("trigger create recorded");
    println!("  task_id: {}", trigger.task_id);
    println!("  trigger_id: {}", trigger.trigger_id);
    println!("  trigger_type: {}", trigger.trigger_type);
    println!("  schedule: {}", trigger.schedule);
    println!("  status: {}", trigger.status.as_str());
    println!("  written_to: {}", output_path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_trigger_inspect(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let trigger_id = option_value(args, "--trigger-id").unwrap_or("trigger-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, trigger) = match load_trigger(root, task_id, trigger_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect trigger: {error}");
            return ExitCode::from(1);
        }
    };

    println!("trigger inspect loaded");
    println!("  task_id: {}", trigger.task_id);
    println!("  trigger_id: {}", trigger.trigger_id);
    println!("  trigger_type: {}", trigger.trigger_type);
    println!("  schedule: {}", trigger.schedule);
    println!("  status: {}", trigger.status.as_str());
    println!("  fire_count: {}", trigger.fire_count);
    println!("  consumed_fire_count: {}", trigger.consumed_fire_count);
    println!(
        "  last_fired_at: {}",
        trigger.last_fired_at.as_deref().unwrap_or("<none>")
    );
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_trigger_list(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, triggers) = match list_triggers(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list triggers: {error}");
            return ExitCode::from(1);
        }
    };

    println!("trigger list loaded");
    println!("  task_id: {task_id}");
    println!("  read_from: {}", dir.display());
    println!("  trigger_count: {}", triggers.len());
    for trigger in triggers {
        println!(
            "  - {} type={} schedule={} status={} fire_count={} consumed_fire_count={}",
            trigger.trigger_id,
            trigger.trigger_type,
            trigger.schedule,
            trigger.status.as_str(),
            trigger.fire_count,
            trigger.consumed_fire_count
        );
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_trigger_pause(args: &[String]) -> ExitCode {
    handle_trigger_status_update(args, TriggerStatus::Paused, "trigger pause recorded")
}

pub(crate) fn handle_trigger_resume(args: &[String]) -> ExitCode {
    handle_trigger_status_update(args, TriggerStatus::Active, "trigger resume recorded")
}

pub(crate) fn handle_trigger_fire(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let trigger_id = option_value(args, "--trigger-id").unwrap_or("trigger-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let (path, trigger) = match fire_trigger_internal(root, task_id, trigger_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to fire trigger: {error}");
            return ExitCode::from(1);
        }
    };

    println!("trigger fire recorded");
    println!("  task_id: {}", trigger.task_id);
    println!("  trigger_id: {}", trigger.trigger_id);
    println!("  fire_count: {}", trigger.fire_count);
    println!(
        "  last_fired_at: {}",
        trigger.last_fired_at.as_deref().unwrap_or("<none>")
    );
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_trigger_clear_ready(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let trigger_id = option_value(args, "--trigger-id").unwrap_or("trigger-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();

    let (path, trigger) = match update_trigger(root, task_id, trigger_id, |trigger| {
        if trigger.has_unconsumed_fire() {
            trigger.consume_fire();
        }
        Ok(())
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to clear ready trigger state: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!(
                "event-{trigger_id}-clear-ready-{}",
                trigger.consumed_fire_count
            ),
            "trigger_ready_cleared".to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "trigger_id={} fire_count={} consumed_fire_count={} status={}",
                trigger.trigger_id,
                trigger.fire_count,
                trigger.consumed_fire_count,
                trigger.status.as_str()
            ),
        ),
    ) {
        eprintln!("failed to append trigger clear-ready event: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!(
                "audit-{trigger_id}-clear-ready-{}",
                trigger.consumed_fire_count
            ),
            timestamp.clone(),
            "user".to_owned(),
            "local-cli".to_owned(),
            "trigger_clear_ready".to_owned(),
            "trigger".to_owned(),
            trigger.trigger_id.clone(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "fire_count={} consumed_fire_count={} status={}",
                trigger.fire_count,
                trigger.consumed_fire_count,
                trigger.status.as_str()
            ),
        ),
    ) {
        eprintln!("failed to append trigger clear-ready audit: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!(
                "span-{trigger_id}-clear-ready-{}",
                trigger.consumed_fire_count
            ),
            Some(format!("span-{task_id}-submit")),
            timestamp,
            "trigger_clear_ready".to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "trigger_id={} fire_count={} consumed_fire_count={} status={}",
                trigger.trigger_id,
                trigger.fire_count,
                trigger.consumed_fire_count,
                trigger.status.as_str()
            ),
        ),
    ) {
        eprintln!("failed to append trigger clear-ready trace: {error}");
        return ExitCode::from(1);
    }

    println!("trigger clear-ready recorded");
    println!("  task_id: {}", trigger.task_id);
    println!("  trigger_id: {}", trigger.trigger_id);
    println!("  status: {}", trigger.status.as_str());
    println!("  fire_count: {}", trigger.fire_count);
    println!("  consumed_fire_count: {}", trigger.consumed_fire_count);
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_trigger_status_update(
    args: &[String],
    next_status: TriggerStatus,
    title: &str,
) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let trigger_id = option_value(args, "--trigger-id").unwrap_or("trigger-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();

    let (path, trigger) = match update_trigger(root, task_id, trigger_id, |trigger| {
        match next_status {
            TriggerStatus::Active => trigger.resume(),
            TriggerStatus::Paused => trigger.pause(),
        }
        Ok(())
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to update trigger status: {error}");
            return ExitCode::from(1);
        }
    };

    let event_type = match next_status {
        TriggerStatus::Active => "trigger_resumed",
        TriggerStatus::Paused => "trigger_paused",
    };
    let audit_action = match next_status {
        TriggerStatus::Active => "trigger_resume",
        TriggerStatus::Paused => "trigger_pause",
    };

    if let Err(error) = append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{trigger_id}-{event_type}"),
            event_type.to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "trigger_id={} status={}",
                trigger.trigger_id,
                trigger.status.as_str()
            ),
        ),
    ) {
        eprintln!("failed to append trigger status event: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{trigger_id}-{audit_action}"),
            timestamp.clone(),
            "user".to_owned(),
            "local-cli".to_owned(),
            audit_action.to_owned(),
            "trigger".to_owned(),
            trigger.trigger_id.clone(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!("status={}", trigger.status.as_str()),
        ),
    ) {
        eprintln!("failed to append trigger status audit: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{trigger_id}-{audit_action}"),
            Some(format!("span-{task_id}-submit")),
            timestamp,
            audit_action.to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "trigger_id={} status={}",
                trigger.trigger_id,
                trigger.status.as_str()
            ),
        ),
    ) {
        eprintln!("failed to append trigger status trace: {error}");
        return ExitCode::from(1);
    }

    println!("{title}");
    println!("  task_id: {}", trigger.task_id);
    println!("  trigger_id: {}", trigger.trigger_id);
    println!("  status: {}", trigger.status.as_str());
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}
