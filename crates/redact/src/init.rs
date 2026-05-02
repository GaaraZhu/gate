use common::error::exit_with_error;
use common::harness::is_agent_harness;

pub fn run(_harness: &str) {
    if is_agent_harness() {
        exit_with_error("redact init is not available inside an agent harness");
    }
    // TODO: implement in Milestone 5
    eprintln!("redact init: not yet implemented");
    std::process::exit(1);
}
