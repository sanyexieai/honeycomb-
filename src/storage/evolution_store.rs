use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::EVOLUTION_SCHEMA_VERSION;
use crate::governance::{
    ArchitectureReflectionRecord, ArchitectureReviewRecord, EvolutionPlan, FitnessRecord,
    FitnessReport,
};
use crate::runtime::AuditRecord;

use super::{append_jsonl, from_json, sanitize_filename, to_pretty_json, write_atomic};

pub fn persist_fitness_run(
    root: impl AsRef<Path>,
    report: &FitnessReport,
    plan: &EvolutionPlan,
) -> io::Result<PathBuf> {
    let root = root.as_ref();
    let fitness_dir = root.join("evolution").join("fitness");
    fs::create_dir_all(&fitness_dir)?;

    let path = fitness_dir.join(format!(
        "{}.json",
        sanitize_filename(report.implementation_id())
    ));
    let record = FitnessRecord {
        schema_version: EVOLUTION_SCHEMA_VERSION.to_owned(),
        fitness_report: report.clone(),
        evolution_plan: plan.clone(),
    };
    let body = to_pretty_json(&record)?;

    write_atomic(&path, &body)?;
    append_evolution_audit(
        root,
        &AuditRecord::now(
            format!("audit-{}-fitness-run", report.implementation_id()),
            "system".to_owned(),
            "honeycomb-evolution".to_owned(),
            "fitness_run".to_owned(),
            "implementation".to_owned(),
            report.implementation_id().to_owned(),
            String::new(),
            "recorded".to_owned(),
            format!(
                "score={} decision={} skill={} executor={} mode={} max_cost={} max_latency_ms={} skills={} tools={}",
                report.score,
                plan.decision.as_str(),
                report.implementation.skill_id,
                report.implementation.executor,
                report
                    .implementation
                    .strategy_mode
                    .as_deref()
                    .unwrap_or("<none>"),
                report
                    .implementation
                    .max_cost
                    .as_deref()
                    .unwrap_or("<none>"),
                report
                    .implementation
                    .max_latency_ms
                    .as_deref()
                    .unwrap_or("<none>"),
                if report.skill_refs.is_empty() {
                    "<none>".to_owned()
                } else {
                    report.skill_refs.join(", ")
                },
                if report.tool_refs.is_empty() {
                    "<none>".to_owned()
                } else {
                    report.tool_refs.join(", ")
                }
            ),
        ),
    )?;
    Ok(path)
}

pub fn load_fitness_run(
    root: impl AsRef<Path>,
    implementation_id: &str,
) -> io::Result<(PathBuf, FitnessRecord)> {
    let path = root
        .as_ref()
        .join("evolution")
        .join("fitness")
        .join(format!("{}.json", sanitize_filename(implementation_id)));
    let body = fs::read_to_string(&path)?;
    let record = from_json::<FitnessRecord>(&body)?;
    Ok((path, record))
}

pub fn update_fitness_plan(
    root: impl AsRef<Path>,
    implementation_id: &str,
    plan: &EvolutionPlan,
) -> io::Result<(PathBuf, FitnessRecord)> {
    let (path, mut record) = load_fitness_run(root.as_ref(), implementation_id)?;
    record.evolution_plan = plan.clone();
    let body = to_pretty_json(&record)?;
    write_atomic(&path, &body)?;
    Ok((path, record))
}

pub fn list_fitness_runs(root: impl AsRef<Path>) -> io::Result<(PathBuf, Vec<FitnessRecord>)> {
    let dir = root.as_ref().join("evolution").join("fitness");
    let mut records = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            records.push(from_json::<FitnessRecord>(&body)?);
        }
        records.sort_by(|a, b| {
            a.fitness_report
                .implementation_id()
                .cmp(b.fitness_report.implementation_id())
        });
    }

    Ok((dir, records))
}

pub fn append_evolution_audit(root: impl AsRef<Path>, audit: &AuditRecord) -> io::Result<PathBuf> {
    let audit_dir = root.as_ref().join("evolution").join("audit");
    fs::create_dir_all(&audit_dir)?;

    let path = audit_dir.join("audit.jsonl");
    append_jsonl(&path, audit)?;
    Ok(path)
}

