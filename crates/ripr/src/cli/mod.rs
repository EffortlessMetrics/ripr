mod command;
mod commands;
mod help;
mod parse;

use command::CliCommand;

pub fn run(args: Vec<String>) -> Result<(), String> {
    match CliCommand::from_arg(args.get(1).map(|s| s.as_str()))? {
        CliCommand::Help => {
            help::print_help();
            Ok(())
        }
        CliCommand::Version => {
            println!("ripr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliCommand::Init => commands::init(&args[2..]),
        CliCommand::Check => commands::check(&args[2..]),
        CliCommand::Explain => commands::explain(&args[2..]),
        CliCommand::Context => commands::context(&args[2..]),
        CliCommand::Doctor => commands::doctor(&args[2..]),
        CliCommand::Lsp => commands::lsp(&args[2..]),
    }
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
            Err("unknown format \"xml\"".to_string())
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
}
