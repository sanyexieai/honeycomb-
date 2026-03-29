use super::super::overview::support::*;
use super::super::*;
use super::basic;

#[derive(Clone, serde::Deserialize, Serialize)]
pub(crate) struct TaskRerunJson {
    pub(crate) task_id: String,
    pub(crate) status: String,
    pub(crate) trigger_id: Option<String>,
    pub(crate) trigger_fired: bool,
    pub(crate) schedule_now: bool,
    pub(crate) scheduled: bool,
    pub(crate) scheduled_assignment_id: Option<String>,
}

#[derive(serde::Deserialize, Serialize)]
pub(crate) struct TaskRerunBatchJson {
    pub(crate) mode: String,
    pub(crate) dry_run: bool,
    pub(crate) summary_only: bool,
    pub(crate) task_count: usize,
    pub(crate) tasks: Vec<TaskRerunJson>,
}

#[derive(Serialize)]
struct TaskRerunPlanSummaryJson {
    path: String,
    mode: String,
    dry_run: bool,
    summary_only: bool,
    task_count: usize,
    task_ids: Vec<String>,
}

fn rerun_task_internal(
    root: &str,
    task_id: &str,
    trigger_id: Option<&str>,
    fire_trigger: bool,
    schedule_now: bool,
    worker_node_id: &str,
    auto_complete: bool,
    result_status: &str,
    output_prefix: &str,
) -> std::io::Result<TaskRerunJson> {
    basic::reopen_task_internal(root, task_id)?;

    let mut trigger_fired = false;
    if fire_trigger {
        let Some(trigger_id) = trigger_id else {
            return Err(std::io::Error::other(
                "--fire-trigger requires --trigger-id",
            ));
        };
        trigger::fire_trigger_internal(root, task_id, trigger_id)?;
        trigger_fired = true;
    }

    let mut scheduled = false;
    let mut scheduled_assignment_id = None::<String>;
    if schedule_now {
        let assignment_result_status = scheduler::scheduler_assignment_result_status(result_status);
        let (_, task) = load_task_submission(root, task_id)?;
        let outcome = scheduler::schedule_single_task_internal(
            root,
            task,
            worker_node_id,
            false,
            auto_complete,
            assignment_result_status,
            output_prefix,
        )?;
        if let Some(assignment) = outcome.assignment {
            scheduled = true;
            scheduled_assignment_id = Some(assignment.assignment_id);
        }
    }

    let (_, task) = load_task_submission(root, task_id)?;

    Ok(TaskRerunJson {
        task_id: task.task_spec.task_id,
        status: task.task_runtime.status.as_str().to_owned(),
        trigger_id: trigger_id.map(str::to_owned),
        trigger_fired,
        schedule_now,
        scheduled,
        scheduled_assignment_id,
    })
}

fn rerun_batch_filters_match(
    root: &str,
    task: &crate::runtime::TaskRecord,
    tenant_filter: Option<&str>,
    namespace_filter: Option<&str>,
    skill_ref_filter: Option<&str>,
    implementation_ref_filter: Option<&str>,
    goal_contains_filter: Option<&str>,
    assignment_status_filter: Option<&str>,
    has_trigger_filter: bool,
    without_trigger_filter: bool,
    with_active_resident_filter: bool,
    without_resident_filter: bool,
) -> bool {
    if tenant_filter.is_some_and(|tenant| task.task_spec.tenant_id != tenant) {
        return false;
    }
    if namespace_filter.is_some_and(|namespace| task.task_spec.namespace != namespace) {
        return false;
    }
    if skill_ref_filter
        .is_some_and(|skill| !task.task_spec.skill_refs.iter().any(|value| value == skill))
    {
        return false;
    }
    if implementation_ref_filter.is_some_and(|implementation| {
        task.task_spec.implementation_ref.as_deref() != Some(implementation)
    }) {
        return false;
    }
    if goal_contains_filter.is_some_and(|goal| !task.task_spec.goal.contains(goal)) {
        return false;
    }
    if let Some(status) = assignment_status_filter {
        let assignments = match load_task_assignments(root, &task.task_spec.task_id) {
            Ok((_, assignments)) => assignments,
            Err(_) => return false,
        };
        let latest_assignment = assignments.into_iter().max_by_key(|assignment| {
            assignment
                .attempt_id
                .strip_prefix("attempt-")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0)
        });
        if latest_assignment
            .as_ref()
            .is_none_or(|assignment| assignment.status.as_str() != status)
        {
            return false;
        }
    }
    if has_trigger_filter || without_trigger_filter {
        let has_trigger = match list_triggers(root, &task.task_spec.task_id) {
            Ok((_, triggers)) => !triggers.is_empty(),
            Err(_) => return false,
        };
        if has_trigger_filter && !has_trigger {
            return false;
        }
        if without_trigger_filter && has_trigger {
            return false;
        }
    }
    if with_active_resident_filter || without_resident_filter {
        let residents = match list_residents(root, &task.task_spec.task_id) {
            Ok((_, residents)) => residents,
            Err(_) => return false,
        };
        let has_active_resident = residents
            .iter()
            .any(|resident| resident.status.as_str() == "running");
        if with_active_resident_filter && !has_active_resident {
            return false;
        }
        if without_resident_filter && !residents.is_empty() {
            return false;
        }
    }
    true
}

