use std::process::ExitCode;

use crate::governance::{EvolutionPlan, FitnessReport, GovernanceDecision};
use crate::runtime::AuditRecord;
use crate::storage::{
    append_evolution_audit, list_fitness_runs, list_skills, list_task_submissions, list_tools,
    load_evolution_audits, load_fitness_run, load_skill, load_task_assignments, load_tool,
    persist_fitness_run, update_fitness_plan, update_skill,
};

use super::cli::{BinaryRole, Command, option_value, option_values};

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

fn print_runtime_usage(root: &str, implementation_id: &str) -> std::io::Result<()> {
    let (_, tasks) = list_task_submissions(root)?;
    let matched_tasks = tasks
        .into_iter()
        .filter(|record| record.task_spec.implementation_ref.as_deref() == Some(implementation_id))
        .collect::<Vec<_>>();

    println!("  runtime_task_count: {}", matched_tasks.len());
    for task in matched_tasks {
        let (_, assignments) = load_task_assignments(root, &task.task_spec.task_id)?;
        let matched_assignments = assignments
            .into_iter()
            .filter(|assignment| assignment.implementation_ref.as_deref() == Some(implementation_id))
            .collect::<Vec<_>>();

        println!(
            "  - task={} status={} goal={} assignment_count={}",
            task.task_spec.task_id,
            task.task_runtime.status.as_str(),
            task.task_spec.goal,
            matched_assignments.len()
        );
        for assignment in matched_assignments {
            println!(
                "    assignment={} worker={} status={} skills={} tools={}",
                assignment.assignment_id,
                assignment.worker_node_id,
                assignment.status.as_str(),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            );
        }
    }
    Ok(())
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
        Command::FitnessRun => handle_fitness_run(args),
        Command::FitnessExplain => handle_fitness_explain(args),
        Command::AuditTail => handle_evolution_audit_tail(args),
        Command::LineageShow => handle_lineage_show(args),
        Command::GovernancePlan => handle_governance_plan(args),
        Command::GovernanceApply => handle_governance_apply(args),
        Command::RegistrySync => handle_registry_sync(args),
        Command::RegistryOverview => handle_registry_overview(args),
        other => {
            println!(
                "{} command scaffold: {}",
                BinaryRole::Evolution.binary_name(),
                super::cli::command_name(&other)
            );
            ExitCode::SUCCESS
        }
    }
}

fn handle_fitness_run(args: &[String]) -> ExitCode {
    let implementation_id = option_value(args, "--implementation").unwrap_or("impl-demo");
    let score = option_value(args, "--score").unwrap_or("0.80");
    let summary = option_value(args, "--summary").unwrap_or("initial fitness report");
    let skill_refs = option_values(args, "--skill-ref");
    let tool_refs = option_values(args, "--tool-ref");
    let root = option_value(args, "--root").unwrap_or(".");

    if let Err(error) = validate_registry_refs(root, &skill_refs, &tool_refs) {
        eprintln!("failed to validate fitness capability refs: {error}");
        return ExitCode::from(1);
    }

    let report = FitnessReport::new(
        implementation_id.to_owned(),
        score.to_owned(),
        summary.to_owned(),
        skill_refs,
        tool_refs,
    );
    let plan = EvolutionPlan::observe(
        implementation_id.to_owned(),
        "default to observe until governance thresholds are wired".to_owned(),
    );

    let output_path = match persist_fitness_run(root, &report, &plan) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist fitness run: {error}");
            return ExitCode::from(1);
        }
    };

    println!("fitness run completed");
    println!("  implementation_id: {}", report.implementation_id);
    println!("  score: {}", report.score);
    println!("  summary: {}", report.summary);
    println!("  skill_refs: {}", joined_or_none(&report.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&report.tool_refs));
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    println!("  written_to: {}", output_path.display());

    ExitCode::SUCCESS
}

