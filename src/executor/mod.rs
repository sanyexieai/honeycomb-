use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::core::{EXECUTION_SCHEMA_VERSION, current_timestamp};
use crate::runtime::ImplementationSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    Skill,
    Tool,
}

impl ExecutionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Simulated,
    Succeeded,
    Failed,
    TimedOut,
}

impl ExecutionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simulated => "simulated",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }
}

const DEFAULT_SHELL_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 4_096;

fn default_execution_status() -> ExecutionStatus {
    ExecutionStatus::Simulated
}

fn default_execution_runner() -> String {
    "local-simulated".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub schema_version: String,
    pub execution_id: String,
    pub kind: ExecutionKind,
    pub target_id: String,
    pub task_id: Option<String>,
    pub assignment_id: Option<String>,
    pub implementation_ref: Option<String>,
    #[serde(default)]
    pub implementation_snapshot: Option<ImplementationSnapshot>,
    #[serde(default)]
    pub skill_refs: Vec<String>,
    #[serde(default)]
    pub tool_refs: Vec<String>,
    pub input: String,
    #[serde(default)]
    pub plan_steps: Vec<String>,
    pub output: String,
    #[serde(default = "default_execution_runner")]
    pub runner: String,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default = "default_execution_status")]
    pub status: ExecutionStatus,
    pub recorded_at: String,
}

impl ExecutionRecord {
    pub fn new(
        execution_id: String,
        kind: ExecutionKind,
        target_id: String,
        task_id: Option<String>,
        assignment_id: Option<String>,
        implementation_ref: Option<String>,
        implementation_snapshot: Option<ImplementationSnapshot>,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
        input: String,
        plan_steps: Vec<String>,
        output: String,
        runner: String,
        exit_code: Option<i32>,
        status: ExecutionStatus,
    ) -> Self {
        Self {
            schema_version: EXECUTION_SCHEMA_VERSION.to_owned(),
            execution_id,
            kind,
            target_id,
            task_id,
            assignment_id,
            implementation_ref,
            implementation_snapshot,
            skill_refs,
            tool_refs,
            input,
            plan_steps,
            output,
            runner,
            exit_code,
            status,
            recorded_at: current_timestamp(),
        }
    }