fn sort_rerun_batch_tasks(tasks: &mut [TaskRerunJson], sort: &str) {
    match sort {
        "status" => tasks.sort_by(|a, b| {
            a.status
                .cmp(&b.status)
                .then_with(|| a.task_id.cmp(&b.task_id))
        }),
        _ => tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id)),
    }
}

pub(crate) fn save_rerun_plan(path: &str, batch: &TaskRerunBatchJson) -> std::io::Result<()> {
    let plan_path = std::path::Path::new(path);
    if let Some(parent) = plan_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let body = serde_json::to_string_pretty(batch)
        .map(|value| format!("{value}\n"))
        .map_err(std::io::Error::other)?;
    std::fs::write(plan_path, body)?;
    Ok(())
}

pub(crate) fn load_rerun_plan(path: &str) -> std::io::Result<TaskRerunBatchJson> {
    let body = std::fs::read_to_string(path)?;
    serde_json::from_str(&body).map_err(std::io::Error::other)
}

fn rerun_plan_summary(path: &str) -> std::io::Result<TaskRerunPlanSummaryJson> {
    let plan = load_rerun_plan(path)?;
    Ok(TaskRerunPlanSummaryJson {
        path: path.to_owned(),
        mode: plan.mode,
        dry_run: plan.dry_run,
        summary_only: plan.summary_only,
        task_count: plan.task_count,
        task_ids: plan.tasks.into_iter().map(|task| task.task_id).collect(),
    })
}

pub(crate) fn list_rerun_plans(root: &str) -> std::io::Result<Vec<(String, TaskRerunBatchJson)>> {
    let plans_dir = std::path::Path::new(root).join("plans");
    if !plans_dir.exists() {
        return Ok(Vec::new());
    }

    let mut plans = Vec::new();
    for entry in std::fs::read_dir(&plans_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let path_string = path.to_string_lossy().into_owned();
        let plan = load_rerun_plan(&path_string)?;
        plans.push((path_string, plan));
    }
    Ok(plans)
}

pub(crate) fn collect_rerun_plan_alerts(
    root: &str,
    tasks: &[crate::runtime::TaskRecord],
    owner_filter: Option<&str>,
    kind_filter: Option<&str>,
    severity_filter: Option<&str>,
) -> std::io::Result<Vec<SystemAlertJson>> {
    if !alert_kind_matches(kind_filter, "rerun_plan_pending") {
        return Ok(Vec::new());
    }
    if !alert_severity_matches(severity_filter, "attention") {
        return Ok(Vec::new());
    }

    let plans = list_rerun_plans(root)?;
    let mut task_counts = std::collections::BTreeMap::<String, usize>::new();
    for (_, plan) in plans {
        for task in plan.tasks {
            *task_counts.entry(task.task_id).or_insert(0) += 1;
        }
    }

    let task_index = tasks
        .iter()
        .map(|task| (task.task_spec.task_id.clone(), task))
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut alerts = Vec::new();
    for (task_id, plan_count) in task_counts {
        let owner = task_index
            .get(&task_id)
            .map(|task| task.task_spec.tenant_id.clone());
        if owner_filter.is_some_and(|filter| owner.as_deref() != Some(filter)) {
            continue;
        }
        alerts.push(SystemAlertJson {
            kind: "rerun_plan_pending".to_owned(),
            severity: "attention".to_owned(),
            owner,
            target: task_id,
            detail: format!("pending_rerun_plan_count={plan_count}"),
        });
    }
    Ok(alerts)
}

