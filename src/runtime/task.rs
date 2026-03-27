use serde::{Deserialize, Serialize};

use super::{TransitionOutcome, apply_task_status_transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl TaskStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub task_id: String,
    pub tenant_id: String,
    pub namespace: String,
    pub goal: String,
    #[serde(default)]
    pub implementation_ref: Option<String>,
    #[serde(default)]
    pub skill_refs: Vec<String>,
    #[serde(default)]
    pub tool_refs: Vec<String>,
}

impl TaskSpec {
    pub fn new(
        task_id: String,
        tenant_id: String,
        namespace: String,
        goal: String,
        implementation_ref: Option<String>,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
    ) -> Self {
        Self {
            task_id,
            tenant_id,
            namespace,
            goal,
            implementation_ref,
            skill_refs,
            tool_refs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRuntime {
    pub task_id: String,
    pub queen_node_id: String,
    pub status: TaskStatus,
}

impl TaskRuntime {
    pub fn queued(task_id: String, queen_node_id: String) -> Self {
        Self {
            task_id,
            queen_node_id,
            status: TaskStatus::Queued,
        }
    }

    pub fn transition_to(&mut self, next: TaskStatus) -> Result<TransitionOutcome, &'static str> {
        apply_task_status_transition(&mut self.status, next)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskHiveSession {
    pub session_id: String,
    pub hive_id: String,
    pub worker_node_id: String,
    pub status: String,
}
