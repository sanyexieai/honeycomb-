use super::common_support::joined_or_none;
use super::*;

#[derive(Serialize)]
pub(crate) struct SchedulerRunOnceJson {
    pub(crate) root: String,
    pub(crate) worker_node_id: String,
    pub(crate) limit: Option<usize>,
    pub(crate) triggered_only: bool,
    pub(crate) auto_complete: bool,
    pub(crate) result_status: String,
    pub(crate) scanned_task_count: usize,
    pub(crate) scheduled_task_count: usize,
    pub(crate) completed_task_count: usize,
    pub(crate) failed_task_count: usize,
    pub(crate) skipped_task_count: usize,
    pub(crate) assignments: Vec<SchedulerAssignmentJson>,
}

#[derive(Serialize)]
pub(crate) struct SchedulerLoopJson {
    pub(crate) root: String,
    pub(crate) worker_node_id: String,
    pub(crate) iterations_requested: usize,
    pub(crate) until_idle: bool,
    pub(crate) sleep_ms: u64,
    pub(crate) triggered_only: bool,
    pub(crate) iterations_run: usize,
    pub(crate) stopped_idle: bool,
    pub(crate) auto_complete: bool,
    pub(crate) result_status: String,
    pub(crate) total_scanned_task_count: usize,
    pub(crate) total_scheduled_task_count: usize,
    pub(crate) total_completed_task_count: usize,
    pub(crate) total_failed_task_count: usize,
    pub(crate) total_skipped_task_count: usize,
    pub(crate) runs: Vec<SchedulerRunOnceJson>,
}

#[derive(Serialize)]
pub(crate) struct SchedulerAssignmentJson {
    pub(crate) task_id: String,
    pub(crate) assignment_id: String,
    pub(crate) attempt_id: String,
    pub(crate) worker_node_id: String,
    pub(crate) implementation_ref: Option<String>,
    pub(crate) status: String,
    pub(crate) output: Option<String>,
}

pub(crate) struct ScheduleSingleTaskOutcome {
    pub(crate) assignment: Option<SchedulerAssignmentJson>,
    pub(crate) completed_task_count: usize,
    pub(crate) failed_task_count: usize,
}

pub(crate) fn scheduler_assignment_result_status(result_status: &str) -> AssignmentStatus {
    if result_status == "failed" {
        AssignmentStatus::Failed
    } else {
        AssignmentStatus::Completed
    }
}

fn task_triggers_ready(triggers: &[Trigger]) -> bool {
    triggers.is_empty() || triggers.iter().any(Trigger::has_unconsumed_fire)
}

