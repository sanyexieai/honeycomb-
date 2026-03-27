use std::process::ExitCode;

use crate::protocol::{
    HandshakeTranscript, HeartbeatTranscript, QueenEndpoint, ShutdownTranscript,
    simulate_handshake, simulate_heartbeat, simulate_shutdown,
};
use crate::registry::{SkillRecord, ToolRecord};
use crate::runtime::{
    Assignment, AssignmentStatus, AuditRecord, EventRecord, ResidentHive, TaskRuntime, TaskSpec,
    TaskStatus, TraceRecord, TransitionOutcome, Trigger, TriggerStatus,
};
use crate::storage::{
    append_task_audit, append_task_event, append_task_trace, list_fitness_runs, list_residents,
    list_skills, list_tools, list_triggers, load_assignment, load_resident, load_skill,
    load_task_assignments, load_task_audits, load_task_events, load_task_submission,
    load_task_traces, load_tool, load_trigger, persist_resident, persist_skill,
    persist_task_submission, persist_tool, persist_trigger, update_assignment, update_resident,
    update_task_runtime, update_task_submission, update_trigger,
};

use super::cli::{
    BinaryRole, Command, execute_command, has_flag, option_value, option_values, parse_command,
};
use super::execution_support::{
    apply_record_write, classify_active_task, should_write_event_record, transition_outcome_label,
};

fn joined_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "<none>".to_owned()
    } else {
        values.join(", ")
    }
}

fn is_legacy_demo_task(record: &crate::runtime::TaskRecord) -> bool {
    record.task_spec.skill_refs.is_empty()
        && (record.task_spec.task_id.starts_with("task-demo-flow-")
            || record.task_spec.task_id == "task-xhs-demo")
}

fn merge_unique(primary: Vec<String>, secondary: Vec<String>) -> Vec<String> {
    let mut merged = primary;
    for value in secondary {
        if !merged.iter().any(|existing| existing == &value) {
            merged.push(value);
        }
    }
    merged
}

fn resolve_skill_submission_preset(
    root: &str,
    from_skill: Option<&str>,
    explicit_goal: Option<&str>,
    use_recommended_impl: bool,
    explicit_skill_refs: Vec<String>,
    explicit_tool_refs: Vec<String>,
) -> std::io::Result<(String, Option<String>, Vec<String>, Vec<String>, Option<SkillRecord>)> {
    if let Some(skill_id) = from_skill {
        let (_, skill) = load_skill(root, skill_id)?;
        let mut skill_refs = explicit_skill_refs;
        if !skill_refs.iter().any(|value| value == skill_id) {
            skill_refs.insert(0, skill_id.to_owned());
        }
        let tool_refs = merge_unique(skill.default_tool_refs.clone(), explicit_tool_refs);
        let goal = explicit_goal
            .map(str::to_owned)
            .or_else(|| skill.goal_template.clone())
            .unwrap_or_else(|| format!("run skill {}", skill.display_name));
        let implementation_ref = if use_recommended_impl {
            skill
                .recommended_implementation_id
                .clone()
                .or_else(|| Some(skill.implementation_ref.clone()))
        } else {
            Some(skill.implementation_ref.clone())
        };
        Ok((goal, implementation_ref, skill_refs, tool_refs, Some(skill)))
    } else {
        Ok((
            explicit_goal.unwrap_or("bootstrap-task").to_owned(),
            None,
            explicit_skill_refs,
            explicit_tool_refs,
            None,
        ))
    }
}

fn validate_registry_refs(
    root: &str,
    skill_refs: &[String],
    tool_refs: &[String],
) -> std::io::Result<()> {
    for skill_ref in skill_refs {
        load_skill(root, skill_ref)?;
    }

    for tool_ref in tool_refs {
        load_tool(root, tool_ref)?;
    }

    Ok(())
}

pub(crate) fn handle(command: Command, args: &[String]) -> ExitCode {
    match command {
        Command::QueenRun => handle_queen_run(args),
        Command::WorkerRun => handle_worker_run(args),
        Command::TaskSubmit => handle_task_submit(args),
        Command::TaskDemoFlow => handle_task_demo_flow(args),
        Command::TaskAssign => handle_task_assign(args),
        Command::AssignmentList => handle_assignment_list(args),
        Command::AssignmentInspect => handle_assignment_inspect(args),
        Command::TaskResult => handle_task_result(args),
        Command::TaskList => handle_task_list(args),
        Command::TaskBackfillImplementation => handle_task_backfill_implementation(args),
        Command::TaskInspect => handle_task_inspect(args),
        Command::TaskReplay => handle_task_replay(args),
        Command::AuditTail => handle_task_audit_tail(args),
        Command::TraceTail => handle_task_trace_tail(args),
        Command::TriggerCreate => handle_trigger_create(args),
        Command::TriggerInspect => handle_trigger_inspect(args),
        Command::TriggerList => handle_trigger_list(args),
        Command::TriggerPause => handle_trigger_pause(args),
        Command::TriggerResume => handle_trigger_resume(args),
        Command::TriggerFire => handle_trigger_fire(args),
        Command::SkillRegister => handle_skill_register(args),
        Command::SkillInspect => handle_skill_inspect(args),
        Command::SkillList => handle_skill_list(args),
        Command::ToolRegister => handle_tool_register(args),
        Command::ToolInspect => handle_tool_inspect(args),
        Command::ToolList => handle_tool_list(args),
        Command::HeartbeatSend => handle_heartbeat_send(args),
        Command::ShutdownSend => handle_shutdown_send(args),
        Command::ResidentRun => handle_resident_run(args),
        Command::ResidentInspect => handle_resident_inspect(args),
        Command::ResidentHeartbeat => handle_resident_heartbeat(args),
        Command::ResidentPause => handle_resident_pause(args),
        Command::ResidentResume => handle_resident_resume(args),
        Command::ResidentStop => handle_resident_stop(args),
        Command::RuntimeOverview => handle_runtime_overview(args),
        other => {
            println!(
                "{} command scaffold: {}",
                BinaryRole::Execution.binary_name(),
                super::cli::command_name(&other)
            );
            ExitCode::SUCCESS
        }
    }
}

fn handle_queen_run(args: &[String]) -> ExitCode {
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-local");
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let queen_token = option_value(args, "--queen-token").unwrap_or("queen-token-demo");

    let endpoint = QueenEndpoint::new(
        queen_node_id.to_owned(),
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        queen_token.to_owned(),
    );

    println!("queen run ready");
    println!("  queen_node_id: {}", endpoint.queen_node_id);
    println!("  task_id: {}", endpoint.task_id);
    println!("  tenant_id: {}", endpoint.tenant_id);
    println!("  namespace: {}", endpoint.namespace);
    println!("  protocol_version: {}", crate::core::PROTOCOL_VERSION);
    println!("  queen_token: {}", endpoint.queen_token);

    ExitCode::SUCCESS
}

