use std::process::ExitCode;

use honeycomb::app::{BinaryRole, run};

fn main() -> ExitCode {
    // Bee runtime binary: no args → Code session.
    run(BinaryRole::Bee)
}
