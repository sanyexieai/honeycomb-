use std::path::PathBuf;
use std::process::ExitCode;

use serde::Serialize;

use crate::executor::{ExecutionKind, ExecutionRecord, execute_tool_entrypoint};
use crate::protocol::{
    HandshakeTranscript, HeartbeatTranscript, QueenEndpoint, ShutdownTranscript,
    simulate_handshake, simulate_heartbeat, simulate_shutdown,
};
use crate::registry::{ApprovalRequestStatus, ShellApprovalRequest, SkillRecord, ToolRecord};
use crate::runtime::{
    Assignment, AssignmentStatus, AuditRecord, EventRecord, ResidentHive, TaskRuntime, TaskSpec,
    TaskStatus, TraceRecord, TransitionOutcome, Trigger, TriggerStatus,
};
use crate::storage::{
    append_task_audit, append_task_event, append_task_trace, list_execution_records,
    list_fitness_runs, list_policy_alert_acks, list_residents, list_shell_approval_requests,
    list_skills, list_tools, list_triggers, load_assignment, load_evolution_audits,
    load_execution_record, load_implementation, load_resident, load_shell_approval_request,
    load_skill, load_skill_implementations, load_task_assignments, load_task_audits,
    load_task_events, load_task_submission, load_task_traces, load_tool, load_trigger,
    persist_execution_record, persist_resident, persist_task_submission, persist_trigger,
    update_assignment, update_resident, update_task_runtime, update_task_submission,
    update_trigger, validate_skill_implementation_refs,
};

use super::cli::{
    BinaryRole, Command, execute_command, has_flag, option_value, option_values, parse_command,
};
use super::execution_support::{
    apply_record_write, classify_active_task, should_write_event_record, transition_outcome_label,
};

#[path = "execution/capability/mod.rs"]
mod capability;
#[path = "execution/common_support.rs"]
mod common_support;
#[path = "execution/control/mod.rs"]
mod control;
#[path = "execution/overview/mod.rs"]
mod overview;
#[path = "execution/protocol/mod.rs"]
mod protocol;
#[path = "execution/resident/mod.rs"]
mod resident;
#[path = "execution/scheduler/mod.rs"]
mod scheduler;
#[path = "execution/task/mod.rs"]
mod task;
#[path = "execution/trigger/mod.rs"]
mod trigger;

use task::{collect_rerun_plan_alerts, list_rerun_plans};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use task::{
    TaskRerunBatchJson, TaskRerunJson, append_rerun_plan, load_rerun_plan, prune_rerun_plan,
    save_rerun_plan,
};

pub(crate) fn handle(command: Command, args: &[String]) -> ExitCode {
    match command {
        Command::QueenRun => protocol::handle_queen_run(args),
        Command::WorkerRun => protocol::handle_worker_run(args),
        Command::TaskSubmit => task::handle_task_submit(args),
        Command::TaskDemoFlow => task::handle_task_demo_flow(args),
        Command::TaskAssign => task::handle_task_assign(args),
        Command::AssignmentList => task::handle_assignment_list(args),
        Command::AssignmentInspect => task::handle_assignment_inspect(args),
        Command::TaskResult => task::handle_task_result(args),
        Command::TaskList => task::handle_task_list(args),
        Command::TaskReopen => task::handle_task_reopen(args),
        Command::TaskRerun => task::handle_task_rerun(args),
        Command::TaskInspect => task::handle_task_inspect(args),
        Command::TaskReplay => task::handle_task_replay(args),
        Command::AuditTail => task::handle_task_audit_tail(args),
        Command::TraceTail => task::handle_task_trace_tail(args),
        Command::TriggerCreate => trigger::handle_trigger_create(args),
        Command::TriggerInspect => trigger::handle_trigger_inspect(args),
        Command::TriggerList => trigger::handle_trigger_list(args),
        Command::TriggerPause => trigger::handle_trigger_pause(args),
        Command::TriggerResume => trigger::handle_trigger_resume(args),
        Command::TriggerFire => trigger::handle_trigger_fire(args),
        Command::TriggerClearReady => trigger::handle_trigger_clear_ready(args),
        Command::SkillInspect => capability::handle_skill_inspect(args),
        Command::SkillList => capability::handle_skill_list(args),
        Command::SkillExecute => capability::handle_skill_execute(args),
        Command::ToolInspect => capability::handle_tool_inspect(args),
        Command::ToolList => capability::handle_tool_list(args),
        Command::ToolApprovalInspect => capability::handle_tool_approval_inspect(args),
        Command::ToolApprovalList => capability::handle_tool_approval_list(args),
        Command::ToolApprovalQueue => capability::handle_tool_approval_queue(args),
        Command::ToolApprovalOverdue => capability::handle_tool_approval_overdue(args),
        Command::ToolApprovalAlerts => capability::handle_tool_approval_alerts(args),
        Command::ToolApprovalInbox => capability::handle_tool_approval_inbox(args),
        Command::ToolExecute => capability::handle_tool_execute(args),
        Command::ExecutionInspect => capability::handle_execution_inspect(args),
        Command::ExecutionList => capability::handle_execution_list(args),
        Command::HeartbeatSend => protocol::handle_heartbeat_send(args),
        Command::ShutdownSend => protocol::handle_shutdown_send(args),
        Command::ResidentRun => resident::handle_resident_run(args),
        Command::ResidentInspect => resident::handle_resident_inspect(args),
        Command::ResidentHeartbeat => resident::handle_resident_heartbeat(args),
        Command::ResidentPause => resident::handle_resident_pause(args),
        Command::ResidentResume => resident::handle_resident_resume(args),
        Command::ResidentStop => resident::handle_resident_stop(args),
        Command::SchedulerRunOnce => scheduler::handle_scheduler_run_once(args),
        Command::SchedulerLoop => scheduler::handle_scheduler_loop(args),
        Command::RuntimeOverview => overview::handle_runtime_overview(args),
        Command::SystemOverview => overview::handle_system_overview(args),
        Command::SystemAlerts => overview::handle_system_alerts(args),
        other => {
            println!(
                "{} command scaffold: {}",
                BinaryRole::Execution.binary_name(),
                super::cli::command_name(&other)
            );
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
#[path = "execution/tests.rs"]
mod tests;