fn handle_worker_run(args: &[String]) -> ExitCode {
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-local");
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-local");
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let queen_token = option_value(args, "--queen-token").unwrap_or("queen-token-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let endpoint = QueenEndpoint::new(
        queen_node_id.to_owned(),
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        queen_token.to_owned(),
    );
    let transcript = simulate_handshake(
        &endpoint,
        worker_node_id,
        tenant_id,
        namespace,
        task_id,
        queen_node_id,
        queen_token,
    );

    let (_, task_record) = match load_task_submission(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load task runtime after handshake: {error}");
            return ExitCode::from(1);
        }
    };
    let hello_payload = format!(
        "worker={} queen={}",
        transcript.hello.from, transcript.hello_payload.queen_node_id
    );
    let should_write =
        match should_write_event_record(root, task_id, "hello_received", &hello_payload) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to inspect existing handshake events: {error}");
                return ExitCode::from(1);
            }
        };
    let mut runtime_outcome = TransitionOutcome::NoOp;
    let record_write = match apply_record_write(
        should_write
            && !(transcript.ack_payload.accepted
                && task_record.task_runtime.status != TaskStatus::Queued),
        || persist_handshake_records(root, &transcript),
    ) {
        Ok(label) => {
            if label == "applied" && transcript.ack_payload.accepted {
                runtime_outcome = match update_task_runtime(root, task_id, TaskStatus::Running) {
                    Ok((_, outcome)) => outcome,
                    Err(error) => {
                        eprintln!("failed to update task runtime after handshake: {error}");
                        return ExitCode::from(1);
                    }
                };
            }
            label
        }
        Err(error) => {
            eprintln!("failed to persist handshake records: {error}");
            return ExitCode::from(1);
        }
    };

    println!("worker run handshake");
    println!(
        "  hello: {} -> {} kind={:?}",
        transcript.hello.from, transcript.hello.to, transcript.hello.kind
    );
    println!(
        "  hello_context: task_id={} tenant_id={} namespace={}",
        transcript.hello.task_id, transcript.hello.tenant_id, transcript.hello.namespace
    );
    println!(
        "  hello_ack: {} accepted={} reason={}",
        transcript.ack.msg_id, transcript.ack_payload.accepted, transcript.ack_payload.reason
    );
    println!(
        "  runtime_update: {}",
        transition_outcome_label(runtime_outcome)
    );
    println!("  record_write: {record_write}");

    if transcript.ack_payload.accepted {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn handle_heartbeat_send(args: &[String]) -> ExitCode {
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-local");
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-local");
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let queen_token = option_value(args, "--queen-token").unwrap_or("queen-token-demo");
    let state = option_value(args, "--state").unwrap_or("idle");
    let root = option_value(args, "--root").unwrap_or(".");

    let endpoint = QueenEndpoint::new(
        queen_node_id.to_owned(),
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        queen_token.to_owned(),
    );
    let transcript = simulate_heartbeat(&endpoint, worker_node_id, state);

    let heartbeat_payload = format!(
        "worker={} state={}",
        transcript.payload.worker_node_id, transcript.payload.state
    );
    let should_write =
        match should_write_event_record(root, task_id, "heartbeat_received", &heartbeat_payload) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to inspect existing heartbeat events: {error}");
                return ExitCode::from(1);
            }
        };
    let record_write = match apply_record_write(should_write, || {
        persist_heartbeat_records(root, &transcript)
    }) {
        Ok(label) => label,
        Err(error) => {
            eprintln!("failed to persist heartbeat records: {error}");
            return ExitCode::from(1);
        }
    };

    println!("heartbeat send recorded");
    println!(
        "  heartbeat: {} -> {} state={}",
        transcript.heartbeat.from, transcript.heartbeat.to, transcript.payload.state
    );
    println!("  task_id: {}", transcript.heartbeat.task_id);
    println!("  record_write: {record_write}");
    ExitCode::SUCCESS
}

fn handle_shutdown_send(args: &[String]) -> ExitCode {
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-local");
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-local");
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let queen_token = option_value(args, "--queen-token").unwrap_or("queen-token-demo");
    let reason = option_value(args, "--reason").unwrap_or("manual_shutdown");
    let root = option_value(args, "--root").unwrap_or(".");

    let endpoint = QueenEndpoint::new(
        queen_node_id.to_owned(),
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        queen_token.to_owned(),
    );
    let transcript = simulate_shutdown(&endpoint, worker_node_id, reason);

    let (_, task_record) = match load_task_submission(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load task runtime before shutdown: {error}");
            return ExitCode::from(1);
        }
    };
    let should_write =
        match should_write_event_record(root, task_id, "shutdown_sent", &transcript.payload.reason)
        {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to inspect existing shutdown events: {error}");
                return ExitCode::from(1);
            }
        };
    let (runtime_outcome, record_write) =
        if task_record.task_runtime.status == TaskStatus::Completed || !should_write {
            (TransitionOutcome::NoOp, "skipped")
        } else {
            let label =
                match apply_record_write(true, || persist_shutdown_records(root, &transcript)) {
                    Ok(label) => label,
                    Err(error) => {
                        eprintln!("failed to persist shutdown records: {error}");
                        return ExitCode::from(1);
                    }
                };
            let outcome = match update_task_runtime(root, task_id, TaskStatus::Completed) {
                Ok((_, outcome)) => outcome,
                Err(error) => {
                    eprintln!("failed to update task runtime after shutdown: {error}");
                    return ExitCode::from(1);
                }
            };
            (outcome, label)
        };

    println!("shutdown send recorded");
    println!(
        "  shutdown: {} -> {} reason={}",
        transcript.shutdown.from, transcript.shutdown.to, transcript.payload.reason
    );
    println!("  task_id: {}", transcript.shutdown.task_id);
    println!(
        "  runtime_update: {}",
        transition_outcome_label(runtime_outcome)
    );
    println!("  record_write: {record_write}");
    ExitCode::SUCCESS
}