pub(crate) fn schedule_single_task_internal(
    root: &str,
    task: crate::runtime::TaskRecord,
    worker_node_id: &str,
    triggered_only: bool,
    auto_complete: bool,
    assignment_result_status: AssignmentStatus,
    output_prefix: &str,
) -> std::io::Result<ScheduleSingleTaskOutcome> {
    if task.task_runtime.status != TaskStatus::Queued {
        return Ok(ScheduleSingleTaskOutcome {
            assignment: None,
            completed_task_count: 0,
            failed_task_count: 0,
        });
    }

    let (_, triggers) = list_triggers(root, &task.task_spec.task_id).map_err(|error| {
        std::io::Error::other(format!(
            "failed to load triggers for scheduler task {}: {error}",
            task.task_spec.task_id
        ))
    })?;
    if triggered_only && triggers.is_empty() {
        return Ok(ScheduleSingleTaskOutcome {
            assignment: None,
            completed_task_count: 0,
            failed_task_count: 0,
        });
    }
    if !task_triggers_ready(&triggers) {
        return Ok(ScheduleSingleTaskOutcome {
            assignment: None,
            completed_task_count: 0,
            failed_task_count: 0,
        });
    }

    let (_, existing_assignments) =
        load_task_assignments(root, &task.task_spec.task_id).map_err(|error| {
            std::io::Error::other(format!(
                "failed to load assignments for scheduler task {}: {error}",
                task.task_spec.task_id
            ))
        })?;

    if existing_assignments.iter().any(|assignment| {
        matches!(
            assignment.status,
            AssignmentStatus::Created
                | AssignmentStatus::Assigned
                | AssignmentStatus::Running
                | AssignmentStatus::RetryPending
        )
    }) {
        return Ok(ScheduleSingleTaskOutcome {
            assignment: None,
            completed_task_count: 0,
            failed_task_count: 0,
        });
    }

    let attempt_index = existing_assignments.len() + 1;
    let assignment_id = format!("sched-{}-{attempt_index}", task.task_spec.task_id);
    let attempt_id = format!("attempt-{attempt_index}");
    let input = task.task_spec.goal.clone();
    let assignment = Assignment::assigned(
        assignment_id.clone(),
        task.task_spec.task_id.clone(),
        attempt_id.clone(),
        worker_node_id.to_owned(),
        input.clone(),
        task.task_spec.implementation_ref.clone(),
        task.task_spec.implementation_snapshot.clone(),
        task.task_spec.skill_refs.clone(),
        task.task_spec.tool_refs.clone(),
    );

    crate::storage::persist_assignment(root, &assignment).map_err(|error| {
        std::io::Error::other(format!(
            "failed to persist scheduler assignment for task {}: {error}",
            task.task_spec.task_id
        ))
    })?;

    let runtime_outcome = update_task_runtime(root, &task.task_spec.task_id, TaskStatus::Running)
        .map_err(|error| {
            std::io::Error::other(format!(
                "failed to update task runtime for scheduler task {}: {error}",
                task.task_spec.task_id
            ))
        })?
        .1;
    if runtime_outcome != TransitionOutcome::Applied && runtime_outcome != TransitionOutcome::NoOp {
        return Err(std::io::Error::other(format!(
            "scheduler runtime transition failed for task {}",
            task.task_spec.task_id
        )));
    }

    let timestamp = crate::core::current_timestamp();
    append_task_event(
        root,
        &task.task_spec.task_id,
        &EventRecord::new(
            format!("event-{assignment_id}-scheduled"),
            "assignment_scheduled".to_owned(),
            task.task_spec.task_id.clone(),
            timestamp.clone(),
            format!(
                "worker={worker_node_id} attempt_id={attempt_id} implementation={} skills={} tools={}",
                assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            ),
        ),
    )?;
    append_task_audit(
        root,
        &task.task_spec.task_id,
        &AuditRecord::new(
            format!("audit-{assignment_id}-schedule"),
            timestamp.clone(),
            "scheduler".to_owned(),
            "local-scheduler".to_owned(),
            "task_schedule".to_owned(),
            "assignment".to_owned(),
            assignment_id.clone(),
            task.task_spec.task_id.clone(),
            "recorded".to_owned(),
            format!(
                "worker={worker_node_id} implementation={} skills={} tools={}",
                assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            ),
        ),
    )?;
    append_task_trace(
        root,
        &task.task_spec.task_id,
        &TraceRecord::new(
            format!("trace-{}", task.task_spec.task_id),
            format!("span-{assignment_id}-schedule"),
            Some(format!("span-{}-submit", task.task_spec.task_id)),
            timestamp,
            "task_schedule".to_owned(),
            task.task_spec.task_id.clone(),
            "assigned".to_owned(),
            format!(
                "attempt_id={attempt_id} implementation={} skills={} tools={}",
                assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            ),
        ),
    )?;

    for trigger in triggers
        .iter()
        .filter(|trigger| trigger.has_unconsumed_fire())
    {
        let consume_timestamp = crate::core::current_timestamp();
        let (_, trigger) = update_trigger(
            root,
            &task.task_spec.task_id,
            &trigger.trigger_id,
            |trigger| {
                trigger.consume_fire();
                Ok(())
            },
        )
        .map_err(|error| {
            std::io::Error::other(format!(
                "failed to consume trigger {} for task {}: {error}",
                trigger.trigger_id, task.task_spec.task_id
            ))
        })?;

        append_task_event(
            root,
            &task.task_spec.task_id,
            &EventRecord::new(
                format!(
                    "event-{}-consumed-{}",
                    trigger.trigger_id, trigger.consumed_fire_count
                ),
                "trigger_consumed".to_owned(),
                task.task_spec.task_id.clone(),
                consume_timestamp.clone(),
                format!(
                    "trigger_id={} fire_count={} consumed_fire_count={} status={}",
                    trigger.trigger_id,
                    trigger.fire_count,
                    trigger.consumed_fire_count,
                    trigger.status.as_str()
                ),
            ),
        )?;
        append_task_audit(
            root,
            &task.task_spec.task_id,
            &AuditRecord::new(
                format!(
                    "audit-{}-consumed-{}",
                    trigger.trigger_id, trigger.consumed_fire_count
                ),
                consume_timestamp.clone(),
                "scheduler".to_owned(),
                "local-scheduler".to_owned(),
                "trigger_consume".to_owned(),
                "trigger".to_owned(),
                trigger.trigger_id.clone(),
                task.task_spec.task_id.clone(),
                "recorded".to_owned(),
                format!(
                    "fire_count={} consumed_fire_count={} status={}",
                    trigger.fire_count,
                    trigger.consumed_fire_count,
                    trigger.status.as_str()
                ),
            ),
        )?;
        append_task_trace(
            root,
            &task.task_spec.task_id,
            &TraceRecord::new(
                format!("trace-{}", task.task_spec.task_id),
                format!(
                    "span-{}-consume-{}",
                    trigger.trigger_id, trigger.consumed_fire_count
                ),
                Some(format!("span-{assignment_id}-schedule")),
                consume_timestamp,
                "trigger_consume".to_owned(),
                task.task_spec.task_id.clone(),
                "recorded".to_owned(),
                format!(
                    "trigger_id={} fire_count={} consumed_fire_count={} status={}",
                    trigger.trigger_id,
                    trigger.fire_count,
                    trigger.consumed_fire_count,
                    trigger.status.as_str()
                ),
            ),
        )?;
    }

    let mut assignment_status = assignment.status.as_str().to_owned();
    let mut assignment_output = None;
    let mut completed_task_count = 0usize;
    let mut failed_task_count = 0usize;

    if auto_complete {
        let result_output = format!("{output_prefix}:{}", task.task_spec.task_id);
        let (_, assignment, outcome) = update_assignment(
            root,
            &task.task_spec.task_id,
            &assignment_id,
            |assignment| {
                assignment.mark_running().map_err(std::io::Error::other)?;
                if assignment_result_status == AssignmentStatus::Failed {
                    assignment
                        .fail(result_output.clone())
                        .map_err(std::io::Error::other)?;
                } else {
                    assignment
                        .complete(result_output.clone())
                        .map_err(std::io::Error::other)?;
                }
                Ok(TransitionOutcome::Applied)
            },
        )
        .map_err(|error| {
            std::io::Error::other(format!(
                "failed to auto-complete assignment {} for task {}: {error}",
                assignment_id, task.task_spec.task_id
            ))
        })?;

        if outcome == TransitionOutcome::Applied {
            let result_timestamp = crate::core::current_timestamp();
            append_task_event(
                root,
                &task.task_spec.task_id,
                &EventRecord::new(
                    format!("event-{assignment_id}-result"),
                    "task_result".to_owned(),
                    task.task_spec.task_id.clone(),
                    result_timestamp.clone(),
                    format!(
                        "worker={worker_node_id} status={} implementation={} skills={} tools={}",
                        assignment.status.as_str(),
                        assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                        joined_or_none(&assignment.skill_refs),
                        joined_or_none(&assignment.tool_refs)
                    ),
                ),
            )?;
            append_task_audit(
                root,
                &task.task_spec.task_id,
                &AuditRecord::new(
                    format!("audit-{assignment_id}-result"),
                    result_timestamp.clone(),
                    "scheduler".to_owned(),
                    "local-scheduler".to_owned(),
                    "task_result".to_owned(),
                    "assignment".to_owned(),
                    assignment_id.clone(),
                    task.task_spec.task_id.clone(),
                    assignment.status.as_str().to_owned(),
                    format!(
                        "{} implementation={} skills={} tools={}",
                        result_output,
                        assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                        joined_or_none(&assignment.skill_refs),
                        joined_or_none(&assignment.tool_refs)
                    ),
                ),
            )?;
            append_task_trace(
                root,
                &task.task_spec.task_id,
                &TraceRecord::new(
                    format!("trace-{}", task.task_spec.task_id),
                    format!("span-{assignment_id}-result"),
                    Some(format!("span-{assignment_id}-schedule")),
                    result_timestamp,
                    "task_result".to_owned(),
                    task.task_spec.task_id.clone(),
                    assignment.status.as_str().to_owned(),
                    format!(
                        "output={} implementation={} skills={} tools={}",
                        result_output,
                        assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                        joined_or_none(&assignment.skill_refs),
                        joined_or_none(&assignment.tool_refs)
                    ),
                ),
            )?;
        }

        let next_task_status = if assignment_result_status == AssignmentStatus::Failed {
            TaskStatus::Failed
        } else {
            TaskStatus::Completed
        };
        update_task_runtime(root, &task.task_spec.task_id, next_task_status).map_err(|error| {
            std::io::Error::other(format!(
                "failed to finalize task runtime for scheduler task {}: {error}",
                task.task_spec.task_id
            ))
        })?;

        assignment_status = assignment.status.as_str().to_owned();
        assignment_output = assignment.output.clone();
        if assignment_result_status == AssignmentStatus::Failed {
            failed_task_count = 1;
        } else {
            completed_task_count = 1;
        }
    }

    Ok(ScheduleSingleTaskOutcome {
        assignment: Some(SchedulerAssignmentJson {
            task_id: task.task_spec.task_id,
            assignment_id,
            attempt_id,
            worker_node_id: worker_node_id.to_owned(),
            implementation_ref: assignment.implementation_ref.clone(),
            status: assignment_status,
            output: assignment_output,
        }),
        completed_task_count,
        failed_task_count,
    })
}

