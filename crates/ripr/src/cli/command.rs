#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CliCommand {
    Help,
    Version,
    Init(Vec<String>),
    Pilot(Vec<String>),
    Outcome(Vec<String>),
    Calibrate(Vec<String>),
    Agent(Vec<String>),
    Check(Vec<String>),
    Explain(Vec<String>),
    Context(Vec<String>),
    Doctor(Vec<String>),
    Lsp(Vec<String>),
}

impl CliCommand {
    pub(super) fn from_parts(arg: Option<&str>, command_args: Vec<String>) -> Result<Self, String> {
        match arg {
            None | Some("--help" | "-h") => Ok(Self::Help),
            Some("--version" | "-V") => Ok(Self::Version),
            Some("init") => Ok(Self::Init(command_args)),
            Some("pilot") => Ok(Self::Pilot(command_args)),
            Some("outcome") => Ok(Self::Outcome(command_args)),
            Some("calibrate") => Ok(Self::Calibrate(command_args)),
            Some("agent") => Ok(Self::Agent(command_args)),
            Some("check") => Ok(Self::Check(command_args)),
            Some("explain") => Ok(Self::Explain(command_args)),
            Some("context") => Ok(Self::Context(command_args)),
            Some("doctor") => Ok(Self::Doctor(command_args)),
            Some("lsp") => Ok(Self::Lsp(command_args)),
            Some(command) => Err(format!("unknown command {command:?}. Run `ripr --help`.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliCommand;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn cli_command_from_parts_maps_current_command_surface() {
        for (arg, expected) in [
            (None, CliCommand::Help),
            (Some("--help"), CliCommand::Help),
            (Some("-h"), CliCommand::Help),
            (Some("--version"), CliCommand::Version),
            (Some("-V"), CliCommand::Version),
            (Some("init"), CliCommand::Init(Vec::new())),
            (Some("pilot"), CliCommand::Pilot(Vec::new())),
            (Some("outcome"), CliCommand::Outcome(Vec::new())),
            (Some("calibrate"), CliCommand::Calibrate(Vec::new())),
            (Some("agent"), CliCommand::Agent(Vec::new())),
            (Some("check"), CliCommand::Check(Vec::new())),
            (Some("explain"), CliCommand::Explain(Vec::new())),
            (Some("context"), CliCommand::Context(Vec::new())),
            (Some("doctor"), CliCommand::Doctor(Vec::new())),
            (Some("lsp"), CliCommand::Lsp(Vec::new())),
        ] {
            assert_eq!(CliCommand::from_parts(arg, Vec::new()), Ok(expected));
        }
    }

    #[test]
    fn cli_command_from_parts_preserves_subcommand_args() {
        assert_eq!(
            CliCommand::from_parts(Some("check"), args(&["--format", "json"])),
            Ok(CliCommand::Check(args(&["--format", "json"])))
        );
    }

    #[test]
    fn cli_command_from_parts_preserves_unknown_command_error() {
        assert_eq!(
            CliCommand::from_parts(Some("unknown"), Vec::new()),
            Err("unknown command \"unknown\". Run `ripr --help`.".to_string())
        );
    }
}
