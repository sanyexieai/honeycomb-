use std::process::ExitCode;

use honeycomb::app::{run, BinaryRole};

fn main() -> ExitCode {
    run(BinaryRole::Execution)
}