fn handle_skill_register(args: &[String]) -> ExitCode {
    let skill = SkillRecord::new(
        option_value(args, "--skill-id")
            .unwrap_or("skill-demo")
            .to_owned(),
        option_value(args, "--display-name")
            .unwrap_or("Skill Demo")
            .to_owned(),
        option_value(args, "--description")
            .unwrap_or("Demo skill")
            .to_owned(),
        option_value(args, "--implementation-ref")
            .unwrap_or("impl://demo/skill")
            .to_owned(),
        option_value(args, "--owner")
            .unwrap_or("tenant-local")
            .to_owned(),
        option_value(args, "--version")
            .unwrap_or("1.0.0")
            .to_owned(),
        option_values(args, "--default-tool-ref"),
        option_value(args, "--goal-template").map(str::to_owned),
    );
    let root = option_value(args, "--root").unwrap_or(".");

    let path = match persist_skill(root, &skill) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist skill: {error}");
            return ExitCode::from(1);
        }
    };

    println!("skill register recorded");
    println!("  skill_id: {}", skill.skill_id);
    println!("  display_name: {}", skill.display_name);
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
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_skill_inspect(args: &[String]) -> ExitCode {
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
                .is_some_and(|value| value == a.fitness_report.implementation_id);
            let b_match = skill
                .recommended_implementation_id
                .as_deref()
                .is_some_and(|value| value == b.fitness_report.implementation_id);
            b_match
                .cmp(&a_match)
                .then_with(|| b.fitness_report.score.cmp(&a.fitness_report.score))
                .then_with(|| {
                    a.fitness_report
                        .implementation_id
                        .cmp(&b.fitness_report.implementation_id)
                })
        });

        println!("  lineage_count: {}", lineage.len());
        for record in lineage {
            println!(
                "  - {} score={} decision={} tools={}",
                record.fitness_report.implementation_id,
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
        println!("  runtime_scope: {}", if recommended_only { "recommended_only" } else { "all_skill_tasks" });

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
                    let skill_match = assignment.skill_refs.iter().any(|value| value == &skill.skill_id);
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
                task.task_spec.implementation_ref.as_deref().unwrap_or("<none>"),
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

fn handle_skill_list(args: &[String]) -> ExitCode {
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
        println!(
            "  - {} version={} impl={} default_tools={} goal_template={} recommended={} decision={} owner={}",
            skill.skill_id,
            skill.version,
            skill.implementation_ref,
            joined_or_none(&skill.default_tool_refs),
            skill.goal_template.as_deref().unwrap_or("<none>"),
            skill
                .recommended_implementation_id
                .as_deref()
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

fn handle_tool_register(args: &[String]) -> ExitCode {
    let tool = ToolRecord::new(
        option_value(args, "--tool-id")
            .unwrap_or("tool-demo")
            .to_owned(),
        option_value(args, "--display-name")
            .unwrap_or("Tool Demo")
            .to_owned(),
        option_value(args, "--description")
            .unwrap_or("Demo tool")
            .to_owned(),
        option_value(args, "--entrypoint")
            .unwrap_or("tool://demo")
            .to_owned(),
        option_value(args, "--owner")
            .unwrap_or("tenant-local")
            .to_owned(),
        option_value(args, "--version")
            .unwrap_or("1.0.0")
            .to_owned(),
    );
    let root = option_value(args, "--root").unwrap_or(".");

    let path = match persist_tool(root, &tool) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist tool: {error}");
            return ExitCode::from(1);
        }
    };

    println!("tool register recorded");
    println!("  tool_id: {}", tool.tool_id);
    println!("  display_name: {}", tool.display_name);
    println!("  entrypoint: {}", tool.entrypoint);
    println!("  owner: {}", tool.owner);
    println!("  version: {}", tool.version);
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_tool_inspect(args: &[String]) -> ExitCode {
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
                .filter(|assignment| assignment.tool_refs.iter().any(|value| value == &tool.tool_id))
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
                task.task_spec.implementation_ref.as_deref().unwrap_or("<none>"),
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

fn handle_tool_list(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list tools: {error}");
            return ExitCode::from(1);
        }
    };

    println!("tool list loaded");
    println!("  read_from: {}", dir.display());
    println!("  tool_count: {}", tools.len());
    for tool in tools {
        println!(
            "  - {} version={} entrypoint={} owner={}",
            tool.tool_id, tool.version, tool.entrypoint, tool.owner
        );
    }
    ExitCode::SUCCESS
}

fn handle_resident_run(args: &[String]) -> ExitCode {
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

fn handle_resident_inspect(args: &[String]) -> ExitCode {
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

fn handle_resident_heartbeat(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident heartbeat recorded",
        "resident_heartbeat",
        |resident, timestamp| {
            resident.refresh(timestamp);
        },
    )
}

fn handle_resident_pause(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident pause recorded",
        "resident_pause",
        |resident, timestamp| {
            resident.pause(timestamp);
        },
    )
}

fn handle_resident_resume(args: &[String]) -> ExitCode {
    handle_resident_update(
        args,
        "resident resume recorded",
        "resident_resume",
        |resident, timestamp| {
            resident.refresh(timestamp);
        },
    )
}

fn handle_resident_stop(args: &[String]) -> ExitCode {
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

fn handle_trigger_create(args: &[String]) -> ExitCode {
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

fn handle_trigger_inspect(args: &[String]) -> ExitCode {
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
    println!(
        "  last_fired_at: {}",
        trigger.last_fired_at.as_deref().unwrap_or("<none>")
    );
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_trigger_list(args: &[String]) -> ExitCode {
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
            "  - {} type={} schedule={} status={} fire_count={}",
            trigger.trigger_id,
            trigger.trigger_type,
            trigger.schedule,
            trigger.status.as_str(),
            trigger.fire_count
        );
    }

    ExitCode::SUCCESS
}

fn handle_trigger_pause(args: &[String]) -> ExitCode {
    handle_trigger_status_update(args, TriggerStatus::Paused, "trigger pause recorded")
}

fn handle_trigger_resume(args: &[String]) -> ExitCode {
    handle_trigger_status_update(args, TriggerStatus::Active, "trigger resume recorded")
}

fn handle_trigger_fire(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let trigger_id = option_value(args, "--trigger-id").unwrap_or("trigger-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();

    let (path, trigger) = match update_trigger(root, task_id, trigger_id, |trigger| {
        trigger
            .try_record_fire(timestamp.clone())
            .map_err(std::io::Error::other)
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to fire trigger: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_task_event(
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
    ) {
        eprintln!("failed to append trigger event: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_audit(
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
    ) {
        eprintln!("failed to append trigger audit: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_trace(
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
    ) {
        eprintln!("failed to append trigger trace: {error}");
        return ExitCode::from(1);
    }

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

fn persist_handshake_records(root: &str, transcript: &HandshakeTranscript) -> std::io::Result<()> {
    let task_id = &transcript.hello.task_id;
    let hello_event = EventRecord::new(
        transcript.hello.msg_id.clone(),
        "hello_received".to_owned(),
        task_id.clone(),
        transcript.hello.timestamp.clone(),
        format!(
            "worker={} queen={}",
            transcript.hello.from, transcript.hello_payload.queen_node_id
        ),
    );
    append_task_event(root, task_id, &hello_event)?;

    let ack_event = EventRecord::new(
        transcript.ack.msg_id.clone(),
        if transcript.ack_payload.accepted {
            "hello_accepted".to_owned()
        } else {
            "hello_rejected".to_owned()
        },
        task_id.clone(),
        transcript.ack.timestamp.clone(),
        transcript.ack_payload.reason.clone(),
    );
    append_task_event(root, task_id, &ack_event)?;

    let audit = AuditRecord::new(
        format!("audit-{}-worker-handshake", transcript.hello.from),
        transcript.ack.timestamp.clone(),
        "worker".to_owned(),
        transcript.hello.from.clone(),
        "hello_handshake".to_owned(),
        "queen".to_owned(),
        transcript.ack.from.clone(),
        task_id.clone(),
        if transcript.ack_payload.accepted {
            "accepted".to_owned()
        } else {
            "rejected".to_owned()
        },
        transcript.ack_payload.reason.clone(),
    );
    append_task_audit(root, task_id, &audit)?;

    let trace = TraceRecord::new(
        format!("trace-{task_id}"),
        format!("span-{}-hello", transcript.hello.from),
        Some(format!("span-{}-submit", task_id)),
        transcript.ack.timestamp.clone(),
        "worker_handshake".to_owned(),
        task_id.clone(),
        if transcript.ack_payload.accepted {
            "accepted".to_owned()
        } else {
            "rejected".to_owned()
        },
        format!(
            "worker={} reason={}",
            transcript.hello.from, transcript.ack_payload.reason
        ),
    );
    append_task_trace(root, task_id, &trace)?;

    Ok(())
}

fn persist_heartbeat_records(root: &str, transcript: &HeartbeatTranscript) -> std::io::Result<()> {
    let task_id = &transcript.heartbeat.task_id;
    append_task_event(
        root,
        task_id,
        &EventRecord::new(
            transcript.heartbeat.msg_id.clone(),
            "heartbeat_received".to_owned(),
            task_id.clone(),
            transcript.heartbeat.timestamp.clone(),
            format!(
                "worker={} state={}",
                transcript.payload.worker_node_id, transcript.payload.state
            ),
        ),
    )?;
    append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{}-heartbeat", transcript.payload.worker_node_id),
            transcript.heartbeat.timestamp.clone(),
            "worker".to_owned(),
            transcript.payload.worker_node_id.clone(),
            "heartbeat_send".to_owned(),
            "queen".to_owned(),
            transcript.heartbeat.to.clone(),
            task_id.clone(),
            "recorded".to_owned(),
            transcript.payload.state.clone(),
        ),
    )?;
    append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{}-heartbeat", transcript.payload.worker_node_id),
            Some(format!("span-{}-hello", transcript.payload.worker_node_id)),
            transcript.heartbeat.timestamp.clone(),
            "heartbeat".to_owned(),
            task_id.clone(),
            "recorded".to_owned(),
            format!("state={}", transcript.payload.state),
        ),
    )?;
    Ok(())
}

fn persist_shutdown_records(root: &str, transcript: &ShutdownTranscript) -> std::io::Result<()> {
    let task_id = &transcript.shutdown.task_id;
    append_task_event(
        root,
        task_id,
        &EventRecord::new(
            transcript.shutdown.msg_id.clone(),
            "shutdown_sent".to_owned(),
            task_id.clone(),
            transcript.shutdown.timestamp.clone(),
            transcript.payload.reason.clone(),
        ),
    )?;
    append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{}-shutdown", transcript.payload.worker_node_id),
            transcript.shutdown.timestamp.clone(),
            "queen".to_owned(),
            transcript.payload.queen_node_id.clone(),
            "shutdown_send".to_owned(),
            "worker".to_owned(),
            transcript.payload.worker_node_id.clone(),
            task_id.clone(),
            "recorded".to_owned(),
            transcript.payload.reason.clone(),
        ),
    )?;
    append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{}-shutdown", transcript.payload.worker_node_id),
            Some(format!("span-{}-hello", transcript.payload.worker_node_id)),
            transcript.shutdown.timestamp.clone(),
            "shutdown".to_owned(),
            task_id.clone(),
            "recorded".to_owned(),
            format!("reason={}", transcript.payload.reason),
        ),
    )?;
    Ok(())
}

fn handle_task_submit(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let goal = option_value(args, "--goal");
    let from_skill = option_value(args, "--from-skill");
    let use_recommended_impl = has_flag(args, "--use-recommended-impl");
    let explicit_implementation_ref =
        option_value(args, "--implementation-ref").map(str::to_owned);
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-local");
    let skill_refs = option_values(args, "--skill-ref");
    let tool_refs = option_values(args, "--tool-ref");
    let root = option_value(args, "--root").unwrap_or(".");

    let (goal, mut implementation_ref, skill_refs, tool_refs, source_skill) =
        match resolve_skill_submission_preset(
            root,
            from_skill,
            goal,
            use_recommended_impl,
            skill_refs,
            tool_refs,
        ) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to resolve task skill preset: {error}");
                return ExitCode::from(1);
            }
        };
    if explicit_implementation_ref.is_some() {
        implementation_ref = explicit_implementation_ref;
    }

    if let Err(error) = validate_registry_refs(root, &skill_refs, &tool_refs) {
        eprintln!("failed to validate task capability refs: {error}");
        return ExitCode::from(1);
    }

    let spec = TaskSpec::new(
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        goal.clone(),
        implementation_ref,
        skill_refs,
        tool_refs,
    );
    let runtime = TaskRuntime::queued(task_id.to_owned(), queen_node_id.to_owned());

    let output_path = match persist_task_submission(root, &spec, &runtime) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist task submission: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task submit accepted");
    println!("  task_id: {}", spec.task_id);
    println!("  tenant_id: {}", spec.tenant_id);
    println!("  namespace: {}", spec.namespace);
    println!("  goal: {}", spec.goal);
    println!(
        "  implementation_ref: {}",
        spec.implementation_ref.as_deref().unwrap_or("<none>")
    );
    if let Some(skill) = source_skill {
        println!("  source_skill: {}", skill.skill_id);
    }
    println!("  skill_refs: {}", joined_or_none(&spec.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&spec.tool_refs));
    println!("  queen_node_id: {}", runtime.queen_node_id);
    println!("  status: {:?}", runtime.status);
    println!("  written_to: {}", output_path.display());

    ExitCode::SUCCESS
}

