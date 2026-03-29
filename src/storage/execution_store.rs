use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::executor::ExecutionRecord;

use super::{from_json, sanitize_filename, to_pretty_json, write_atomic};

pub fn persist_execution_record(
    root: impl AsRef<Path>,
    record: &ExecutionRecord,
) -> io::Result<PathBuf> {
    let dir = root.as_ref().join("runtime").join("executions");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", sanitize_filename(&record.execution_id)));
    let body = to_pretty_json(record)?;
    write_atomic(&path, &body)?;
    Ok(path)
}

pub fn load_execution_record(
    root: impl AsRef<Path>,
    execution_id: &str,
) -> io::Result<(PathBuf, ExecutionRecord)> {
    let path = root
        .as_ref()
        .join("runtime")
        .join("executions")
        .join(format!("{}.json", sanitize_filename(execution_id)));
    let body = fs::read_to_string(&path)?;
    let record = from_json::<ExecutionRecord>(&body)?;
    Ok((path, record))
}

pub fn list_execution_records(
    root: impl AsRef<Path>,
) -> io::Result<(PathBuf, Vec<ExecutionRecord>)> {
    let dir = root.as_ref().join("runtime").join("executions");
    let mut records = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            records.push(from_json::<ExecutionRecord>(&body)?);
        }
        records.sort_by(|a, b| a.execution_id.cmp(&b.execution_id));
    }

    Ok((dir, records))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::executor::{ExecutionKind, ExecutionRecord};
    use crate::runtime::ImplementationSnapshot;

    use super::*;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-execution-test-{nanos}"))
    }

    #[test]
    fn persist_and_load_execution_record() {
        let root = unique_test_root();
        let record = ExecutionRecord::simulated(
            "exec-a".to_owned(),
            ExecutionKind::Tool,
            "tool-a".to_owned(),
            Some("task-a".to_owned()),
            Some("assign-a".to_owned()),
            Some("impl-a".to_owned()),
            Some(ImplementationSnapshot {
                implementation_id: "impl-a".to_owned(),
                skill_id: "skill-a".to_owned(),
                executor: "shell".to_owned(),
                entry_kind: "script".to_owned(),
                entry_path: "scripts/tool-a.sh".to_owned(),
                strategy_mode: Some("safe".to_owned()),
                prompt_component: None,
                config_component: Some("default".to_owned()),
                max_cost: Some("low".to_owned()),
                max_latency_ms: Some("500".to_owned()),
            }),
            vec!["skill-a".to_owned()],
            vec!["tool-a".to_owned()],
            "input-a".to_owned(),
            vec!["invoke tool-a".to_owned()],
            "simulated output".to_owned(),
        );

        persist_execution_record(&root, &record).expect("execution record should persist");
        let (_, loaded) =
            load_execution_record(&root, "exec-a").expect("execution record should load");

        assert_eq!(loaded.execution_id, "exec-a");
        assert_eq!(loaded.kind, ExecutionKind::Tool);
        assert_eq!(loaded.task_id.as_deref(), Some("task-a"));
        assert_eq!(loaded.runner, "local-simulated");
        assert_eq!(
            loaded
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.implementation_id.as_str()),
            Some("impl-a")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
