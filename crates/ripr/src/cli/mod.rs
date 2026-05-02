mod commands;
mod help;
mod parse;

pub fn run(args: Vec<String>) -> Result<(), String> {
    match args.get(1).map(|s| s.as_str()) {
        None | Some("--help" | "-h") => {
            help::print_help();
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("ripr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("check") => commands::check(&args[2..]),
        Some("explain") => commands::explain(&args[2..]),
        Some("context") => commands::context(&args[2..]),
        Some("doctor") => commands::doctor(&args[2..]),
        Some("lsp") => commands::lsp(&args[2..]),
        Some(command) => Err(format!("unknown command {command:?}. Run `ripr --help`.")),
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
}
