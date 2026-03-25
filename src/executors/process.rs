use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};

use crate::core::{Artifact, HiveOutput, ImplementationSpec, MetricValue, WorkerRequest, WorkerResponse};

#[derive(Debug, Default, Clone)]
pub struct ProcessExecutor;

impl ProcessExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, hive_dir: &Path, implementation: &ImplementationSpec, request: &WorkerRequest) -> Result<HiveOutput> {
        let components = &implementation.components;

        let mut command = if let Some(script) = &components.script {
            self.command_for_script(script)
        } else if let Some(binary) = &components.binary {
            Command::new(hive_dir.join(binary))
        } else {
            bail!("implementation has neither script nor binary component")
        };

        let input = serde_json::to_vec(request)?;
        let mut child = command
            .current_dir(hive_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn worker process")?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(&input)
                .context("failed to write worker stdin")?;
        }

        let output = child
            .wait_with_output()
            .context("failed to wait for worker process")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("worker exited with status {}: {}", output.status, stderr.trim()));
        }

        let response: WorkerResponse = serde_json::from_slice(&output.stdout)
            .context("worker stdout is not valid json")?;

        Ok(HiveOutput {
            task_id: request.task_id.clone(),
            hive_id: request.hive_id.clone(),
            impl_id: request.impl_id.clone(),
            success: response.success,
            payload: response.payload,
            artifacts: response.artifacts.unwrap_or_else(|| {
                vec![Artifact {
                    name: "worker_stdout".to_string(),
                    kind: "process_output".to_string(),
                    path: None,
                    value: None,
                }]
            }),
            metrics: response.metrics.unwrap_or_else(|| {
                vec![MetricValue {
                    name: "worker_exit_code".to_string(),
                    value: 0.0,
                }]
            }),
        })
    }

    fn command_for_script(&self, script: &Path) -> Command {
        match script.extension().and_then(|ext| ext.to_str()) {
            Some("py") => {
                let mut command = Command::new("python");
                command.arg(script);
                command
            }
            _ => Command::new(script),
        }
    }
}
