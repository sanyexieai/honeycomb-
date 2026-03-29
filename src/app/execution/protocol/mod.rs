use super::*;

pub(crate) fn handle_queen_run(args: &[String]) -> ExitCode {
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

pub(crate) fn handle_worker_run(args: &[String]) -> ExitCode {
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

pub(crate) fn handle_heartbeat_send(args: &[String]) -> ExitCode {
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

pub(crate) fn handle_shutdown_send(args: &[String]) -> ExitCode {
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
