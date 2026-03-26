use std::process::ExitCode;

use crate::protocol::{
    simulate_handshake, simulate_heartbeat, simulate_shutdown, HandshakeTranscript,
    HeartbeatTranscript, QueenEndpoint, ShutdownTranscript,
};
use crate::runtime::{
    Assignment, AssignmentStatus, AuditRecord, EventRecord, TaskRuntime, TaskSpec, TaskStatus,
    TraceRecord, TransitionOutcome,
};
use crate::storage::{
    append_task_audit, append_task_event, append_task_trace, load_assignment,
    load_task_assignments, load_task_audits, load_task_events, load_task_submission,
    load_task_traces, persist_task_submission, update_assignment, update_task_runtime,
};

use super::cli::{
    execute_command, has_flag, option_value, parse_command, BinaryRole, Command,
};

pub(crate) fn handle(command: Command, args: &[String]) -> ExitCode {
    match command {
        Command::QueenRun => handle_queen_run(args),
        Command::WorkerRun => handle_worker_run(args),
        Command::TaskSubmit => handle_task_submit(args),
        Command::TaskDemoFlow => handle_task_demo_flow(args),
        Command::TaskAssign => handle_task_assign(args),
        Command::AssignmentInspect => handle_assignment_inspect(args),
        Command::TaskResult => handle_task_result(args),
        Command::TaskInspect => handle_task_inspect(args),
        Command::TaskReplay => handle_task_replay(args),
        Command::AuditTail => handle_task_audit_tail(args),
        Command::TraceTail => handle_task_trace_tail(args),
        Command::HeartbeatSend => handle_heartbeat_send(args),
        Command::ShutdownSend => handle_shutdown_send(args),
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
    let should_write = match should_write_event_record(root, task_id, "hello_received", &hello_payload) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect existing handshake events: {error}");
            return ExitCode::from(1);
        }
    };
    let mut runtime_outcome = TransitionOutcome::NoOp;
    let record_write = if !should_write
        || (transcript.ack_payload.accepted && task_record.task_runtime.status != TaskStatus::Queued)
    {
        "skipped"
    } else {
        if let Err(error) = persist_handshake_records(root, &transcript) {
            eprintln!("failed to persist handshake records: {error}");
            return ExitCode::from(1);
        }
        if transcript.ack_payload.accepted {
            runtime_outcome = match update_task_runtime(root, task_id, TaskStatus::Running) {
                Ok((_, outcome)) => outcome,
                Err(error) => {
                    eprintln!("failed to update task runtime after handshake: {error}");
                    return ExitCode::from(1);
                }
            };
        }
        "applied"
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
    println!("  runtime_update: {}", transition_outcome_label(runtime_outcome));
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
    let should_write = match should_write_event_record(root, task_id, "heartbeat_received", &heartbeat_payload) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect existing heartbeat events: {error}");
            return ExitCode::from(1);
        }
    };
    let record_write = if should_write {
        if let Err(error) = persist_heartbeat_records(root, &transcript) {
            eprintln!("failed to persist heartbeat records: {error}");
            return ExitCode::from(1);
        }
        "applied"
    } else {
        "skipped"
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
    let should_write = match should_write_event_record(root, task_id, "shutdown_sent", &transcript.payload.reason) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect existing shutdown events: {error}");
            return ExitCode::from(1);
        }
    };
    let (runtime_outcome, record_write) = if task_record.task_runtime.status == TaskStatus::Completed || !should_write {
        (TransitionOutcome::NoOp, "skipped")
    } else {
        if let Err(error) = persist_shutdown_records(root, &transcript) {
            eprintln!("failed to persist shutdown records: {error}");
            return ExitCode::from(1);
        }
        let outcome = match update_task_runtime(root, task_id, TaskStatus::Completed) {
            Ok((_, outcome)) => outcome,
            Err(error) => {
                eprintln!("failed to update task runtime after shutdown: {error}");
                return ExitCode::from(1);
            }
        };
        (outcome, "applied")
    };

    println!("shutdown send recorded");
    println!(
        "  shutdown: {} -> {} reason={}",
        transcript.shutdown.from, transcript.shutdown.to, transcript.payload.reason
    );
    println!("  task_id: {}", transcript.shutdown.task_id);
    println!("  runtime_update: {}", transition_outcome_label(runtime_outcome));
    println!("  record_write: {record_write}");
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
    let goal = option_value(args, "--goal").unwrap_or("bootstrap-task");
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-local");

    let spec = TaskSpec::new(
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        goal.to_owned(),
    );
    let runtime = TaskRuntime::queued(task_id.to_owned(), queen_node_id.to_owned());
    let root = option_value(args, "--root").unwrap_or(".");

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
    println!("  queen_node_id: {}", runtime.queen_node_id);
    println!("  status: {:?}", runtime.status);
    println!("  written_to: {}", output_path.display());

    ExitCode::SUCCESS
}

