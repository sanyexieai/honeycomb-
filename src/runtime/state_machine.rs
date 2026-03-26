use super::{AssignmentStatus, TaskStatus};

pub const TASK_STATUS_TRANSITION_ERR: &str = "invalid_task_status_transition";
pub const ASSIGNMENT_STATUS_TRANSITION_ERR: &str = "invalid_assignment_status_transition";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionOutcome {
    Applied,
    NoOp,
}

pub fn can_transition_task_status(current: TaskStatus, next: TaskStatus) -> bool {
    current == next
        || matches!(
            (current, next),
            (TaskStatus::Queued, TaskStatus::Running)
                | (TaskStatus::Queued, TaskStatus::Cancelled)
                | (TaskStatus::Queued, TaskStatus::Interrupted)
                | (TaskStatus::Running, TaskStatus::Completed)
                | (TaskStatus::Running, TaskStatus::Failed)
                | (TaskStatus::Running, TaskStatus::Cancelled)
                | (TaskStatus::Running, TaskStatus::Interrupted)
                | (TaskStatus::Interrupted, TaskStatus::Running)
                | (TaskStatus::Interrupted, TaskStatus::Cancelled)
        )
}

pub fn apply_task_status_transition(
    current: &mut TaskStatus,
    next: TaskStatus,
) -> Result<TransitionOutcome, &'static str> {
    if *current == next {
        return Ok(TransitionOutcome::NoOp);
    }
    if can_transition_task_status(*current, next) {
        *current = next;
        Ok(TransitionOutcome::Applied)
    } else {
        Err(TASK_STATUS_TRANSITION_ERR)
    }
}

pub fn can_transition_assignment_status(
    current: AssignmentStatus,
    next: AssignmentStatus,
) -> bool {
    current == next
        || matches!(
            (current, next),
            (AssignmentStatus::Created, AssignmentStatus::Assigned)
                | (AssignmentStatus::Assigned, AssignmentStatus::Running)
                | (AssignmentStatus::Assigned, AssignmentStatus::Completed)
                | (AssignmentStatus::Assigned, AssignmentStatus::Failed)
                | (AssignmentStatus::Assigned, AssignmentStatus::Cancelled)
                | (AssignmentStatus::Running, AssignmentStatus::Completed)
                | (AssignmentStatus::Running, AssignmentStatus::Failed)
                | (AssignmentStatus::Running, AssignmentStatus::RetryPending)
                | (AssignmentStatus::Running, AssignmentStatus::Cancelled)
                | (AssignmentStatus::RetryPending, AssignmentStatus::Assigned)
                | (AssignmentStatus::RetryPending, AssignmentStatus::Cancelled)
        )
}

pub fn apply_assignment_status_transition(
    current: &mut AssignmentStatus,
    next: AssignmentStatus,
) -> Result<TransitionOutcome, &'static str> {
    if *current == next {
        return Ok(TransitionOutcome::NoOp);
    }
    if can_transition_assignment_status(*current, next) {
        *current = next;
        Ok(TransitionOutcome::Applied)
    } else {
        Err(ASSIGNMENT_STATUS_TRANSITION_ERR)
    }
}
