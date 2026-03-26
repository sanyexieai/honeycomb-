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
    AssignmentInspect,
    TaskResult,
    TaskInspect,
    TaskReplay,
    TraceTail,
    TriggerCreate,
    TriggerList,
    TriggerFire,
    HeartbeatSend,
    ShutdownSend,
    ResidentRun,
    AuditTail,
    FitnessRun,
    FitnessExplain,
    GovernancePlan,
    GovernanceApply,
    RegistrySync,
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
            ("assignment", "inspect") => Ok(Command::AssignmentInspect),
            ("task", "result") => Ok(Command::TaskResult),
            ("task", "inspect") => Ok(Command::TaskInspect),
            ("task", "replay") => Ok(Command::TaskReplay),
            ("trace", "tail") => Ok(Command::TraceTail),
            ("trigger", "create") => Ok(Command::TriggerCreate),
            ("trigger", "list") => Ok(Command::TriggerList),
            ("trigger", "fire") => Ok(Command::TriggerFire),
            ("heartbeat", "send") => Ok(Command::HeartbeatSend),
            ("shutdown", "send") => Ok(Command::ShutdownSend),
            ("resident", "run") => Ok(Command::ResidentRun),
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
        Command::AssignmentInspect => "assignment inspect",
        Command::TaskResult => "task result",
        Command::TaskInspect => "task inspect",
        Command::TaskReplay => "task replay",
        Command::TraceTail => "trace tail",
        Command::TriggerCreate => "trigger create",
        Command::TriggerList => "trigger list",
        Command::TriggerFire => "trigger fire",
        Command::HeartbeatSend => "heartbeat send",
        Command::ShutdownSend => "shutdown send",
        Command::ResidentRun => "resident run",
        Command::AuditTail => "audit tail",
        Command::FitnessRun => "fitness run",
        Command::FitnessExplain => "fitness explain",
        Command::GovernancePlan => "governance plan",
        Command::GovernanceApply => "governance apply",
        Command::RegistrySync => "registry sync",
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

pub(crate) fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn print_help(role: BinaryRole) {
    match role {
        BinaryRole::Execution => {
            println!("Usage: honeycomb <group> <command>");
            println!();
            println!("Execution commands:");
            println!("  queen run [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN]");
            println!("  worker run [--worker-node ID] [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN] [--root PATH]");
            println!("  task submit [--task-id ID] [--tenant ID] [--namespace NS] [--goal TEXT] [--queen-node ID] [--root PATH]");
            println!("  task demo-flow [--task-id ID] [--tenant ID] [--namespace NS] [--goal TEXT] [--queen-node ID] [--worker-node ID] [--queen-token TOKEN] [--assignment-id ID] [--attempt-id ID] [--input TEXT] [--output TEXT] [--root PATH]");
            println!("  task assign [--task-id ID] [--assignment-id ID] [--attempt-id ID] [--worker-node ID] [--input TEXT] [--root PATH]");
            println!("  assignment inspect [--task-id ID] [--assignment-id ID] [--root PATH]");
            println!("  task result [--task-id ID] [--assignment-id ID] [--attempt-id ID] [--worker-node ID] [--input TEXT] [--output TEXT] [--status completed|failed] [--root PATH]");
            println!("  task inspect [--task-id ID] [--root PATH] [--with-assignments]");
            println!("  task replay [--task-id ID] [--root PATH]");
            println!("  trace tail [--task-id ID] [--root PATH]");
            println!("  heartbeat send [--worker-node ID] [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN] [--state TEXT] [--root PATH]");
            println!("  shutdown send [--worker-node ID] [--queen-node ID] [--task-id ID] [--tenant ID] [--namespace NS] [--queen-token TOKEN] [--reason TEXT] [--root PATH]");
            println!("  trigger create");
            println!("  trigger list");
            println!("  trigger fire");
            println!("  resident run");
            println!("  audit tail [--task-id ID] [--root PATH]");
        }
        BinaryRole::Evolution => {
            println!("Usage: honeycomb-evolution <group> <command>");
            println!();
            println!("Evolution commands:");
            println!("  audit tail [--root PATH]");
            println!("  fitness run [--implementation ID] [--score VALUE] [--summary TEXT] [--root PATH]");
            println!("  fitness explain [--implementation ID] [--root PATH]");
            println!("  governance plan");
            println!("  governance apply");
            println!("  registry sync");
            println!("  lineage show");
            println!("  practice publish");
        }
    }
}