    pub fn simulated(
        execution_id: String,
        kind: ExecutionKind,
        target_id: String,
        task_id: Option<String>,
        assignment_id: Option<String>,
        implementation_ref: Option<String>,
        implementation_snapshot: Option<ImplementationSnapshot>,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
        input: String,
        plan_steps: Vec<String>,
        output: String,
    ) -> Self {
        Self::new(
            execution_id,
            kind,
            target_id,
            task_id,
            assignment_id,
            implementation_ref,
            implementation_snapshot,
            skill_refs,
            tool_refs,
            input,
            plan_steps,
            output,
            "local-simulated".to_owned(),
            None,
            ExecutionStatus::Simulated,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionOutcome {
    pub runner: String,
    pub exit_code: Option<i32>,
    pub status: ExecutionStatus,
    pub plan_steps: Vec<String>,
    pub output: String,
}

impl ToolExecutionOutcome {
    pub fn simulated(entrypoint: &str, input: &str) -> Self {
        Self {
            runner: "local-simulated".to_owned(),
            exit_code: None,
            status: ExecutionStatus::Simulated,
            plan_steps: vec![format!(
                "simulate_tool entrypoint={} input={}",
                entrypoint, input
            )],
            output: format!("simulated tool execution via {entrypoint}"),
        }
    }
}

pub fn execute_tool_entrypoint(entrypoint: &str, input: &str) -> io::Result<ToolExecutionOutcome> {
    if let Some(command) = entrypoint.strip_prefix("shell://") {
        return execute_local_shell(command, input);
    }

    Ok(ToolExecutionOutcome::simulated(entrypoint, input))
}

fn execute_local_shell(command: &str, input: &str) -> io::Result<ToolExecutionOutcome> {
    execute_local_shell_with_limits(
        command,
        input,
        DEFAULT_SHELL_TIMEOUT_MS,
        DEFAULT_MAX_OUTPUT_BYTES,
    )
}

fn execute_local_shell_with_limits(
    command: &str,
    input: &str,
    timeout_ms: u64,
    max_output_bytes: usize,
) -> io::Result<ToolExecutionOutcome> {
    let mut child = Command::new("sh")
        .arg("-lc")
        .arg(command)
        .env("HONEYCOMB_TOOL_INPUT", input)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut timed_out = false;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            child.kill()?;
            break;
        }
        sleep(Duration::from_millis(10));
    }

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined_output = match (stdout.is_empty(), stderr.is_empty()) {
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n[stderr]\n{stderr}"),
        (true, true) => "<no output>".to_owned(),
    };
    let (combined_output, output_truncated) = truncate_output(combined_output, max_output_bytes);
    let status = if timed_out {
        ExecutionStatus::TimedOut
    } else if output.status.success() {
        ExecutionStatus::Succeeded
    } else {
        ExecutionStatus::Failed
    };
    let mut plan_steps = vec![
        format!("shell_execute command={command}"),
        format!("shell_timeout_ms={timeout_ms}"),
    ];
    if output_truncated {
        plan_steps.push(format!("output_truncated max_bytes={max_output_bytes}"));
    }
    if timed_out {
        plan_steps.push("shell_killed_on_timeout".to_owned());
    }

    Ok(ToolExecutionOutcome {
        runner: "local-shell".to_owned(),
        exit_code: output.status.code(),
        status,
        plan_steps,
        output: combined_output,
    })
}

fn truncate_output(output: String, max_output_bytes: usize) -> (String, bool) {
    if output.len() <= max_output_bytes {
        return (output, false);
    }

    let mut truncated = String::new();
    for ch in output.chars() {
        if truncated.len() + ch.len_utf8() > max_output_bytes {
            break;
        }
        truncated.push(ch);
    }
    truncated.push_str("\n[truncated]");
    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_shell_entrypoint_stays_simulated() {
        let outcome = execute_tool_entrypoint("tool://browser/login", "hello")
            .expect("simulated entrypoint should succeed");

        assert_eq!(outcome.runner, "local-simulated");
        assert_eq!(outcome.status, ExecutionStatus::Simulated);
    }

    #[test]
    fn shell_entrypoint_executes_command() {
        let outcome = execute_tool_entrypoint(
            "shell://printf 'shell:%s' \"$HONEYCOMB_TOOL_INPUT\"",
            "world",
        )
        .expect("shell entrypoint should execute");

        assert_eq!(outcome.runner, "local-shell");
        assert_eq!(outcome.status, ExecutionStatus::Succeeded);
        assert_eq!(outcome.output, "shell:world");
    }

    #[test]
    fn shell_entrypoint_times_out() {
        let outcome = execute_local_shell_with_limits("sleep 0.05", "", 10, 1024)
            .expect("shell command should return timeout outcome");

        assert_eq!(outcome.runner, "local-shell");
        assert_eq!(outcome.status, ExecutionStatus::TimedOut);
        assert!(
            outcome
                .plan_steps
                .iter()
                .any(|step| step == "shell_killed_on_timeout")
        );
    }

    #[test]
    fn shell_entrypoint_truncates_output() {
        let outcome =
            execute_local_shell_with_limits("printf 'abcdefghijklmnopqrstuvwxyz'", "", 1000, 8)
                .expect("shell command should execute");

        assert_eq!(outcome.status, ExecutionStatus::Succeeded);
        assert!(outcome.output.ends_with("[truncated]"));
        assert!(
            outcome
                .plan_steps
                .iter()
                .any(|step| step == "output_truncated max_bytes=8")
        );
    }
}