pub fn persist_architecture_review(
    root: impl AsRef<Path>,
    review: &ArchitectureReviewRecord,
) -> io::Result<PathBuf> {
    let review_dir = root.as_ref().join("evolution").join("reviews");
    fs::create_dir_all(&review_dir)?;

    let path = review_dir.join(format!("{}.json", sanitize_filename(&review.review_id)));
    let body = to_pretty_json(review)?;
    write_atomic(&path, &body)?;
    append_evolution_audit(
        root,
        &AuditRecord::now(
            format!("audit-{}-architecture-review", review.review_id),
            review.requested_by.clone(),
            "honeycomb-evolution".to_owned(),
            "architecture_review_record".to_owned(),
            "architecture_review".to_owned(),
            review.review_id.clone(),
            String::new(),
            review.decision.as_str().to_owned(),
            format!(
                "plane={} scope={} writes_runtime={} writes_long_term={} mutates_historical_facts={}",
                review.target_plane.as_str(),
                review.change_scope,
                if review.writes_runtime {
                    "true"
                } else {
                    "false"
                },
                if review.writes_long_term {
                    "true"
                } else {
                    "false"
                },
                if review.mutates_historical_facts {
                    "true"
                } else {
                    "false"
                }
            ),
        ),
    )?;
    Ok(path)
}

pub fn persist_architecture_reflection(
    root: impl AsRef<Path>,
    reflection: &ArchitectureReflectionRecord,
) -> io::Result<PathBuf> {
    let reflection_dir = root.as_ref().join("evolution").join("reflections");
    fs::create_dir_all(&reflection_dir)?;

    let path = reflection_dir.join(format!(
        "{}.json",
        sanitize_filename(&reflection.reflection_id)
    ));
    let body = to_pretty_json(reflection)?;
    write_atomic(&path, &body)?;
    append_evolution_audit(
        root,
        &AuditRecord::now(
            format!("audit-{}-architecture-reflection", reflection.reflection_id),
            reflection.recorded_by.clone(),
            "honeycomb-evolution".to_owned(),
            "architecture_reflection_record".to_owned(),
            "architecture_reflection".to_owned(),
            reflection.reflection_id.clone(),
            String::new(),
            reflection.decision.as_str().to_owned(),
            format!(
                "period={} drifts={} freezes={} next_actions={}",
                reflection.period_label,
                reflection.detected_drifts.len(),
                reflection.freeze_actions.len(),
                reflection.next_actions.len()
            ),
        ),
    )?;
    Ok(path)
}

pub fn load_architecture_review(
    root: impl AsRef<Path>,
    review_id: &str,
) -> io::Result<(PathBuf, ArchitectureReviewRecord)> {
    let path = root
        .as_ref()
        .join("evolution")
        .join("reviews")
        .join(format!("{}.json", sanitize_filename(review_id)));
    let body = fs::read_to_string(&path)?;
    let record = from_json::<ArchitectureReviewRecord>(&body)?;
    Ok((path, record))
}

pub fn load_architecture_reflection(
    root: impl AsRef<Path>,
    reflection_id: &str,
) -> io::Result<(PathBuf, ArchitectureReflectionRecord)> {
    let path = root
        .as_ref()
        .join("evolution")
        .join("reflections")
        .join(format!("{}.json", sanitize_filename(reflection_id)));
    let body = fs::read_to_string(&path)?;
    let record = from_json::<ArchitectureReflectionRecord>(&body)?;
    Ok((path, record))
}

pub fn list_architecture_reviews(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, Vec<ArchitectureReviewRecord>)> {
    let dir = root.as_ref().join("evolution").join("reviews");
    let mut records = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            records.push(from_json::<ArchitectureReviewRecord>(&body)?);
        }
        records.sort_by(|a, b| a.review_id.cmp(&b.review_id));
    }

    Ok((dir, records))
}

pub fn list_architecture_reflections(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, Vec<ArchitectureReflectionRecord>)> {
    let dir = root.as_ref().join("evolution").join("reflections");
    let mut records = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            records.push(from_json::<ArchitectureReflectionRecord>(&body)?);
        }
        records.sort_by(|a, b| a.reflection_id.cmp(&b.reflection_id));
    }

    Ok((dir, records))
}