fn run_scheduler_once_internal(
    root: &str,
    worker_node_id: &str,
    limit: Option<usize>,
    triggered_only: bool,
    auto_complete: bool,
    assignment_result_status: AssignmentStatus,
    output_prefix: &str,
) -> std::io::Result<SchedulerRunOnceJson> {
    let (_, tasks) = match crate::storage::list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            return Err(std::io::Error::other(format!(
                "failed to list tasks for scheduler run-once: {error}"
            )));
        }
    };

    let mut scanned_task_count = 0usize;
    let mut scheduled_task_count = 0usize;
    let mut completed_task_count = 0usize;
    let mut failed_task_count = 0usize;
    let mut skipped_task_count = 0usize;
    let mut assignments = Vec::<SchedulerAssignmentJson>::new();

    for task in tasks {
        if limit.is_some_and(|value| scheduled_task_count >= value) {
            break;
        }

        scanned_task_count += 1;
        let outcome = schedule_single_task_internal(
            root,
            task,
            worker_node_id,
            triggered_only,
            auto_complete,
            assignment_result_status,
            output_prefix,
        )?;
        if let Some(assignment) = outcome.assignment {
            assignments.push(assignment);
            scheduled_task_count += 1;
            completed_task_count += outcome.completed_task_count;
            failed_task_count += outcome.failed_task_count;
        } else {
            skipped_task_count += 1;
        }
    }

    Ok(SchedulerRunOnceJson {
        root: root.to_owned(),
        worker_node_id: worker_node_id.to_owned(),
        limit,
        triggered_only,
        auto_complete,
        result_status: assignment_result_status.as_str().to_owned(),
        scanned_task_count,
        scheduled_task_count,
        completed_task_count,
        failed_task_count,
        skipped_task_count,
        assignments,
    })
}

