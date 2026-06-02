#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CliCommand {
    Help,
    Version,
    Init(Vec<String>),
    Pilot(Vec<String>),
    Outcome(Vec<String>),
    EvidenceHealth(Vec<String>),
    ReviewComments(Vec<String>),
    Gate(Vec<String>),
    Baseline(Vec<String>),
    Zero(Vec<String>),
    Policy(Vec<String>),
    PrLedger(Vec<String>),
    PrComments(Vec<String>),
    PrReview(Vec<String>),
    CoverageGrip(Vec<String>),
    AssistantLoop(Vec<String>),
    FirstPr(Vec<String>),
    FirstAction(Vec<String>),
    Reports(Vec<String>),
    Calibrate(Vec<String>),
    Agent(Vec<String>),
    Swarm(Vec<String>),
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
            Some("evidence-health") => Ok(Self::EvidenceHealth(command_args)),
            Some("review-comments") => Ok(Self::ReviewComments(command_args)),
            Some("gate") => Ok(Self::Gate(command_args)),
            Some("baseline") => Ok(Self::Baseline(command_args)),
            Some("zero") => Ok(Self::Zero(command_args)),
            Some("policy") => Ok(Self::Policy(command_args)),
            Some("pr-ledger") => Ok(Self::PrLedger(command_args)),
            Some("pr-comments") => Ok(Self::PrComments(command_args)),
            Some("pr-review") => Ok(Self::PrReview(command_args)),
            Some("coverage-grip") => Ok(Self::CoverageGrip(command_args)),
            Some("assistant-loop") => Ok(Self::AssistantLoop(command_args)),
            Some("first-pr") | Some("start-here") => Ok(Self::FirstPr(command_args)),
            Some("first-action") => Ok(Self::FirstAction(command_args)),
            Some("reports") => Ok(Self::Reports(command_args)),
            Some("calibrate") => Ok(Self::Calibrate(command_args)),
            Some("agent") => Ok(Self::Agent(command_args)),
            Some("swarm") => Ok(Self::Swarm(command_args)),
            Some("check") => Ok(Self::Check(command_args)),
            Some("explain") => Ok(Self::Explain(command_args)),
            Some("context") => Ok(Self::Context(command_args)),
            Some("doctor") => Ok(Self::Doctor(command_args)),
            Some("lsp") => Ok(Self::Lsp(command_args)),
            Some(command) => Err(unknown_command_error(command)),
        }
    }
}

const KNOWN_COMMANDS: &[&str] = &[
    "init",
    "pilot",
    "outcome",
    "evidence-health",
    "review-comments",
    "gate",
    "baseline",
    "zero",
    "policy",
    "pr-ledger",
    "pr-comments",
    "pr-review",
    "coverage-grip",
    "assistant-loop",
    "first-action",
    "reports",
    "calibrate",
    "agent",
    "swarm",
    "check",
    "explain",
    "context",
    "doctor",
    "lsp",
];

fn unknown_command_error(command: &str) -> String {
    match closest_command(command) {
        Some(suggestion) => {
            format!("unknown command {command:?}. Did you mean `{suggestion}`? Run `ripr --help`.")
        }
        None => format!("unknown command {command:?}. Run `ripr --help`."),
    }
}

fn closest_command(command: &str) -> Option<&'static str> {
    let typo_budget = if command.len() <= 4 { 1 } else { 3 };
    KNOWN_COMMANDS
        .iter()
        .copied()
        .map(|known| (known, edit_distance(command, known)))
        .filter(|(_, distance)| *distance <= typo_budget)
        .min_by_key(|(known, distance)| (*distance, *known))
        .map(|(known, _)| known)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_idx, left_char) in left.chars().enumerate() {
        current[0] = left_idx + 1;
        for (right_idx, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            let deletion = previous[right_idx + 1] + 1;
            let insertion = current[right_idx] + 1;
            let substitution = previous[right_idx] + substitution_cost;
            current[right_idx + 1] = deletion.min(insertion).min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
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
            (
                Some("evidence-health"),
                CliCommand::EvidenceHealth(Vec::new()),
            ),
            (
                Some("review-comments"),
                CliCommand::ReviewComments(Vec::new()),
            ),
            (Some("gate"), CliCommand::Gate(Vec::new())),
            (Some("baseline"), CliCommand::Baseline(Vec::new())),
            (Some("zero"), CliCommand::Zero(Vec::new())),
            (Some("policy"), CliCommand::Policy(Vec::new())),
            (Some("pr-ledger"), CliCommand::PrLedger(Vec::new())),
            (Some("pr-comments"), CliCommand::PrComments(Vec::new())),
            (Some("pr-review"), CliCommand::PrReview(Vec::new())),
            (Some("coverage-grip"), CliCommand::CoverageGrip(Vec::new())),
            (
                Some("assistant-loop"),
                CliCommand::AssistantLoop(Vec::new()),
            ),
            (Some("first-pr"), CliCommand::FirstPr(Vec::new())),
            (Some("start-here"), CliCommand::FirstPr(Vec::new())),
            (Some("first-action"), CliCommand::FirstAction(Vec::new())),
            (Some("reports"), CliCommand::Reports(Vec::new())),
            (Some("calibrate"), CliCommand::Calibrate(Vec::new())),
            (Some("agent"), CliCommand::Agent(Vec::new())),
            (Some("swarm"), CliCommand::Swarm(Vec::new())),
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

    #[test]
    fn cli_command_from_parts_suggests_nearest_known_command_for_typos() {
        assert_eq!(
            CliCommand::from_parts(Some("chekc"), Vec::new()),
            Err("unknown command \"chekc\". Did you mean `check`? Run `ripr --help`.".to_string())
        );
        assert_eq!(
            CliCommand::from_parts(Some("review-comment"), Vec::new()),
            Err(
                "unknown command \"review-comment\". Did you mean `review-comments`? Run `ripr --help`."
                    .to_string()
            )
        );
    }
}