fn handle_fitness_explain(args: &[String]) -> ExitCode {
    let implementation_id = option_value(args, "--implementation").unwrap_or("impl-demo");
    let with_runtime = args.iter().any(|arg| arg == "--with-runtime");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, record) = match load_fitness_run(root, implementation_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to explain fitness run: {error}");
            return ExitCode::from(1);
        }
    };

    println!("fitness explain loaded");
    println!("  schema_version: {}", record.schema_version);
    println!(
        "  implementation_id: {}",
        record.fitness_report.implementation_id
    );
    println!("  score: {}", record.fitness_report.score);
    println!("  summary: {}", record.fitness_report.summary);
    println!(
        "  skill_refs: {}",
        joined_or_none(&record.fitness_report.skill_refs)
    );
    println!(
        "  tool_refs: {}",
        joined_or_none(&record.fitness_report.tool_refs)
    );
    println!("  decision: {}", record.evolution_plan.decision.as_str());
    println!("  rationale: {}", record.evolution_plan.rationale);
    println!("  read_from: {}", path.display());
    if with_runtime {
        if let Err(error) = print_runtime_usage(root, &record.fitness_report.implementation_id) {
            eprintln!("failed to load runtime usage for fitness explain: {error}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

fn handle_evolution_audit_tail(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, audits) = match load_evolution_audits(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to read evolution audit: {error}");
            return ExitCode::from(1);
        }
    };

    println!("evolution audit loaded");
    println!("  read_from: {}", path.display());
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

fn handle_lineage_show(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");
    let with_runtime = args.iter().any(|arg| arg == "--with-runtime");

    let (dir, records) = match list_fitness_runs(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list fitness lineage: {error}");
            return ExitCode::from(1);
        }
    };

    let filtered = records
        .into_iter()
        .filter(|record| {
            let skill_match = skill_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|skill| skill == value)
            });
            let tool_match = tool_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .tool_refs
                    .iter()
                    .any(|tool| tool == value)
            });
            skill_match && tool_match
        })
        .collect::<Vec<_>>();

    println!("lineage show loaded");
    println!("  read_from: {}", dir.display());
    println!("  skill_ref: {}", skill_ref.unwrap_or("<none>"));
    println!("  tool_ref: {}", tool_ref.unwrap_or("<none>"));
    println!("  record_count: {}", filtered.len());
    for record in filtered {
        println!(
            "  - {} score={} decision={} skills={} tools={}",
            record.fitness_report.implementation_id,
            record.fitness_report.score,
            record.evolution_plan.decision.as_str(),
            joined_or_none(&record.fitness_report.skill_refs),
            joined_or_none(&record.fitness_report.tool_refs)
        );
        if with_runtime {
            if let Err(error) = print_runtime_usage(root, &record.fitness_report.implementation_id) {
                eprintln!("failed to load runtime usage for lineage show: {error}");
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}

fn parse_score(score: &str) -> f64 {
    score.parse::<f64>().unwrap_or(0.0)
}

fn recommended_decision(score: f64) -> GovernanceDecision {
    if score >= 0.95 {
        GovernanceDecision::Promote
    } else if score >= 0.85 {
        GovernanceDecision::Hold
    } else {
        GovernanceDecision::Observe
    }
}

fn decision_rank(decision: GovernanceDecision) -> u8 {
    match decision {
        GovernanceDecision::Promote => 3,
        GovernanceDecision::Hold => 2,
        GovernanceDecision::Observe => 1,
        GovernanceDecision::Deprecate => 0,
    }
}

fn build_plan_for_record(record: &crate::governance::FitnessRecord) -> EvolutionPlan {
    let score = parse_score(&record.fitness_report.score);
    let decision = recommended_decision(score);
    let rationale = format!(
        "score={} skills={} tools={}",
        record.fitness_report.score,
        joined_or_none(&record.fitness_report.skill_refs),
        joined_or_none(&record.fitness_report.tool_refs)
    );

    EvolutionPlan {
        implementation_id: record.fitness_report.implementation_id.clone(),
        decision,
        rationale,
    }
}

fn select_governance_candidate(
    root: &str,
    implementation_id: Option<&str>,
    skill_ref: Option<&str>,
    tool_ref: Option<&str>,
) -> std::io::Result<Option<crate::governance::FitnessRecord>> {
    let (_, records) = list_fitness_runs(root)?;
    let mut filtered = records
        .into_iter()
        .filter(|record| {
            let impl_match = implementation_id
                .is_none_or(|value| record.fitness_report.implementation_id == value);
            let skill_match = skill_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|skill| skill == value)
            });
            let tool_match = tool_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .tool_refs
                    .iter()
                    .any(|tool| tool == value)
            });
            impl_match && skill_match && tool_match
        })
        .collect::<Vec<_>>();

    filtered.sort_by(|a, b| {
        parse_score(&b.fitness_report.score)
            .partial_cmp(&parse_score(&a.fitness_report.score))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.fitness_report
                    .implementation_id
                    .cmp(&b.fitness_report.implementation_id)
            })
    });

    Ok(filtered.into_iter().next())
}

