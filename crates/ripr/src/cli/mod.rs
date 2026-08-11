mod agent;
mod command;
mod commands;
mod commands_agent_support;
mod commands_context;
mod commands_numeric;
mod commands_options;
mod commands_timestamps;
mod execute;
mod help;
mod parse;
mod rerun;
mod suggest;

pub fn run(args: Vec<String>) -> Result<(), String> {
    execute::execute(parse::parse_args(args)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn run_rejects_unknown_command() {
        assert_eq!(
            run(args(&["ripr", "unknown"])),
            Err("unknown command \"unknown\". Run `ripr --help`.".to_string())
        );
    }

    #[test]
    fn run_dispatches_check_parse_errors() {
        assert_eq!(
            run(args(&["ripr", "check", "--format", "xml"])),
            Err(
                "unknown format \"xml\"; see `ripr check --help` for the accepted formats"
                    .to_string()
            )
        );
    }

    #[test]
    fn run_dispatches_doctor_root_parse_errors() {
        assert_eq!(
            run(args(&["ripr", "doctor", "--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn run_dispatches_init_parse_errors() {
        assert_eq!(
            run(args(&["ripr", "init", "--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn run_dispatches_remaining_top_level_commands() {
        assert_eq!(run(args(&["ripr"])), Ok(()));
        assert_eq!(run(args(&["ripr", "--version"])), Ok(()));
        assert_eq!(
            run(args(&["ripr", "explain"])),
            Err("missing finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "context"])),
            Err("missing --at or --finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "diff", "--format", "xml"])),
            Err("unknown diff format \"xml\"; expected `human`, `text`, `md`, `markdown`, or `json`".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "lsp", "--bad"])),
            Err("unknown lsp argument \"--bad\". Run `ripr lsp --help`.".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "agent", "brief", "--diff", "change.diff"])),
            Err(
                "agent brief requires --json (the supported output for this subcommand)"
                    .to_string()
            )
        );
        assert_eq!(
            run(args(&["ripr", "first-pr", "--gap-ledger"])),
            Err("missing value for --gap-ledger".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "start-here", "--gap-ledger"])),
            Err("missing value for --gap-ledger".to_string())
        );
        assert_eq!(
            run(args(&["ripr", "first-action", "--assistant-proof"])),
            Err("missing value for --assistant-proof".to_string())
        );
    }
}
