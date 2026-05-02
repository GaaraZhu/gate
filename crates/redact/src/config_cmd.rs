use common::error::exit_with_error;
use common::harness::is_agent_harness;

pub fn run(show_path: bool, print_config: bool, _init_only: bool) {
    // --path and --print are permitted inside a harness; interactive edit is not
    let interactive = !show_path && !print_config;
    if interactive && is_agent_harness() {
        exit_with_error("redact config: interactive mode is not available inside an agent harness");
    }
    // TODO: implement in Milestone 5
    eprintln!("redact config: not yet implemented");
    std::process::exit(1);
}
