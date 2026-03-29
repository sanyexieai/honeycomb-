use crate::runtime::{
    Assignment, AssignmentStatus, ResidentHive, ResidentStatus, TaskRecord, TaskStatus,
    TransitionOutcome, Trigger, TriggerStatus,
};
use crate::storage::load_task_events;

pub(crate) fn transition_outcome_label(outcome: TransitionOutcome) -> &'static str {
    match outcome {
        TransitionOutcome::Applied => "applied",
        TransitionOutcome::NoOp => "noop",
    }
}

pub(crate) fn apply_record_write<F>(
    should_write: bool,
    write_fn: F,
) -> std::io::Result<&'static str>
where
    F: FnOnce() -> std::io::Result<()>,
{
    if should_write {
        write_fn()?;
        Ok("applied")
    } else {
        Ok("skipped")
    }
}

pub(crate) fn should_write_event_record(
    root: &str,
    task_id: &str,
    event_type: &str,
    payload: &str,
) -> std::io::Result<bool> {
    match load_task_events(root, task_id) {
        Ok((_, events)) => Ok(events
            .iter()
            .rev()
            .find(|event| event.event_type == event_type)
            .is_none_or(|event| event.payload != payload)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveTaskReason {
    AwaitingAssignment,
    AssignmentInProgress,
    AwaitingRetry,
    ResidentMaintainingSession,
    WaitingForTrigger,
    RunningWithoutAssignment,
}

impl ActiveTaskReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingAssignment => "awaiting_assignment",
            Self::AssignmentInProgress => "assignment_in_progress",
            Self::AwaitingRetry => "awaiting_retry",
            Self::ResidentMaintainingSession => "resident_maintaining_session",
            Self::WaitingForTrigger => "waiting_for_trigger",
            Self::RunningWithoutAssignment => "running_without_assignment",
        }
    }
}

pub(crate) fn is_non_terminal_task(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Queued | TaskStatus::Running)
}