fn select_registry_sync_candidate(
    root: &str,
    skill_id: &str,
) -> std::io::Result<Option<crate::governance::FitnessRecord>> {
    let (_, records) = list_fitness_runs(root)?;
    let mut filtered = records
        .into_iter()
        .filter(|record| {
            record
                .fitness_report
                .skill_refs
                .iter()
                .any(|skill| skill == skill_id)
        })
        .collect::<Vec<_>>();

    filtered.sort_by(|a, b| {
        decision_rank(b.evolution_plan.decision)
            .cmp(&decision_rank(a.evolution_plan.decision))
            .then_with(|| {
                parse_score(&b.fitness_report.score)
                    .partial_cmp(&parse_score(&a.fitness_report.score))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.fitness_report
                    .implementation_id
                    .cmp(&b.fitness_report.implementation_id)
            })
    });

    Ok(filtered.into_iter().next())
}

fn handle_governance_plan(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let implementation_id = option_value(args, "--implementation");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");

    let candidate = match select_governance_candidate(root, implementation_id, skill_ref, tool_ref)
    {
        Ok(Some(record)) => record,
        Ok(None) => {
            eprintln!("failed to build governance plan: no matching fitness record");
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("failed to build governance plan: {error}");
            return ExitCode::from(1);
        }
    };
    let plan = build_plan_for_record(&candidate);

    println!("governance plan generated");
    println!(
        "  implementation_id: {}",
        candidate.fitness_report.implementation_id
    );
    println!("  score: {}", candidate.fitness_report.score);
    println!(
        "  skill_refs: {}",
        joined_or_none(&candidate.fitness_report.skill_refs)
    );
    println!(
        "  tool_refs: {}",
        joined_or_none(&candidate.fitness_report.tool_refs)
    );
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    ExitCode::SUCCESS
}

fn handle_governance_apply(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let implementation_id = option_value(args, "--implementation");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");

    let candidate = match select_governance_candidate(root, implementation_id, skill_ref, tool_ref)
    {
        Ok(Some(record)) => record,
        Ok(None) => {
            eprintln!("failed to apply governance plan: no matching fitness record");
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("failed to apply governance plan: {error}");
            return ExitCode::from(1);
        }
    };
    let plan = build_plan_for_record(&candidate);

    let (path, record) = match update_fitness_plan(root, &plan.implementation_id, &plan) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to persist governance plan: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_evolution_audit(
        root,
        &AuditRecord::now(
            format!("audit-{}-governance-apply", plan.implementation_id),
            "user".to_owned(),
            "local-cli".to_owned(),
            "governance_apply".to_owned(),
            "implementation".to_owned(),
            plan.implementation_id.clone(),
            String::new(),
            plan.decision.as_str().to_owned(),
            format!(
                "score={} skills={} tools={}",
                record.fitness_report.score,
                joined_or_none(&record.fitness_report.skill_refs),
                joined_or_none(&record.fitness_report.tool_refs)
            ),
        ),
    ) {
        eprintln!("failed to append governance audit: {error}");
        return ExitCode::from(1);
    }

    println!("governance apply completed");
    println!("  implementation_id: {}", plan.implementation_id);
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_registry_sync(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let sync_all = args.iter().any(|arg| arg == "--all");

    let skill_ids = if sync_all {
        let (_, skills) = match list_skills(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load skills for registry sync: {error}");
                return ExitCode::from(1);
            }
        };
        skills.into_iter().map(|skill| skill.skill_id).collect::<Vec<_>>()
    } else {
        vec![
            option_value(args, "--skill-id")
                .unwrap_or("xhs_publish")
                .to_owned(),
        ]
    };

    let mut synced = Vec::new();
    let mut skipped = Vec::new();

    for skill_id in skill_ids {
        if let Err(error) = load_skill(root, &skill_id) {
            eprintln!("failed to load skill for registry sync: {error}");
            return ExitCode::from(1);
        }

        let candidate = match select_registry_sync_candidate(root, &skill_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                skipped.push((skill_id, "no lineage candidate".to_owned()));
                continue;
            }
            Err(error) => {
                eprintln!("failed to sync registry: {error}");
                return ExitCode::from(1);
            }
        };

        let timestamp = crate::core::current_timestamp();
        let (path, skill) = match update_skill(root, &skill_id, |skill| {
            skill.recommended_implementation_id =
                Some(candidate.fitness_report.implementation_id.clone());
            skill.governance_decision = Some(candidate.evolution_plan.decision);
            skill.last_synced_at = Some(timestamp.clone());
            Ok(())
        }) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to update skill registry: {error}");
                return ExitCode::from(1);
            }
        };

        if let Err(error) = append_evolution_audit(
            root,
            &AuditRecord::new(
                format!("audit-{}-registry-sync", skill.skill_id),
                timestamp,
                "user".to_owned(),
                "local-cli".to_owned(),
                "registry_sync".to_owned(),
                "skill".to_owned(),
                skill.skill_id.clone(),
                String::new(),
                skill
                    .governance_decision
                    .map(|decision| decision.as_str().to_owned())
                    .unwrap_or_else(|| "unknown".to_owned()),
                format!(
                    "recommended_implementation={} source_score={}",
                    skill
                        .recommended_implementation_id
                        .clone()
                        .unwrap_or_else(|| "<none>".to_owned()),
                    candidate.fitness_report.score
                ),
            ),
        ) {
            eprintln!("failed to append registry sync audit: {error}");
            return ExitCode::from(1);
        }

        synced.push((path, skill));
    }

    println!("registry sync completed");
    println!("  mode: {}", if sync_all { "all" } else { "single" });
    println!("  synced_count: {}", synced.len());
    for (path, skill) in synced {
        println!(
            "  synced: skill={} recommended_implementation_id={} governance_decision={} last_synced_at={} written_to={}",
            skill.skill_id,
            skill
                .recommended_implementation_id
                .as_deref()
                .unwrap_or("<none>"),
            skill
                .governance_decision
                .map(|decision| decision.as_str())
                .unwrap_or("<none>"),
            skill.last_synced_at.as_deref().unwrap_or("<none>"),
            path.display()
        );
    }
    println!("  skipped_count: {}", skipped.len());
    for (skill_id, reason) in skipped {
        println!("  skipped: skill={} reason={}", skill_id, reason);
    }
    ExitCode::SUCCESS
}

