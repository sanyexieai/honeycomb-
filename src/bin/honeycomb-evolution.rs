use std::process::ExitCode;

use honeycomb::app::{BinaryRole, run};

fn main() -> ExitCode {
    run(BinaryRole::Evolution)
}
