use serde::{Deserialize, Serialize};

use super::{apply_assignment_status_transition, TransitionOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentStatus {
    Created,
    Assigned,
    Running,
    RetryPending,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

impl AssignmentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Assigned => "assigned",
            Self::Running => "running",
            Self::RetryPending => "retry_pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    pub assignment_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub worker_node_id: String,
    pub status: AssignmentStatus,
    pub input: String,
    pub output: Option<String>,
}

impl Assignment {
    pub fn assigned(
        assignment_id: String,
        task_id: String,
        attempt_id: String,
        worker_node_id: String,
        input: String,
    ) -> Self {
        Self {
            assignment_id,
            task_id,
            attempt_id,
            worker_node_id,
            status: AssignmentStatus::Assigned,
            input,
            output: None,
        }
    }

    pub fn with_result(mut self, output: String, status: AssignmentStatus) -> Self {
        self.output = Some(output);
        self.status = status;
        self
    }

    pub fn transition_to(
        &mut self,
        next: AssignmentStatus,
    ) -> Result<TransitionOutcome, &'static str> {
        apply_assignment_status_transition(&mut self.status, next)
    }

    pub fn mark_running(&mut self) -> Result<TransitionOutcome, &'static str> {
        self.transition_to(AssignmentStatus::Running)
    }

    pub fn complete(&mut self, output: String) -> Result<TransitionOutcome, &'static str> {
        self.output = Some(output);
        self.transition_to(AssignmentStatus::Completed)
    }

    pub fn fail(&mut self, output: String) -> Result<TransitionOutcome, &'static str> {
        self.output = Some(output);
        self.transition_to(AssignmentStatus::Failed)
    }
}
