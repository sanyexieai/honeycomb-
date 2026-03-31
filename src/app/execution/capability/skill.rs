use super::super::common_support::*;
use super::super::control::*;
use super::super::task::validate_registry_refs;
use super::super::*;
use super::execution_record;
use crate::executor::execute_skill_implementation;
use serde_json::Value;

pub(crate) fn handle_skill_inspect(args: &[String]) -> ExitCode {
    let skill_id = option_value(args, "--skill-id").unwrap_or("skill-demo");
    let with_lineage = has_flag(args, "--with-lineage");
    let with_runtime = has_flag(args, "--with-runtime");
    let recommended_only = has_flag(args, "--recommended-only");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, skill) = match load_skill(root, skill_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect skill: {error}");
            return ExitCode::from(1);
        }
    };
    let implementation_details = match load_skill_implementations(root, &skill) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to validate skill implementations: {error}");
            return ExitCode::from(1);
        }
    };
    let ((_, primary_implementation), recommended_implementation) = implementation_details;
    let recommended_implementation = recommended_implementation
        .as_ref()
        .map(|(_, record)| record);

    println!("skill inspect loaded");
    println!("  skill_id: {}", skill.skill_id);
    println!("  display_name: {}", skill.display_name);
    println!("  description: {}", skill.description);
    println!("  implementation_ref: {}", skill.implementation_ref);
    println!("  owner: {}", skill.owner);
    println!("  version: {}", skill.version);
    println!(
        "  default_tool_refs: {}",
        joined_or_none(&skill.default_tool_refs)
    );
    println!(
        "  goal_template: {}",
        skill.goal_template.as_deref().unwrap_or("<none>")
    );
    println!(
        "  recommended_implementation_id: {}",
        skill
            .recommended_implementation_id
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_executor: {}",
        primary_implementation.executor
    );
    println!(
        "  implementation_entry: {}:{}",
        primary_implementation.entry.kind, primary_implementation.entry.path
    );
    println!(
        "  recommended_executor: {}",
        recommended_implementation
            .map(|record| record.executor.as_str())
            .unwrap_or("<none>")
    );
    println!(
        "  governance_decision: {}",
        skill
            .governance_decision
            .map(|decision| decision.as_str())
            .unwrap_or("<none>")
    );
    println!(
        "  last_synced_at: {}",
        skill.last_synced_at.as_deref().unwrap_or("<none>")
    );
    println!("  read_from: {}", path.display());

    if with_lineage {
        let (_, records) = match list_fitness_runs(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load skill lineage: {error}");
                return ExitCode::from(1);
            }
        };

        let mut lineage = records
            .into_iter()
            .filter(|record| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|record_skill| record_skill == &skill.skill_id)
            })
            .collect::<Vec<_>>();
        lineage.sort_by(|a, b| {
            let a_match = skill
                .recommended_implementation_id
                .as_deref()
                .is_some_and(|value| value == a.fitness_report.implementation_id());
            let b_match = skill
                .recommended_implementation_id
                .as_deref()
                .is_some_and(|value| value == b.fitness_report.implementation_id());
            b_match
                .cmp(&a_match)
                .then_with(|| b.fitness_report.score.cmp(&a.fitness_report.score))
                .then_with(|| {
                    a.fitness_report
                        .implementation_id()
                        .cmp(b.fitness_report.implementation_id())
                })
        });

        println!("  lineage_count: {}", lineage.len());
        for record in lineage {
            println!(
                "  - {} score={} decision={} tools={}",
                record.fitness_report.implementation_id(),
                record.fitness_report.score,
                record.evolution_plan.decision.as_str(),
                joined_or_none(&record.fitness_report.tool_refs)
            );
        }
    }

    if with_runtime {
        let target_implementation = skill
            .recommended_implementation_id
            .as_deref()
            .or(Some(skill.implementation_ref.as_str()));
        let implementation_ref = target_implementation.unwrap_or("<none>");
        println!("  runtime_implementation_ref: {implementation_ref}");
        println!(
            "  runtime_scope: {}",
            if recommended_only {
                "recommended_only"
            } else {
                "all_skill_tasks"
            }
        );

        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load skill runtime tasks: {error}");
                return ExitCode::from(1);
            }
        };

        let matched_tasks = tasks
            .into_iter()
            .filter(|record| {
                let skill_match = record
                    .task_spec
                    .skill_refs
                    .iter()
                    .any(|record_skill| record_skill == &skill.skill_id);
                let implementation_match = if recommended_only {
                    record.task_spec.implementation_ref.as_deref() == Some(implementation_ref)
                } else {
                    true
                };
                skill_match && implementation_match
            })
            .collect::<Vec<_>>();

        println!("  runtime_task_count: {}", matched_tasks.len());
        for task in matched_tasks {
            let (_, assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "failed to load assignments for runtime task {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            let matched_assignments = assignments
                .into_iter()
                .filter(|assignment| {
                    let skill_match = assignment
                        .skill_refs
                        .iter()
                        .any(|value| value == &skill.skill_id);
                    let implementation_match = if recommended_only {
                        assignment.implementation_ref.as_deref() == Some(implementation_ref)
                    } else {
                        true
                    };
                    skill_match && implementation_match
                })
                .collect::<Vec<_>>();
            let task_audit_count = match load_task_audits(root, &task.task_spec.task_id) {
                Ok((_, audits)) => audits.len(),
                Err(error) => {
                    eprintln!(
                        "failed to load audits for runtime task {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            let task_trace_count = match load_task_traces(root, &task.task_spec.task_id) {
                Ok((_, traces)) => traces.len(),
                Err(error) => {
                    eprintln!(
                        "failed to load traces for runtime task {}: {error}",
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
                    "    assignment={} worker={} status={} implementation={} tools={}",
                    assignment.assignment_id,
                    assignment.worker_node_id,
                    assignment.status.as_str(),
                    assignment.implementation_ref.as_deref().unwrap_or("<none>"),
                    joined_or_none(&assignment.tool_refs)
                );
            }
        }
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_skill_list(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list skills: {error}");
            return ExitCode::from(1);
        }
    };

    println!("skill list loaded");
    println!("  read_from: {}", dir.display());
    println!("  skill_count: {}", skills.len());
    for skill in skills {
        let implementation_details = match load_skill_implementations(root, &skill) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to validate implementations for skill {}: {error}",
                    skill.skill_id
                );
                return ExitCode::from(1);
            }
        };
        let ((_, primary_implementation), recommended_implementation) = implementation_details;
        let recommended_implementation = recommended_implementation
            .as_ref()
            .map(|(_, record)| record);
        println!(
            "  - {} version={} impl={} impl_executor={} default_tools={} goal_template={} recommended={} recommended_executor={} decision={} owner={}",
            skill.skill_id,
            skill.version,
            skill.implementation_ref,
            primary_implementation.executor,
            joined_or_none(&skill.default_tool_refs),
            skill.goal_template.as_deref().unwrap_or("<none>"),
            skill
                .recommended_implementation_id
                .as_deref()
                .unwrap_or("<none>"),
            recommended_implementation
                .map(|record| record.executor.as_str())
                .unwrap_or("<none>"),
            skill
                .governance_decision
                .map(|decision| decision.as_str())
                .unwrap_or("<none>"),
            skill.owner
        );
    }
    ExitCode::SUCCESS
}

