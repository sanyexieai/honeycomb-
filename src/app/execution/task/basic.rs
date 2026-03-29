use super::super::common_support::joined_or_none;
use super::super::*;
use crate::runtime::ImplementationSnapshot;

fn merge_unique(primary: Vec<String>, secondary: Vec<String>) -> Vec<String> {
    let mut merged = primary;
    for value in secondary {
        if !merged.iter().any(|existing| existing == &value) {
            merged.push(value);
        }
    }
    merged
}

fn validate_task_implementation_ref(
    root: &str,
    implementation_ref: Option<&str>,
    source_skill: Option<&SkillRecord>,
    skill_refs: &[String],
) -> std::io::Result<()> {
    let Some(implementation_id) = implementation_ref else {
        return Ok(());
    };

    let (_, implementation) = load_implementation(root, implementation_id)?;
    if let Some(skill) = source_skill {
        if implementation.skill_id != skill.skill_id {
            return Err(std::io::Error::other(format!(
                "implementation {} belongs to skill {}, expected {}",
                implementation.implementation_id, implementation.skill_id, skill.skill_id
            )));
        }
        return Ok(());
    }

    if !skill_refs.is_empty()
        && !skill_refs
            .iter()
            .any(|skill_id| skill_id == &implementation.skill_id)
    {
        return Err(std::io::Error::other(format!(
            "implementation {} belongs to skill {}, which is missing from task skill_refs",
            implementation.implementation_id, implementation.skill_id
        )));
    }

    Ok(())
}

fn load_task_implementation_snapshot(
    root: &str,
    implementation_ref: Option<&str>,
) -> std::io::Result<Option<ImplementationSnapshot>> {
    let Some(implementation_id) = implementation_ref else {
        return Ok(None);
    };
    let (_, implementation) = load_implementation(root, implementation_id)?;
    Ok(Some(ImplementationSnapshot {
        implementation_id: implementation.implementation_id,
        skill_id: implementation.skill_id,
        executor: implementation.executor,
        entry_kind: implementation.entry.kind,
        entry_path: implementation.entry.path,
        strategy_mode: implementation.strategy.get("mode").cloned(),
        prompt_component: implementation.components.get("prompt").cloned(),
        config_component: implementation.components.get("config").cloned(),
        max_cost: implementation.constraints.get("max_cost").cloned(),
        max_latency_ms: implementation.constraints.get("max_latency_ms").cloned(),
    }))
}

