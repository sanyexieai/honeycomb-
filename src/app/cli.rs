use std::env;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryRole {
    Execution,
    Evolution,
}

impl BinaryRole {
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Execution => "honeycomb",
            Self::Evolution => "honeycomb-evolution",
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
    TaskBackfillImplementation,
    TaskInspect,
    TaskReplay,
    TraceTail,
    TriggerCreate,
    TriggerInspect,
    TriggerList,
    TriggerPause,
    TriggerResume,
    TriggerFire,
    SkillRegister,
    SkillInspect,
    SkillList,
    ToolRegister,
    ToolInspect,
    ToolList,
    HeartbeatSend,
    ShutdownSend,
    ResidentRun,
    ResidentInspect,
    ResidentHeartbeat,
    ResidentPause,
    ResidentResume,
    ResidentStop,
    RuntimeOverview,
    AuditTail,
    FitnessRun,
    FitnessExplain,
    GovernancePlan,
    GovernanceApply,
    RegistrySync,
    RegistryOverview,
    LineageShow,
    PracticePublish,
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
        return Ok(Command::Help);
    }

    let tokens: Vec<&str> = args.iter().map(String::as_str).collect();
    match role {
        BinaryRole::Execution => parse_execution_command(&tokens),
        BinaryRole::Evolution => parse_evolution_command(&tokens),
    }
}

fn parse_execution_command(tokens: &[&str]) -> Result<Command, String> {
    match tokens {
        ["help"] | ["--help"] | ["-h"] => Ok(Command::Help),
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
            ("task", "backfill-implementation") => Ok(Command::TaskBackfillImplementation),
            ("task", "inspect") => Ok(Command::TaskInspect),
            ("task", "replay") => Ok(Command::TaskReplay),
            ("trace", "tail") => Ok(Command::TraceTail),
            ("trigger", "create") => Ok(Command::TriggerCreate),
            ("trigger", "inspect") => Ok(Command::TriggerInspect),
            ("trigger", "list") => Ok(Command::TriggerList),
            ("trigger", "pause") => Ok(Command::TriggerPause),
            ("trigger", "resume") => Ok(Command::TriggerResume),
            ("trigger", "fire") => Ok(Command::TriggerFire),
            ("skill", "register") => Ok(Command::SkillRegister),
            ("skill", "inspect") => Ok(Command::SkillInspect),
            ("skill", "list") => Ok(Command::SkillList),
            ("tool", "register") => Ok(Command::ToolRegister),
            ("tool", "inspect") => Ok(Command::ToolInspect),
            ("tool", "list") => Ok(Command::ToolList),
            ("heartbeat", "send") => Ok(Command::HeartbeatSend),
            ("shutdown", "send") => Ok(Command::ShutdownSend),
            ("resident", "run") => Ok(Command::ResidentRun),
            ("resident", "inspect") => Ok(Command::ResidentInspect),
            ("resident", "heartbeat") => Ok(Command::ResidentHeartbeat),
            ("resident", "pause") => Ok(Command::ResidentPause),
            ("resident", "resume") => Ok(Command::ResidentResume),
            ("resident", "stop") => Ok(Command::ResidentStop),
            ("runtime", "overview") => Ok(Command::RuntimeOverview),
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
            ("registry", "sync") => Ok(Command::RegistrySync),
            ("registry", "overview") => Ok(Command::RegistryOverview),
            ("lineage", "show") => Ok(Command::LineageShow),
            ("practice", "publish") => Ok(Command::PracticePublish),
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
        Command::TaskBackfillImplementation => "task backfill-implementation",
        Command::TaskInspect => "task inspect",
        Command::TaskReplay => "task replay",
        Command::TraceTail => "trace tail",
        Command::TriggerCreate => "trigger create",
        Command::TriggerInspect => "trigger inspect",
        Command::TriggerList => "trigger list",
        Command::TriggerPause => "trigger pause",
        Command::TriggerResume => "trigger resume",
        Command::TriggerFire => "trigger fire",
        Command::SkillRegister => "skill register",
        Command::SkillInspect => "skill inspect",
        Command::SkillList => "skill list",
        Command::ToolRegister => "tool register",
        Command::ToolInspect => "tool inspect",
        Command::ToolList => "tool list",
        Command::HeartbeatSend => "heartbeat send",
        Command::ShutdownSend => "shutdown send",
        Command::ResidentRun => "resident run",
        Command::ResidentInspect => "resident inspect",
        Command::ResidentHeartbeat => "resident heartbeat",
        Command::ResidentPause => "resident pause",
        Command::ResidentResume => "resident resume",
        Command::ResidentStop => "resident stop",
        Command::RuntimeOverview => "runtime overview",
        Command::AuditTail => "audit tail",
        Command::FitnessRun => "fitness run",
        Command::FitnessExplain => "fitness explain",
        Command::GovernancePlan => "governance plan",
        Command::GovernanceApply => "governance apply",
        Command::RegistrySync => "registry sync",
        Command::RegistryOverview => "registry overview",
        Command::LineageShow => "lineage show",
        Command::PracticePublish => "practice publish",
    }
}

pub(crate) fn execute_command(role: BinaryRole, command: Command, args: &[String]) -> ExitCode {
    match role {
        BinaryRole::Execution => super::execution::handle(command, args),
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
        BinaryRole::Execution => {
            println!("Usage: honeycomb <group> <command>");
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
            println!(
                "  task backfill-implementation [--task-id ID] [--all] [--root PATH]"
            );
            println!(
                "  task inspect [--task-id ID] [--root PATH] [--with-assignments] [--with-residents] [--with-triggers]"
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
            println!(
                "  skill register [--skill-id ID] [--display-name TEXT] [--description TEXT] [--implementation-ref TEXT] [--owner ID] [--version TEXT] [--default-tool-ref ID] [--goal-template TEXT] [--root PATH]"
            );
            println!(
                "  skill inspect [--skill-id ID] [--with-lineage] [--with-runtime] [--recommended-only] [--root PATH]"
            );
            println!("  skill list [--root PATH]");
            println!(
                "  tool register [--tool-id ID] [--display-name TEXT] [--description TEXT] [--entrypoint TEXT] [--owner ID] [--version TEXT] [--root PATH]"
            );
            println!("  tool inspect [--tool-id ID] [--with-runtime] [--root PATH]");
            println!("  tool list [--root PATH]");
            println!(
                "  resident run [--task-id ID] [--resident-id ID] [--worker-node ID] [--purpose TEXT] [--root PATH]"
            );
            println!("  resident inspect [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident heartbeat [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident pause [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident resume [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  resident stop [--task-id ID] [--resident-id ID] [--root PATH]");
            println!("  runtime overview [--with-details] [--with-gaps] [--exclude-legacy] [--root PATH]");
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
            println!("  registry sync [--skill-id ID] [--all] [--root PATH]");
            println!("  registry overview [--with-details] [--with-gaps] [--exclude-legacy] [--root PATH]");
            println!("  lineage show [--skill-ref ID] [--tool-ref ID] [--with-runtime] [--root PATH]");
            println!("  practice publish");
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
    fn parse_execution_skill_register_command() {
        let args = vec!["skill".to_owned(), "register".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::SkillRegister));
    }

    #[test]
    fn parse_execution_task_list_command() {
        let args = vec!["task".to_owned(), "list".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TaskList));
    }

    #[test]
    fn parse_execution_task_backfill_command() {
        let args = vec!["task".to_owned(), "backfill-implementation".to_owned()];
        let command = parse_command(BinaryRole::Execution, &args);

        assert_eq!(command, Ok(Command::TaskBackfillImplementation));
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