pub(crate) fn append_rerun_plan(path: &str, incoming: &TaskRerunBatchJson) -> std::io::Result<()> {
    let mut merged = match load_rerun_plan(path) {
        Ok(plan) => plan,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => TaskRerunBatchJson {
            mode: "appended".to_owned(),
            dry_run: incoming.dry_run,
            summary_only: false,
            task_count: 0,
            tasks: Vec::new(),
        },
        Err(error) => return Err(error),
    };

    let mut by_task_id = std::collections::BTreeMap::<String, TaskRerunJson>::new();
    for task in merged.tasks.drain(..) {
        by_task_id.insert(task.task_id.clone(), task);
    }
    for task in incoming.tasks.iter().cloned() {
        by_task_id.insert(task.task_id.clone(), task);
    }

    merged.mode = "appended".to_owned();
    merged.dry_run = merged.dry_run && incoming.dry_run;
    merged.summary_only = false;
    merged.tasks = by_task_id.into_values().collect();
    merged.task_count = merged.tasks.len();
    save_rerun_plan(path, &merged)
}

pub(crate) fn prune_rerun_plan(
    path: &str,
    root: &str,
    prune_status: &str,
) -> std::io::Result<TaskRerunBatchJson> {
    let mut plan = load_rerun_plan(path)?;
    let mut retained = Vec::new();

    for task in plan.tasks {
        let keep = match load_task_submission(root, &task.task_id) {
            Ok((_, record)) => record.task_runtime.status.as_str() != prune_status,
            Err(_) => true,
        };
        if keep {
            retained.push(task);
        }
    }

    plan.mode = format!("pruned:{}", plan.mode);
    plan.summary_only = false;
    plan.task_count = retained.len();
    plan.tasks = retained;
    save_rerun_plan(path, &plan)?;
    Ok(plan)
}

