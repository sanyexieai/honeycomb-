use std::process::ExitCode;

use crate::governance::{EvolutionPlan, FitnessReport};
use crate::storage::{load_evolution_audits, load_fitness_run, persist_fitness_run};

use super::cli::{BinaryRole, Command, option_value};

pub(crate) fn handle(command: Command, args: &[String]) -> ExitCode {
    match command {
        Command::FitnessRun => handle_fitness_run(args),
        Command::FitnessExplain => handle_fitness_explain(args),
        Command::AuditTail => handle_evolution_audit_tail(args),
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

    let report = FitnessReport::new(
        implementation_id.to_owned(),
        score.to_owned(),
        summary.to_owned(),
    );
    let plan = EvolutionPlan::observe(
        implementation_id.to_owned(),
        "default to observe until governance thresholds are wired".to_owned(),
    );
    let root = option_value(args, "--root").unwrap_or(".");

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
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    println!("  written_to: {}", output_path.display());

    ExitCode::SUCCESS
}

fn handle_fitness_explain(args: &[String]) -> ExitCode {
    let implementation_id = option_value(args, "--implementation").unwrap_or("impl-demo");
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
    println!("  decision: {}", record.evolution_plan.decision.as_str());
    println!("  rationale: {}", record.evolution_plan.rationale);
    println!("  read_from: {}", path.display());

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
