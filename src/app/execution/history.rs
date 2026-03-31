use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::current_timestamp;

#[derive(Debug, Serialize)]
struct ConversationEvent<'a> {
    ts: String,
    session_id: &'a str,
    role: &'a str,
    text: &'a str,
    root: &'a str,
}

fn history_path(root: &str) -> PathBuf {
    let base = Path::new(root);
    base.join(".honeycomb").join("conversation_history.jsonl")
}

pub(crate) fn append_event(
    root: &str,
    session_id: &str,
    role: &str,
    text: &str,
) -> io::Result<()> {
    let path = history_path(root);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let event = ConversationEvent {
        ts: current_timestamp(),
        session_id,
        role,
        text,
        root,
    };
    let line = serde_json::to_string(&event).unwrap_or_default();
    writeln!(file, "{line}")?;
    Ok(())
}

