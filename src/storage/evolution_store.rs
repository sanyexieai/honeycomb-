use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::EVOLUTION_SCHEMA_VERSION;
use crate::governance::{EvolutionPlan, FitnessRecord, FitnessReport};
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

    let path = fitness_dir.join(format!("{}.json", sanitize_filename(&report.implementation_id)));
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
            format!("audit-{}-fitness-run", report.implementation_id),
            "system".to_owned(),
            "honeycomb-evolution".to_owned(),
            "fitness_run".to_owned(),
            "implementation".to_owned(),
            report.implementation_id.clone(),
            String::new(),
            "recorded".to_owned(),
            format!("score={} decision={}", report.score, plan.decision.as_str()),
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

pub fn append_evolution_audit(root: impl AsRef<Path>, audit: &AuditRecord) -> io::Result<PathBuf> {
    let audit_dir = root.as_ref().join("evolution").join("audit");
    fs::create_dir_all(&audit_dir)?;

    let path = audit_dir.join("audit.jsonl");
    append_jsonl(&path, audit)?;
    Ok(path)
}

pub fn load_evolution_audits(root: impl AsRef<Path>) -> io::Result<(PathBuf, Vec<AuditRecord>)> {
    let path = root.as_ref().join("evolution").join("audit").join("audit.jsonl");
    let body = fs::read_to_string(&path)?;
    let audits = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(from_json::<AuditRecord>)
        .collect::<io::Result<Vec<_>>>()?;
    Ok((path, audits))
}