fn handle_task_demo_flow(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo-flow");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let goal = option_value(args, "--goal");
    let from_skill = option_value(args, "--from-skill");
    let use_recommended_impl = has_flag(args, "--use-recommended-impl");
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-demo");
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-demo");
    let queen_token = option_value(args, "--queen-token").unwrap_or("queen-token-demo");
    let assignment_id = option_value(args, "--assignment-id").unwrap_or("assignment-demo");
    let attempt_id = option_value(args, "--attempt-id").unwrap_or("attempt-1");
    let resident_id = option_value(args, "--resident-id").unwrap_or("resident-demo");
    let skill_refs = option_values(args, "--skill-ref");
    let tool_refs = option_values(args, "--tool-ref");
    let input = option_value(args, "--input").unwrap_or("demo-input");
    let output = option_value(args, "--output").unwrap_or("demo-output");
    let root = option_value(args, "--root").unwrap_or(".");

    let (goal, implementation_ref, skill_refs, tool_refs, source_skill) =
        match resolve_skill_submission_preset(
            root,
            from_skill,
            goal,
            use_recommended_impl,
            skill_refs,
            tool_refs,
        ) {
            Ok((goal, implementation_ref, skill_refs, tool_refs, source_skill)) => {
                let goal = if goal == "bootstrap-task" {
                    "demo-flow".to_owned()
                } else {
                    goal
                };
                (
                    goal,
                    implementation_ref,
                    skill_refs,
                    tool_refs,
                    source_skill,
                )
            }
            Err(error) => {
                eprintln!("failed to resolve demo-flow skill preset: {error}");
                return ExitCode::from(1);
            }
        };

    if let Err(error) = validate_registry_refs(root, &skill_refs, &tool_refs) {
        eprintln!("failed to validate demo-flow capability refs: {error}");
        return ExitCode::from(1);
    }

    let mut submit_step = vec![
        "task".to_owned(),
        "submit".to_owned(),
        "--task-id".to_owned(),
        task_id.to_owned(),
        "--tenant".to_owned(),
        tenant_id.to_owned(),
        "--namespace".to_owned(),
        namespace.to_owned(),
        "--goal".to_owned(),
        goal.to_owned(),
        "--queen-node".to_owned(),
        queen_node_id.to_owned(),
        "--root".to_owned(),
        root.to_owned(),
    ];
    if let Some(implementation_ref) = &implementation_ref {
        submit_step.push("--implementation-ref".to_owned());
        submit_step.push(implementation_ref.clone());
    }
    if use_recommended_impl {
        submit_step.push("--use-recommended-impl".to_owned());
    }
    for skill_ref in &skill_refs {
        submit_step.push("--skill-ref".to_owned());
        submit_step.push(skill_ref.clone());
    }
    for tool_ref in &tool_refs {
        submit_step.push("--tool-ref".to_owned());
        submit_step.push(tool_ref.clone());
    }

    let steps = vec![
        ("task submit", submit_step),
        (
            "worker run",
            vec![
                "worker".to_owned(),
                "run".to_owned(),
                "--worker-node".to_owned(),
                worker_node_id.to_owned(),
                "--queen-node".to_owned(),
                queen_node_id.to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--tenant".to_owned(),
                tenant_id.to_owned(),
                "--namespace".to_owned(),
                namespace.to_owned(),
                "--queen-token".to_owned(),
                queen_token.to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "heartbeat send",
            vec![
                "heartbeat".to_owned(),
                "send".to_owned(),
                "--worker-node".to_owned(),
                worker_node_id.to_owned(),
                "--queen-node".to_owned(),
                queen_node_id.to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--tenant".to_owned(),
                tenant_id.to_owned(),
                "--namespace".to_owned(),
                namespace.to_owned(),
                "--queen-token".to_owned(),
                queen_token.to_owned(),
                "--state".to_owned(),
                "idle".to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "resident run",
            vec![
                "resident".to_owned(),
                "run".to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--resident-id".to_owned(),
                resident_id.to_owned(),
                "--worker-node".to_owned(),
                worker_node_id.to_owned(),
                "--purpose".to_owned(),
                "demo-resident".to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "resident heartbeat",
            vec![
                "resident".to_owned(),
                "heartbeat".to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--resident-id".to_owned(),
                resident_id.to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "task assign",
            vec![
                "task".to_owned(),
                "assign".to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--assignment-id".to_owned(),
                assignment_id.to_owned(),
                "--attempt-id".to_owned(),
                attempt_id.to_owned(),
                "--worker-node".to_owned(),
                worker_node_id.to_owned(),
                "--input".to_owned(),
                input.to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "task result",
            vec![
                "task".to_owned(),
                "result".to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--assignment-id".to_owned(),
                assignment_id.to_owned(),
                "--attempt-id".to_owned(),
                attempt_id.to_owned(),
                "--worker-node".to_owned(),
                worker_node_id.to_owned(),
                "--input".to_owned(),
                input.to_owned(),
                "--output".to_owned(),
                output.to_owned(),
                "--status".to_owned(),
                "completed".to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "resident stop",
            vec![
                "resident".to_owned(),
                "stop".to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--resident-id".to_owned(),
                resident_id.to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
        (
            "shutdown send",
            vec![
                "shutdown".to_owned(),
                "send".to_owned(),
                "--worker-node".to_owned(),
                worker_node_id.to_owned(),
                "--queen-node".to_owned(),
                queen_node_id.to_owned(),
                "--task-id".to_owned(),
                task_id.to_owned(),
                "--tenant".to_owned(),
                tenant_id.to_owned(),
                "--namespace".to_owned(),
                namespace.to_owned(),
                "--queen-token".to_owned(),
                queen_token.to_owned(),
                "--reason".to_owned(),
                "demo-flow-complete".to_owned(),
                "--root".to_owned(),
                root.to_owned(),
            ],
        ),
    ];

    println!("task demo-flow started");
    println!("  task_id: {task_id}");
    if let Some(skill) = source_skill {
        println!("  source_skill: {}", skill.skill_id);
    }
    println!("  assignment_id: {assignment_id}");
    println!("  worker_node_id: {worker_node_id}");
    println!("  root: {root}");

    for (step_name, step_args) in steps {
        println!("  step: {step_name}");
        let exit = match parse_command(BinaryRole::Execution, &step_args) {
            Ok(command) => execute_command(BinaryRole::Execution, command, &step_args),
            Err(message) => {
                eprintln!("failed to parse demo-flow step {step_name}: {message}");
                return ExitCode::from(1);
            }
        };
        if exit != ExitCode::SUCCESS {
            eprintln!("demo-flow step failed: {step_name}");
            return exit;
        }
    }

    println!("task demo-flow completed");
    println!(
        "  implementation_ref: {}",
        implementation_ref.as_deref().unwrap_or("<none>")
    );
    ExitCode::SUCCESS
}

fn handle_task_assign(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let assignment_id = option_value(args, "--assignment-id").unwrap_or("assignment-demo");
    let attempt_id = option_value(args, "--attempt-id").unwrap_or("attempt-1");
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-local");
    let input = option_value(args, "--input").unwrap_or("assignment-input");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();
    let (_, task_record) = match load_task_submission(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load task for assignment: {error}");
            return ExitCode::from(1);
        }
    };

    let assignment = Assignment::assigned(
        assignment_id.to_owned(),
        task_id.to_owned(),
        attempt_id.to_owned(),
        worker_node_id.to_owned(),
        input.to_owned(),
        task_record.task_spec.implementation_ref.clone(),
        task_record.task_spec.skill_refs.clone(),
        task_record.task_spec.tool_refs.clone(),
    );
    let output_path = match crate::storage::persist_assignment(root, &assignment) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist assignment: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{assignment_id}-assigned"),
            "assignment_assigned".to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!(
                "worker={worker_node_id} attempt_id={attempt_id} implementation={} skills={} tools={}",
                assignment
                    .implementation_ref
                    .as_deref()
                    .unwrap_or("<none>"),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            ),
        ),
    ) {
        eprintln!("failed to append assignment event: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{assignment_id}-assign"),
            timestamp.clone(),
            "queen".to_owned(),
            "queen-local".to_owned(),
            "task_assign".to_owned(),
            "assignment".to_owned(),
            assignment_id.to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!(
                "worker={worker_node_id} implementation={} skills={} tools={}",
                assignment
                    .implementation_ref
                    .as_deref()
                    .unwrap_or("<none>"),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            ),
        ),
    ) {
        eprintln!("failed to append assignment audit: {error}");
        return ExitCode::from(1);
    }

    if let Err(error) = append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{assignment_id}-assign"),
            Some(format!("span-{worker_node_id}-hello")),
            timestamp,
            "task_assign".to_owned(),
            task_id.to_owned(),
            "assigned".to_owned(),
            format!(
                "attempt_id={attempt_id} implementation={} skills={} tools={}",
                assignment
                    .implementation_ref
                    .as_deref()
                    .unwrap_or("<none>"),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            ),
        ),
    ) {
        eprintln!("failed to append assignment trace: {error}");
        return ExitCode::from(1);
    }

    println!("task assign recorded");
    println!("  task_id: {}", assignment.task_id);
    println!("  assignment_id: {}", assignment.assignment_id);
    println!("  attempt_id: {}", assignment.attempt_id);
    println!("  worker_node_id: {}", assignment.worker_node_id);
    println!(
        "  implementation_ref: {}",
        assignment.implementation_ref.as_deref().unwrap_or("<none>")
    );
    println!("  skill_refs: {}", joined_or_none(&assignment.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&assignment.tool_refs));
    println!("  status: {}", assignment.status.as_str());
    println!("  written_to: {}", output_path.display());
    ExitCode::SUCCESS
}

fn handle_task_result(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let assignment_id = option_value(args, "--assignment-id").unwrap_or("assignment-demo");
    let attempt_id = option_value(args, "--attempt-id").unwrap_or("attempt-1");
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-local");
    let input = option_value(args, "--input").unwrap_or("assignment-input");
    let output = option_value(args, "--output").unwrap_or("assignment-output");
    let status_arg = option_value(args, "--status").unwrap_or("completed");
    let root = option_value(args, "--root").unwrap_or(".");
    let timestamp = crate::core::current_timestamp();
    let status = if status_arg == "failed" {
        AssignmentStatus::Failed
    } else {
        AssignmentStatus::Completed
    };

    let (output_path, assignment, outcome) =
        match update_assignment(root, task_id, assignment_id, |assignment| {
            if assignment.attempt_id != attempt_id {
                return Err(std::io::Error::other("assignment_attempt_id_mismatch"));
            }
            if assignment.worker_node_id != worker_node_id {
                return Err(std::io::Error::other("assignment_worker_node_id_mismatch"));
            }
            if assignment.input != input {
                return Err(std::io::Error::other("assignment_input_mismatch"));
            }
            if assignment.status == status && assignment.output.as_deref() == Some(output) {
                return Ok(TransitionOutcome::NoOp);
            }
            assignment.mark_running().map_err(std::io::Error::other)?;
            if status == AssignmentStatus::Failed {
                assignment
                    .fail(output.to_owned())
                    .map_err(std::io::Error::other)?;
            } else {
                assignment
                    .complete(output.to_owned())
                    .map_err(std::io::Error::other)?;
            }
            Ok(TransitionOutcome::Applied)
        }) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to update assignment result: {error}");
                return ExitCode::from(1);
            }
        };

    let records_written = match apply_record_write(outcome == TransitionOutcome::Applied, || {
        append_task_event(
            root,
            task_id,
            &EventRecord::new(
                format!("event-{assignment_id}-result"),
                "task_result".to_owned(),
                task_id.to_owned(),
                timestamp.clone(),
                format!(
                    "worker={worker_node_id} status={} implementation={} skills={} tools={}",
                    status.as_str(),
                    assignment
                        .implementation_ref
                        .as_deref()
                        .unwrap_or("<none>"),
                    joined_or_none(&assignment.skill_refs),
                    joined_or_none(&assignment.tool_refs)
                ),
            ),
        )?;

        append_task_audit(
            root,
            task_id,
            &AuditRecord::new(
                format!("audit-{assignment_id}-result"),
                timestamp.clone(),
                "worker".to_owned(),
                worker_node_id.to_owned(),
                "task_result".to_owned(),
                "assignment".to_owned(),
                assignment_id.to_owned(),
                task_id.to_owned(),
                status.as_str().to_owned(),
                format!(
                    "{output} implementation={} skills={} tools={}",
                    assignment
                        .implementation_ref
                        .as_deref()
                        .unwrap_or("<none>"),
                    joined_or_none(&assignment.skill_refs),
                    joined_or_none(&assignment.tool_refs)
                ),
            ),
        )?;

        append_task_trace(
            root,
            task_id,
            &TraceRecord::new(
                format!("trace-{task_id}"),
                format!("span-{assignment_id}-result"),
                Some(format!("span-{assignment_id}-assign")),
                timestamp,
                "task_result".to_owned(),
                task_id.to_owned(),
                status.as_str().to_owned(),
                format!(
                    "output={output} implementation={} skills={} tools={}",
                    assignment
                        .implementation_ref
                        .as_deref()
                        .unwrap_or("<none>"),
                    joined_or_none(&assignment.skill_refs),
                    joined_or_none(&assignment.tool_refs)
                ),
            ),
        )?;

        Ok(())
    }) {
        Ok(label) => label,
        Err(error) => {
            eprintln!("failed to append task result records: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task result recorded");
    println!("  task_id: {}", assignment.task_id);
    println!("  assignment_id: {}", assignment.assignment_id);
    println!("  attempt_id: {}", assignment.attempt_id);
    println!("  worker_node_id: {}", assignment.worker_node_id);
    println!(
        "  implementation_ref: {}",
        assignment.implementation_ref.as_deref().unwrap_or("<none>")
    );
    println!("  skill_refs: {}", joined_or_none(&assignment.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&assignment.tool_refs));
    println!("  status: {}", assignment.status.as_str());
    println!("  update_outcome: {}", transition_outcome_label(outcome));
    println!("  record_write: {records_written}");
    println!("  written_to: {}", output_path.display());
    ExitCode::SUCCESS
}

fn handle_assignment_inspect(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let assignment_id = option_value(args, "--assignment-id").unwrap_or("assignment-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, assignment) = match load_assignment(root, task_id, assignment_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect assignment: {error}");
            return ExitCode::from(1);
        }
    };

    println!("assignment inspect loaded");
    println!("  task_id: {}", assignment.task_id);
    println!("  assignment_id: {}", assignment.assignment_id);
    println!("  attempt_id: {}", assignment.attempt_id);
    println!("  worker_node_id: {}", assignment.worker_node_id);
    println!(
        "  implementation_ref: {}",
        assignment.implementation_ref.as_deref().unwrap_or("<none>")
    );
    println!("  skill_refs: {}", joined_or_none(&assignment.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&assignment.tool_refs));
    println!("  status: {}", assignment.status.as_str());
    println!("  input: {}", assignment.input);
    println!(
        "  output: {}",
        assignment.output.unwrap_or_else(|| "<none>".to_owned())
    );
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_assignment_list(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id");
    let implementation_ref = option_value(args, "--implementation-ref");
    let skill_ref = option_value(args, "--skill-ref");
    let worker_node = option_value(args, "--worker-node");
    let status = option_value(args, "--status");
    let root = option_value(args, "--root").unwrap_or(".");

    let mut assignments = Vec::new();

    if let Some(task_id) = task_id {
        let (_, task_assignments) = match load_task_assignments(root, task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load assignments for task {task_id}: {error}");
                return ExitCode::from(1);
            }
        };
        assignments.extend(task_assignments);
    } else {
        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to list tasks for assignment query: {error}");
                return ExitCode::from(1);
            }
        };

        for task in tasks {
            let (_, task_assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "failed to load assignments for task {}: {error}",
                        task.task_spec.task_id
                    );
                    return ExitCode::from(1);
                }
            };
            assignments.extend(task_assignments);
        }
    }

    assignments.sort_by(|a, b| {
        a.task_id
            .cmp(&b.task_id)
            .then_with(|| a.assignment_id.cmp(&b.assignment_id))
    });

    let filtered = assignments
        .into_iter()
        .filter(|assignment| {
            let implementation_match = implementation_ref.is_none_or(|value| {
                assignment.implementation_ref.as_deref() == Some(value)
            });
            let skill_match = skill_ref.is_none_or(|value| {
                assignment.skill_refs.iter().any(|skill| skill == value)
            });
            let worker_match =
                worker_node.is_none_or(|value| assignment.worker_node_id == value);
            let status_match = status.is_none_or(|value| assignment.status.as_str() == value);
            implementation_match && skill_match && worker_match && status_match
        })
        .collect::<Vec<_>>();

    println!("assignment list loaded");
    println!("  task_id: {}", task_id.unwrap_or("<all>"));
    println!(
        "  implementation_ref: {}",
        implementation_ref.unwrap_or("<none>")
    );
    println!("  skill_ref: {}", skill_ref.unwrap_or("<none>"));
    println!("  worker_node: {}", worker_node.unwrap_or("<none>"));
    println!("  status: {}", status.unwrap_or("<none>"));
    println!("  assignment_count: {}", filtered.len());
    for assignment in filtered {
        println!(
            "  - task={} assignment={} worker={} status={} implementation={} skills={} tools={}",
            assignment.task_id,
            assignment.assignment_id,
            assignment.worker_node_id,
            assignment.status.as_str(),
            assignment.implementation_ref.as_deref().unwrap_or("<none>"),
            joined_or_none(&assignment.skill_refs),
            joined_or_none(&assignment.tool_refs)
        );
    }

    ExitCode::SUCCESS
}