pub(crate) fn handle_skill_execute(args: &[String]) -> ExitCode {
    let skill_id = option_value(args, "--skill-id").unwrap_or("skill-demo");
    let task_id = option_value(args, "--task-id").map(str::to_owned);
    let assignment_id = option_value(args, "--assignment-id").map(str::to_owned);
    let input = option_value(args, "--input").unwrap_or("skill-input");
    let use_recommended_impl = has_flag(args, "--use-recommended-impl");
    let run_tools = has_flag(args, "--run-tools");
    let root = option_value(args, "--root").unwrap_or(".");

    let (_, skill) = match load_skill(root, skill_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skill for execution: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = validate_skill_implementation_refs(root, &skill) {
        eprintln!("failed to validate skill implementations for execution: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) =
        validate_registry_refs(root, &[skill.skill_id.clone()], &skill.default_tool_refs)
    {
        eprintln!("failed to validate skill execution refs: {error}");
        return ExitCode::from(1);
    }

    let implementation_ref = if use_recommended_impl {
        skill
            .recommended_implementation_id
            .clone()
            .or_else(|| Some(skill.implementation_ref.clone()))
    } else {
        Some(skill.implementation_ref.clone())
    };
    let execution_id = format!(
        "exec-skill-{}-{}",
        skill.skill_id,
        crate::core::current_timestamp().replace(':', "_")
    );
    let implementation_record = implementation_ref
        .as_deref()
        .map(|implementation_id| load_implementation(root, implementation_id))
        .transpose();
    let implementation_record = match implementation_record {
        Ok(value) => value.map(|(_, record)| record),
        Err(error) => {
            eprintln!("failed to load implementation for skill execution: {error}");
            return ExitCode::from(1);
        }
    };
    let implementation_snapshot = match execution_record::resolve_execution_implementation_snapshot(
        root,
        task_id.as_deref(),
        assignment_id.as_deref(),
        implementation_ref.as_deref(),
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("failed to resolve execution implementation snapshot: {error}");
            return ExitCode::from(1);
        }
    };
    let mut plan_steps = vec![format!(
        "resolve_skill skill={} implementation={}",
        skill.skill_id,
        implementation_ref.as_deref().unwrap_or("<none>")
    )];
    for tool_id in &skill.default_tool_refs {
        let (_, tool) = match load_tool(root, tool_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load default tool for skill execution: {error}");
                return ExitCode::from(1);
            }
        };
        plan_steps.push(format!(
            "invoke_tool tool={} entrypoint={}",
            tool.tool_id, tool.entrypoint
        ));
    }

    let mut tool_execution_ids = Vec::new();
    let mut child_statuses = Vec::new();
    let mut child_outputs = Vec::new();
    if run_tools {
        for tool_id in &skill.default_tool_refs {
            let (_, tool) = match load_tool(root, tool_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("failed to load tool for skill tool-run: {error}");
                    return ExitCode::from(1);
                }
            };
            if let Err(error) = ensure_shell_execution_allowed(&tool) {
                eprintln!("failed to authorize tool during skill run: {error}");
                return ExitCode::from(1);
            }
            let outcome = match execute_tool_entrypoint(&tool.entrypoint, input) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("failed to execute tool during skill run: {error}");
                    return ExitCode::from(1);
                }
            };
            let child_execution_id = format!(
                "exec-tool-{}-{}",
                tool.tool_id,
                crate::core::current_timestamp().replace(':', "_")
            );
            let child_record = ExecutionRecord::new(
                child_execution_id.clone(),
                ExecutionKind::Tool,
                tool.tool_id.clone(),
                task_id.clone(),
                assignment_id.clone(),
                implementation_ref.clone(),
                implementation_snapshot.clone(),
                vec![skill.skill_id.clone()],
                vec![tool.tool_id.clone()],
                input.to_owned(),
                outcome.plan_steps.clone(),
                outcome.output.clone(),
                outcome.runner.clone(),
                outcome.exit_code,
                outcome.status,
            );
            if let Err(error) = persist_execution_record(root, &child_record) {
                eprintln!("failed to persist child tool execution record: {error}");
                return ExitCode::from(1);
            }
            if let Err(error) =
                execution_record::persist_execution_task_records(root, &child_record)
            {
                eprintln!("failed to append child tool execution task records: {error}");
                return ExitCode::from(1);
            }
            tool_execution_ids.push(child_execution_id);
            child_statuses.push(outcome.status);
            child_outputs.push(format!("tool={} output={}", tool.tool_id, outcome.output));
        }
    }

    if !tool_execution_ids.is_empty() {
        plan_steps.push(format!(
            "child_tool_executions={}",
            tool_execution_ids.join(",")
        ));
    }
    let outcome = if let Some(implementation) = implementation_record.as_ref() {
        match execute_skill_implementation(root, implementation, input, &child_outputs) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to execute skill implementation: {error}");
                return ExitCode::from(1);
            }
        }
    } else if run_tools && !child_statuses.is_empty() {
        let has_failed = child_statuses
            .iter()
            .any(|status| *status == crate::executor::ExecutionStatus::Failed);
        let has_succeeded = child_statuses
            .iter()
            .any(|status| *status == crate::executor::ExecutionStatus::Succeeded);
        let status = if has_failed {
            crate::executor::ExecutionStatus::Failed
        } else if has_succeeded {
            crate::executor::ExecutionStatus::Succeeded
        } else {
            crate::executor::ExecutionStatus::Simulated
        };
        let output = child_outputs.join(" | ");
        crate::executor::ToolExecutionOutcome {
            runner: "skill-orchestrator".to_owned(),
            exit_code: None,
            status,
            plan_steps: Vec::new(),
            output: if output.is_empty() {
                format!("orchestrated {} tool(s)", tool_execution_ids.len())
            } else {
                output
            },
        }
    } else {
        crate::executor::ToolExecutionOutcome {
            runner: "local-simulated".to_owned(),
            exit_code: None,
            status: crate::executor::ExecutionStatus::Simulated,
            plan_steps: Vec::new(),
            output: format!(
                "simulated skill execution for {} with {} tool(s)",
                skill.display_name,
                skill.default_tool_refs.len()
            ),
        }
    };
    plan_steps.extend(outcome.plan_steps.clone());

    let record = ExecutionRecord::new(
        execution_id.clone(),
        ExecutionKind::Skill,
        skill.skill_id.clone(),
        task_id.clone(),
        assignment_id.clone(),
        implementation_ref.clone(),
        implementation_snapshot,
        vec![skill.skill_id.clone()],
        skill.default_tool_refs.clone(),
        input.to_owned(),
        plan_steps,
        outcome.output,
        outcome.runner,
        outcome.exit_code,
        outcome.status,
    );
    let path = match persist_execution_record(root, &record) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist skill execution record: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = execution_record::persist_execution_task_records(root, &record) {
        eprintln!("failed to append skill execution task records: {error}");
        return ExitCode::from(1);
    }

    println!("skill execute recorded");
    println!("  execution_id: {}", record.execution_id);
    println!("  skill_id: {}", record.target_id);
    println!(
        "  implementation_ref: {}",
        record.implementation_ref.as_deref().unwrap_or("<none>")
    );
    if let Some(snapshot) = &record.implementation_snapshot {
        println!("  implementation_skill: {}", snapshot.skill_id);
        println!("  implementation_executor: {}", snapshot.executor);
    }
    println!("  runner: {}", record.runner);
    println!("  tool_refs: {}", joined_or_none(&record.tool_refs));
    println!(
        "  task_id: {}",
        record.task_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "  assignment_id: {}",
        record.assignment_id.as_deref().unwrap_or("<none>")
    );
    println!("  child_tool_execution_count: {}", tool_execution_ids.len());
    println!("  status: {}", record.status.as_str());
    println!("  written_to: {}", path.display());

    // Experimental: if the skill output is a tool_call JSON, parse it so that
    // future versions can automatically dispatch tools based on the model plan.
    if let Some(summary) = maybe_parse_tool_call(&record.output) {
        println!("{summary}");
    }

    ExitCode::SUCCESS
}

fn maybe_parse_tool_call(output: &str) -> Option<String> {
    let value: Value = serde_json::from_str(output).ok()?;
    if value.get("action")?.as_str()? != "tool_call" {
        return None;
    }
    let tool_id = value.get("tool_id")?.as_str()?.to_owned();
    let args = value.get("args").cloned().unwrap_or(Value::Null);
    Some(format!("tool_call parsed: tool_id={tool_id} args={args}"))
}
