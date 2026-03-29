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
pub struct ImplementationSnapshot {
    pub implementation_id: String,
    pub skill_id: String,
    pub executor: String,
    pub entry_kind: String,
    pub entry_path: String,
    #[serde(default)]
    pub strategy_mode: Option<String>,
    #[serde(default)]
    pub prompt_component: Option<String>,
    #[serde(default)]
    pub config_component: Option<String>,
    #[serde(default)]
    pub max_cost: Option<String>,
    #[serde(default)]
    pub max_latency_ms: Option<String>,
}

impl ImplementationSnapshot {
    pub fn new(
        implementation_id: String,
        skill_id: String,
        executor: String,
        entry_kind: String,
        entry_path: String,
    ) -> Self {
        Self {
            implementation_id,
            skill_id,
            executor,
            entry_kind,
            entry_path,
            strategy_mode: None,
            prompt_component: None,
            config_component: None,
            max_cost: None,
            max_latency_ms: None,
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
    pub implementation_snapshot: Option<ImplementationSnapshot>,
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
            implementation_snapshot: None,
            skill_refs,
            tool_refs,
        }
    }

    pub fn with_implementation_snapshot(
        mut self,
        implementation_snapshot: Option<ImplementationSnapshot>,
    ) -> Self {
        self.implementation_snapshot = implementation_snapshot;
        self
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