fn handle_task_inspect(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let with_assignments = has_flag(args, "--with-assignments");
    let with_residents = has_flag(args, "--with-residents");
    let with_triggers = has_flag(args, "--with-triggers");

    let (path, record) = match load_task_submission(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect task: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task inspect loaded");
    println!("  schema_version: {}", record.schema_version);
    println!("  task_id: {}", record.task_spec.task_id);
    println!("  tenant_id: {}", record.task_spec.tenant_id);
    println!("  namespace: {}", record.task_spec.namespace);
    println!("  goal: {}", record.task_spec.goal);
    println!(
        "  implementation_ref: {}",
        record.task_spec.implementation_ref.as_deref().unwrap_or("<none>")
    );
    println!(
        "  skill_refs: {}",
        joined_or_none(&record.task_spec.skill_refs)
    );
    println!(
        "  tool_refs: {}",
        joined_or_none(&record.task_spec.tool_refs)
    );
    println!("  queen_node_id: {}", record.task_runtime.queen_node_id);
    println!("  status: {}", record.task_runtime.status.as_str());
    println!("  read_from: {}", path.display());

    if with_assignments {
        match load_task_assignments(root, task_id) {
            Ok((dir, assignments)) => {
                println!("  assignments_dir: {}", dir.display());
                println!("  assignment_count: {}", assignments.len());
                for assignment in assignments {
                    println!(
                        "  - {} worker={} status={} attempt_id={} implementation={} skills={} tools={}",
                        assignment.assignment_id,
                        assignment.worker_node_id,
                        assignment.status.as_str(),
                        assignment.attempt_id,
                        assignment
                            .implementation_ref
                            .as_deref()
                            .unwrap_or("<none>"),
                        joined_or_none(&assignment.skill_refs),
                        joined_or_none(&assignment.tool_refs)
                    );
                }
            }
            Err(error) => {
                eprintln!("failed to load assignments for task inspect: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if with_triggers {
        match list_triggers(root, task_id) {
            Ok((dir, triggers)) => {
                println!("  triggers_dir: {}", dir.display());
                println!("  trigger_count: {}", triggers.len());
                for trigger in triggers {
                    println!(
                        "  - {} type={} status={} schedule={} fire_count={}",
                        trigger.trigger_id,
                        trigger.trigger_type,
                        trigger.status.as_str(),
                        trigger.schedule,
                        trigger.fire_count
                    );
                }
            }
            Err(error) => {
                eprintln!("failed to load triggers for task inspect: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if with_residents {
        match list_residents(root, task_id) {
            Ok((dir, residents)) => {
                println!("  residents_dir: {}", dir.display());
                println!("  resident_count: {}", residents.len());
                for resident in residents {
                    println!(
                        "  - {} worker={} status={} purpose={}",
                        resident.resident_id,
                        resident.worker_node_id,
                        resident.status.as_str(),
                        resident.purpose
                    );
                }
            }
            Err(error) => {
                eprintln!("failed to load residents for task inspect: {error}");
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}

fn handle_task_list(args: &[String]) -> ExitCode {
    let implementation_ref = option_value(args, "--implementation-ref");
    let skill_ref = option_value(args, "--skill-ref");
    let status = option_value(args, "--status");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, tasks) = match crate::storage::list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list tasks: {error}");
            return ExitCode::from(1);
        }
    };

    let filtered = tasks
        .into_iter()
        .filter(|record| {
            let implementation_match = implementation_ref.is_none_or(|value| {
                record.task_spec.implementation_ref.as_deref() == Some(value)
            });
            let skill_match = skill_ref.is_none_or(|value| {
                record
                    .task_spec
                    .skill_refs
                    .iter()
                    .any(|skill| skill == value)
            });
            let status_match =
                status.is_none_or(|value| record.task_runtime.status.as_str() == value);
            implementation_match && skill_match && status_match
        })
        .collect::<Vec<_>>();

    println!("task list loaded");
    println!("  read_from: {}", dir.display());
    println!(
        "  implementation_ref: {}",
        implementation_ref.unwrap_or("<none>")
    );
    println!("  skill_ref: {}", skill_ref.unwrap_or("<none>"));
    println!("  status: {}", status.unwrap_or("<none>"));
    println!("  task_count: {}", filtered.len());
    for record in filtered {
        println!(
            "  - {} status={} implementation={} skills={} tools={} goal={}",
            record.task_spec.task_id,
            record.task_runtime.status.as_str(),
            record.task_spec.implementation_ref.as_deref().unwrap_or("<none>"),
            joined_or_none(&record.task_spec.skill_refs),
            joined_or_none(&record.task_spec.tool_refs),
            record.task_spec.goal
        );
    }

    ExitCode::SUCCESS
}

fn handle_task_backfill_implementation(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id");
    let sync_all = has_flag(args, "--all");
    let root = option_value(args, "--root").unwrap_or(".");

    let task_ids = if sync_all {
        let (_, tasks) = match crate::storage::list_task_submissions(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load tasks for backfill: {error}");
                return ExitCode::from(1);
            }
        };
        tasks.into_iter().map(|task| task.task_spec.task_id).collect::<Vec<_>>()
    } else {
        vec![task_id.unwrap_or("task-demo").to_owned()]
    };

    let mut updated_count = 0usize;
    let mut skipped = Vec::new();

    for task_id in task_ids {
        let (_, existing) = match load_task_submission(root, &task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load task for backfill: {error}");
                return ExitCode::from(1);
            }
        };

        if existing.task_spec.implementation_ref.is_some() {
            skipped.push((task_id, "already has implementation_ref".to_owned()));
            continue;
        }

        let mut selected_implementation = None;
        for skill_ref in &existing.task_spec.skill_refs {
            let (_, skill) = match load_skill(root, skill_ref) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(recommended) = skill.recommended_implementation_id {
                selected_implementation = Some(recommended);
                break;
            }
        }

        let implementation_ref = match selected_implementation {
            Some(value) => value,
            None => {
                skipped.push((task_id, "no recommended implementation on referenced skills".to_owned()));
                continue;
            }
        };

        let (_, record) = match update_task_submission(root, &task_id, |record| {
            record.task_spec.implementation_ref = Some(implementation_ref.clone());
            Ok(())
        }) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to update task for backfill: {error}");
                return ExitCode::from(1);
            }
        };

        let (_, assignments) = match load_task_assignments(root, &task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load assignments for task backfill: {error}");
                return ExitCode::from(1);
            }
        };
        for assignment in assignments {
            if assignment.implementation_ref.is_some() {
                continue;
            }
            if let Err(error) = update_assignment(root, &task_id, &assignment.assignment_id, |assignment| {
                assignment.implementation_ref = Some(implementation_ref.clone());
                Ok(TransitionOutcome::Applied)
            }) {
                eprintln!("failed to update assignment for task backfill: {error}");
                return ExitCode::from(1);
            }
        }

        let timestamp = crate::core::current_timestamp();
        if let Err(error) = append_task_event(
            root,
            &task_id,
            &EventRecord::new(
                format!("event-{task_id}-implementation-backfill"),
                "task_implementation_backfilled".to_owned(),
                task_id.clone(),
                timestamp.clone(),
                format!("implementation={implementation_ref}"),
            ),
        ) {
            eprintln!("failed to append task backfill event: {error}");
            return ExitCode::from(1);
        }
        if let Err(error) = append_task_audit(
            root,
            &task_id,
            &AuditRecord::new(
                format!("audit-{task_id}-implementation-backfill"),
                timestamp.clone(),
                "user".to_owned(),
                "local-cli".to_owned(),
                "task_backfill_implementation".to_owned(),
                "task".to_owned(),
                task_id.clone(),
                task_id.clone(),
                "recorded".to_owned(),
                format!("implementation={implementation_ref}"),
            ),
        ) {
            eprintln!("failed to append task backfill audit: {error}");
            return ExitCode::from(1);
        }
        if let Err(error) = append_task_trace(
            root,
            &task_id,
            &TraceRecord::new(
                format!("trace-{task_id}"),
                format!("span-{task_id}-implementation-backfill"),
                Some(format!("span-{task_id}-submit")),
                timestamp,
                "task_backfill_implementation".to_owned(),
                task_id.clone(),
                "recorded".to_owned(),
                format!("implementation={implementation_ref}"),
            ),
        ) {
            eprintln!("failed to append task backfill trace: {error}");
            return ExitCode::from(1);
        }

        updated_count += 1;
        println!(
            "backfilled: task={} implementation={} status={}",
            record.task_spec.task_id,
            record.task_spec.implementation_ref.as_deref().unwrap_or("<none>"),
            record.task_runtime.status.as_str()
        );
    }

    println!("task backfill implementation completed");
    println!("  mode: {}", if sync_all { "all" } else { "single" });
    println!("  updated_count: {}", updated_count);
    println!("  skipped_count: {}", skipped.len());
    for (task_id, reason) in skipped {
        println!("  skipped: task={} reason={}", task_id, reason);
    }

    ExitCode::SUCCESS
}

fn handle_task_audit_tail(args: &[String]) -> ExitCode {
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
        for task in tasks.into_iter().filter(|task| {
            task.task_spec.implementation_ref.as_deref() == Some(implementation_ref)
        }) {
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

fn handle_task_replay(args: &[String]) -> ExitCode {
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

fn handle_task_trace_tail(args: &[String]) -> ExitCode {
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
        for task in tasks.into_iter().filter(|task| {
            task.task_spec.implementation_ref.as_deref() == Some(implementation_ref)
        }) {
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

fn handle_runtime_overview(args: &[String]) -> ExitCode {
    let with_details = has_flag(args, "--with-details");
    let with_gaps = has_flag(args, "--with-gaps");
    let exclude_legacy = has_flag(args, "--exclude-legacy");
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

    println!("runtime overview loaded");
    println!("  tasks_dir: {}", tasks_dir.display());
    println!("  exclude_legacy: {}", if exclude_legacy { "true" } else { "false" });
    println!("  task_count: {}", tasks.len());
    println!("  completed_task_count: {}", completed_task_count);
    println!("  implementation_bound_task_count: {}", bound_task_count);
    println!("  assignment_count: {}", assignment_count);
    println!("  resident_count: {}", resident_count);
    println!("  trigger_count: {}", trigger_count);
    println!("  audit_count: {}", audit_count);
    println!("  trace_count: {}", trace_count);

    if with_gaps {
        let mut unbound_tasks_no_skill = Vec::new();
        let mut unbound_tasks_missing_recommendation = Vec::new();
        for task in &tasks {
            if task.task_spec.implementation_ref.is_some() {
                continue;
            }
            if task.task_spec.skill_refs.is_empty() {
                unbound_tasks_no_skill.push(task);
            } else {
                unbound_tasks_missing_recommendation.push(task);
            }
        }
        println!(
            "  gap_task_without_implementation_no_skill_count: {}",
            unbound_tasks_no_skill.len()
        );
        for task in unbound_tasks_no_skill {
            println!(
                "  gap_task_without_implementation_no_skill: task={} status={} goal={}",
                task.task_spec.task_id,
                task.task_runtime.status.as_str(),
                task.task_spec.goal
            );
        }
        println!(
            "  gap_task_without_implementation_missing_recommendation_count: {}",
            unbound_tasks_missing_recommendation.len()
        );
        for task in unbound_tasks_missing_recommendation {
            println!(
                "  gap_task_without_implementation_missing_recommendation: task={} status={} skills={} goal={}",
                task.task_spec.task_id,
                task.task_runtime.status.as_str(),
                joined_or_none(&task.task_spec.skill_refs),
                task.task_spec.goal
            );
        }
        println!("  gap_active_task_count: {}", active_task_rows.len());
        println!(
            "  gap_active_task_reason_count: {}",
            active_reason_counts.len()
        );
        for (reason, count) in &active_reason_counts {
            println!("  gap_active_task_reason: reason={} count={}", reason, count);
        }
        for (
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
        ) in &active_task_rows
        {
            println!(
                "  gap_active_task: task={} status={} reason={} assignments={}/{} residents={}/{} triggers={}/{} skills={} tools={}",
                task_id,
                status,
                reason,
                assignment_active,
                assignment_total,
                resident_running,
                resident_total,
                trigger_active,
                trigger_total,
                skills,
                tools
            );
        }
    }

    if with_details {
        let mut implementation_usage_rows = implementation_usage.into_iter().collect::<Vec<_>>();
        implementation_usage_rows.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
        });
        println!(
            "  implementation_usage_detail_count: {}",
            implementation_usage_rows.len()
        );
        for (implementation_ref, usage_count) in implementation_usage_rows {
            println!(
                "  implementation_usage: implementation={} task_count={}",
                implementation_ref, usage_count
            );
        }
        println!("  task_status_detail_count: {}", task_status_counts.len());
        for (status, count) in task_status_counts {
            println!("  task_status: status={} count={}", status, count);
        }
        println!(
            "  active_task_reason_detail_count: {}",
            active_reason_counts.len()
        );
        for (reason, count) in active_reason_counts {
            println!("  active_task_reason: reason={} count={}", reason, count);
        }
        println!(
            "  assignment_status_detail_count: {}",
            assignment_status_counts.len()
        );
        for (status, count) in assignment_status_counts {
            println!("  assignment_status: status={} count={}", status, count);
        }
        println!(
            "  resident_status_detail_count: {}",
            resident_status_counts.len()
        );
        for (status, count) in resident_status_counts {
            println!("  resident_status: status={} count={}", status, count);
        }
        println!(
            "  trigger_status_detail_count: {}",
            trigger_status_counts.len()
        );
        for (status, count) in trigger_status_counts {
            println!("  trigger_status: status={} count={}", status, count);
        }
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::registry::SkillRecord;
    use crate::storage::persist_skill;

    use super::resolve_skill_submission_preset;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-execution-test-{nanos}"))
    }

    #[test]
    fn resolve_skill_submission_preset_uses_skill_defaults() {
        let root = unique_test_root();
        let skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl://xhs/publish/v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec!["xhs_browser_login".to_owned()],
            Some("publish xhs draft".to_owned()),
        );

        persist_skill(&root, &skill).expect("skill should persist");
        let (goal, implementation_ref, skill_refs, tool_refs, source_skill) =
            resolve_skill_submission_preset(
            root.to_str().expect("temp dir should be valid utf-8"),
            Some("xhs_publish"),
            None,
            false,
            vec![],
            vec![],
        )
        .expect("preset should resolve");

        assert_eq!(goal, "publish xhs draft");
        assert_eq!(
            implementation_ref.as_deref(),
            Some("impl://xhs/publish/v1")
        );
        assert_eq!(skill_refs, vec!["xhs_publish"]);
        assert_eq!(tool_refs, vec!["xhs_browser_login"]);
        assert_eq!(
            source_skill.expect("source skill should exist").skill_id,
            "xhs_publish"
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn resolve_skill_submission_preset_keeps_explicit_values() {
        let root = unique_test_root();
        let skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl://xhs/publish/v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec!["xhs_browser_login".to_owned()],
            Some("publish xhs draft".to_owned()),
        );

        persist_skill(&root, &skill).expect("skill should persist");
        let (goal, implementation_ref, skill_refs, tool_refs, _) =
            resolve_skill_submission_preset(
            root.to_str().expect("temp dir should be valid utf-8"),
            Some("xhs_publish"),
            Some("manual goal"),
            false,
            vec!["custom_skill".to_owned()],
            vec![
                "custom_tool".to_owned(),
                "xhs_browser_login".to_owned(),
            ],
        )
        .expect("preset should resolve");

        assert_eq!(goal, "manual goal");
        assert_eq!(
            implementation_ref.as_deref(),
            Some("impl://xhs/publish/v1")
        );
        assert_eq!(
            skill_refs,
            vec!["xhs_publish".to_owned(), "custom_skill".to_owned()]
        );
        assert_eq!(
            tool_refs,
            vec!["xhs_browser_login".to_owned(), "custom_tool".to_owned()]
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn resolve_skill_submission_preset_uses_recommended_implementation_when_requested() {
        let root = unique_test_root();
        let mut skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "Publish a post to Xiaohongshu".to_owned(),
            "impl://xhs/publish/v1".to_owned(),
            "tenant-local".to_owned(),
            "1.0.0".to_owned(),
            vec!["xhs_browser_login".to_owned()],
            Some("publish xhs draft".to_owned()),
        );
        skill.recommended_implementation_id = Some("impl-xhs-v4".to_owned());

        persist_skill(&root, &skill).expect("skill should persist");
        let (_, implementation_ref, _, _, _) = resolve_skill_submission_preset(
            root.to_str().expect("temp dir should be valid utf-8"),
            Some("xhs_publish"),
            None,
            true,
            vec![],
            vec![],
        )
        .expect("preset should resolve");

        assert_eq!(implementation_ref.as_deref(), Some("impl-xhs-v4"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
