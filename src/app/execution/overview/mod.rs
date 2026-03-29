pub(super) mod support;

use super::common_support::*;
use super::control::*;
use super::*;
use support::*;

pub(crate) fn handle_runtime_overview(args: &[String]) -> ExitCode {
    let with_details = has_flag(args, "--with-details");
    let with_gaps = has_flag(args, "--with-gaps");
    let with_policy = has_flag(args, "--with-policy");
    let exclude_legacy = has_flag(args, "--exclude-legacy");
    let as_json = has_flag(args, "--json");
    let root = option_value(args, "--root").unwrap_or(".");

    let (tasks_dir, all_tasks) = match crate::storage::list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list runtime tasks: {error}");
            return ExitCode::from(1);
        }
    };
    let tasks = all_tasks
        .into_iter()
        .filter(|task| !exclude_legacy || !is_legacy_demo_task(task))
        .collect::<Vec<_>>();
    let (_, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tools for runtime overview: {error}");
            return ExitCode::from(1);
        }
    };

    let mut assignment_count = 0usize;
    let mut resident_count = 0usize;
    let mut trigger_count = 0usize;
    let mut audit_count = 0usize;
    let mut trace_count = 0usize;
    let mut implementation_usage = std::collections::BTreeMap::<String, usize>::new();
    let mut task_status_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut assignment_status_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut resident_status_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut trigger_status_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut trigger_consumption_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut active_reason_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut active_task_rows = Vec::<(
        String,
        &'static str,
        &'static str,
        String,
        String,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
    )>::new();

    for task in &tasks {
        if let Some(implementation_ref) = &task.task_spec.implementation_ref {
            *implementation_usage
                .entry(implementation_ref.clone())
                .or_insert(0) += 1;
        }
        *task_status_counts
            .entry(task.task_runtime.status.as_str().to_owned())
            .or_insert(0) += 1;

        let (_, assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load assignments for runtime overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        assignment_count += assignments.len();
        for assignment in &assignments {
            *assignment_status_counts
                .entry(assignment.status.as_str().to_owned())
                .or_insert(0) += 1;
        }

        let (_, residents) = match list_residents(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load residents for runtime overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        resident_count += residents.len();
        for resident in &residents {
            *resident_status_counts
                .entry(resident.status.as_str().to_owned())
                .or_insert(0) += 1;
        }

        let (_, triggers) = match list_triggers(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load triggers for runtime overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        trigger_count += triggers.len();
        for trigger in &triggers {
            *trigger_status_counts
                .entry(trigger.status.as_str().to_owned())
                .or_insert(0) += 1;
            let trigger_consumption_id = if trigger.has_unconsumed_fire() {
                format!("ready_unconsumed:{}", trigger.trigger_type)
            } else if trigger.fire_count > 0 {
                format!("consumed:{}", trigger.trigger_type)
            } else {
                format!("idle:{}", trigger.trigger_type)
            };
            *trigger_consumption_counts
                .entry(trigger_consumption_id)
                .or_insert(0) += 1;
        }
        if let Some(reason) = classify_active_task(task, &assignments, &residents, &triggers) {
            let active_assignment_count = assignments
                .iter()
                .filter(|assignment| {
                    matches!(
                        assignment.status,
                        AssignmentStatus::Created
                            | AssignmentStatus::Assigned
                            | AssignmentStatus::Running
                            | AssignmentStatus::RetryPending
                    )
                })
                .count();
            let running_resident_count = residents
                .iter()
                .filter(|resident| resident.status.as_str() == "running")
                .count();
            let active_trigger_count = triggers
                .iter()
                .filter(|trigger| trigger.status.as_str() == "active")
                .count();
            *active_reason_counts
                .entry(reason.as_str().to_owned())
                .or_insert(0) += 1;
            active_task_rows.push((
                task.task_spec.task_id.clone(),
                task.task_runtime.status.as_str(),
                reason.as_str(),
                joined_or_none(&task.task_spec.skill_refs),
                joined_or_none(&task.task_spec.tool_refs),
                assignments.len(),
                active_assignment_count,
                residents.len(),
                running_resident_count,
                triggers.len(),
                active_trigger_count,
            ));
        }

        let (_, audits) = match load_task_audits(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load audits for runtime overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        audit_count += audits.len();

        let (_, traces) = match load_task_traces(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load traces for runtime overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        trace_count += traces.len();
    }

    let completed_task_count = tasks
        .iter()
        .filter(|task| task.task_runtime.status.as_str() == "completed")
        .count();
    let bound_task_count = tasks
        .iter()
        .filter(|task| task.task_spec.implementation_ref.is_some())
        .count();

    let policy = if with_policy {
        let shell_tools = tools
            .iter()
            .filter(|tool| is_shell_tool(tool))
            .collect::<Vec<_>>();
        let (_, approval_requests) = match list_shell_approval_requests(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load shell approval requests for runtime overview: {error}");
                return ExitCode::from(1);
            }
        };
        let shell_tools_allowed = shell_tools.iter().filter(|tool| tool.allow_shell).count();
        let shell_tools_pending = shell_tools
            .iter()
            .filter(|tool| tool.shell_approval_pending)
            .count();
        let shell_tools_blocked = shell_tools.len().saturating_sub(shell_tools_allowed);
        let shell_tool_rows = shell_tools
            .iter()
            .map(|tool| RuntimeOverviewPolicyShellToolJson {
                tool_id: tool.tool_id.clone(),
                owner: tool.owner.clone(),
                policy: tool_policy_summary(tool),
            })
            .collect::<Vec<_>>();
        let recent_changes = match recent_tool_policy_audits(root, 5) {
            Ok(audits) => audits
                .into_iter()
                .map(|audit| RuntimeOverviewPolicyRecentChangeJson {
                    timestamp: audit.timestamp,
                    action: audit.action,
                    tool_id: audit.target_id,
                    result: audit.result,
                    detail: audit.payload,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                eprintln!("failed to load recent tool policy audits: {error}");
                return ExitCode::from(1);
            }
        };

        Some(RuntimeOverviewPolicyJson {
            tool_count: tools.len(),
            shell_tool_count: shell_tools.len(),
            shell_tool_allowed_count: shell_tools_allowed,
            shell_tool_pending_count: shell_tools_pending,
            shell_tool_blocked_count: shell_tools_blocked,
            shell_request_count: approval_requests.len(),
            shell_request_pending_count: approval_requests
                .iter()
                .filter(|request| request.status == ApprovalRequestStatus::Pending)
                .count(),
            recent_change_count: recent_changes.len(),
            shell_tools: shell_tool_rows,
            recent_changes,
        })
    } else {
        None
    };

    let gaps = if with_gaps {
        let mut unbound_tasks_no_skill = Vec::new();
        let mut unbound_tasks_missing_recommendation = Vec::new();
        for task in &tasks {
            if task.task_spec.implementation_ref.is_some() {
                continue;
            }
            if task.task_spec.skill_refs.is_empty() {
                unbound_tasks_no_skill.push(RuntimeOverviewGapTaskJson {
                    task_id: task.task_spec.task_id.clone(),
                    status: task.task_runtime.status.as_str().to_owned(),
                    goal: task.task_spec.goal.clone(),
                    skills: None,
                });
            } else {
                unbound_tasks_missing_recommendation.push(RuntimeOverviewGapTaskJson {
                    task_id: task.task_spec.task_id.clone(),
                    status: task.task_runtime.status.as_str().to_owned(),
                    goal: task.task_spec.goal.clone(),
                    skills: Some(joined_or_none(&task.task_spec.skill_refs)),
                });
            }
        }
        let mut active_reason_rows = active_reason_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        active_reason_rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut trigger_waiting_consumption_rows = tasks
            .iter()
            .filter_map(|task| {
                let (_, triggers) = match list_triggers(root, &task.task_spec.task_id) {
                    Ok(value) => value,
                    Err(error) => {
                        eprintln!(
                            "failed to load triggers for runtime overview gaps task {}: {error}",
                            task.task_spec.task_id
                        );
                        return None;
                    }
                };
                let ready_count = triggers
                    .iter()
                    .filter(|trigger| trigger.has_unconsumed_fire())
                    .count();
                (ready_count > 0).then(|| RuntimeOverviewUsageJson {
                    id: task.task_spec.task_id.clone(),
                    count: ready_count,
                })
            })
            .collect::<Vec<_>>();
        trigger_waiting_consumption_rows
            .sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let active_task_rows_json = active_task_rows
            .iter()
            .map(
                |(
                    task_id,
                    status,
                    reason,
                    skills,
                    tools,
                    assignment_total,
                    assignment_active,
                    resident_total,
                    resident_running,
                    trigger_total,
                    trigger_active,
                )| RuntimeOverviewActiveTaskJson {
                    task_id: task_id.clone(),
                    status: (*status).to_owned(),
                    reason: (*reason).to_owned(),
                    skills: skills.clone(),
                    tools: tools.clone(),
                    assignment_total: *assignment_total,
                    assignment_active: *assignment_active,
                    resident_total: *resident_total,
                    resident_running: *resident_running,
                    trigger_total: *trigger_total,
                    trigger_active: *trigger_active,
                },
            )
            .collect::<Vec<_>>();
        let blocked_shell_tools = tools
            .iter()
            .filter(|tool| is_shell_tool(tool) && !tool.allow_shell)
            .collect::<Vec<_>>();
        let blocked_shell_tool_rows = blocked_shell_tools
            .iter()
            .map(|tool| RuntimeOverviewPolicyShellToolJson {
                tool_id: tool.tool_id.clone(),
                owner: tool.owner.clone(),
                policy: tool_policy_summary(tool),
            })
            .collect::<Vec<_>>();
        let pending_shell_tool_rows = blocked_shell_tools
            .iter()
            .filter(|tool| tool.shell_approval_pending)
            .map(|tool| RuntimeOverviewPolicyShellToolJson {
                tool_id: tool.tool_id.clone(),
                owner: tool.owner.clone(),
                policy: tool_policy_summary(tool),
            })
            .collect::<Vec<_>>();

        Some(RuntimeOverviewGapsJson {
            task_without_implementation_no_skill_count: unbound_tasks_no_skill.len(),
            task_without_implementation_no_skill: unbound_tasks_no_skill,
            task_without_implementation_missing_recommendation_count:
                unbound_tasks_missing_recommendation.len(),
            task_without_implementation_missing_recommendation:
                unbound_tasks_missing_recommendation,
            active_task_count: active_task_rows_json.len(),
            active_task_reason_count: active_reason_rows.len(),
            active_task_reasons: active_reason_rows,
            active_tasks: active_task_rows_json,
            blocked_shell_tool_count: blocked_shell_tool_rows.len(),
            blocked_shell_tools: blocked_shell_tool_rows,
            pending_shell_tool_count: pending_shell_tool_rows.len(),
            pending_shell_tools: pending_shell_tool_rows,
            trigger_waiting_consumption_count: trigger_waiting_consumption_rows.len(),
            trigger_waiting_consumption: trigger_waiting_consumption_rows,
        })
    } else {
        None
    };

    let details = if with_details {
        let mut implementation_usage_rows = implementation_usage
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        implementation_usage_rows
            .sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut task_status_rows = task_status_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        task_status_rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut active_reason_rows = active_reason_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        active_reason_rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut assignment_status_rows = assignment_status_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        assignment_status_rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut resident_status_rows = resident_status_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        resident_status_rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut trigger_status_rows = trigger_status_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        trigger_status_rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
        let mut trigger_consumption_rows = trigger_consumption_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        trigger_consumption_rows
            .sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));

        Some(RuntimeOverviewDetailsJson {
            implementation_usage_detail_count: implementation_usage_rows.len(),
            implementation_usage: implementation_usage_rows,
            task_status_detail_count: task_status_rows.len(),
            task_statuses: task_status_rows,
            active_task_reason_detail_count: active_reason_rows.len(),
            active_task_reasons: active_reason_rows,
            assignment_status_detail_count: assignment_status_rows.len(),
            assignment_statuses: assignment_status_rows,
            resident_status_detail_count: resident_status_rows.len(),
            resident_statuses: resident_status_rows,
            trigger_status_detail_count: trigger_status_rows.len(),
            trigger_statuses: trigger_status_rows,
            trigger_consumption_detail_count: trigger_consumption_rows.len(),
            trigger_consumption: trigger_consumption_rows,
        })
    } else {
        None
    };

    if as_json {
        let payload = RuntimeOverviewJson {
            tasks_dir: tasks_dir.display().to_string(),
            exclude_legacy,
            task_count: tasks.len(),
            completed_task_count,
            implementation_bound_task_count: bound_task_count,
            assignment_count,
            resident_count,
            trigger_count,
            audit_count,
            trace_count,
            policy,
            gaps,
            details,
        };
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to render runtime overview json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("runtime overview loaded");
    println!("  tasks_dir: {}", tasks_dir.display());
    println!(
        "  exclude_legacy: {}",
        if exclude_legacy { "true" } else { "false" }
    );
    println!("  task_count: {}", tasks.len());
    println!("  completed_task_count: {}", completed_task_count);
    println!("  implementation_bound_task_count: {}", bound_task_count);
    println!("  assignment_count: {}", assignment_count);
    println!("  resident_count: {}", resident_count);
    println!("  trigger_count: {}", trigger_count);
    println!("  audit_count: {}", audit_count);
    println!("  trace_count: {}", trace_count);

    if let Some(policy) = &policy {
        println!("  policy_tool_count: {}", policy.tool_count);
        println!("  policy_shell_tool_count: {}", policy.shell_tool_count);
        println!(
            "  policy_shell_tool_allowed_count: {}",
            policy.shell_tool_allowed_count
        );
        println!(
            "  policy_shell_tool_pending_count: {}",
            policy.shell_tool_pending_count
        );
        println!(
            "  policy_shell_tool_blocked_count: {}",
            policy.shell_tool_blocked_count
        );
        println!(
            "  policy_shell_request_count: {}",
            policy.shell_request_count
        );
        println!(
            "  policy_shell_request_pending_count: {}",
            policy.shell_request_pending_count
        );
        for tool in &policy.shell_tools {
            println!(
                "  policy_shell_tool: tool={} owner={} policy={}",
                tool.tool_id, tool.owner, tool.policy
            );
        }
        println!(
            "  policy_recent_change_count: {}",
            policy.recent_change_count
        );
        for audit in &policy.recent_changes {
            println!(
                "  policy_recent_change: ts={} action={} tool={} result={} detail={}",
                audit.timestamp, audit.action, audit.tool_id, audit.result, audit.detail
            );
        }
    }

    if let Some(gaps) = &gaps {
        println!(
            "  gap_task_without_implementation_no_skill_count: {}",
            gaps.task_without_implementation_no_skill_count
        );
        for task in &gaps.task_without_implementation_no_skill {
            println!(
                "  gap_task_without_implementation_no_skill: task={} status={} goal={}",
                task.task_id, task.status, task.goal
            );
        }
        println!(
            "  gap_task_without_implementation_missing_recommendation_count: {}",
            gaps.task_without_implementation_missing_recommendation_count
        );
        for task in &gaps.task_without_implementation_missing_recommendation {
            println!(
                "  gap_task_without_implementation_missing_recommendation: task={} status={} skills={} goal={}",
                task.task_id,
                task.status,
                task.skills.as_deref().unwrap_or("<none>"),
                task.goal
            );
        }
        println!("  gap_active_task_count: {}", gaps.active_task_count);
        println!(
            "  gap_active_task_reason_count: {}",
            gaps.active_task_reason_count
        );
        for row in &gaps.active_task_reasons {
            println!(
                "  gap_active_task_reason: reason={} count={}",
                row.id, row.count
            );
        }
        for row in &gaps.active_tasks {
            println!(
                "  gap_active_task: task={} status={} reason={} assignments={}/{} residents={}/{} triggers={}/{} skills={} tools={}",
                row.task_id,
                row.status,
                row.reason,
                row.assignment_active,
                row.assignment_total,
                row.resident_running,
                row.resident_total,
                row.trigger_active,
                row.trigger_total,
                row.skills,
                row.tools
            );
        }
        println!(
            "  gap_blocked_shell_tool_count: {}",
            gaps.blocked_shell_tool_count
        );
        for tool in &gaps.blocked_shell_tools {
            println!(
                "  gap_blocked_shell_tool: tool={} owner={} policy={}",
                tool.tool_id, tool.owner, tool.policy
            );
        }
        println!(
            "  gap_pending_shell_tool_count: {}",
            gaps.pending_shell_tool_count
        );
        for tool in &gaps.pending_shell_tools {
            println!(
                "  gap_pending_shell_tool: tool={} owner={} policy={}",
                tool.tool_id, tool.owner, tool.policy
            );
        }
        println!(
            "  gap_trigger_waiting_consumption_count: {}",
            gaps.trigger_waiting_consumption_count
        );
        for row in &gaps.trigger_waiting_consumption {
            println!(
                "  gap_trigger_waiting_consumption: task={} ready_trigger_count={}",
                row.id, row.count
            );
        }
    }

    if let Some(details) = &details {
        println!(
            "  implementation_usage_detail_count: {}",
            details.implementation_usage_detail_count
        );
        for row in &details.implementation_usage {
            println!(
                "  implementation_usage: implementation={} task_count={}",
                row.id, row.count
            );
        }
        println!(
            "  task_status_detail_count: {}",
            details.task_status_detail_count
        );
        for row in &details.task_statuses {
            println!("  task_status: status={} count={}", row.id, row.count);
        }
        println!(
            "  active_task_reason_detail_count: {}",
            details.active_task_reason_detail_count
        );
        for row in &details.active_task_reasons {
            println!(
                "  active_task_reason: reason={} count={}",
                row.id, row.count
            );
        }
        println!(
            "  assignment_status_detail_count: {}",
            details.assignment_status_detail_count
        );
        for row in &details.assignment_statuses {
            println!("  assignment_status: status={} count={}", row.id, row.count);
        }
        println!(
            "  resident_status_detail_count: {}",
            details.resident_status_detail_count
        );
        for row in &details.resident_statuses {
            println!("  resident_status: status={} count={}", row.id, row.count);
        }
        println!(
            "  trigger_status_detail_count: {}",
            details.trigger_status_detail_count
        );
        for row in &details.trigger_statuses {
            println!("  trigger_status: status={} count={}", row.id, row.count);
        }
        println!(
            "  trigger_consumption_detail_count: {}",
            details.trigger_consumption_detail_count
        );
        for row in &details.trigger_consumption {
            println!(
                "  trigger_consumption: state={} count={}",
                row.id, row.count
            );
        }
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_system_overview(args: &[String]) -> ExitCode {
    let owner_filter = option_value(args, "--owner");
    let with_details = has_flag(args, "--with-details");
    let with_gaps = has_flag(args, "--with-gaps");
    let with_policy = has_flag(args, "--with-policy");
    let with_runtime_health = has_flag(args, "--with-runtime-health");
    let sort = option_value(args, "--sort").unwrap_or("count");
    let limit = option_value(args, "--limit").and_then(|value| value.parse::<usize>().ok());
    let summary_only = has_flag(args, "--summary-only");
    let include_acked_policy = has_flag(args, "--include-acked-policy");
    let exclude_legacy = has_flag(args, "--exclude-legacy");
    let as_json = has_flag(args, "--json");
    let root = option_value(args, "--root").unwrap_or(".");

    let (_, all_tasks) = match crate::storage::list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list tasks for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let tasks = all_tasks
        .into_iter()
        .filter(|task| !exclude_legacy || !is_legacy_demo_task(task))
        .collect::<Vec<_>>();
    let (_, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skills for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tools for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, fitness_runs) = match list_fitness_runs(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load fitness runs for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let skill_owners = skills
        .iter()
        .map(|skill| (skill.skill_id.clone(), skill.owner.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let tool_owners = tools
        .iter()
        .map(|tool| (tool.tool_id.clone(), tool.owner.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let tasks = tasks
        .into_iter()
        .filter(|task| task_matches_owner_filter(task, owner_filter, &skill_owners, &tool_owners))
        .collect::<Vec<_>>();
    let filtered_skills = skills
        .iter()
        .filter(|skill| owner_filter.is_none_or(|owner| skill.owner == owner))
        .cloned()
        .collect::<Vec<_>>();
    let filtered_tools = tools
        .iter()
        .filter(|tool| owner_filter.is_none_or(|owner| tool.owner == owner))
        .cloned()
        .collect::<Vec<_>>();
    let filtered_fitness_runs = fitness_runs
        .iter()
        .filter(|record| {
            owner_filter.is_none_or(|owner| {
                record.fitness_report.skill_refs.iter().any(|skill_ref| {
                    skill_owners
                        .get(skill_ref)
                        .is_some_and(|value| value == owner)
                }) || record.fitness_report.tool_refs.iter().any(|tool_ref| {
                    tool_owners
                        .get(tool_ref)
                        .is_some_and(|value| value == owner)
                })
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let rerun_plans = match list_rerun_plans(root) {
        Ok(plans) => plans,
        Err(error) => {
            eprintln!("failed to load rerun plans for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let rerun_plan_count = rerun_plans.len();
    let mut rerun_plan_task_rows = rerun_plans
        .iter()
        .flat_map(|(_, plan)| plan.tasks.iter().map(|task| task.task_id.clone()))
        .fold(
            std::collections::BTreeMap::<String, usize>::new(),
            |mut acc, task_id| {
                *acc.entry(task_id).or_insert(0) += 1;
                acc
            },
        )
        .into_iter()
        .map(|(id, count)| RuntimeOverviewUsageJson { id, count })
        .collect::<Vec<_>>();
    let rerun_plan_task_count = rerun_plan_task_rows.len();

    let mut assignment_count = 0usize;
    let mut resident_count = 0usize;
    let mut trigger_count = 0usize;
    let mut audit_count = 0usize;
    let mut trace_count = 0usize;
    let mut implementation_usage = std::collections::BTreeMap::<String, usize>::new();
    let mut active_reason_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut trigger_waiting_consumption_rows = Vec::<RuntimeOverviewUsageJson>::new();
    let mut active_task_rows = Vec::<RuntimeOverviewActiveTaskJson>::new();

    for task in &tasks {
        if let Some(implementation_ref) = &task.task_spec.implementation_ref {
            *implementation_usage
                .entry(implementation_ref.clone())
                .or_insert(0) += 1;
        }

        let (_, assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load assignments for system overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        assignment_count += assignments.len();

        let (_, residents) = match list_residents(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load residents for system overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        resident_count += residents.len();

        let (_, triggers) = match list_triggers(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load triggers for system overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        trigger_count += triggers.len();
        let ready_trigger_count = triggers
            .iter()
            .filter(|trigger| trigger.has_unconsumed_fire())
            .count();
        if ready_trigger_count > 0 {
            trigger_waiting_consumption_rows.push(RuntimeOverviewUsageJson {
                id: task.task_spec.task_id.clone(),
                count: ready_trigger_count,
            });
        }

        if let Some(reason) = classify_active_task(task, &assignments, &residents, &triggers) {
            *active_reason_counts
                .entry(reason.as_str().to_owned())
                .or_insert(0) += 1;
            let active_assignment_count = assignments
                .iter()
                .filter(|assignment| {
                    matches!(
                        assignment.status,
                        AssignmentStatus::Created
                            | AssignmentStatus::Assigned
                            | AssignmentStatus::Running
                            | AssignmentStatus::RetryPending
                    )
                })
                .count();
            let running_resident_count = residents
                .iter()
                .filter(|resident| resident.status.as_str() == "running")
                .count();
            let active_trigger_count = triggers
                .iter()
                .filter(|trigger| trigger.status.as_str() == "active")
                .count();
            active_task_rows.push(RuntimeOverviewActiveTaskJson {
                task_id: task.task_spec.task_id.clone(),
                status: task.task_runtime.status.as_str().to_owned(),
                reason: reason.as_str().to_owned(),
                skills: joined_or_none(&task.task_spec.skill_refs),
                tools: joined_or_none(&task.task_spec.tool_refs),
                assignment_total: assignments.len(),
                assignment_active: active_assignment_count,
                resident_total: residents.len(),
                resident_running: running_resident_count,
                trigger_total: triggers.len(),
                trigger_active: active_trigger_count,
            });
        }

        let (_, audits) = match load_task_audits(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load audits for system overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        audit_count += audits.len();

        let (_, traces) = match load_task_traces(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load traces for system overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        trace_count += traces.len();
    }

    let completed_task_count = tasks
        .iter()
        .filter(|task| task.task_runtime.status.as_str() == "completed")
        .count();
    let implementation_bound_task_count = tasks
        .iter()
        .filter(|task| task.task_spec.implementation_ref.is_some())
        .count();
    let skill_with_recommendation_count = filtered_skills
        .iter()
        .filter(|skill| skill.recommended_implementation_id.is_some())
        .count();

    let (_, approval_requests) = match list_shell_approval_requests(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load shell approval requests for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, alert_acks) = match list_policy_alert_acks(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load policy alert acknowledgements for system overview: {error}");
            return ExitCode::from(1);
        }
    };
    let acked_ids = alert_acks
        .into_iter()
        .map(|ack| ack.alert_id)
        .collect::<std::collections::BTreeSet<_>>();
    let shell_tools = filtered_tools
        .iter()
        .filter(|tool| is_shell_tool(tool))
        .collect::<Vec<_>>();
    let shell_tool_allowed_count = shell_tools.iter().filter(|tool| tool.allow_shell).count();
    let shell_tool_blocked_count = shell_tools.len().saturating_sub(shell_tool_allowed_count);
    let pending_requests = approval_requests
        .iter()
        .filter(|request| request.status == ApprovalRequestStatus::Pending)
        .collect::<Vec<_>>();
    let overdue_requests = pending_requests
        .iter()
        .copied()
        .filter(|request| approval_request_age_ms(request).is_some_and(|age| age >= 60 * 60 * 1000))
        .collect::<Vec<_>>();
    let unacked_blocked_count = shell_tools
        .iter()
        .filter(|tool| !tool.allow_shell)
        .filter(|tool| !acked_ids.contains(&blocked_tool_alert_id(&tool.tool_id)))
        .count();
    let unacked_overdue_count = overdue_requests
        .iter()
        .filter(|request| !acked_ids.contains(&overdue_request_alert_id(&request.request_id)))
        .count();
    let visible_blocked_count = if include_acked_policy {
        shell_tools.iter().filter(|tool| !tool.allow_shell).count()
    } else {
        unacked_blocked_count
    };
    let visible_overdue_count = if include_acked_policy {
        overdue_requests.len()
    } else {
        unacked_overdue_count
    };
    let alert_summaries = match collect_system_alerts(
        root,
        &tasks,
        &filtered_tools,
        &approval_requests,
        &acked_ids,
        &skill_owners,
        &tool_owners,
        owner_filter,
        None,
        None,
        include_acked_policy,
    ) {
        Ok(mut alerts) => {
            let rerun_plan_alerts =
                match collect_rerun_plan_alerts(root, &tasks, owner_filter, None, None) {
                    Ok(alerts) => alerts,
                    Err(error) => {
                        eprintln!(
                            "failed to collect rerun plan alerts for system overview: {error}"
                        );
                        return ExitCode::from(1);
                    }
                };
            alerts.extend(rerun_plan_alerts);
            Some(SystemOverviewAlertSummariesJson {
                by_kind: build_system_alert_summaries(&alerts, "kind", sort),
                by_owner: build_system_alert_summaries(&alerts, "owner", sort),
                by_severity: build_system_alert_summaries(&alerts, "severity", sort),
            })
        }
        Err(error) => {
            eprintln!("failed to collect system overview alerts: {error}");
            return ExitCode::from(1);
        }
    };

    let gaps = if with_gaps {
        let skill_without_recommendation_count = filtered_skills
            .iter()
            .filter(|skill| skill.recommended_implementation_id.is_none())
            .count();
        let mut task_without_implementation_no_skill_count = 0usize;
        let mut task_without_implementation_missing_recommendation_count = 0usize;
        for task in &tasks {
            if task.task_spec.implementation_ref.is_some() {
                continue;
            }
            if task.task_spec.skill_refs.is_empty() {
                task_without_implementation_no_skill_count += 1;
            } else {
                let has_recommended_skill = task.task_spec.skill_refs.iter().any(|skill_ref| {
                    filtered_skills.iter().any(|skill| {
                        skill.skill_id == *skill_ref
                            && skill.recommended_implementation_id.is_some()
                    })
                });
                if !has_recommended_skill {
                    task_without_implementation_missing_recommendation_count += 1;
                }
            }
        }
        let mut active_reason_rows = active_reason_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        if sort == "target" {
            overview_target_sort(&mut active_reason_rows);
        } else {
            overview_count_sort(&mut active_reason_rows);
        }
        Some(SystemOverviewGapsJson {
            skill_without_recommendation_count,
            task_without_implementation_no_skill_count,
            task_without_implementation_missing_recommendation_count,
            blocked_shell_tool_count: visible_blocked_count,
            active_task_reason_count: active_reason_rows.len(),
            active_task_reasons: active_reason_rows,
        })
    } else {
        None
    };

    let details = if with_details {
        let mut recommended_skills = filtered_skills
            .iter()
            .filter_map(|skill| {
                skill
                    .recommended_implementation_id
                    .as_ref()
                    .map(|implementation_id| RegistryOverviewRecommendedSkillJson {
                        skill_id: skill.skill_id.clone(),
                        implementation_id: implementation_id.clone(),
                        decision: skill
                            .governance_decision
                            .map(|decision| decision.as_str().to_owned())
                            .unwrap_or_else(|| "<none>".to_owned()),
                    })
            })
            .collect::<Vec<_>>();
        let mut implementation_usage_rows = implementation_usage
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        recommended_skills.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
        if sort == "target" {
            implementation_usage_rows.sort_by(|a, b| a.id.cmp(&b.id));
        } else {
            overview_count_sort(&mut implementation_usage_rows);
        }
        sort_active_task_rows(&mut active_task_rows, sort);
        Some(SystemOverviewDetailsJson {
            recommended_skill_count: recommended_skills.len(),
            recommended_skills,
            implementation_usage_count: implementation_usage_rows.len(),
            implementation_usage: implementation_usage_rows,
            active_task_count: active_task_rows.len(),
            active_tasks: active_task_rows.clone(),
        })
    } else {
        None
    };

    let policy = if with_policy {
        let mut inbox_by_owner = std::collections::BTreeMap::<String, usize>::new();
        for tool in shell_tools
            .iter()
            .filter(|tool| !tool.allow_shell)
            .filter(|tool| {
                include_acked_policy || !acked_ids.contains(&blocked_tool_alert_id(&tool.tool_id))
            })
        {
            *inbox_by_owner.entry(tool.owner.clone()).or_insert(0) += 1;
        }
        for request in overdue_requests.iter().filter(|request| {
            include_acked_policy
                || !acked_ids.contains(&overdue_request_alert_id(&request.request_id))
        }) {
            *inbox_by_owner.entry(request.owner.clone()).or_insert(0) += 1;
        }
        let mut inbox_owners = inbox_by_owner
            .into_iter()
            .map(|(owner, count)| AlertOwnerSummary { owner, count })
            .collect::<Vec<_>>();
        let mut shell_tool_rows = shell_tools
            .iter()
            .map(|tool| RuntimeOverviewPolicyShellToolJson {
                tool_id: tool.tool_id.clone(),
                owner: tool.owner.clone(),
                policy: tool_policy_summary(tool),
            })
            .collect::<Vec<_>>();
        let mut recent_changes = match recent_tool_policy_audits(root, 5) {
            Ok(audits) => audits
                .into_iter()
                .map(|audit| RuntimeOverviewPolicyRecentChangeJson {
                    timestamp: audit.timestamp,
                    action: audit.action,
                    tool_id: audit.target_id,
                    result: audit.result,
                    detail: audit.payload,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                eprintln!("failed to load recent tool policy audits for system overview: {error}");
                return ExitCode::from(1);
            }
        };
        sort_alert_owner_summaries(&mut inbox_owners, sort);
        sort_policy_shell_tool_rows(&mut shell_tool_rows, sort);
        sort_policy_recent_changes(&mut recent_changes, sort);
        Some(SystemOverviewPolicyJson {
            shell_request_count: approval_requests.len(),
            shell_request_pending_count: pending_requests.len(),
            shell_request_overdue_count: overdue_requests.len(),
            unacked_alert_count: unacked_blocked_count + unacked_overdue_count,
            acked_alert_count: acked_ids.len(),
            inbox_owner_count: inbox_owners.len(),
            shell_tools: shell_tool_rows,
            inbox_owners,
            recent_changes,
        })
    } else {
        None
    };

    let runtime_health = if with_runtime_health {
        let mut active_reason_rows = active_reason_counts
            .iter()
            .map(|(id, count)| RuntimeOverviewUsageJson {
                id: id.clone(),
                count: *count,
            })
            .collect::<Vec<_>>();
        if sort == "target" {
            overview_target_sort(&mut trigger_waiting_consumption_rows);
        } else {
            overview_count_sort(&mut trigger_waiting_consumption_rows);
        }
        if sort == "target" {
            overview_target_sort(&mut active_reason_rows);
        } else {
            overview_count_sort(&mut active_reason_rows);
        }
        let health_severity = if visible_blocked_count + visible_overdue_count > 0 {
            "warning"
        } else if !pending_requests.is_empty()
            || !active_task_rows.is_empty()
            || !rerun_plan_task_rows.is_empty()
        {
            "attention"
        } else {
            "healthy"
        };
        if sort == "target" {
            overview_target_sort(&mut rerun_plan_task_rows);
        } else {
            overview_count_sort(&mut rerun_plan_task_rows);
        }
        Some(SystemOverviewRuntimeHealthJson {
            active_task_count: active_task_rows.len(),
            active_task_reason_count: active_reason_rows.len(),
            active_task_reasons: active_reason_rows,
            trigger_waiting_consumption_count: trigger_waiting_consumption_rows.len(),
            trigger_waiting_consumption: trigger_waiting_consumption_rows,
            rerun_plan_task_count,
            rerun_plan_tasks: rerun_plan_task_rows,
            shell_request_pending_count: pending_requests.len(),
            shell_request_overdue_count: overdue_requests.len(),
            blocked_shell_tool_count: visible_blocked_count,
            unacked_alert_count: unacked_blocked_count + unacked_overdue_count,
            health_severity: health_severity.to_owned(),
        })
    } else {
        None
    };

    let gaps = gaps.map(|mut gaps| {
        if let Some(limit) = limit {
            if gaps.active_task_reasons.len() > limit {
                gaps.active_task_reasons.truncate(limit);
            }
        }
        gaps
    });
    let details = details.map(|mut details| {
        if let Some(limit) = limit {
            if details.recommended_skills.len() > limit {
                details.recommended_skills.truncate(limit);
            }
            if details.implementation_usage.len() > limit {
                details.implementation_usage.truncate(limit);
            }
            if details.active_tasks.len() > limit {
                details.active_tasks.truncate(limit);
            }
        }
        details
    });
    let policy = policy.map(|mut policy| {
        if let Some(limit) = limit {
            if policy.shell_tools.len() > limit {
                policy.shell_tools.truncate(limit);
            }
            if policy.inbox_owners.len() > limit {
                policy.inbox_owners.truncate(limit);
            }
            if policy.recent_changes.len() > limit {
                policy.recent_changes.truncate(limit);
            }
        }
        policy
    });
    let alert_summaries = alert_summaries.map(|mut summaries| {
        if let Some(limit) = limit {
            if summaries.by_kind.len() > limit {
                summaries.by_kind.truncate(limit);
            }
            if summaries.by_owner.len() > limit {
                summaries.by_owner.truncate(limit);
            }
            if summaries.by_severity.len() > limit {
                summaries.by_severity.truncate(limit);
            }
        }
        summaries
    });
    let runtime_health = runtime_health.map(|mut runtime_health| {
        if let Some(limit) = limit {
            if runtime_health.active_task_reasons.len() > limit {
                runtime_health.active_task_reasons.truncate(limit);
            }
            if runtime_health.trigger_waiting_consumption.len() > limit {
                runtime_health.trigger_waiting_consumption.truncate(limit);
            }
            if runtime_health.rerun_plan_tasks.len() > limit {
                runtime_health.rerun_plan_tasks.truncate(limit);
            }
        }
        runtime_health
    });

    let gaps = if summary_only { None } else { gaps };
    let details = if summary_only { None } else { details };
    let policy = if summary_only { None } else { policy };
    let alert_summaries = if summary_only { None } else { alert_summaries };
    let runtime_health = if summary_only { None } else { runtime_health };

    if as_json {
        let output = SystemOverviewJson {
            root: root.to_owned(),
            owner: owner_filter.map(str::to_owned),
            sort: sort.to_owned(),
            limit,
            summary_only,
            include_acked_policy,
            exclude_legacy,
            registry_skill_count: filtered_skills.len(),
            registry_skill_with_recommendation_count: skill_with_recommendation_count,
            registry_tool_count: filtered_tools.len(),
            registry_fitness_count: filtered_fitness_runs.len(),
            runtime_task_count: tasks.len(),
            runtime_completed_task_count: completed_task_count,
            runtime_implementation_bound_task_count: implementation_bound_task_count,
            runtime_active_task_count: active_task_rows.len(),
            runtime_assignment_count: assignment_count,
            runtime_resident_count: resident_count,
            runtime_trigger_count: trigger_count,
            runtime_audit_count: audit_count,
            runtime_trace_count: trace_count,
            rerun_plan_count,
            rerun_plan_task_count,
            policy_alert_count: visible_blocked_count + visible_overdue_count,
            policy_inbox_count: unacked_blocked_count + unacked_overdue_count,
            policy_shell_tool_count: shell_tools.len(),
            policy_shell_tool_allowed_count: shell_tool_allowed_count,
            policy_shell_tool_blocked_count: shell_tool_blocked_count,
            alert_summaries,
            runtime_health,
            gaps,
            details,
            policy,
        };
        match serde_json::to_string_pretty(&output) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to render system overview json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("system overview loaded");
    println!("  root: {}", root);
    println!("  owner: {}", owner_filter.unwrap_or("<all>"));
    println!("  sort: {}", sort);
    println!(
        "  limit: {}",
        limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_owned())
    );
    println!(
        "  summary_only: {}",
        if summary_only { "true" } else { "false" }
    );
    println!(
        "  include_acked_policy: {}",
        if include_acked_policy {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  exclude_legacy: {}",
        if exclude_legacy { "true" } else { "false" }
    );
    println!("  registry_skill_count: {}", filtered_skills.len());
    println!(
        "  registry_skill_with_recommendation_count: {}",
        skill_with_recommendation_count
    );
    println!("  registry_tool_count: {}", filtered_tools.len());
    println!("  registry_fitness_count: {}", filtered_fitness_runs.len());
    println!("  runtime_task_count: {}", tasks.len());
    println!("  runtime_completed_task_count: {}", completed_task_count);
    println!(
        "  runtime_implementation_bound_task_count: {}",
        implementation_bound_task_count
    );
    println!("  runtime_active_task_count: {}", active_task_rows.len());
    println!("  runtime_assignment_count: {}", assignment_count);
    println!("  runtime_resident_count: {}", resident_count);
    println!("  runtime_trigger_count: {}", trigger_count);
    println!("  runtime_audit_count: {}", audit_count);
    println!("  runtime_trace_count: {}", trace_count);
    println!("  rerun_plan_count: {}", rerun_plan_count);
    println!("  rerun_plan_task_count: {}", rerun_plan_task_count);
    println!(
        "  policy_alert_count: {}",
        visible_blocked_count + visible_overdue_count
    );
    println!(
        "  policy_inbox_count: {}",
        unacked_blocked_count + unacked_overdue_count
    );
    println!("  policy_shell_tool_count: {}", shell_tools.len());
    println!(
        "  policy_shell_tool_allowed_count: {}",
        shell_tool_allowed_count
    );
    println!(
        "  policy_shell_tool_blocked_count: {}",
        shell_tool_blocked_count
    );
    if let Some(alert_summaries) = &alert_summaries {
        for row in &alert_summaries.by_kind {
            println!(
                "  alert_summary_kind: key={} count={} highest_severity={}",
                row.key, row.count, row.highest_severity
            );
        }
        for row in &alert_summaries.by_owner {
            println!(
                "  alert_summary_owner: key={} count={} highest_severity={}",
                row.key, row.count, row.highest_severity
            );
        }
        for row in &alert_summaries.by_severity {
            println!(
                "  alert_summary_severity: key={} count={} highest_severity={}",
                row.key, row.count, row.highest_severity
            );
        }
    }

    if let Some(runtime_health) = &runtime_health {
        println!(
            "  runtime_health_severity: {}",
            runtime_health.health_severity
        );
        println!(
            "  runtime_health_active_task_count: {}",
            runtime_health.active_task_count
        );
        println!(
            "  runtime_health_active_task_reason_count: {}",
            runtime_health.active_task_reason_count
        );
        for row in &runtime_health.active_task_reasons {
            println!(
                "  runtime_health_active_task_reason: reason={} count={}",
                row.id, row.count
            );
        }
        println!(
            "  runtime_health_trigger_waiting_consumption_count: {}",
            runtime_health.trigger_waiting_consumption_count
        );
        for row in &runtime_health.trigger_waiting_consumption {
            println!(
                "  runtime_health_trigger_waiting_consumption: task={} ready_trigger_count={}",
                row.id, row.count
            );
        }
        println!(
            "  runtime_health_rerun_plan_task_count: {}",
            runtime_health.rerun_plan_task_count
        );
        for row in &runtime_health.rerun_plan_tasks {
            println!(
                "  runtime_health_rerun_plan_task: task={} plan_count={}",
                row.id, row.count
            );
        }
        println!(
            "  runtime_health_shell_request_pending_count: {}",
            runtime_health.shell_request_pending_count
        );
        println!(
            "  runtime_health_shell_request_overdue_count: {}",
            runtime_health.shell_request_overdue_count
        );
        println!(
            "  runtime_health_blocked_shell_tool_count: {}",
            runtime_health.blocked_shell_tool_count
        );
        println!(
            "  runtime_health_unacked_alert_count: {}",
            runtime_health.unacked_alert_count
        );
    }

    if let Some(gaps) = &gaps {
        println!(
            "  gap_skill_without_recommendation_count: {}",
            gaps.skill_without_recommendation_count
        );
        println!(
            "  gap_task_without_implementation_no_skill_count: {}",
            gaps.task_without_implementation_no_skill_count
        );
        println!(
            "  gap_task_without_implementation_missing_recommendation_count: {}",
            gaps.task_without_implementation_missing_recommendation_count
        );
        println!(
            "  gap_blocked_shell_tool_count: {}",
            gaps.blocked_shell_tool_count
        );
        println!(
            "  gap_active_task_reason_count: {}",
            gaps.active_task_reason_count
        );
        for row in &gaps.active_task_reasons {
            println!(
                "  gap_active_task_reason: reason={} count={}",
                row.id, row.count
            );
        }
    }

    if let Some(details) = &details {
        println!(
            "  recommended_skill_count: {}",
            details.recommended_skill_count
        );
        for row in &details.recommended_skills {
            println!(
                "  recommended_skill: skill={} implementation={} decision={}",
                row.skill_id, row.implementation_id, row.decision
            );
        }
        println!(
            "  implementation_usage_count: {}",
            details.implementation_usage_count
        );
        for row in &details.implementation_usage {
            println!(
                "  implementation_usage: implementation={} task_count={}",
                row.id, row.count
            );
        }
        println!("  active_task_count: {}", details.active_task_count);
        for row in &details.active_tasks {
            println!(
                "  active_task: task={} status={} reason={} assignments={}/{} residents={}/{} triggers={}/{} skills={} tools={}",
                row.task_id,
                row.status,
                row.reason,
                row.assignment_active,
                row.assignment_total,
                row.resident_running,
                row.resident_total,
                row.trigger_active,
                row.trigger_total,
                row.skills,
                row.tools
            );
        }
    }

    if let Some(policy) = &policy {
        println!(
            "  policy_shell_request_count: {}",
            policy.shell_request_count
        );
        println!(
            "  policy_shell_request_pending_count: {}",
            policy.shell_request_pending_count
        );
        println!(
            "  policy_shell_request_overdue_count: {}",
            policy.shell_request_overdue_count
        );
        println!(
            "  policy_unacked_alert_count: {}",
            policy.unacked_alert_count
        );
        println!("  policy_acked_alert_count: {}", policy.acked_alert_count);
        println!("  policy_inbox_owner_count: {}", policy.inbox_owner_count);
        for row in &policy.inbox_owners {
            println!(
                "  policy_inbox_owner: owner={} count={}",
                row.owner, row.count
            );
        }
        for row in &policy.shell_tools {
            println!(
                "  policy_shell_tool: tool={} owner={} policy={}",
                row.tool_id, row.owner, row.policy
            );
        }
        println!(
            "  policy_recent_change_count: {}",
            policy.recent_changes.len()
        );
        for row in &policy.recent_changes {
            println!(
                "  policy_recent_change: ts={} action={} tool={} result={} detail={}",
                row.timestamp, row.action, row.tool_id, row.result, row.detail
            );
        }
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_system_alerts(args: &[String]) -> ExitCode {
    let kind_filter = option_value(args, "--kind");
    let owner_filter = option_value(args, "--owner");
    let severity_filter = option_value(args, "--severity");
    let summary_by = option_value(args, "--summary-by");
    let sort = option_value(args, "--sort").unwrap_or("severity");
    let limit = option_value(args, "--limit").and_then(|value| value.parse::<usize>().ok());
    let summary_only = has_flag(args, "--summary-only");
    let include_acked_policy = has_flag(args, "--include-acked-policy");
    let exclude_legacy = has_flag(args, "--exclude-legacy");
    let as_json = has_flag(args, "--json");
    let root = option_value(args, "--root").unwrap_or(".");

    let (_, all_tasks) = match crate::storage::list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list tasks for system alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let tasks = all_tasks
        .into_iter()
        .filter(|task| !exclude_legacy || !is_legacy_demo_task(task))
        .collect::<Vec<_>>();
    let (_, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skills for system alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tools for system alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let skill_owners = skills
        .into_iter()
        .map(|skill| (skill.skill_id, skill.owner))
        .collect::<std::collections::BTreeMap<_, _>>();
    let tool_owners = tools
        .iter()
        .map(|tool| (tool.tool_id.clone(), tool.owner.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let (_, approval_requests) = match list_shell_approval_requests(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load shell approval requests for system alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, alert_acks) = match list_policy_alert_acks(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load policy alert acknowledgements for system alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let acked_ids = alert_acks
        .into_iter()
        .map(|ack| ack.alert_id)
        .collect::<std::collections::BTreeSet<_>>();
    let rerun_plan_count = match list_rerun_plans(root) {
        Ok(plans) => plans.len(),
        Err(error) => {
            eprintln!("failed to load rerun plans for system alerts: {error}");
            return ExitCode::from(1);
        }
    };

    let mut alerts = match collect_system_alerts(
        root,
        &tasks,
        &tools,
        &approval_requests,
        &acked_ids,
        &skill_owners,
        &tool_owners,
        owner_filter,
        kind_filter,
        severity_filter,
        include_acked_policy,
    ) {
        Ok(alerts) => alerts,
        Err(error) => {
            eprintln!("failed to collect system alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let rerun_plan_alerts =
        match collect_rerun_plan_alerts(root, &tasks, owner_filter, kind_filter, severity_filter) {
            Ok(alerts) => alerts,
            Err(error) => {
                eprintln!("failed to collect rerun plan alerts: {error}");
                return ExitCode::from(1);
            }
        };
    alerts.extend(rerun_plan_alerts);

    let highest_severity = if alerts.iter().any(|alert| alert.severity == "warning") {
        "warning"
    } else if alerts.iter().any(|alert| alert.severity == "attention") {
        "attention"
    } else {
        "healthy"
    };
    let mut summaries = if let Some(summary_by) = summary_by {
        build_system_alert_summaries(&alerts, summary_by, sort)
    } else {
        Vec::new()
    };
    match sort {
        "target" => {
            alerts.sort_by(|a, b| {
                a.target
                    .cmp(&b.target)
                    .then_with(|| a.kind.cmp(&b.kind))
                    .then_with(|| a.severity.cmp(&b.severity))
            });
        }
        _ => {
            alerts.sort_by(|a, b| {
                alert_severity_rank(&a.severity)
                    .cmp(&alert_severity_rank(&b.severity))
                    .then_with(|| a.kind.cmp(&b.kind))
                    .then_with(|| a.target.cmp(&b.target))
            });
        }
    }
    let total_alert_count = alerts.len();
    if let Some(limit) = limit {
        if summaries.len() > limit {
            summaries.truncate(limit);
        }
    }
    if let Some(limit) = limit {
        if alerts.len() > limit {
            alerts.truncate(limit);
        }
    }
    if summary_only {
        alerts.clear();
    }

    if as_json {
        let payload = SystemAlertsJson {
            root: root.to_owned(),
            kind: kind_filter.map(str::to_owned),
            owner: owner_filter.map(str::to_owned),
            severity: severity_filter.map(str::to_owned),
            summary_by: summary_by.map(str::to_owned),
            sort: sort.to_owned(),
            limit,
            summary_only,
            include_acked_policy,
            exclude_legacy,
            rerun_plan_count,
            alert_count: total_alert_count,
            highest_severity: highest_severity.to_owned(),
            summaries,
            alerts,
        };
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to render system alerts json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("system alerts loaded");
    println!("  root: {}", root);
    println!("  kind: {}", kind_filter.unwrap_or("<all>"));
    println!("  owner: {}", owner_filter.unwrap_or("<all>"));
    println!("  severity: {}", severity_filter.unwrap_or("<all>"));
    println!("  summary_by: {}", summary_by.unwrap_or("<none>"));
    println!("  sort: {}", sort);
    println!(
        "  summary_only: {}",
        if summary_only { "true" } else { "false" }
    );
    println!(
        "  include_acked_policy: {}",
        if include_acked_policy {
            "true"
        } else {
            "false"
        }
    );
    println!("  rerun_plan_count: {}", rerun_plan_count);
    println!(
        "  limit: {}",
        limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_owned())
    );
    println!(
        "  exclude_legacy: {}",
        if exclude_legacy { "true" } else { "false" }
    );
    println!("  alert_count: {}", total_alert_count);
    println!("  highest_severity: {}", highest_severity);
    for summary in summaries {
        println!(
            "  summary: key={} count={} highest_severity={}",
            summary.key, summary.count, summary.highest_severity
        );
    }
    for alert in alerts {
        println!(
            "  alert: kind={} severity={} owner={} target={} detail={}",
            alert.kind,
            alert.severity,
            alert.owner.as_deref().unwrap_or("<none>"),
            alert.target,
            alert.detail
        );
    }

    ExitCode::SUCCESS
}
