use std::env;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryRole {
    Execution,
    Evolution,
    /// Bee runtime binary (`honeycomb-bee`): same CLI as execution, but no args → interactive Code.
    Bee,
}

impl BinaryRole {
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Execution => "honeycomb",
            Self::Evolution => "honeycomb-evolution",
            Self::Bee => "honeycomb-bee",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    QueenRun,
    WorkerRun,
    TaskSubmit,
    TaskDemoFlow,
    TaskAssign,
    AssignmentList,
    AssignmentInspect,
    TaskResult,
    TaskList,
    TaskReopen,
    TaskRerun,
    TaskInspect,
    TaskReplay,
    TraceTail,
    TriggerCreate,
    TriggerInspect,
    TriggerList,
    TriggerPause,
    TriggerResume,
    TriggerFire,
    TriggerClearReady,
    SkillInspect,
    SkillList,
    SkillExecute,
    ToolInspect,
    ToolList,
    ToolApprovalInspect,
    ToolApprovalList,
    ToolApprovalQueue,
    ToolApprovalOverdue,
    ToolApprovalAlerts,
    ToolApprovalInbox,
    ToolExecute,
    ExecutionInspect,
    ExecutionList,
    HeartbeatSend,
    ShutdownSend,
    ResidentRun,
    ResidentInspect,
    ResidentHeartbeat,
    ResidentPause,
    ResidentResume,
    ResidentStop,
    SchedulerRunOnce,
    SchedulerLoop,
    RuntimeOverview,
    SystemOverview,
    SystemAlerts,
    AuditTail,
    /// Interactive Code session (REPL), Claude Code–style entry.
    Code,
    FitnessRun,
    FitnessExplain,
    GovernancePlan,
    GovernanceApply,
    ReflectionRecord,
    ReflectionInspect,
    ReflectionList,
    ReviewRecord,
    ReviewSuggest,
    ReviewMaterialize,
    ReviewInspect,
    ReviewList,
    GovernanceDefaultsInspect,
    GovernanceDefaultsSet,
    RegistrySync,
    RegistryOverview,
    ImplementationInspect,
    ImplementationList,
    LineageShow,
}

pub fn run(role: BinaryRole) -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_command(role, &args) {
        Ok(Command::Help) => {
            print_help(role);
            ExitCode::SUCCESS
        }
        Ok(command) => execute_command(role, command, &args),
        Err(message) => {
            eprintln!("{message}");
            eprintln!();
            print_help(role);
            ExitCode::from(2)
        }
    }
}

pub(crate) fn parse_command(role: BinaryRole, args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return match role {
            BinaryRole::Bee => Ok(Command::Code),
            _ => Ok(Command::Help),
        };
    }

    let tokens: Vec<&str> = args.iter().map(String::as_str).collect();
    match role {
        BinaryRole::Execution | BinaryRole::Bee => parse_execution_command(&tokens),
        BinaryRole::Evolution => parse_evolution_command(&tokens),
    }
}