fn handle_registry_overview(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let with_details = args.iter().any(|arg| arg == "--with-details");
    let with_gaps = args.iter().any(|arg| arg == "--with-gaps");
    let exclude_legacy = args.iter().any(|arg| arg == "--exclude-legacy");

    let (skills_dir, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skills for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (tools_dir, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tools for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (fitness_dir, fitness_runs) = match list_fitness_runs(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load fitness runs for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (tasks_dir, all_tasks) = match list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tasks for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let tasks = all_tasks
        .into_iter()
        .filter(|task| !exclude_legacy || !is_legacy_demo_task(task))
        .collect::<Vec<_>>();

    let recommended_skill_count = skills
        .iter()
        .filter(|skill| skill.recommended_implementation_id.is_some())
        .count();
    let promoted_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Promote)
        .count();
    let hold_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Hold)
        .count();
    let observe_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Observe)
        .count();
    let deprecated_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Deprecate)
        .count();

    let task_count = tasks.len();
    let completed_task_count = tasks
        .iter()
        .filter(|task| task.task_runtime.status.as_str() == "completed")
        .count();
    let implementation_bound_task_count = tasks
        .iter()
        .filter(|task| task.task_spec.implementation_ref.is_some())
        .count();

    let mut assignment_count = 0usize;
    let mut completed_assignment_count = 0usize;
    let mut implementation_usage = std::collections::BTreeMap::<String, usize>::new();
    let mut skill_usage = std::collections::BTreeMap::<String, usize>::new();
    let mut tool_usage = std::collections::BTreeMap::<String, usize>::new();
    for task in &tasks {
        if let Some(implementation_ref) = &task.task_spec.implementation_ref {
            *implementation_usage
                .entry(implementation_ref.clone())
                .or_insert(0) += 1;
        }
        for skill_ref in &task.task_spec.skill_refs {
            *skill_usage.entry(skill_ref.clone()).or_insert(0) += 1;
        }
        for tool_ref in &task.task_spec.tool_refs {
            *tool_usage.entry(tool_ref.clone()).or_insert(0) += 1;
        }
        let (_, assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load assignments for registry overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        assignment_count += assignments.len();
        completed_assignment_count += assignments
            .iter()
            .filter(|assignment| assignment.status.as_str() == "completed")
            .count();
    }

    println!("registry overview loaded");
    println!("  skills_dir: {}", skills_dir.display());
    println!("  tools_dir: {}", tools_dir.display());
    println!("  fitness_dir: {}", fitness_dir.display());
    println!("  tasks_dir: {}", tasks_dir.display());
    println!("  exclude_legacy: {}", if exclude_legacy { "true" } else { "false" });
    println!("  skill_count: {}", skills.len());
    println!("  skill_with_recommendation_count: {}", recommended_skill_count);
    println!("  tool_count: {}", tools.len());
    println!("  fitness_count: {}", fitness_runs.len());
    println!("  fitness_promote_count: {}", promoted_count);
    println!("  fitness_hold_count: {}", hold_count);
    println!("  fitness_observe_count: {}", observe_count);
    println!("  fitness_deprecate_count: {}", deprecated_count);
    println!("  task_count: {}", task_count);
    println!("  completed_task_count: {}", completed_task_count);
    println!(
        "  implementation_bound_task_count: {}",
        implementation_bound_task_count
    );
    println!("  assignment_count: {}", assignment_count);
    println!(
        "  completed_assignment_count: {}",
        completed_assignment_count
    );

    if with_gaps {
        let unrecommended_skills = skills
            .iter()
            .filter(|skill| skill.recommended_implementation_id.is_none())
            .map(|skill| skill.skill_id.as_str())
            .collect::<Vec<_>>();
        let mut unbound_tasks_no_skill = Vec::new();
        let mut unbound_tasks_missing_recommendation = Vec::new();
        for task in &tasks {
            if task.task_spec.implementation_ref.is_some() {
                continue;
            }
            if task.task_spec.skill_refs.is_empty() {
                unbound_tasks_no_skill.push(task.task_spec.task_id.as_str());
                continue;
            }
            let has_recommended_skill = task.task_spec.skill_refs.iter().any(|skill_ref| {
                skills.iter().any(|skill| {
                    skill.skill_id == *skill_ref && skill.recommended_implementation_id.is_some()
                })
            });
            if has_recommended_skill {
                continue;
            }
            unbound_tasks_missing_recommendation.push(task.task_spec.task_id.as_str());
        }

        println!("  gap_skill_without_recommendation_count: {}", unrecommended_skills.len());
        for skill_id in unrecommended_skills {
            println!("  gap_skill_without_recommendation: skill={skill_id}");
        }
        println!(
            "  gap_task_without_implementation_no_skill_count: {}",
            unbound_tasks_no_skill.len()
        );
        for task_id in unbound_tasks_no_skill {
            println!("  gap_task_without_implementation_no_skill: task={task_id}");
        }
        println!(
            "  gap_task_without_implementation_missing_recommendation_count: {}",
            unbound_tasks_missing_recommendation.len()
        );
        for task_id in unbound_tasks_missing_recommendation {
            println!(
                "  gap_task_without_implementation_missing_recommendation: task={task_id}"
            );
        }
    }

    if with_details {
        let recommended_skills = skills
            .iter()
            .filter_map(|skill| {
                skill.recommended_implementation_id.as_ref().map(|implementation| {
                    (
                        skill.skill_id.as_str(),
                        implementation.as_str(),
                        skill
                            .governance_decision
                            .map(|decision| decision.as_str())
                            .unwrap_or("<none>"),
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut implementation_usage_rows = implementation_usage.into_iter().collect::<Vec<_>>();
        implementation_usage_rows.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
        });
        let mut skill_usage_rows = skill_usage.into_iter().collect::<Vec<_>>();
        skill_usage_rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut tool_usage_rows = tool_usage.into_iter().collect::<Vec<_>>();
        tool_usage_rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        println!("  recommended_skill_detail_count: {}", recommended_skills.len());
        for (skill_id, implementation_id, decision) in recommended_skills {
            println!(
                "  recommended_skill: skill={} implementation={} decision={}",
                skill_id, implementation_id, decision
            );
        }

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
        println!("  skill_usage_detail_count: {}", skill_usage_rows.len());
        for (skill_id, usage_count) in skill_usage_rows {
            println!("  skill_usage: skill={} task_count={}", skill_id, usage_count);
        }
        println!("  tool_usage_detail_count: {}", tool_usage_rows.len());
        for (tool_id, usage_count) in tool_usage_rows {
            println!("  tool_usage: tool={} task_count={}", tool_id, usage_count);
        }
    }

    ExitCode::SUCCESS
}