pub(crate) fn classify_active_task(
    task: &TaskRecord,
    assignments: &[Assignment],
    residents: &[ResidentHive],
    triggers: &[Trigger],
) -> Option<ActiveTaskReason> {
    if !is_non_terminal_task(task.task_runtime.status) {
        return None;
    }

    let has_retry_pending = assignments
        .iter()
        .any(|assignment| assignment.status == AssignmentStatus::RetryPending);
    let has_live_assignment = assignments.iter().any(|assignment| {
        matches!(
            assignment.status,
            AssignmentStatus::Created | AssignmentStatus::Assigned | AssignmentStatus::Running
        )
    });
    let has_running_resident = residents
        .iter()
        .any(|resident| resident.status == ResidentStatus::Running);
    let has_active_trigger = triggers
        .iter()
        .any(|trigger| trigger.status == TriggerStatus::Active);

    match task.task_runtime.status {
        TaskStatus::Queued => {
            if has_retry_pending {
                Some(ActiveTaskReason::AwaitingRetry)
            } else if has_live_assignment {
                Some(ActiveTaskReason::AssignmentInProgress)
            } else if has_running_resident {
                Some(ActiveTaskReason::ResidentMaintainingSession)
            } else if has_active_trigger {
                Some(ActiveTaskReason::WaitingForTrigger)
            } else {
                Some(ActiveTaskReason::AwaitingAssignment)
            }
        }
        TaskStatus::Running => {
            if has_retry_pending {
                Some(ActiveTaskReason::AwaitingRetry)
            } else if has_live_assignment {
                Some(ActiveTaskReason::AssignmentInProgress)
            } else if has_running_resident {
                Some(ActiveTaskReason::ResidentMaintainingSession)
            } else {
                Some(ActiveTaskReason::RunningWithoutAssignment)
            }
        }
        TaskStatus::Completed
        | TaskStatus::Failed
        | TaskStatus::Cancelled
        | TaskStatus::Interrupted => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::runtime::{
        EventRecord, ResidentHive, ResidentStatus, TaskRecord, TaskRuntime, TaskSpec, TaskStatus,
        TransitionOutcome, Trigger,
    };
    use crate::storage::append_task_event;

    use super::*;

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-test-{nanos}"))
    }

    #[test]
    fn transition_outcome_label_matches_variants() {
        assert_eq!(
            transition_outcome_label(TransitionOutcome::Applied),
            "applied"
        );
        assert_eq!(transition_outcome_label(TransitionOutcome::NoOp), "noop");
    }

    #[test]
    fn apply_record_write_skips_closure_when_disabled() {
        let mut called = false;
        let result = apply_record_write(false, || {
            called = true;
            Ok(())
        });

        assert_eq!(result.expect("write should be skipped"), "skipped");
        assert!(!called);
    }

    #[test]
    fn should_write_event_record_returns_true_when_missing() {
        let root = unique_test_root();
        let result = should_write_event_record(
            root.to_str().expect("temp path should be valid utf-8"),
            "task-missing",
            "heartbeat_received",
            "worker=a state=idle",
        )
        .expect("missing event log should be treated as writable");

        assert!(result);
    }

    #[test]
    fn should_write_event_record_detects_duplicate_latest_payload() {
        let root = unique_test_root();
        let root_str = root.to_str().expect("temp path should be valid utf-8");
        let task_id = "task-dup";

        append_task_event(
            root_str,
            task_id,
            &EventRecord::new(
                "event-1".to_owned(),
                "heartbeat_received".to_owned(),
                task_id.to_owned(),
                "unix_ms:1".to_owned(),
                "worker=a state=idle".to_owned(),
            ),
        )
        .expect("seed event should be written");

        let duplicate = should_write_event_record(
            root_str,
            task_id,
            "heartbeat_received",
            "worker=a state=idle",
        )
        .expect("duplicate check should succeed");
        let changed = should_write_event_record(
            root_str,
            task_id,
            "heartbeat_received",
            "worker=a state=busy",
        )
        .expect("changed payload check should succeed");

        assert!(!duplicate);
        assert!(changed);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    fn sample_task(status: TaskStatus) -> TaskRecord {
        TaskRecord {
            schema_version: "execution/v1".to_owned(),
            task_spec: TaskSpec::new(
                "task-a".to_owned(),
                "tenant-a".to_owned(),
                "ns/a".to_owned(),
                "goal-a".to_owned(),
                None,
                Vec::new(),
                Vec::new(),
            ),
            task_runtime: TaskRuntime {
                task_id: "task-a".to_owned(),
                queen_node_id: "queen-a".to_owned(),
                status,
            },
        }
    }

    #[test]
    fn classify_active_task_prefers_assignment_progress() {
        let task = sample_task(TaskStatus::Queued);
        let assignments = vec![crate::runtime::Assignment::assigned(
            "assign-a".to_owned(),
            "task-a".to_owned(),
            "attempt-1".to_owned(),
            "worker-a".to_owned(),
            "input".to_owned(),
            None,
            None,
            Vec::new(),
            Vec::new(),
        )];
        let residents = vec![ResidentHive {
            resident_id: "resident-a".to_owned(),
            task_id: "task-a".to_owned(),
            worker_node_id: "worker-a".to_owned(),
            purpose: "watch".to_owned(),
            status: ResidentStatus::Running,
            started_at: "unix_ms:1".to_owned(),
            last_seen_at: "unix_ms:2".to_owned(),
        }];
        let triggers = vec![Trigger::active(
            "trigger-a".to_owned(),
            "task-a".to_owned(),
            "schedule".to_owned(),
            "hourly".to_owned(),
        )];

        let reason = classify_active_task(&task, &assignments, &residents, &triggers);

        assert_eq!(reason, Some(ActiveTaskReason::AssignmentInProgress));
    }

    #[test]
    fn classify_active_task_detects_trigger_waiting() {
        let task = sample_task(TaskStatus::Queued);
        let triggers = vec![Trigger::active(
            "trigger-a".to_owned(),
            "task-a".to_owned(),
            "schedule".to_owned(),
            "hourly".to_owned(),
        )];

        let reason = classify_active_task(&task, &[], &[], &triggers);

        assert_eq!(reason, Some(ActiveTaskReason::WaitingForTrigger));
    }

    #[test]
    fn classify_active_task_returns_none_for_completed_task() {
        let task = sample_task(TaskStatus::Completed);

        let reason = classify_active_task(&task, &[], &[], &[]);

        assert_eq!(reason, None);
    }
}