fn parse_execution_command(tokens: &[&str]) -> Result<Command, String> {
    match tokens {
        ["help"] | ["--help"] | ["-h"] => Ok(Command::Help),
        ["code"] | ["code", ..] => Ok(Command::Code),
        [group, command, ..] => match (*group, *command) {
            ("queen", "run") => Ok(Command::QueenRun),
            ("worker", "run") => Ok(Command::WorkerRun),
            ("task", "submit") => Ok(Command::TaskSubmit),
            ("task", "demo-flow") => Ok(Command::TaskDemoFlow),
            ("task", "assign") => Ok(Command::TaskAssign),
            ("assignment", "list") => Ok(Command::AssignmentList),
            ("assignment", "inspect") => Ok(Command::AssignmentInspect),
            ("task", "result") => Ok(Command::TaskResult),
            ("task", "list") => Ok(Command::TaskList),
            ("task", "reopen") => Ok(Command::TaskReopen),
            ("task", "rerun") => Ok(Command::TaskRerun),
            ("task", "inspect") => Ok(Command::TaskInspect),
            ("task", "replay") => Ok(Command::TaskReplay),
            ("trace", "tail") => Ok(Command::TraceTail),
            ("trigger", "create") => Ok(Command::TriggerCreate),
            ("trigger", "inspect") => Ok(Command::TriggerInspect),
            ("trigger", "list") => Ok(Command::TriggerList),
            ("trigger", "pause") => Ok(Command::TriggerPause),
            ("trigger", "resume") => Ok(Command::TriggerResume),
            ("trigger", "fire") => Ok(Command::TriggerFire),
            ("trigger", "clear-ready") => Ok(Command::TriggerClearReady),
            ("skill", "inspect") => Ok(Command::SkillInspect),
            ("skill", "list") => Ok(Command::SkillList),
            ("skill", "execute") => Ok(Command::SkillExecute),
            ("tool", "inspect") => Ok(Command::ToolInspect),
            ("tool", "list") => Ok(Command::ToolList),
            ("tool", "approval-inspect") => Ok(Command::ToolApprovalInspect),
            ("tool", "approval-list") => Ok(Command::ToolApprovalList),
            ("tool", "approval-queue") => Ok(Command::ToolApprovalQueue),
            ("tool", "approval-overdue") => Ok(Command::ToolApprovalOverdue),
            ("tool", "approval-alerts") => Ok(Command::ToolApprovalAlerts),
            ("tool", "approval-inbox") => Ok(Command::ToolApprovalInbox),
            ("tool", "execute") => Ok(Command::ToolExecute),
            ("execution", "inspect") => Ok(Command::ExecutionInspect),
            ("execution", "list") => Ok(Command::ExecutionList),
            ("heartbeat", "send") => Ok(Command::HeartbeatSend),
            ("shutdown", "send") => Ok(Command::ShutdownSend),
            ("resident", "run") => Ok(Command::ResidentRun),
            ("resident", "inspect") => Ok(Command::ResidentInspect),
            ("resident", "heartbeat") => Ok(Command::ResidentHeartbeat),
            ("resident", "pause") => Ok(Command::ResidentPause),
            ("resident", "resume") => Ok(Command::ResidentResume),
            ("resident", "stop") => Ok(Command::ResidentStop),
            ("scheduler", "run-once") => Ok(Command::SchedulerRunOnce),
            ("scheduler", "loop") => Ok(Command::SchedulerLoop),
            ("runtime", "overview") => Ok(Command::RuntimeOverview),
            ("system", "overview") => Ok(Command::SystemOverview),
            ("system", "alerts") => Ok(Command::SystemAlerts),
            ("audit", "tail") => Ok(Command::AuditTail),
            _ => Err(format!("unknown honeycomb command: {}", tokens.join(" "))),
        },
        _ => Err(format!("unknown honeycomb command: {}", tokens.join(" "))),
    }
}