pub(crate) fn handle_scheduler_run_once(args: &[String]) -> ExitCode {
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-scheduler");
    let limit = option_value(args, "--limit").and_then(|value| value.parse::<usize>().ok());
    let triggered_only = has_flag(args, "--triggered-only");
    let auto_complete = has_flag(args, "--auto-complete");
    let result_status = option_value(args, "--result-status").unwrap_or("completed");
    let output_prefix = option_value(args, "--output-prefix").unwrap_or("scheduler-output");
    let as_json = has_flag(args, "--json");
    let root = option_value(args, "--root").unwrap_or(".");
    let assignment_result_status = scheduler_assignment_result_status(result_status);

    let payload = match run_scheduler_once_internal(
        root,
        worker_node_id,
        limit,
        triggered_only,
        auto_complete,
        assignment_result_status,
        output_prefix,
    ) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };

    if as_json {
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to render scheduler run-once json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("scheduler run-once completed");
    println!("  worker_node_id: {}", payload.worker_node_id);
    println!(
        "  limit: {}",
        payload
            .limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_owned())
    );
    println!(
        "  triggered_only: {}",
        if payload.triggered_only {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  auto_complete: {}",
        if payload.auto_complete {
            "true"
        } else {
            "false"
        }
    );
    println!("  result_status: {}", payload.result_status);
    println!("  scanned_task_count: {}", payload.scanned_task_count);
    println!("  scheduled_task_count: {}", payload.scheduled_task_count);
    println!("  completed_task_count: {}", payload.completed_task_count);
    println!("  failed_task_count: {}", payload.failed_task_count);
    println!("  skipped_task_count: {}", payload.skipped_task_count);
    for assignment in payload.assignments {
        println!(
            "  assignment: task={} assignment={} attempt={} worker={} implementation={} status={} output={}",
            assignment.task_id,
            assignment.assignment_id,
            assignment.attempt_id,
            assignment.worker_node_id,
            assignment.implementation_ref.as_deref().unwrap_or("<none>"),
            assignment.status,
            assignment.output.as_deref().unwrap_or("<none>")
        );
    }
    ExitCode::SUCCESS
}