fn handle_task_demo_flow(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo-flow");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let goal = option_value(args, "--goal").unwrap_or("demo-flow");
    let queen_node_id = option_value(args, "--queen-node").unwrap_or("queen-demo");
    let worker_node_id = option_value(args, "--worker-node").unwrap_or("worker-demo");
    let queen_token = option_value(args, "--queen-token").unwrap_or("queen-token-demo");
    let assignment_id = option_value(args, "--assignment-id").unwrap_or("assignment-demo");
    let attempt_id = option_value(args, "--attempt-id").unwrap_or("attempt-1");
    let input = option_value(args, "--input").unwrap_or("demo-input");
    let output = option_value(args, "--output").unwrap_or("demo-output");
    let root = option_value(args, "--root").unwrap_or(".");

    let steps = vec![
        (
            "task submit",
            vec![
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
            ],
        ),
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

    let assignment = Assignment::assigned(
        assignment_id.to_owned(),
        task_id.to_owned(),
        attempt_id.to_owned(),
        worker_node_id.to_owned(),
        input.to_owned(),
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
            format!("worker={worker_node_id} attempt_id={attempt_id}"),
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
            format!("worker={worker_node_id}"),
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
            format!("attempt_id={attempt_id}"),
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

    let (output_path, assignment, outcome) = match update_assignment(root, task_id, assignment_id, |assignment| {
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
        assignment
            .mark_running()
            .map_err(std::io::Error::other)?;
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

    let records_written = if outcome == TransitionOutcome::Applied {
        if let Err(error) = append_task_event(
            root,
            task_id,
            &EventRecord::new(
                format!("event-{assignment_id}-result"),
                "task_result".to_owned(),
                task_id.to_owned(),
                timestamp.clone(),
                format!("worker={worker_node_id} status={}", status.as_str()),
            ),
        ) {
            eprintln!("failed to append task result event: {error}");
            return ExitCode::from(1);
        }

        if let Err(error) = append_task_audit(
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
                output.to_owned(),
            ),
        ) {
            eprintln!("failed to append task result audit: {error}");
            return ExitCode::from(1);
        }

        if let Err(error) = append_task_trace(
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
                format!("output={output}"),
            ),
        ) {
            eprintln!("failed to append task result trace: {error}");
            return ExitCode::from(1);
        }
        "applied"
    } else {
        "skipped"
    };

    println!("task result recorded");
    println!("  task_id: {}", assignment.task_id);
    println!("  assignment_id: {}", assignment.assignment_id);
    println!("  attempt_id: {}", assignment.attempt_id);
    println!("  worker_node_id: {}", assignment.worker_node_id);
    println!("  status: {}", assignment.status.as_str());
    println!("  update_outcome: {}", transition_outcome_label(outcome));
    println!("  record_write: {records_written}");
    println!("  written_to: {}", output_path.display());
    ExitCode::SUCCESS
}

fn transition_outcome_label(outcome: TransitionOutcome) -> &'static str {
    match outcome {
        TransitionOutcome::Applied => "applied",
        TransitionOutcome::NoOp => "noop",
    }
}

fn should_write_event_record(
    root: &str,
    task_id: &str,
    event_type: &str,
    payload: &str,
) -> std::io::Result<bool> {
    match load_task_events(root, task_id) {
        Ok((_, events)) => Ok(events
            .iter()
            .rev()
            .find(|event| event.event_type == event_type)
            .is_none_or(|event| event.payload != payload)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
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
    println!("  status: {}", assignment.status.as_str());
    println!("  input: {}", assignment.input);
    println!(
        "  output: {}",
        assignment.output.unwrap_or_else(|| "<none>".to_owned())
    );
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_task_inspect(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let with_assignments = has_flag(args, "--with-assignments");

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
                        "  - {} worker={} status={} attempt_id={}",
                        assignment.assignment_id,
                        assignment.worker_node_id,
                        assignment.status.as_str(),
                        assignment.attempt_id
                    );
                }
            }
            Err(error) => {
                eprintln!("failed to load assignments for task inspect: {error}");
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}

fn handle_task_audit_tail(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, audits) = match load_task_audits(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to read task audit: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task audit loaded");
    println!("  task_id: {task_id}");
    println!("  read_from: {}", path.display());
    println!("  audit_count: {}", audits.len());
    for audit in audits {
        println!(
            "  - [{}] {} {} {} -> {} ({})",
            audit.timestamp,
            audit.actor_type,
            audit.actor_id,
            audit.action,
            audit.target_id,
            audit.result
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
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, traces) = match load_task_traces(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to read task trace: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task trace loaded");
    println!("  task_id: {task_id}");
    println!("  read_from: {}", path.display());
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
