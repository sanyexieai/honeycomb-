//! Interactive Code session (Claude Code–style REPL) backed by `skill execute` via BeeSession.

use std::process::ExitCode;

use crate::app::bee::{BeeProfile, BeeSession};
use crate::app::cli::option_value;

pub(crate) fn handle_code(args: &[String]) -> ExitCode {
    let rest = if !args.is_empty() && args[0] == "code" {
        &args[1..]
    } else {
        args
    };

    let root = option_value(rest, "--root").unwrap_or(".").to_string();
    let profile = BeeProfile::from_env_and_args(rest);
    let mut session = BeeSession::new(profile, root, rest);
    session.print_banner();
    session.repl_loop()
}