pub fn load_evolution_audits(root: impl AsRef<Path>) -> io::Result<(PathBuf, Vec<AuditRecord>)> {
    let path = root
        .as_ref()
        .join("evolution")
        .join("audit")
        .join("audit.jsonl");
    let body = fs::read_to_string(&path)?;
    let audits = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(from_json::<AuditRecord>)
        .collect::<io::Result<Vec<_>>>()?;
    Ok((path, audits))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::governance::{
        ArchitectureGuardrailSnapshot, ArchitectureReflectionDecision,
        ArchitectureReflectionRecord, ArchitectureReviewDecision, ArchitectureReviewRecord,
        ArchitectureReviewStatus, EvolutionPlan, FitnessReport, GovernedImplementation,
        GuardrailSnapshotCount, ReviewTargetPlane,
    };

    use super::*;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-evolution-test-{nanos}"))
    }

    fn governed_implementation(implementation_id: &str) -> GovernedImplementation {
        GovernedImplementation {
            implementation_id: implementation_id.to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            entry_kind: "script".to_owned(),
            entry_path: format!("scripts/{implementation_id}.sh"),
            component_count: 2,
            strategy_count: 1,
            constraint_count: 2,
            prompt_component: Some("prompts/xhs.md".to_owned()),
            config_component: Some("config/xhs.json".to_owned()),
            strategy_mode: Some("draft_then_publish".to_owned()),
            max_cost: Some("0.02".to_owned()),
            max_latency_ms: Some("5000".to_owned()),
            origin_source: Some("manual".to_owned()),
        }
    }

    #[test]
    fn persist_fitness_run_preserves_skill_and_tool_refs() {
        let root = unique_test_root();
        let report = FitnessReport::new(
            governed_implementation("impl-xhs-v1"),
            "0.91".to_owned(),
            "stable posting flow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec!["xhs_browser_login".to_owned()],
        );
        let plan = EvolutionPlan::observe(
            governed_implementation("impl-xhs-v1"),
            "observe until more signals arrive".to_owned(),
        );

        persist_fitness_run(&root, &report, &plan).expect("fitness run should persist");
        let (_, record) = load_fitness_run(&root, "impl-xhs-v1").expect("fitness run should load");

        assert_eq!(record.fitness_report.skill_refs, vec!["xhs_publish"]);
        assert_eq!(record.fitness_report.tool_refs, vec!["xhs_browser_login"]);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn list_fitness_runs_reads_multiple_records() {
        let root = unique_test_root();
        let plan =
            EvolutionPlan::observe(governed_implementation("impl-xhs-v1"), "observe".to_owned());
        let report_a = FitnessReport::new(
            governed_implementation("impl-xhs-v1"),
            "0.91".to_owned(),
            "stable posting flow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec!["xhs_browser_login".to_owned()],
        );
        let report_b = FitnessReport::new(
            governed_implementation("impl-xhs-v2"),
            "0.88".to_owned(),
            "candidate flow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec!["xhs_browser_login".to_owned()],
        );

        persist_fitness_run(&root, &report_a, &plan).expect("first fitness run should persist");
        persist_fitness_run(&root, &report_b, &plan).expect("second fitness run should persist");
        let (_, records) = list_fitness_runs(&root).expect("fitness runs should list");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fitness_report.implementation_id(), "impl-xhs-v1");
        assert_eq!(records[1].fitness_report.implementation_id(), "impl-xhs-v2");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_fitness_run_preserves_implementation_snapshot() {
        let root = unique_test_root();
        let implementation = governed_implementation("impl-xhs-v3");
        let report = FitnessReport::new(
            implementation.clone(),
            "0.95".to_owned(),
            "promising mutation".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let plan = EvolutionPlan::observe(implementation, "observe".to_owned());

        persist_fitness_run(&root, &report, &plan).expect("fitness run should persist");
        let (_, record) = load_fitness_run(&root, "impl-xhs-v3").expect("fitness run should load");

        assert_eq!(record.fitness_report.implementation.skill_id, "xhs_publish");
        assert_eq!(
            record.fitness_report.implementation.executor,
            "worker_process"
        );
        assert_eq!(
            record.fitness_report.implementation.entry_path,
            "scripts/impl-xhs-v3.sh"
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_load_architecture_review() {
        let root = unique_test_root();
        let review = ArchitectureReviewRecord::new(
            "arch-review-001".to_owned(),
            "backfill behavior".to_owned(),
            "execution_cli_command".to_owned(),
            "local-dev".to_owned(),
            ReviewTargetPlane::Execution,
            vec!["app".to_owned(), "runtime".to_owned()],
            true,
            false,
            true,
            false,
            false,
            ArchitectureReviewStatus::Completed,
            ArchitectureReviewDecision::NeedsRedesign,
            "historical runtime facts would be overwritten".to_owned(),
            vec!["convert to suggestion-only output".to_owned()],
            vec!["docs/specs/execution-vs-evolution-plane.md".to_owned()],
            Some(ArchitectureGuardrailSnapshot::new(
                "last_30_days".to_owned(),
                1,
                vec![GuardrailSnapshotCount::new(
                    "governance_plan_guardrail_block".to_owned(),
                    1,
                )],
                vec![GuardrailSnapshotCount::new(
                    "extreme_cost_budget".to_owned(),
                    1,
                )],
                vec![GuardrailSnapshotCount::new(
                    "governance_candidate".to_owned(),
                    1,
                )],
                vec![GuardrailSnapshotCount::new("impl-risky".to_owned(), 1)],
                vec![GuardrailSnapshotCount::new("xhs_publish".to_owned(), 1)],
            )),
        );

        persist_architecture_review(&root, &review).expect("review should persist");
        let (_, loaded) =
            load_architecture_review(&root, "arch-review-001").expect("review should load");

        assert_eq!(loaded.review_id, "arch-review-001");
        assert_eq!(loaded.decision, ArchitectureReviewDecision::NeedsRedesign);
        assert!(loaded.mutates_historical_facts);
        assert_eq!(
            loaded
                .guardrail_snapshot
                .as_ref()
                .map(|snapshot| snapshot.total_count),
            Some(1)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn persist_and_load_architecture_reflection() {
        let root = unique_test_root();
        let reflection = ArchitectureReflectionRecord::new(
            "arch-reflection-001".to_owned(),
            "phase-one reflection".to_owned(),
            "2026-W13".to_owned(),
            "local-dev".to_owned(),
            ArchitectureReflectionDecision::DriftDetected,
            "execution plane had absorbed long-term writes".to_owned(),
            vec!["execution plane wrote long-term registry state".to_owned()],
            vec!["remove execution-side long-term write commands".to_owned()],
            vec!["split execution app entry by domain".to_owned()],
            vec!["arch-review-001".to_owned()],
            vec!["docs/specs/current-capability-audit-and-aggressive-convergence.md".to_owned()],
            Some(ArchitectureGuardrailSnapshot::new(
                "last_30_days".to_owned(),
                2,
                vec![GuardrailSnapshotCount::new(
                    "registry_sync_guardrail_block".to_owned(),
                    1,
                )],
                vec![GuardrailSnapshotCount::new(
                    "extreme_cost_budget".to_owned(),
                    2,
                )],
                vec![GuardrailSnapshotCount::new("skill".to_owned(), 1)],
                vec![GuardrailSnapshotCount::new("xhs_publish".to_owned(), 1)],
                vec![GuardrailSnapshotCount::new("xhs_publish".to_owned(), 1)],
            )),
        );

        persist_architecture_reflection(&root, &reflection).expect("reflection should persist");
        let (_, loaded) = load_architecture_reflection(&root, "arch-reflection-001")
            .expect("reflection should load");

        assert_eq!(loaded.reflection_id, "arch-reflection-001");
        assert_eq!(
            loaded.decision,
            ArchitectureReflectionDecision::DriftDetected
        );
        assert_eq!(loaded.period_label, "2026-W13");
        assert_eq!(
            loaded
                .guardrail_snapshot
                .as_ref()
                .map(|snapshot| snapshot.total_count),
            Some(2)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
