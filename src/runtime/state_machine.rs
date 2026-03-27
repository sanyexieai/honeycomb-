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

pub fn can_transition_assignment_status(current: AssignmentStatus, next: AssignmentStatus) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_transition_applies_for_valid_path() {
        let mut status = TaskStatus::Queued;
        let outcome = apply_task_status_transition(&mut status, TaskStatus::Running);

        assert_eq!(outcome, Ok(TransitionOutcome::Applied));
        assert_eq!(status, TaskStatus::Running);
    }

    #[test]
    fn task_transition_is_noop_for_same_status() {
        let mut status = TaskStatus::Completed;
        let outcome = apply_task_status_transition(&mut status, TaskStatus::Completed);

        assert_eq!(outcome, Ok(TransitionOutcome::NoOp));
        assert_eq!(status, TaskStatus::Completed);
    }

    #[test]
    fn task_transition_rejects_invalid_path() {
        let mut status = TaskStatus::Completed;
        let outcome = apply_task_status_transition(&mut status, TaskStatus::Running);

        assert_eq!(outcome, Err(TASK_STATUS_TRANSITION_ERR));
        assert_eq!(status, TaskStatus::Completed);
    }

    #[test]
    fn assignment_transition_applies_for_valid_path() {
        let mut status = AssignmentStatus::Assigned;
        let outcome = apply_assignment_status_transition(&mut status, AssignmentStatus::Running);

        assert_eq!(outcome, Ok(TransitionOutcome::Applied));
        assert_eq!(status, AssignmentStatus::Running);
    }

    #[test]
    fn assignment_transition_is_noop_for_same_status() {
        let mut status = AssignmentStatus::Completed;
        let outcome = apply_assignment_status_transition(&mut status, AssignmentStatus::Completed);

        assert_eq!(outcome, Ok(TransitionOutcome::NoOp));
        assert_eq!(status, AssignmentStatus::Completed);
    }

    #[test]
    fn assignment_transition_rejects_invalid_path() {
        let mut status = AssignmentStatus::Completed;
        let outcome = apply_assignment_status_transition(&mut status, AssignmentStatus::Assigned);

        assert_eq!(outcome, Err(ASSIGNMENT_STATUS_TRANSITION_ERR));
        assert_eq!(status, AssignmentStatus::Completed);
    }
}
