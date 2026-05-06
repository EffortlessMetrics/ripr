use crate::cli::command::CliCommand;
use crate::cli::{commands, help};

pub(super) fn execute(command: CliCommand) -> Result<(), String> {
    match command {
        CliCommand::Help => {
            help::print_help();
            Ok(())
        }
        CliCommand::Version => {
            println!("ripr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliCommand::Init(args) => commands::init(&args),
        CliCommand::Check(args) => commands::check(&args),
        CliCommand::Explain(args) => commands::explain(&args),
        CliCommand::Context(args) => commands::context(&args),
        CliCommand::Doctor(args) => commands::doctor(&args),
        CliCommand::Lsp(args) => commands::lsp(&args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn execute_handles_top_level_help_and_version() {
        assert_eq!(execute(CliCommand::Help), Ok(()));
        assert_eq!(execute(CliCommand::Version), Ok(()));
    }

    #[test]
    fn execute_dispatches_subcommand_args_without_reparsing_argv() {
        assert_eq!(
            execute(CliCommand::Check(args(&["--format", "xml"]))),
            Err("unknown format \"xml\"".to_string())
        );
        assert_eq!(
            execute(CliCommand::Doctor(args(&["--root"]))),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            execute(CliCommand::Init(args(&["--root"]))),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn execute_dispatches_remaining_command_handlers() {
        assert_eq!(
            execute(CliCommand::Explain(Vec::new())),
            Err("missing finding selector".to_string())
        );
        assert_eq!(
            execute(CliCommand::Context(Vec::new())),
            Err("missing --at or --finding selector".to_string())
        );
        assert_eq!(
            execute(CliCommand::Lsp(args(&["--bad"]))),
            Err("unknown lsp argument \"--bad\"".to_string())
        );
    }
}