pub(crate) fn handle_task_rerun(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let from_plan = option_value(args, "--from-plan");
    let prune_plan = option_value(args, "--prune-plan");
    let plan_summary = option_value(args, "--plan-summary");
    let all_failed = has_flag(args, "--all-failed");
    let all_completed = has_flag(args, "--all-completed");
    let tenant_filter = option_value(args, "--tenant");
    let namespace_filter = option_value(args, "--namespace");
    let skill_ref_filter = option_value(args, "--skill-ref");
    let implementation_ref_filter = option_value(args, "--implementation-ref");
    let goal_contains_filter = option_value(args, "--goal-contains");
    let assignment_status_filter = option_value(args, "--assignment-status");
    let has_trigger_filter = has_flag(args, "--has-trigger");
    let without_trigger_filter = has_flag(args, "--without-trigger");
    let with_active_resident_filter = has_flag(args, "--with-active-resident");
    let without_resident_filter = has_flag(args, "--without-resident");
    let sort = option_value(args, "--sort").unwrap_or("target");
    let limit = option_value(args, "--limit").and_then(|value| value.parse::<usize>().ok());
    let dry_run = has_flag(args, "--dry-run");
    let summary_only = has_flag(args, "--summary-only");
    let save_plan = option_value(args, "--save-plan");
    let append_plan = option_value(args, "--append-plan");
    let trigger_id = option_value(args, "--trigger-id");
    let fire_trigger = has_flag(args, "--fire-trigger");
    let schedule_now = has_flag(args, "--schedule-now");
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-rerun");
    let auto_complete = has_flag(args, "--auto-complete");
    let result_status = option_value(args, "--result-status").unwrap_or("completed");
    let output_prefix = option_value(args, "--output-prefix").unwrap_or("rerun-output");
    let as_json = has_flag(args, "--json");
    let root = option_value(args, "--root").unwrap_or(".");

    let plan_mode_count = [from_plan, prune_plan, plan_summary]
        .into_iter()
        .flatten()
        .count();
    if plan_mode_count > 1 {
        eprintln!(
            "failed to rerun task: --from-plan, --prune-plan, and --plan-summary are mutually exclusive"
        );
        return ExitCode::from(1);
    }
    if from_plan.is_some() && prune_plan.is_some() {
        eprintln!("failed to rerun task: --from-plan and --prune-plan are mutually exclusive");
        return ExitCode::from(1);
    }
    if save_plan.is_some() && append_plan.is_some() {
        eprintln!("failed to rerun task: --save-plan and --append-plan are mutually exclusive");
        return ExitCode::from(1);
    }
    if prune_plan.is_some()
        && (all_failed
            || all_completed
            || tenant_filter.is_some()
            || namespace_filter.is_some()
            || skill_ref_filter.is_some()
            || implementation_ref_filter.is_some()
            || goal_contains_filter.is_some()
            || assignment_status_filter.is_some()
            || has_trigger_filter
            || without_trigger_filter
            || with_active_resident_filter
            || without_resident_filter
            || limit.is_some()
            || dry_run
            || summary_only
            || save_plan.is_some()
            || append_plan.is_some()
            || trigger_id.is_some()
            || fire_trigger)
    {
        eprintln!(
            "failed to rerun task: --prune-plan cannot be combined with batch filters, plan writes, dry-run, summary-only, or trigger-specific options"
        );
        return ExitCode::from(1);
    }
    if plan_summary.is_some()
        && (all_failed
            || all_completed
            || tenant_filter.is_some()
            || namespace_filter.is_some()
            || skill_ref_filter.is_some()
            || implementation_ref_filter.is_some()
            || goal_contains_filter.is_some()
            || assignment_status_filter.is_some()
            || has_trigger_filter
            || without_trigger_filter
            || with_active_resident_filter
            || without_resident_filter
            || limit.is_some()
            || dry_run
            || summary_only
            || save_plan.is_some()
            || append_plan.is_some()
            || trigger_id.is_some()
            || fire_trigger
            || schedule_now
            || auto_complete)
    {
        eprintln!(
            "failed to rerun task: --plan-summary cannot be combined with execution, filters, plan writes, or trigger-specific options"
        );
        return ExitCode::from(1);
    }
    if from_plan.is_some() && (all_failed || all_completed) {
        eprintln!(
            "failed to rerun task: --from-plan cannot be combined with --all-failed or --all-completed"
        );
        return ExitCode::from(1);
    }
    if from_plan.is_some()
        && (tenant_filter.is_some()
            || namespace_filter.is_some()
            || skill_ref_filter.is_some()
            || implementation_ref_filter.is_some()
            || goal_contains_filter.is_some()
            || assignment_status_filter.is_some()
            || has_trigger_filter
            || without_trigger_filter
            || with_active_resident_filter
            || without_resident_filter
            || limit.is_some()
            || dry_run
            || summary_only
            || trigger_id.is_some()
            || fire_trigger)
    {
        eprintln!(
            "failed to rerun task: --from-plan cannot be combined with filters, dry-run, summary-only, or trigger-specific options"
        );
        return ExitCode::from(1);
    }
    if all_failed && all_completed {
        eprintln!("failed to rerun task: --all-failed and --all-completed are mutually exclusive");
        return ExitCode::from(1);
    }
    if has_trigger_filter && without_trigger_filter {
        eprintln!(
            "failed to rerun task: --has-trigger and --without-trigger are mutually exclusive"
        );
        return ExitCode::from(1);
    }
    if with_active_resident_filter && without_resident_filter {
        eprintln!(
            "failed to rerun task: --with-active-resident and --without-resident are mutually exclusive"
        );
        return ExitCode::from(1);
    }
    if (all_failed || all_completed) && fire_trigger {
        eprintln!("failed to rerun task: batch rerun cannot be combined with --fire-trigger");
        return ExitCode::from(1);
    }
    if (all_failed || all_completed) && trigger_id.is_some() {
        eprintln!("failed to rerun task: batch rerun cannot be combined with --trigger-id");
        return ExitCode::from(1);
    }
    if !(all_failed || all_completed)
        && (tenant_filter.is_some()
            || namespace_filter.is_some()
            || skill_ref_filter.is_some()
            || implementation_ref_filter.is_some()
            || goal_contains_filter.is_some()
            || assignment_status_filter.is_some()
            || has_trigger_filter
            || without_trigger_filter
            || with_active_resident_filter
            || without_resident_filter
            || limit.is_some())
    {
        eprintln!(
            "failed to rerun task: --tenant/--namespace/--skill-ref/--implementation-ref/--goal-contains/--assignment-status/--has-trigger/--without-trigger/--with-active-resident/--without-resident/--limit require --all-failed or --all-completed"
        );
        return ExitCode::from(1);
    }
    if dry_run && !(all_failed || all_completed) {
        eprintln!("failed to rerun task: --dry-run requires --all-failed or --all-completed");
        return ExitCode::from(1);
    }
    if summary_only && !(all_failed || all_completed) {
        eprintln!("failed to rerun task: --summary-only requires --all-failed or --all-completed");
        return ExitCode::from(1);
    }

    if let Some(plan_path) = plan_summary {
        let summary = match rerun_plan_summary(plan_path) {
            Ok(summary) => summary,
            Err(error) => {
                eprintln!("failed to read rerun plan summary: {error}");
                return ExitCode::from(1);
            }
        };
        if as_json {
            match serde_json::to_string_pretty(&summary) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("failed to render rerun plan summary json: {error}");
                    return ExitCode::from(1);
                }
            }
            return ExitCode::SUCCESS;
        }
        println!("rerun plan summary");
        println!("  path: {}", summary.path);
        println!("  mode: {}", summary.mode);
        println!(
            "  dry_run: {}",
            if summary.dry_run { "true" } else { "false" }
        );
        println!(
            "  summary_only: {}",
            if summary.summary_only {
                "true"
            } else {
                "false"
            }
        );
        println!("  task_count: {}", summary.task_count);
        for task_id in summary.task_ids {
            println!("  task_id: {}", task_id);
        }
        return ExitCode::SUCCESS;
    }

    let batch = if let Some(plan_path) = prune_plan {
        match prune_rerun_plan(plan_path, root, result_status) {
            Ok(plan) => plan,
            Err(error) => {
                eprintln!("failed to prune rerun plan: {error}");
                return ExitCode::from(1);
            }
        }
    } else if let Some(plan_path) = from_plan {
        let plan = match load_rerun_plan(plan_path) {
            Ok(plan) => plan,
            Err(error) => {
                eprintln!("failed to load rerun plan: {error}");
                return ExitCode::from(1);
            }
        };
        if plan.tasks.is_empty() {
            eprintln!("failed to rerun task: rerun plan contains no task entries");
            return ExitCode::from(1);
        }
        let mut rerun_tasks = Vec::new();
        for planned_task in plan.tasks {
            let payload = match rerun_task_internal(
                root,
                &planned_task.task_id,
                None,
                false,
                schedule_now,
                worker_node_id,
                auto_complete,
                result_status,
                output_prefix,
            ) {
                Ok(payload) => payload,
                Err(error) => {
                    eprintln!(
                        "failed to rerun task {} from plan: {error}",
                        planned_task.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            rerun_tasks.push(payload);
        }
        TaskRerunBatchJson {
            mode: format!("from_plan:{}", plan.mode),
            dry_run: false,
            summary_only: false,
            task_count: rerun_tasks.len(),
            tasks: rerun_tasks,
        }
    } else if all_failed || all_completed {
        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to list tasks for rerun: {error}");
                return ExitCode::from(1);
            }
        };
        let rerun_task_ids = tasks
            .into_iter()
            .filter(|task| {
                if all_failed {
                    task.task_runtime.status == TaskStatus::Failed
                } else {
                    task.task_runtime.status == TaskStatus::Completed
                }
            })
            .filter(|task| {
                rerun_batch_filters_match(
                    root,
                    task,
                    tenant_filter,
                    namespace_filter,
                    skill_ref_filter,
                    implementation_ref_filter,
                    goal_contains_filter,
                    assignment_status_filter,
                    has_trigger_filter,
                    without_trigger_filter,
                    with_active_resident_filter,
                    without_resident_filter,
                )
            })
            .map(|task| task.task_spec.task_id)
            .take(limit.unwrap_or(usize::MAX))
            .collect::<Vec<_>>();
        let mut rerun_tasks = Vec::new();
        if dry_run {
            for rerun_task_id in rerun_task_ids {
                let (_, task) = match load_task_submission(root, &rerun_task_id) {
                    Ok(value) => value,
                    Err(error) => {
                        eprintln!(
                            "failed to load task {} for rerun dry-run: {error}",
                            rerun_task_id
                        );
                        return ExitCode::from(1);
                    }
                };
                rerun_tasks.push(TaskRerunJson {
                    task_id: rerun_task_id,
                    status: task.task_runtime.status.as_str().to_owned(),
                    trigger_id: None,
                    trigger_fired: false,
                    schedule_now,
                    scheduled: false,
                    scheduled_assignment_id: None,
                });
            }
            sort_rerun_batch_tasks(&mut rerun_tasks, sort);
        } else {
            for rerun_task_id in rerun_task_ids {
                let payload = match rerun_task_internal(
                    root,
                    &rerun_task_id,
                    None,
                    false,
                    schedule_now,
                    worker_node_id,
                    auto_complete,
                    result_status,
                    output_prefix,
                ) {
                    Ok(payload) => payload,
                    Err(error) => {
                        eprintln!("failed to rerun task {}: {error}", rerun_task_id);
                        return ExitCode::from(1);
                    }
                };
                rerun_tasks.push(payload);
            }
        }
        let task_count = rerun_tasks.len();
        if summary_only {
            rerun_tasks.clear();
        }
        TaskRerunBatchJson {
            mode: if all_failed {
                "all_failed".to_owned()
            } else {
                "all_completed".to_owned()
            },
            dry_run,
            summary_only,
            task_count,
            tasks: rerun_tasks,
        }
    } else {
        let payload = match rerun_task_internal(
            root,
            task_id,
            trigger_id,
            fire_trigger,
            schedule_now,
            worker_node_id,
            auto_complete,
            result_status,
            output_prefix,
        ) {
            Ok(payload) => payload,
            Err(error) => {
                eprintln!("failed to rerun task: {error}");
                return ExitCode::from(1);
            }
        };
        TaskRerunBatchJson {
            mode: "single".to_owned(),
            dry_run: false,
            summary_only: false,
            task_count: 1,
            tasks: vec![payload],
        }
    };

    if let Some(path) = save_plan {
        if let Err(error) = save_rerun_plan(path, &batch) {
            eprintln!("failed to save rerun plan: {error}");
            return ExitCode::from(1);
        }
    } else if let Some(path) = append_plan {
        if let Err(error) = append_rerun_plan(path, &batch) {
            eprintln!("failed to append rerun plan: {error}");
            return ExitCode::from(1);
        }
    }

    if as_json {
        match serde_json::to_string_pretty(&batch) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to render task rerun json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("task rerun recorded");
    println!("  mode: {}", batch.mode);
    println!(
        "  dry_run: {}",
        if batch.dry_run { "true" } else { "false" }
    );
    println!(
        "  summary_only: {}",
        if batch.summary_only { "true" } else { "false" }
    );
    println!("  save_plan: {}", save_plan.unwrap_or("<none>"));
    println!("  append_plan: {}", append_plan.unwrap_or("<none>"));
    println!("  task_count: {}", batch.task_count);
    for payload in batch.tasks {
        println!("  task_id: {}", payload.task_id);
        println!("  status: {}", payload.status);
        println!(
            "  trigger_id: {}",
            payload.trigger_id.as_deref().unwrap_or("<none>")
        );
        println!(
            "  trigger_fired: {}",
            if payload.trigger_fired {
                "true"
            } else {
                "false"
            }
        );
        println!(
            "  schedule_now: {}",
            if payload.schedule_now {
                "true"
            } else {
                "false"
            }
        );
        println!(
            "  scheduled: {}",
            if payload.scheduled { "true" } else { "false" }
        );
        println!(
            "  scheduled_assignment_id: {}",
            payload
                .scheduled_assignment_id
                .as_deref()
                .unwrap_or("<none>")
        );
    }
    ExitCode::SUCCESS
}
