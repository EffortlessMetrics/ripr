#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CliCommand {
    Help,
    /// `ripr help --all`: the exhaustive command reference.
    HelpAll,
    Version,
    Init(Vec<String>),
    Config(Vec<String>),
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
    Receipt(Vec<String>),
    Agent(Vec<String>),
    Swarm(Vec<String>),
    Diff(Vec<String>),
    Check(Vec<String>),
    Explain(Vec<String>),
    Context(Vec<String>),
    Doctor(Vec<String>),
    Lsp(Vec<String>),
    PrSummary(Vec<String>),
    Annotations(Vec<String>),
    PrEvidence(Vec<String>),
    ImpactedEvidence(Vec<String>),
    RiprPlus(Vec<String>),
    Cache(Vec<String>),
    Rerun(Vec<String>),
}

impl CliCommand {
    pub(super) fn from_parts(arg: Option<&str>, command_args: Vec<String>) -> Result<Self, String> {
        match arg {
            // `--all` after a help request selects the exhaustive reference
            // rather than the one-screen overview (#1613). Accepted on both
            // spellings so neither `ripr --help --all` nor `ripr help --all`
            // is a surprising error.
            Some("--help" | "-h") if wants_all(&command_args) => Ok(Self::HelpAll),
            None | Some("--help" | "-h") => Ok(Self::Help),
            // `ripr help` (no subcommand) prints the top-level help, identical
            // to `ripr --help`. `ripr help <command> [args...]` is the standard
            // CLI convention (git, cargo, kubectl) and is dispatched as
            // `ripr <command> --help [args...]` — i.e. the rest of the args are
            // preserved with `--help` prepended, so each command's existing
            // `--help` branch prints its own help. `help` followed by an
            // unknown command still returns the unknown-command error so a
            // typo is not silently swallowed.
            Some("help") => {
                if wants_all(&command_args) {
                    return Ok(Self::HelpAll);
                }
                let (target, rest) = match command_args.split_first() {
                    Some((target, rest)) => (target.clone(), rest.to_vec()),
                    None => return Ok(Self::Help),
                };
                let mut injected = Vec::with_capacity(rest.len() + 1);
                injected.push("--help".to_string());
                injected.extend(rest);
                Self::from_parts(Some(&target), injected)
            }
            Some("--version" | "-V") => Ok(Self::Version),
            Some("init") => Ok(Self::Init(command_args)),
            Some("config") => Ok(Self::Config(command_args)),
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
            Some("receipt") => Ok(Self::Receipt(command_args)),
            Some("agent") => Ok(Self::Agent(command_args)),
            Some("swarm") => Ok(Self::Swarm(command_args)),
            Some("diff") => Ok(Self::Diff(command_args)),
            Some("check") => Ok(Self::Check(command_args)),
            Some("explain") => Ok(Self::Explain(command_args)),
            Some("context") => Ok(Self::Context(command_args)),
            Some("doctor") => Ok(Self::Doctor(command_args)),
            Some("lsp") => Ok(Self::Lsp(command_args)),
            Some("pr-summary") => Ok(Self::PrSummary(command_args)),
            Some("annotations") => Ok(Self::Annotations(command_args)),
            Some("pr-evidence") => Ok(Self::PrEvidence(command_args)),
            Some("impacted-evidence") => Ok(Self::ImpactedEvidence(command_args)),
            Some("plus") => Ok(Self::RiprPlus(command_args)),
            Some("cache") => Ok(Self::Cache(command_args)),
            Some("rerun") => Ok(Self::Rerun(command_args)),
            Some(command) => Err(unknown_command_error(command)),
        }
    }
}

/// Whether a help invocation asked for the exhaustive reference.
///
/// Scanned rather than positionally matched so `ripr help --all` and
/// `ripr --help --all` behave the same.
/// Whether a help request selects the exhaustive reference.
///
/// Deliberately only the **first** argument. Scanning all of them makes `--all`
/// swallow the rest of the help grammar: `ripr help check --all` would print the
/// global catalog instead of dispatching to `check --help`, and
/// `ripr help <typo> --all` would exit 0 with the catalog instead of the
/// unknown-command error the `from_parts` `help` arm promises. `--all` is a
/// modifier on *bare* help, so it has to be in the position bare help occupies.
fn wants_all(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "--all")
}