fn parse_evolution_command(tokens: &[&str]) -> Result<Command, String> {
    match tokens {
        ["help"] | ["--help"] | ["-h"] => Ok(Command::Help),
        [group, command, ..] => match (*group, *command) {
            ("audit", "tail") => Ok(Command::AuditTail),
            ("fitness", "run") => Ok(Command::FitnessRun),
            ("fitness", "explain") => Ok(Command::FitnessExplain),
            ("governance", "plan") => Ok(Command::GovernancePlan),
            ("governance", "apply") => Ok(Command::GovernanceApply),
            ("reflection", "record") => Ok(Command::ReflectionRecord),
            ("reflection", "inspect") => Ok(Command::ReflectionInspect),
            ("reflection", "list") => Ok(Command::ReflectionList),
            ("review", "record") => Ok(Command::ReviewRecord),
            ("review", "suggest") => Ok(Command::ReviewSuggest),
            ("review", "materialize") => Ok(Command::ReviewMaterialize),
            ("review", "inspect") => Ok(Command::ReviewInspect),
            ("review", "list") => Ok(Command::ReviewList),
            ("governance-defaults", "inspect") => Ok(Command::GovernanceDefaultsInspect),
            ("governance-defaults", "set") => Ok(Command::GovernanceDefaultsSet),
            ("registry", "sync") => Ok(Command::RegistrySync),
            ("registry", "overview") => Ok(Command::RegistryOverview),
            ("implementation", "inspect") => Ok(Command::ImplementationInspect),
            ("implementation", "list") => Ok(Command::ImplementationList),
            ("lineage", "show") => Ok(Command::LineageShow),
            _ => Err(format!(
                "unknown honeycomb-evolution command: {}",
                tokens.join(" ")
            )),
        },
        _ => Err(format!(
            "unknown honeycomb-evolution command: {}",
            tokens.join(" ")
        )),
    }
}

pub(crate) fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Help => "help",
        Command::QueenRun => "queen run",
        Command::WorkerRun => "worker run",
        Command::TaskSubmit => "task submit",
        Command::TaskDemoFlow => "task demo-flow",
        Command::TaskAssign => "task assign",
        Command::AssignmentList => "assignment list",
        Command::AssignmentInspect => "assignment inspect",
        Command::TaskResult => "task result",
        Command::TaskList => "task list",
        Command::TaskReopen => "task reopen",
        Command::TaskRerun => "task rerun",
        Command::TaskInspect => "task inspect",
        Command::TaskReplay => "task replay",
        Command::TraceTail => "trace tail",
        Command::TriggerCreate => "trigger create",
        Command::TriggerInspect => "trigger inspect",
        Command::TriggerList => "trigger list",
        Command::TriggerPause => "trigger pause",
        Command::TriggerResume => "trigger resume",
        Command::TriggerFire => "trigger fire",
        Command::TriggerClearReady => "trigger clear-ready",
        Command::SkillInspect => "skill inspect",
        Command::SkillList => "skill list",
        Command::SkillExecute => "skill execute",
        Command::ToolInspect => "tool inspect",
        Command::ToolList => "tool list",
        Command::ToolApprovalInspect => "tool approval-inspect",
        Command::ToolApprovalList => "tool approval-list",
        Command::ToolApprovalQueue => "tool approval-queue",
        Command::ToolApprovalOverdue => "tool approval-overdue",
        Command::ToolApprovalAlerts => "tool approval-alerts",
        Command::ToolApprovalInbox => "tool approval-inbox",
        Command::ToolExecute => "tool execute",
        Command::ExecutionInspect => "execution inspect",
        Command::ExecutionList => "execution list",
        Command::HeartbeatSend => "heartbeat send",
        Command::ShutdownSend => "shutdown send",
        Command::ResidentRun => "resident run",
        Command::ResidentInspect => "resident inspect",
        Command::ResidentHeartbeat => "resident heartbeat",
        Command::ResidentPause => "resident pause",
        Command::ResidentResume => "resident resume",
        Command::ResidentStop => "resident stop",
        Command::SchedulerRunOnce => "scheduler run-once",
        Command::SchedulerLoop => "scheduler loop",
        Command::RuntimeOverview => "runtime overview",
        Command::SystemOverview => "system overview",
        Command::SystemAlerts => "system alerts",
        Command::AuditTail => "audit tail",
        Command::Code => "code",
        Command::FitnessRun => "fitness run",
        Command::FitnessExplain => "fitness explain",
        Command::GovernancePlan => "governance plan",
        Command::GovernanceApply => "governance apply",
        Command::ReflectionRecord => "reflection record",
        Command::ReflectionInspect => "reflection inspect",
        Command::ReflectionList => "reflection list",
        Command::ReviewRecord => "review record",
        Command::ReviewSuggest => "review suggest",
        Command::ReviewMaterialize => "review materialize",
        Command::ReviewInspect => "review inspect",
        Command::ReviewList => "review list",
        Command::GovernanceDefaultsInspect => "governance-defaults inspect",
        Command::GovernanceDefaultsSet => "governance-defaults set",
        Command::RegistrySync => "registry sync",
        Command::RegistryOverview => "registry overview",
        Command::ImplementationInspect => "implementation inspect",
        Command::ImplementationList => "implementation list",
        Command::LineageShow => "lineage show",
    }
}

