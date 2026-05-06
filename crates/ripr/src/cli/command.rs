#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CliCommand {
    Help,
    Version,
    Init,
    Check,
    Explain,
    Context,
    Doctor,
    Lsp,
}

impl CliCommand {
    pub(super) fn from_arg(arg: Option<&str>) -> Result<Self, String> {
        match arg {
            None | Some("--help" | "-h") => Ok(Self::Help),
            Some("--version" | "-V") => Ok(Self::Version),
            Some("init") => Ok(Self::Init),
            Some("check") => Ok(Self::Check),
            Some("explain") => Ok(Self::Explain),
            Some("context") => Ok(Self::Context),
            Some("doctor") => Ok(Self::Doctor),
            Some("lsp") => Ok(Self::Lsp),
            Some(command) => Err(format!("unknown command {command:?}. Run `ripr --help`.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliCommand;

    #[test]
    fn cli_command_from_arg_maps_current_command_surface() {
        for (arg, expected) in [
            (None, CliCommand::Help),
            (Some("--help"), CliCommand::Help),
            (Some("-h"), CliCommand::Help),
            (Some("--version"), CliCommand::Version),
            (Some("-V"), CliCommand::Version),
            (Some("init"), CliCommand::Init),
            (Some("check"), CliCommand::Check),
            (Some("explain"), CliCommand::Explain),
            (Some("context"), CliCommand::Context),
            (Some("doctor"), CliCommand::Doctor),
            (Some("lsp"), CliCommand::Lsp),
        ] {
            assert_eq!(CliCommand::from_arg(arg), Ok(expected));
        }
    }

    #[test]
    fn cli_command_from_arg_preserves_unknown_command_error() {
        assert_eq!(
            CliCommand::from_arg(Some("unknown")),
            Err("unknown command \"unknown\". Run `ripr --help`.".to_string())
        );
    }
}
