mod evolution_store;
mod registry_store;
mod task_store;

use std::fs;
use std::io;
use std::path::Path;

pub use evolution_store::{
    append_evolution_audit, list_fitness_runs, load_evolution_audits, load_fitness_run,
    persist_fitness_run, update_fitness_plan,
};
pub use registry_store::{
    list_skills, list_tools, load_skill, load_tool, persist_skill, persist_tool, update_skill,
};
pub use task_store::{
    append_task_audit, append_task_event, append_task_trace, list_residents,
    list_task_submissions, list_triggers, load_assignment, load_resident,
    load_task_assignments, load_task_audits, load_task_events, load_task_submission,
    load_task_traces, load_trigger, persist_assignment, persist_resident,
    persist_task_submission, persist_trigger, update_assignment, update_resident,
    update_task_runtime, update_task_submission, update_trigger,
};

fn write_atomic(path: &Path, body: &str) -> io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, body)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn append_jsonl<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: serde::Serialize,
{
    use std::io::Write;

    let line = serde_json::to_string(value).map_err(io::Error::other)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn to_pretty_json<T>(value: &T) -> io::Result<String>
where
    T: serde::Serialize,
{
    serde_json::to_string_pretty(value)
        .map(|body| format!("{body}\n"))
        .map_err(io::Error::other)
}

fn from_json<T>(body: &str) -> io::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(body).map_err(io::Error::other)
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}