pub(crate) fn resolve_skill_submission_preset(
    root: &str,
    from_skill: Option<&str>,
    explicit_goal: Option<&str>,
    use_recommended_impl: bool,
    explicit_skill_refs: Vec<String>,
    explicit_tool_refs: Vec<String>,
) -> std::io::Result<(
    String,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Option<SkillRecord>,
)> {
    if let Some(skill_id) = from_skill {
        let (_, skill) = load_skill(root, skill_id)?;
        validate_skill_implementation_refs(root, &skill)?;
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

pub(crate) fn validate_registry_refs(
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

pub(crate) fn handle_task_submit(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let tenant_id = option_value(args, "--tenant").unwrap_or("tenant-demo");
    let namespace = option_value(args, "--namespace").unwrap_or("user/demo");
    let goal = option_value(args, "--goal");
    let from_skill = option_value(args, "--from-skill");
    let use_recommended_impl = has_flag(args, "--use-recommended-impl");
    let explicit_implementation_ref = option_value(args, "--implementation-ref").map(str::to_owned);
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
    if let Err(error) = validate_task_implementation_ref(
        root,
        implementation_ref.as_deref(),
        source_skill.as_ref(),
        &skill_refs,
    ) {
        eprintln!("failed to validate task implementation ref: {error}");
        return ExitCode::from(1);
    }

    let implementation_snapshot =
        match load_task_implementation_snapshot(root, implementation_ref.as_deref()) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load task implementation snapshot: {error}");
                return ExitCode::from(1);
            }
        };

    let spec = TaskSpec::new(
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        goal.clone(),
        implementation_ref,
        skill_refs,
        tool_refs,
    )
    .with_implementation_snapshot(implementation_snapshot);
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
    if let Some(snapshot) = &spec.implementation_snapshot {
        println!(
            "  implementation_snapshot: implementation={} skill={} executor={} entry={}:{} mode={} max_cost={} max_latency_ms={}",
            snapshot.implementation_id,
            snapshot.skill_id,
            snapshot.executor,
            snapshot.entry_kind,
            snapshot.entry_path,
            snapshot.strategy_mode.as_deref().unwrap_or("<none>"),
            snapshot.max_cost.as_deref().unwrap_or("<none>"),
            snapshot.max_latency_ms.as_deref().unwrap_or("<none>")
        );
    }
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

pub(crate) fn handle_task_demo_flow(args: &[String]) -> ExitCode {
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
    if let Err(error) = validate_task_implementation_ref(
        root,
        implementation_ref.as_deref(),
        source_skill.as_ref(),
        &skill_refs,
    ) {
        eprintln!("failed to validate demo-flow implementation ref: {error}");
        return ExitCode::from(1);
    }

    let implementation_snapshot =
        match load_task_implementation_snapshot(root, implementation_ref.as_deref()) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load demo-flow implementation snapshot: {error}");
                return ExitCode::from(1);
            }
        };

    let _spec = TaskSpec::new(
        task_id.to_owned(),
        tenant_id.to_owned(),
        namespace.to_owned(),
        goal.to_owned(),
        implementation_ref.clone(),
        skill_refs.clone(),
        tool_refs.clone(),
    )
    .with_implementation_snapshot(implementation_snapshot);

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

pub(crate) fn handle_task_assign(args: &[String]) -> ExitCode {
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
        task_record.task_spec.implementation_snapshot.clone(),
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
                assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
                assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
                assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
    if let Some(snapshot) = &assignment.implementation_snapshot {
        println!(
            "  implementation_snapshot: implementation={} skill={} executor={} entry={}:{} mode={} max_cost={} max_latency_ms={}",
            snapshot.implementation_id,
            snapshot.skill_id,
            snapshot.executor,
            snapshot.entry_kind,
            snapshot.entry_path,
            snapshot.strategy_mode.as_deref().unwrap_or("<none>"),
            snapshot.max_cost.as_deref().unwrap_or("<none>"),
            snapshot.max_latency_ms.as_deref().unwrap_or("<none>")
        );
    }
    println!("  skill_refs: {}", joined_or_none(&assignment.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&assignment.tool_refs));
    println!("  status: {}", assignment.status.as_str());
    println!("  written_to: {}", output_path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_task_result(args: &[String]) -> ExitCode {
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
                    assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
                    assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
                    assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
    if let Some(snapshot) = &assignment.implementation_snapshot {
        println!(
            "  implementation_snapshot: implementation={} skill={} executor={} entry={}:{} mode={} max_cost={} max_latency_ms={}",
            snapshot.implementation_id,
            snapshot.skill_id,
            snapshot.executor,
            snapshot.entry_kind,
            snapshot.entry_path,
            snapshot.strategy_mode.as_deref().unwrap_or("<none>"),
            snapshot.max_cost.as_deref().unwrap_or("<none>"),
            snapshot.max_latency_ms.as_deref().unwrap_or("<none>")
        );
    }
    println!("  skill_refs: {}", joined_or_none(&assignment.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&assignment.tool_refs));
    println!("  status: {}", assignment.status.as_str());
    println!("  update_outcome: {}", transition_outcome_label(outcome));
    println!("  record_write: {records_written}");
    println!("  written_to: {}", output_path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_assignment_inspect(args: &[String]) -> ExitCode {
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

pub(crate) fn handle_assignment_list(args: &[String]) -> ExitCode {
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
            let implementation_match = implementation_ref
                .is_none_or(|value| assignment.implementation_ref.as_deref() == Some(value));
            let skill_match = skill_ref
                .is_none_or(|value| assignment.skill_refs.iter().any(|skill| skill == value));
            let worker_match = worker_node.is_none_or(|value| assignment.worker_node_id == value);
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
            "  - task={} assignment={} worker={} status={} implementation={} implementation_skill={} implementation_executor={} skills={} tools={}",
            assignment.task_id,
            assignment.assignment_id,
            assignment.worker_node_id,
            assignment.status.as_str(),
            assignment.implementation_ref.as_deref().unwrap_or("<none>"),
            assignment
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.skill_id.as_str())
                .unwrap_or("<none>"),
            assignment
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.executor.as_str())
                .unwrap_or("<none>"),
            joined_or_none(&assignment.skill_refs),
            joined_or_none(&assignment.tool_refs)
        );
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_task_inspect(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");
    let with_assignments = has_flag(args, "--with-assignments");
    let with_residents = has_flag(args, "--with-residents");
    let with_triggers = has_flag(args, "--with-triggers");
    let with_executions = has_flag(args, "--with-executions");

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
        record
            .task_spec
            .implementation_ref
            .as_deref()
            .unwrap_or("<none>")
    );
    if let Some(snapshot) = &record.task_spec.implementation_snapshot {
        println!(
            "  implementation_snapshot: implementation={} skill={} executor={} entry={}:{} mode={} max_cost={} max_latency_ms={}",
            snapshot.implementation_id,
            snapshot.skill_id,
            snapshot.executor,
            snapshot.entry_kind,
            snapshot.entry_path,
            snapshot.strategy_mode.as_deref().unwrap_or("<none>"),
            snapshot.max_cost.as_deref().unwrap_or("<none>"),
            snapshot.max_latency_ms.as_deref().unwrap_or("<none>")
        );
    }
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
                        assignment.implementation_ref.as_deref().unwrap_or("<none>"),
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
                        "  - {} type={} status={} schedule={} fire_count={} consumed_fire_count={}",
                        trigger.trigger_id,
                        trigger.trigger_type,
                        trigger.status.as_str(),
                        trigger.schedule,
                        trigger.fire_count,
                        trigger.consumed_fire_count
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

    if with_executions {
        match list_execution_records(root) {
            Ok((dir, executions)) => {
                let matched = executions
                    .into_iter()
                    .filter(|record| record.task_id.as_deref() == Some(task_id))
                    .collect::<Vec<_>>();
                println!("  executions_dir: {}", dir.display());
                println!("  execution_count: {}", matched.len());
                for record in matched {
                    println!(
                        "  - {} kind={} target={} assignment={} implementation={} runner={} status={}",
                        record.execution_id,
                        record.kind.as_str(),
                        record.target_id,
                        record.assignment_id.as_deref().unwrap_or("<none>"),
                        record.implementation_ref.as_deref().unwrap_or("<none>"),
                        record.runner,
                        record.status.as_str()
                    );
                }
            }
            Err(error) => {
                eprintln!("failed to load executions for task inspect: {error}");
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}

pub(crate) fn handle_task_list(args: &[String]) -> ExitCode {
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
            let implementation_match = implementation_ref
                .is_none_or(|value| record.task_spec.implementation_ref.as_deref() == Some(value));
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
            "  - {} status={} implementation={} implementation_skill={} implementation_executor={} skills={} tools={} goal={}",
            record.task_spec.task_id,
            record.task_runtime.status.as_str(),
            record
                .task_spec
                .implementation_ref
                .as_deref()
                .unwrap_or("<none>"),
            record
                .task_spec
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.skill_id.as_str())
                .unwrap_or("<none>"),
            record
                .task_spec
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.executor.as_str())
                .unwrap_or("<none>"),
            joined_or_none(&record.task_spec.skill_refs),
            joined_or_none(&record.task_spec.tool_refs),
            record.task_spec.goal
        );
    }

    ExitCode::SUCCESS
}

pub(crate) fn reopen_task_internal(
    root: &str,
    task_id: &str,
) -> std::io::Result<(PathBuf, crate::runtime::TaskRecord)> {
    let (_, assignments) = load_task_assignments(root, task_id)?;

    if assignments.iter().any(|assignment| {
        matches!(
            assignment.status,
            AssignmentStatus::Created
                | AssignmentStatus::Assigned
                | AssignmentStatus::Running
                | AssignmentStatus::RetryPending
        )
    }) {
        return Err(std::io::Error::other("task has active assignments"));
    }

    let timestamp = crate::core::current_timestamp();
    let (path, record) =
        update_task_submission(root, task_id, |record| match record.task_runtime.status {
            TaskStatus::Completed
            | TaskStatus::Failed
            | TaskStatus::Cancelled
            | TaskStatus::Interrupted => {
                record.task_runtime.status = TaskStatus::Queued;
                Ok(())
            }
            TaskStatus::Queued => Ok(()),
            TaskStatus::Running => Err(std::io::Error::other("task_running_cannot_reopen")),
        })?;

    append_task_event(
        root,
        task_id,
        &EventRecord::new(
            format!("event-{task_id}-reopen"),
            "task_reopened".to_owned(),
            task_id.to_owned(),
            timestamp.clone(),
            format!("status={}", record.task_runtime.status.as_str()),
        ),
    )?;
    append_task_audit(
        root,
        task_id,
        &AuditRecord::new(
            format!("audit-{task_id}-reopen"),
            timestamp.clone(),
            "user".to_owned(),
            "local-cli".to_owned(),
            "task_reopen".to_owned(),
            "task".to_owned(),
            task_id.to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!("status={}", record.task_runtime.status.as_str()),
        ),
    )?;
    append_task_trace(
        root,
        task_id,
        &TraceRecord::new(
            format!("trace-{task_id}"),
            format!("span-{task_id}-reopen"),
            Some(format!("span-{task_id}-submit")),
            timestamp,
            "task_reopen".to_owned(),
            task_id.to_owned(),
            "recorded".to_owned(),
            format!("status={}", record.task_runtime.status.as_str()),
        ),
    )?;

    Ok((path, record))
}

pub(crate) fn handle_task_reopen(args: &[String]) -> ExitCode {
    let task_id = option_value(args, "--task-id").unwrap_or("task-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, record) = match reopen_task_internal(root, task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to reopen task: {error}");
            return ExitCode::from(1);
        }
    };

    println!("task reopen recorded");
    println!("  task_id: {}", record.task_spec.task_id);
    println!("  status: {}", record.task_runtime.status.as_str());
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}
