use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::core::{
    Artifact, HiveOutput, LifecycleState, Scheduler, TaskHiveSession, TaskRuntime, TaskSpec,
    TaskStatus,
};

#[derive(Debug, Clone, Default)]
pub struct MemoryScheduler {
    tasks: Arc<Mutex<HashMap<String, TaskRuntime>>>,
}

impl MemoryScheduler {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Scheduler for MemoryScheduler {
    async fn submit(&self, task: TaskSpec) -> Result<String> {
        let task_id = task.task_id.clone();
        let task_value = serde_json::to_value(&task)?;
        let runtime = TaskRuntime {
            task_id: task.task_id.clone(),
            status: TaskStatus::Queued,
            shared_context: task.context.clone(),
            sessions: Vec::new(),
            artifacts: vec![Artifact {
                name: "task_spec".to_string(),
                kind: "task_submission".to_string(),
                path: None,
                value: Some(task_value),
            }],
        };

        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("memory scheduler lock poisoned"))?;
        tasks.insert(task_id.clone(), runtime);
        Ok(task_id)
    }

    async fn poll(&self, task_id: &str) -> Result<TaskRuntime> {
        let tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("memory scheduler lock poisoned"))?;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| anyhow!("task not found: {task_id}"))
    }

    async fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("memory scheduler lock poisoned"))?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        task.status = status;
        Ok(())
    }

    async fn add_session(&self, task_id: &str, session: TaskHiveSession) -> Result<()> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("memory scheduler lock poisoned"))?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        task.sessions.push(session);
        Ok(())
    }

    async fn update_session_lifecycle(
        &self,
        task_id: &str,
        session_id: &str,
        lifecycle: LifecycleState,
    ) -> Result<()> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("memory scheduler lock poisoned"))?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        let session = task
            .sessions
            .iter_mut()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| anyhow!("session not found: {session_id}"))?;
        session.lifecycle = lifecycle;
        Ok(())
    }

    async fn attach_session_output(
        &self,
        task_id: &str,
        session_id: &str,
        output: &HiveOutput,
    ) -> Result<()> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("memory scheduler lock poisoned"))?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        let session = task
            .sessions
            .iter_mut()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| anyhow!("session not found: {session_id}"))?;

        session.artifacts.extend(output.artifacts.clone());
        session.local_state = output.payload.clone();
        task.artifacts.push(Artifact {
            name: format!("session_output_{session_id}"),
            kind: "hive_output".to_string(),
            path: None,
            value: Some(serde_json::to_value(output)?),
        });
        Ok(())
    }
}