/// Every command the parser accepts. Also the source for typo suggestions and
/// for the `ripr help --all` completeness test, so a command cannot be
/// reachable-but-undocumented.
pub(super) const KNOWN_COMMANDS: &[&str] = &[
    "init",
    "config",
    "help",
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
    "first-pr",
    "start-here",
    "first-action",
    "reports",
    "calibrate",
    "receipt",
    "agent",
    "swarm",
    "diff",
    "check",
    "explain",
    "context",
    "doctor",
    "lsp",
    "cache",
    "pr-summary",
    "annotations",
    "pr-evidence",
    "impacted-evidence",
    "plus",
    "rerun",
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
    use super::{CliCommand, KNOWN_COMMANDS, closest_command, unknown_command_error};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    /// `--all` selects the exhaustive reference on both help spellings, and
    /// nothing else changes: a bare help request still gets the short screen,
    /// and `help <command>` still routes to that command's own help (#1613).
    #[test]
    fn help_all_is_reachable_from_both_help_spellings() {
        for args in [
            vec!["--all".to_string()],
            vec!["--all".to_string(), "extra".to_string()],
        ] {
            assert_eq!(
                CliCommand::from_parts(Some("help"), args.clone()),
                Ok(CliCommand::HelpAll),
                "`ripr help {args:?}` should select the full reference"
            );
            assert_eq!(
                CliCommand::from_parts(Some("--help"), args.clone()),
                Ok(CliCommand::HelpAll),
                "`ripr --help {args:?}` should select the full reference"
            );
            assert_eq!(
                CliCommand::from_parts(Some("-h"), args),
                Ok(CliCommand::HelpAll)
            );
        }

        // Unchanged behavior: bare help, and `help <command>`.
        assert_eq!(
            CliCommand::from_parts(Some("help"), Vec::new()),
            Ok(CliCommand::Help)
        );
        assert_eq!(
            CliCommand::from_parts(None, Vec::new()),
            Ok(CliCommand::Help)
        );
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["check"])),
            CliCommand::from_parts(Some("check"), args(&["--help"]))
        );
        // `--all` is a help selector, not a command: it must not fall through
        // to the unknown-command error.
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["--all"])),
            Ok(CliCommand::HelpAll)
        );
    }

    /// A trailing `--all` must not capture the help request away from the
    /// command it was asked about, or away from the unknown-command error.
    ///
    /// The first version of `wants_all` scanned every argument, so both of these
    /// silently became the global catalog — and `ripr help <typo> --all` exited
    /// 0, contradicting the `help` arm's documented promise that a typo is not
    /// swallowed. `help_all_is_reachable_from_both_help_spellings` did not catch
    /// it because every case there puts `--all` first, which is the one position
    /// where scanning and first-arg agree.
    #[test]
    fn trailing_all_does_not_capture_command_local_or_unknown_help() {
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["check", "--all"])),
            CliCommand::from_parts(Some("check"), args(&["--help", "--all"])),
            "`ripr help check --all` should still dispatch to check's own help"
        );
        assert_ne!(
            CliCommand::from_parts(Some("help"), args(&["check", "--all"])),
            Ok(CliCommand::HelpAll),
            "a command-local `--all` must not select the global catalog"
        );

        let typo = CliCommand::from_parts(Some("help"), args(&["chekc", "--all"]));
        assert_eq!(
            typo,
            Err(unknown_command_error("chekc")),
            "`ripr help <typo> --all` should still report the typo"
        );
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
            (Some("config"), CliCommand::Config(Vec::new())),
            (Some("pilot"), CliCommand::Pilot(Vec::new())),
            (Some("outcome"), CliCommand::Outcome(Vec::new())),
            (Some("rerun"), CliCommand::Rerun(Vec::new())),
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
            (Some("receipt"), CliCommand::Receipt(Vec::new())),
            (Some("agent"), CliCommand::Agent(Vec::new())),
            (Some("swarm"), CliCommand::Swarm(Vec::new())),
            (Some("diff"), CliCommand::Diff(Vec::new())),
            (Some("check"), CliCommand::Check(Vec::new())),
            (Some("explain"), CliCommand::Explain(Vec::new())),
            (Some("context"), CliCommand::Context(Vec::new())),
            (Some("doctor"), CliCommand::Doctor(Vec::new())),
            (Some("lsp"), CliCommand::Lsp(Vec::new())),
            (Some("pr-summary"), CliCommand::PrSummary(Vec::new())),
            (Some("annotations"), CliCommand::Annotations(Vec::new())),
            (Some("pr-evidence"), CliCommand::PrEvidence(Vec::new())),
            (
                Some("impacted-evidence"),
                CliCommand::ImpactedEvidence(Vec::new()),
            ),
            (Some("plus"), CliCommand::RiprPlus(Vec::new())),
            (Some("cache"), CliCommand::Cache(Vec::new())),
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

    #[test]
    fn closest_command_suggests_previously_missing_commands() {
        // These commands were missing from KNOWN_COMMANDS before #1769; verify
        // they now produce typo suggestions instead of a bare "unknown" error.
        assert_eq!(closest_command("firts-pr"), Some("first-pr"));
        assert_eq!(closest_command("start-hear"), Some("start-here"));
        assert_eq!(closest_command("pr-sumary"), Some("pr-summary"));
        assert_eq!(closest_command("annotatons"), Some("annotations"));
        assert_eq!(closest_command("pr-evdence"), Some("pr-evidence"));
        assert_eq!(
            closest_command("impacted-evdence"),
            Some("impacted-evidence")
        );
        assert_eq!(closest_command("pls"), Some("plus"));
    }

    #[test]
    fn known_commands_covers_every_parser_spelling() {
        // Every command spelling accepted by from_parts must appear in
        // KNOWN_COMMANDS so typo suggestions work. Help/Version are flags,
        // not subcommands, so they are intentionally excluded.
        for known in KNOWN_COMMANDS {
            assert!(
                CliCommand::from_parts(Some(known), Vec::new()).is_ok(),
                "KNOWN_COMMANDS entry {known:?} is not accepted by from_parts"
            );
        }
    }

    #[test]
    fn help_with_no_subcommand_returns_top_level_help() {
        // `ripr help` is identical to `ripr --help`.
        assert_eq!(
            CliCommand::from_parts(Some("help"), Vec::new()),
            Ok(CliCommand::Help)
        );
    }

    #[test]
    fn help_with_known_subcommand_dispatches_to_that_command_help() {
        // `ripr help check` should dispatch the same way as `ripr check --help`,
        // i.e. parse to CliCommand::Check carrying `["--help"]`. We pick a few
        // representative commands to cover the surface.
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["check"])),
            Ok(CliCommand::Check(args(&["--help"])))
        );
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["outcome"])),
            Ok(CliCommand::Outcome(args(&["--help"])))
        );
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["gate"])),
            Ok(CliCommand::Gate(args(&["--help"])))
        );
        // start-here alias should also work via help.
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["start-here"])),
            Ok(CliCommand::FirstPr(args(&["--help"])))
        );
    }

    #[test]
    fn help_preserves_extra_args_after_the_subcommand() {
        // `ripr help check --root .` keeps --root . so the command's --help
        // branch fires (each command checks args.iter().any(... == "--help")).
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["check", "--root", "."])),
            Ok(CliCommand::Check(args(&["--help", "--root", "."])))
        );
    }

    #[test]
    fn help_with_unknown_subcommand_returns_unknown_command_error() {
        // A typo after `help` should produce the same unknown-command error as
        // the typo would without `help`, including a typo suggestion.
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["nonexistent"])),
            Err("unknown command \"nonexistent\". Run `ripr --help`.".to_string())
        );
        assert_eq!(
            CliCommand::from_parts(Some("help"), args(&["chekc"])),
            Err("unknown command \"chekc\". Did you mean `check`? Run `ripr --help`.".to_string())
        );
    }
}