pub(crate) fn execute_command(role: BinaryRole, command: Command, args: &[String]) -> ExitCode {
    match role {
        BinaryRole::Execution | BinaryRole::Bee => super::execution::handle(command, args),
        BinaryRole::Evolution => super::evolution::handle(command, args),
    }
}

pub(crate) fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2).find_map(|window| match window {
        [flag, value] if flag == name => Some(value.as_str()),
        _ => None,
    })
}

pub(crate) fn option_values(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter_map(|window| match window {
            [flag, value] if flag == name => Some(value.clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn print_help(role: BinaryRole) {
    match role {
        BinaryRole::Bee => {
            println!("Honeycomb BEE — runtime (queen/worker, tasks, skills)");
            println!();
            println!("  With no arguments, starts an interactive Code session (like Claude Code).");
            println!("  Same commands as `honeycomb`; use `honeycomb help` for the full list.");
            println!();
            println!("Primary:");
            println!("  honeycomb-bee");
            println!("  honeycomb-bee code [--skill-id ID] [--root PATH] [--use-recommended-impl] [--run-tools]");
            println!();
            println!("Environment:");
            println!("  HONEYCOMB_CODE_SKILL   default skill for Code (default: code-assistant)");
            println!("  HONEYCOMB_LLM_PROVIDER default provider selector (e.g. minimax)");
            println!("  HONEYCOMB_LLM_API_KEY  API key for selected provider");
            println!();
        }
        BinaryRole::Execution => {
            println!("Usage: honeycomb <group> <command>");
            println!();
            println!("Interactive agent (Claude Code–style):");
            println!("  honeycomb code [--skill-id ID] [--root PATH] [--use-recommended-impl] [--run-tools]");
            println!("  (or run `honeycomb-bee` with no args; default code-assistant reads HONEYCOMB_LLM_* env)");
            println!();
            println!("Execution commands:");
            println!(
                "  queen run [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN]"
            );
            println!(
                "  worker run [--worker-node ID] [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN] [--root PATH]"
            );
            println!(
                "  task submit [--task-id ID] [--tenant ID] [--namespace NS] [--goal TEXT] [--from-skill ID] [--implementation-ref REF] [--use-recommended-impl] [--queen-node ID] [--skill-ref ID] [--tool-ref ID] [--root PATH]"
            );
            println!(
                "  task demo-flow [--task-id ID] [--tenant ID] [--namespace NS] [--goal TEXT] [--from-skill ID] [--use-recommended-impl] [--queen-node ID] [--worker-node ID] [--queen-token TOKEN] [--assignment-id ID] [--attempt-id ID] [--resident-id ID] [--skill-ref ID] [--tool-ref ID] [--input TEXT] [--output TEXT] [--root PATH]"
            );
            println!(
                "  task assign [--task-id ID] [--assignment-id ID] [--attempt-id ID] [--worker-node ID] [--input TEXT] [--root PATH]"
            );
            println!(
                "  assignment list [--task-id ID] [--implementation-ref REF] [--skill-ref ID] [--worker-node ID] [--status STATUS] [--root PATH]"
            );
            println!("  assignment inspect [--task-id ID] [--assignment-id ID] [--root PATH]");
            println!(
                "  task result [--task-id ID] [--assignment-id ID] [--attempt-id ID] [--worker-node ID] [--input TEXT] [--output TEXT] [--status completed|failed] [--root PATH]"
            );
            println!(
                "  task list [--implementation-ref REF] [--skill-ref ID] [--status STATUS] [--root PATH]"
            );
            println!("  task reopen [--task-id ID] [--root PATH]");
            println!(
                "  task rerun [--task-id ID] [--from-plan PATH|--prune-plan PATH|--plan-summary PATH] [--all-failed|--all-completed] [--tenant ID] [--namespace NS] [--skill-ref ID] [--implementation-ref REF] [--goal-contains TEXT] [--assignment-status STATUS] [--has-trigger|--without-trigger] [--with-active-resident|--without-resident] [--sort target|status] [--limit N] [--dry-run] [--summary-only] [--save-plan PATH|--append-plan PATH] [--trigger-id ID] [--fire-trigger] [--schedule-now] [--worker-node ID] [--auto-complete] [--result-status completed|failed] [--output-prefix TEXT] [--json] [--root PATH]"
            );
            println!(
                "  task inspect [--task-id ID] [--root PATH] [--with-assignments] [--with-residents] [--with-triggers] [--with-executions]"
            );
            println!("  task replay [--task-id ID] [--root PATH]");
            println!("  trace tail [--task-id ID] [--implementation-ref REF] [--root PATH]");
            println!(
                "  heartbeat send [--worker-node ID] [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN] [--state TEXT] [--root PATH]"
            );
            println!(
                "  shutdown send [--worker-node ID] [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN] [--reason TEXT] [--root PATH]"
            );
            println!(
                "  trigger create [--task-id ID] [--trigger-id ID] [--trigger-type TYPE] [--schedule TEXT] [--root PATH]"
            );
            println!("  trigger inspect [--task-id ID] [--trigger-id ID] [--root PATH]");
            println!("  trigger list [--task-id ID] [--root PATH]");
            println!("  trigger pause [--task-id ID] [--trigger-id ID] [--root PATH]");
            println!("  trigger resume [--task-id ID] [--trigger-id ID] [--root PATH]");
            println!("  trigger fire [--task-id ID] [--trigger-id ID] [--root PATH]");
            println!("  trigger clear-ready [--task-id ID] [--trigger-id ID] [--root PATH]");
            println!(
                "  skill inspect [--skill-id ID] [--with-lineage] [--with-runtime] [--recommended-only] [--root PATH]"
            );
            println!("  skill list [--root PATH]");
            println!(
                "  skill execute [--skill-id ID] [--task-id ID] [--assignment-id ID] [--input TEXT] [--use-recommended-impl] [--run-tools] [--root PATH]"
            );
            println!("  tool inspect [--tool-id ID] [--with-runtime] [--root PATH]");
            println!("  tool list [--shell-only] [--blocked-only] [--root PATH]");
            println!("  tool approval-inspect [--request-id ID] [--root PATH]");
            println!(
                "  tool approval-list [--status pending|approved|rejected] [--tool-id ID] [--root PATH]"
            );
            println!("  tool approval-queue [--tool-id ID] [--owner ID] [--root PATH]");
            println!(
                "  tool approval-overdue [--tool-id ID] [--owner ID] [--threshold-minutes N] [--root PATH]"
            );
            println!(
                "  tool approval-alerts [--tool-id ID] [--owner ID] [--threshold-minutes N] [--include-acked] [--json] [--root PATH]"
            );
            println!(
                "  tool approval-inbox [--tool-id ID] [--owner ID] [--threshold-minutes N] [--json] [--root PATH]"
            );
            println!(
                "  tool execute [--tool-id ID] [--task-id ID] [--assignment-id ID] [--input TEXT] [--root PATH]"
            );
            println!("  execution inspect [--execution-id ID] [--root PATH]");
            println!(
                "  execution list [--task-id ID] [--skill-ref ID] [--tool-ref ID] [--root PATH]"
            );
            println!(
                "  resident run [--task-id ID] [--resident-id ID] [--worker-node ID] [--purpose TEXT] [--root PATH]"
            );
            println!("  resident inspect [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident heartbeat [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident pause [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident resume [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident stop [--task-id ID] [--resident-id ID] [--root PATH]");
            println!(
                "  scheduler run-once [--worker-node ID] [--limit N] [--triggered-only] [--auto-complete] [--result-status completed|failed] [--output-prefix TEXT] [--json] [--root PATH]"
            );
            println!(
                "  scheduler loop [--worker-node ID] [--iterations N] [--until-idle] [--sleep-ms N] [--limit N] [--triggered-only] [--auto-complete] [--result-status completed|failed] [--output-prefix TEXT] [--json] [--root PATH]"
            );
            println!(
                "  runtime overview [--with-details] [--with-gaps] [--with-policy] [--exclude-legacy] [--json] [--root PATH]"
            );
            println!(
                "  system overview [--owner ID] [--with-details] [--with-gaps] [--with-policy] [--with-runtime-health] [--sort count|target] [--limit N] [--summary-only] [--include-acked-policy] [--exclude-legacy] [--json] [--root PATH]"
            );
            println!(
                "  system alerts [--kind active_task|trigger_waiting_consumption|blocked_tool|overdue_request] [--owner ID] [--severity attention|warning|healthy] [--summary-by kind|owner|severity] [--sort severity|target] [--limit N] [--summary-only] [--include-acked-policy] [--exclude-legacy] [--json] [--root PATH]"
            );
            println!("  audit tail [--task-id ID] [--implementation-ref REF] [--root PATH]");
        }
        BinaryRole::Evolution => {
            println!("Usage: honeycomb-evolution <group> <command>");
            println!();
            println!("Evolution commands:");
            println!("  audit tail [--root PATH]");
            println!(
                "  fitness run [--implementation ID] [--score VALUE] [--summary TEXT] [--skill-ref ID] [--tool-ref ID] [--root PATH]"
            );
            println!("  fitness explain [--implementation ID] [--with-runtime] [--root PATH]");
            println!(
                "  governance plan [--implementation ID] [--skill-ref ID] [--tool-ref ID] [--root PATH]"
            );
            println!(
                "  governance apply [--implementation ID] [--skill-ref ID] [--tool-ref ID] [--root PATH]"
            );
            println!(
                "  reflection record [--reflection-id ID] [--title TEXT] [--period-label TEXT] [--recorded-by ID] [--decision no_major_drift|drift_detected] [--summary TEXT] [--drift TEXT] [--freeze-action TEXT] [--next-action TEXT] [--review-ref ID] [--evidence-ref PATH] [--root PATH]"
            );
            println!("  reflection inspect [--reflection-id ID] [--root PATH]");
            println!("  reflection list [--decision no_major_drift|drift_detected] [--root PATH]");
            println!(
                "  review record [--review-id ID] [--title TEXT] [--change-scope TEXT] [--requested-by ID] [--target-plane execution|evolution|cross_plane] [--target-module ID] [--writes-runtime] [--writes-long-term] [--mutates-historical-facts] [--touches-registry] [--touches-approval-or-policy] [--status open|completed] [--decision pass|pass_with_followup|needs_redesign|blocked] [--rationale TEXT] [--followup TEXT] [--evidence-ref PATH] [--root PATH]"
            );
            println!("  review suggest [--limit N] [--json] [--root PATH]");
            println!("  review materialize [--limit N] [--requested-by ID] [--root PATH]");
            println!("  review inspect [--review-id ID] [--root PATH]");
            println!(
                "  review list [--decision pass|pass_with_followup|needs_redesign|blocked] [--status open|completed] [--root PATH]"
            );
            println!("  governance-defaults inspect [--json] [--root PATH]");
            println!(
                "  governance-defaults set [--policy KEY=VALUE] [--clear-policy KEY] [--root PATH]"
            );
            println!("  registry sync [--skill-id ID] [--all] [--root PATH]");
            println!(
                "  registry overview [--with-details] [--with-gaps] [--with-policy] [--exclude-legacy] [--json] [--root PATH]"
            );
            println!("  implementation inspect [--implementation-id ID] [--root PATH]");
            println!("  implementation list [--skill-id ID] [--executor ID] [--root PATH]");
            println!(
                "  lineage show [--skill-ref ID] [--tool-ref ID] [--with-runtime] [--root PATH]"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_execution_task_demo_flow_command() {
        let args = vec!["task".to_owned(), "demo-flow".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TaskDemoFlow));
    }

    #[test]
    fn parse_bee_empty_is_code_session() {
        let args: Vec<String> = vec![];
        assert_eq!(parse_command(BinaryRole::Bee, &args), Ok(Command::Code));
    }

    #[test]
    fn parse_execution_code_command() {
        let args = vec!["code".to_owned(), "--root".to_owned(), ".".to_owned()];
        assert_eq!(parse_command(BinaryRole::Execution, &args), Ok(Command::Code));
    }

    #[test]
    fn parse_execution_unknown_command_fails() {
        let args = vec!["task".to_owned(), "unknown".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert!(command.is_err());
    }

    #[test]
    fn parse_evolution_fitness_explain_command() {
        let args = vec!["fitness".to_owned(), "explain".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::FitnessExplain));
    }

    #[test]
    fn parse_evolution_registry_overview_command() {
        let args = vec!["registry".to_owned(), "overview".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::RegistryOverview));
    }

    #[test]
    fn parse_evolution_review_record_command() {
        let args = vec!["review".to_owned(), "record".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::ReviewRecord));
    }

    #[test]
    fn parse_evolution_review_suggest_command() {
        let args = vec!["review".to_owned(), "suggest".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::ReviewSuggest));
    }

    #[test]
    fn parse_evolution_review_materialize_command() {
        let args = vec!["review".to_owned(), "materialize".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::ReviewMaterialize));
    }

    #[test]
    fn parse_evolution_governance_defaults_inspect_command() {
        let args = vec!["governance-defaults".to_owned(), "inspect".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::GovernanceDefaultsInspect));
    }

    #[test]
    fn parse_evolution_governance_defaults_set_command() {
        let args = vec!["governance-defaults".to_owned(), "set".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::GovernanceDefaultsSet));
    }

    #[test]
    fn parse_evolution_reflection_record_command() {
        let args = vec!["reflection".to_owned(), "record".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::ReflectionRecord));
    }

    #[test]
    fn parse_evolution_implementation_inspect_command() {
        let args = vec!["implementation".to_owned(), "inspect".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::ImplementationInspect));
    }

    #[test]
    fn parse_evolution_implementation_list_command() {
        let args = vec!["implementation".to_owned(), "list".to_owned()];
        let command = parse_command(BinaryRole::Evolution, &args);

        assert_eq!(command, Ok(Command::ImplementationList));
    }

    #[test]
    fn parse_execution_trigger_inspect_command() {
        let args = vec!["trigger".to_owned(), "inspect".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TriggerInspect));
    }

    #[test]
    fn parse_execution_trigger_pause_command() {
        let args = vec!["trigger".to_owned(), "pause".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TriggerPause));
    }

    #[test]
    fn parse_execution_trigger_clear_ready_command() {
        let args = vec!["trigger".to_owned(), "clear-ready".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TriggerClearReady));
    }

    #[test]
    fn parse_execution_resident_inspect_command() {
        let args = vec!["resident".to_owned(), "inspect".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ResidentInspect));
    }

    #[test]
    fn parse_execution_resident_stop_command() {
        let args = vec!["resident".to_owned(), "stop".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ResidentStop));
    }

    #[test]
    fn parse_execution_resident_pause_command() {
        let args = vec!["resident".to_owned(), "pause".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ResidentPause));
    }

    #[test]
    fn parse_execution_runtime_overview_command() {
        let args = vec!["runtime".to_owned(), "overview".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::RuntimeOverview));
    }

    #[test]
    fn parse_execution_scheduler_run_once_command() {
        let args = vec!["scheduler".to_owned(), "run-once".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::SchedulerRunOnce));
    }

    #[test]
    fn parse_execution_scheduler_loop_command() {
        let args = vec!["scheduler".to_owned(), "loop".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::SchedulerLoop));
    }

    #[test]
    fn parse_execution_system_overview_command() {
        let args = vec!["system".to_owned(), "overview".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::SystemOverview));
    }

    #[test]
    fn parse_execution_system_alerts_command() {
        let args = vec!["system".to_owned(), "alerts".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::SystemAlerts));
    }

    #[test]
    fn parse_execution_skill_execute_command() {
        let args = vec!["skill".to_owned(), "execute".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::SkillExecute));
    }

    #[test]
    fn parse_execution_execution_list_command() {
        let args = vec!["execution".to_owned(), "list".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ExecutionList));
    }

    #[test]
    fn parse_execution_tool_approval_list_command() {
        let args = vec!["tool".to_owned(), "approval-list".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ToolApprovalList));
    }

    #[test]
    fn parse_execution_tool_approval_queue_command() {
        let args = vec!["tool".to_owned(), "approval-queue".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ToolApprovalQueue));
    }

    #[test]
    fn parse_execution_tool_approval_overdue_command() {
        let args = vec!["tool".to_owned(), "approval-overdue".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ToolApprovalOverdue));
    }

    #[test]
    fn parse_execution_tool_approval_alerts_command() {
        let args = vec!["tool".to_owned(), "approval-alerts".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ToolApprovalAlerts));
    }

    #[test]
    fn parse_execution_tool_approval_inbox_command() {
        let args = vec!["tool".to_owned(), "approval-inbox".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::ToolApprovalInbox));
    }

    #[test]
    fn parse_execution_task_list_command() {
        let args = vec!["task".to_owned(), "list".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TaskList));
    }

    #[test]
    fn parse_execution_task_reopen_command() {
        let args = vec!["task".to_owned(), "reopen".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TaskReopen));
    }

    #[test]
    fn parse_execution_task_rerun_command() {
        let args = vec!["task".to_owned(), "rerun".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TaskRerun));
    }

    #[test]
    fn parse_execution_assignment_list_command() {
        let args = vec!["assignment".to_owned(), "list".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::AssignmentList));
    }

    #[test]
    fn option_value_reads_flag_value_pairs() {
        let args = vec![
            "--task-id".to_owned(),
            "task-123".to_owned(),
            "--root".to_owned(),
            ".".to_owned(),
        ];

        assert_eq!(option_value(&args, "--task-id"), Some("task-123"));
        assert_eq!(option_value(&args, "--root"), Some("."));
        assert_eq!(option_value(&args, "--missing"), None);
    }

    #[test]
    fn option_values_reads_repeated_flags() {
        let args = vec![
            "--skill-ref".to_owned(),
            "skill-a".to_owned(),
            "--tool-ref".to_owned(),
            "tool-a".to_owned(),
            "--skill-ref".to_owned(),
            "skill-b".to_owned(),
        ];

        assert_eq!(
            option_values(&args, "--skill-ref"),
            vec!["skill-a", "skill-b"]
        );
        assert_eq!(option_values(&args, "--tool-ref"), vec!["tool-a"]);
    }

    #[test]
    fn has_flag_detects_presence() {
        let args = vec![
            "--with-assignments".to_owned(),
            "--task-id".to_owned(),
            "task-123".to_owned(),
        ];

        assert!(has_flag(&args, "--with-assignments"));
        assert!(!has_flag(&args, "--missing"));
    }
}
