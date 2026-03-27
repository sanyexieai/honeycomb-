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

    let path = fitness_dir.join(format!(
        "{}.json",
        sanitize_filename(&report.implementation_id)
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
            format!("audit-{}-fitness-run", report.implementation_id),
            "system".to_owned(),
            "honeycomb-evolution".to_owned(),
            "fitness_run".to_owned(),
            "implementation".to_owned(),
            report.implementation_id.clone(),
            String::new(),
            "recorded".to_owned(),
            format!(
                "score={} decision={} skills={} tools={}",
                report.score,
                plan.decision.as_str(),
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
                .implementation_id
                .cmp(&b.fitness_report.implementation_id)
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

    use crate::governance::{EvolutionPlan, FitnessReport};

    use super::*;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-evolution-test-{nanos}"))
    }

    #[test]
    fn persist_fitness_run_preserves_skill_and_tool_refs() {
        let root = unique_test_root();
        let report = FitnessReport::new(
            "impl-xhs-v1".to_owned(),
            "0.91".to_owned(),
            "stable posting flow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec!["xhs_browser_login".to_owned()],
        );
        let plan = EvolutionPlan::observe(
            "impl-xhs-v1".to_owned(),
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
        let plan = EvolutionPlan::observe("impl-xhs-v1".to_owned(), "observe".to_owned());
        let report_a = FitnessReport::new(
            "impl-xhs-v1".to_owned(),
            "0.91".to_owned(),
            "stable posting flow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec!["xhs_browser_login".to_owned()],
        );
        let report_b = FitnessReport::new(
            "impl-xhs-v2".to_owned(),
            "0.88".to_owned(),
            "candidate flow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec!["xhs_browser_login".to_owned()],
        );

        persist_fitness_run(&root, &report_a, &plan).expect("first fitness run should persist");
        persist_fitness_run(&root, &report_b, &plan).expect("second fitness run should persist");
        let (_, records) = list_fitness_runs(&root).expect("fitness runs should list");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fitness_report.implementation_id, "impl-xhs-v1");
        assert_eq!(records[1].fitness_report.implementation_id, "impl-xhs-v2");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