pub(crate) fn handle_scheduler_loop(args: &[String]) -> ExitCode {
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-scheduler");
    let iterations = option_value(args, "--iterations")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3);
    let until_idle = has_flag(args, "--until-idle");
    let sleep_ms = option_value(args, "--sleep-ms")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let limit = option_value(args, "--limit").and_then(|value| value.parse::<usize>().ok());
    let triggered_only = has_flag(args, "--triggered-only");
    let auto_complete = has_flag(args, "--auto-complete");
    let result_status = option_value(args, "--result-status").unwrap_or("completed");
    let output_prefix = option_value(args, "--output-prefix").unwrap_or("scheduler-output");
    let as_json = has_flag(args, "--json");
    let root = option_value(args, "--root").unwrap_or(".");
    let assignment_result_status = scheduler_assignment_result_status(result_status);

    let mut runs = Vec::new();
    let mut total_scanned_task_count = 0usize;
    let mut total_scheduled_task_count = 0usize;
    let mut total_completed_task_count = 0usize;
    let mut total_failed_task_count = 0usize;
    let mut total_skipped_task_count = 0usize;
    let mut iterations_run = 0usize;
    let mut stopped_idle = false;

    loop {
        if !until_idle && iterations_run >= iterations {
            break;
        }
        let payload = match run_scheduler_once_internal(
            root,
            worker_node_id,
            limit,
            triggered_only,
            auto_complete,
            assignment_result_status,
            output_prefix,
        ) {
            Ok(payload) => payload,
            Err(error) => {
                eprintln!(
                    "scheduler loop iteration {} failed: {error}",
                    iterations_run + 1
                );
                return ExitCode::from(1);
            }
        };
        total_scanned_task_count += payload.scanned_task_count;
        total_scheduled_task_count += payload.scheduled_task_count;
        total_completed_task_count += payload.completed_task_count;
        total_failed_task_count += payload.failed_task_count;
        total_skipped_task_count += payload.skipped_task_count;
        iterations_run += 1;
        let scheduled_this_round = payload.scheduled_task_count;
        runs.push(payload);
        if scheduled_this_round == 0 {
            stopped_idle = true;
            break;
        }
        if sleep_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    }

    let payload = SchedulerLoopJson {
        root: root.to_owned(),
        worker_node_id: worker_node_id.to_owned(),
        iterations_requested: iterations,
        until_idle,
        sleep_ms,
        triggered_only,
        iterations_run,
        stopped_idle,
        auto_complete,
        result_status: assignment_result_status.as_str().to_owned(),
        total_scanned_task_count,
        total_scheduled_task_count,
        total_completed_task_count,
        total_failed_task_count,
        total_skipped_task_count,
        runs,
    };

    if as_json {
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to render scheduler loop json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("scheduler loop completed");
    println!("  worker_node_id: {}", payload.worker_node_id);
    println!("  iterations_requested: {}", payload.iterations_requested);
    println!(
        "  until_idle: {}",
        if payload.until_idle { "true" } else { "false" }
    );
    println!("  sleep_ms: {}", payload.sleep_ms);
    println!(
        "  triggered_only: {}",
        if payload.triggered_only {
            "true"
        } else {
            "false"
        }
    );
    println!("  iterations_run: {}", payload.iterations_run);
    println!(
        "  stopped_idle: {}",
        if payload.stopped_idle {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  auto_complete: {}",
        if payload.auto_complete {
            "true"
        } else {
            "false"
        }
    );
    println!("  result_status: {}", payload.result_status);
    println!(
        "  total_scanned_task_count: {}",
        payload.total_scanned_task_count
    );
    println!(
        "  total_scheduled_task_count: {}",
        payload.total_scheduled_task_count
    );
    println!(
        "  total_completed_task_count: {}",
        payload.total_completed_task_count
    );
    println!(
        "  total_failed_task_count: {}",
        payload.total_failed_task_count
    );
    println!(
        "  total_skipped_task_count: {}",
        payload.total_skipped_task_count
    );
    for (index, run) in payload.runs.iter().enumerate() {
        println!(
            "  run: iteration={} scheduled={} completed={} failed={} skipped={}",
            index + 1,
            run.scheduled_task_count,
            run.completed_task_count,
            run.failed_task_count,
            run.skipped_task_count
        );
    }
    ExitCode::SUCCESS
}
