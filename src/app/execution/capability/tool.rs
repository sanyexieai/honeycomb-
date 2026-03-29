use super::super::common_support::*;
use super::super::control::*;
use super::super::*;
use super::execution_record;

pub(crate) fn handle_tool_inspect(args: &[String]) -> ExitCode {
    let tool_id = option_value(args, "--tool-id").unwrap_or("tool-demo");
    let with_runtime = has_flag(args, "--with-runtime");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, tool) = match load_tool(root, tool_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect tool: {error}");
            return ExitCode::from(1);
        }
    };

    println!("tool inspect loaded");
    println!("  tool_id: {}", tool.tool_id);
    println!("  display_name: {}", tool.display_name);
    println!("  description: {}", tool.description);
    println!("  entrypoint: {}", tool.entrypoint);
    println!("  owner: {}", tool.owner);
    println!("  version: {}", tool.version);
    println!(
        "  allow_shell: {}",
        if tool.allow_shell { "true" } else { "false" }
    );
    println!(
        "  shell_approval_pending: {}",
        if tool.shell_approval_pending {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  shell_approval_request_id: {}",
        tool.shell_approval_request_id
            .as_deref()
            .unwrap_or("<none>")
    );
    println!("  policy: {}", tool_policy_summary(&tool));
    println!("  read_from: {}", path.display());

    if with_runtime {
        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load tool runtime tasks: {error}");
                return ExitCode::from(1);
            }
        };

        let matched_tasks = tasks
            .into_iter()
            .filter(|record| {
                record
                    .task_spec
                    .tool_refs
                    .iter()
                    .any(|task_tool| task_tool == &tool.tool_id)
            })
            .collect::<Vec<_>>();

        println!("  runtime_task_count: {}", matched_tasks.len());
        for task in matched_tasks {
            let (_, assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "failed to load assignments for tool runtime task {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            let matched_assignments = assignments
                .into_iter()
                .filter(|assignment| {
                    assignment
                        .tool_refs
                        .iter()
                        .any(|value| value == &tool.tool_id)
                })
                .collect::<Vec<_>>();
            let task_audit_count = match load_task_audits(root, &task.task_spec.task_id) {
                Ok((_, audits)) => audits.len(),
                Err(error) => {
                    eprintln!(
                        "failed to load audits for tool runtime task {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            let task_trace_count = match load_task_traces(root, &task.task_spec.task_id) {
                Ok((_, traces)) => traces.len(),
                Err(error) => {
                    eprintln!(
                        "failed to load traces for tool runtime task {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };

            println!(
                "  - task={} status={} implementation={} assignments={} audits={} traces={} goal={}",
                task.task_spec.task_id,
                task.task_runtime.status.as_str(),
                task.task_spec
                    .implementation_ref
                    .as_deref()
                    .unwrap_or("<none>"),
                matched_assignments.len(),
                task_audit_count,
                task_trace_count,
                task.task_spec.goal
            );
            for assignment in matched_assignments {
                println!(
                    "    assignment={} worker={} status={} implementation={} skills={}",
                    assignment.assignment_id,
                    assignment.worker_node_id,
                    assignment.status.as_str(),
                    assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                    joined_or_none(&assignment.skill_refs)
                );
            }
        }
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_execute(args: &[String]) -> ExitCode {
    let tool_id = option_value(args, "--tool-id").unwrap_or("tool-demo");
    let task_id = option_value(args, "--task-id").map(str::to_owned);
    let assignment_id = option_value(args, "--assignment-id").map(str::to_owned);
    let input = option_value(args, "--input").unwrap_or("tool-input");
    let root = option_value(args, "--root").unwrap_or(".");

    let (_, tool) = match load_tool(root, tool_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tool for execution: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = ensure_shell_execution_allowed(&tool) {
        eprintln!("failed to authorize tool execution: {error}");
        return ExitCode::from(1);
    }

    let execution_id = format!(
        "exec-tool-{}-{}",
        tool.tool_id,
        crate::core::current_timestamp().replace(':', "_")
    );
    let outcome = match execute_tool_entrypoint(&tool.entrypoint, input) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to execute tool entrypoint: {error}");
            return ExitCode::from(1);
        }
    };
    let implementation_snapshot = match execution_record::resolve_execution_implementation_snapshot(
        root,
        task_id.as_deref(),
        assignment_id.as_deref(),
        None,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("failed to resolve execution implementation snapshot: {error}");
            return ExitCode::from(1);
        }
    };
    let record = ExecutionRecord::new(
        execution_id,
        ExecutionKind::Tool,
        tool.tool_id.clone(),
        task_id.clone(),
        assignment_id.clone(),
        None,
        implementation_snapshot,
        Vec::new(),
        vec![tool.tool_id.clone()],
        input.to_owned(),
        outcome.plan_steps,
        outcome.output,
        outcome.runner,
        outcome.exit_code,
        outcome.status,
    );
    let path = match persist_execution_record(root, &record) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist tool execution record: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = execution_record::persist_execution_task_records(root, &record) {
        eprintln!("failed to append tool execution task records: {error}");
        return ExitCode::from(1);
    }

    println!("tool execute recorded");
    println!("  execution_id: {}", record.execution_id);
    println!("  tool_id: {}", record.target_id);
    println!("  entrypoint: {}", tool.entrypoint);
    println!("  runner: {}", record.runner);
    println!(
        "  task_id: {}",
        record.task_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "  assignment_id: {}",
        record.assignment_id.as_deref().unwrap_or("<none>")
    );
    if let Some(snapshot) = &record.implementation_snapshot {
        println!("  implementation_skill: {}", snapshot.skill_id);
        println!("  implementation_executor: {}", snapshot.executor);
    }
    println!("  status: {}", record.status.as_str());
    println!(
        "  exit_code: {}",
        record
            .exit_code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_owned())
    );
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_list(args: &[String]) -> ExitCode {
    let shell_only = has_flag(args, "--shell-only");
    let blocked_only = has_flag(args, "--blocked-only");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list tools: {error}");
            return ExitCode::from(1);
        }
    };

    let filtered = tools
        .into_iter()
        .filter(|tool| !shell_only || is_shell_tool(tool))
        .filter(|tool| !blocked_only || (is_shell_tool(tool) && !tool.allow_shell))
        .collect::<Vec<_>>();

    println!("tool list loaded");
    println!("  read_from: {}", dir.display());
    println!(
        "  shell_only: {}",
        if shell_only { "true" } else { "false" }
    );
    println!(
        "  blocked_only: {}",
        if blocked_only { "true" } else { "false" }
    );
    println!("  tool_count: {}", filtered.len());
    for tool in filtered {
        println!(
            "  - {} version={} entrypoint={} allow_shell={} pending={} request_id={} owner={} policy={}",
            tool.tool_id,
            tool.version,
            tool.entrypoint,
            if tool.allow_shell { "true" } else { "false" },
            if tool.shell_approval_pending {
                "true"
            } else {
                "false"
            },
            tool.shell_approval_request_id
                .as_deref()
                .unwrap_or("<none>"),
            tool.owner,
            tool_policy_summary(&tool)
        );
    }
    ExitCode::SUCCESS
}
