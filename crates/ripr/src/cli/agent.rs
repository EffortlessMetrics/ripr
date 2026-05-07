use crate::app::agent_brief::{AGENT_BRIEF_HARD_MAX_SEAMS, DEFAULT_AGENT_BRIEF_MAX_SEAMS};
use crate::cli::parse::expect_value;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AgentCommand {
    Help,
    StartHelp,
    BriefHelp,
    PacketHelp,
    VerifyHelp,
    ReceiptHelp,
    StatusHelp,
    Start(AgentStartOptions),
    Brief(AgentBriefOptions),
    Packet(AgentPacketOptions),
    Verify(AgentVerifyOptions),
    Receipt(AgentReceiptOptions),
    Status(AgentStatusOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentStartOptions {
    pub(super) root: PathBuf,
    pub(super) seam_id: String,
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentBriefOptions {
    pub(super) root: PathBuf,
    pub(super) working_set: AgentBriefWorkingSet,
    pub(super) json: bool,
    pub(super) max_seams: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentPacketOptions {
    pub(super) root: PathBuf,
    pub(super) seam_id: String,
    pub(super) json: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentVerifyOptions {
    pub(super) root: PathBuf,
    pub(super) before: PathBuf,
    pub(super) after: PathBuf,
    pub(super) json: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentReceiptOptions {
    pub(super) root: PathBuf,
    pub(super) verify_json: PathBuf,
    pub(super) seam_id: String,
    pub(super) test_changed: Option<String>,
    pub(super) commands_run: Vec<String>,
    pub(super) json: bool,
    pub(super) out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentStatusOptions {
    pub(super) root: PathBuf,
    pub(super) json: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AgentBriefWorkingSet {
    Diff(PathBuf),
    Base(String),
    Files(Vec<PathBuf>),
    SeamId(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WorkingSetCandidate {
    Diff(PathBuf),
    Base(String),
    Files(Vec<PathBuf>),
    SeamId(String),
}

impl WorkingSetCandidate {
    fn into_working_set(self) -> AgentBriefWorkingSet {
        match self {
            Self::Diff(path) => AgentBriefWorkingSet::Diff(path),
            Self::Base(base) => AgentBriefWorkingSet::Base(base),
            Self::Files(paths) => AgentBriefWorkingSet::Files(paths),
            Self::SeamId(seam_id) => AgentBriefWorkingSet::SeamId(seam_id),
        }
    }
}

pub(super) fn parse_agent_args(args: &[String]) -> Result<AgentCommand, String> {
    match args.first().map(|arg| arg.as_str()) {
        None | Some("--help" | "-h") => Ok(AgentCommand::Help),
        Some("start") => parse_agent_start_command(&args[1..]),
        Some("brief") => parse_agent_brief_command(&args[1..]),
        Some("packet") => parse_agent_packet_command(&args[1..]),
        Some("verify") => parse_agent_verify_command(&args[1..]),
        Some("receipt") => parse_agent_receipt_command(&args[1..]),
        Some("status") => parse_agent_status_command(&args[1..]),
        Some(other) => Err(format!(
            "unknown agent subcommand {other:?}; expected `start`, `brief`, `packet`, `verify`, `receipt`, or `status`"
        )),
    }
}

fn parse_agent_start_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::StartHelp);
    }
    parse_agent_start_options(args).map(AgentCommand::Start)
}

fn parse_agent_brief_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::BriefHelp);
    }
    parse_agent_brief_options(args).map(AgentCommand::Brief)
}

fn parse_agent_packet_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::PacketHelp);
    }
    parse_agent_packet_options(args).map(AgentCommand::Packet)
}

fn parse_agent_verify_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::VerifyHelp);
    }
    parse_agent_verify_options(args).map(AgentCommand::Verify)
}

fn parse_agent_receipt_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::ReceiptHelp);
    }
    parse_agent_receipt_options(args).map(AgentCommand::Receipt)
}

fn parse_agent_status_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::StatusHelp);
    }
    parse_agent_status_options(args).map(AgentCommand::Status)
}

pub(super) fn parse_agent_start_options(args: &[String]) -> Result<AgentStartOptions, String> {
    let mut root = PathBuf::from(".");
    let mut seam_id: Option<String> = None;
    let mut out: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--seam-id" => {
                i += 1;
                let value = expect_value(args, i, "--seam-id")?;
                if value.trim().is_empty() {
                    return Err("agent start --seam-id requires a non-empty ID".to_string());
                }
                seam_id = Some(value.to_string());
            }
            "--out" => {
                i += 1;
                let value = expect_value(args, i, "--out")?;
                if value.trim().is_empty() {
                    return Err("agent start --out requires a non-empty path".to_string());
                }
                out = Some(PathBuf::from(value));
            }
            other => return Err(format!("unknown agent start argument {other:?}")),
        }
        i += 1;
    }

    Ok(AgentStartOptions {
        root,
        seam_id: seam_id.ok_or_else(|| "agent start requires --seam-id".to_string())?,
        out: out.ok_or_else(|| "agent start requires --out <dir>".to_string())?,
    })
}

pub(super) fn parse_agent_brief_options(args: &[String]) -> Result<AgentBriefOptions, String> {
    let mut root = PathBuf::from(".");
    let mut working_set: Option<WorkingSetCandidate> = None;
    let mut json = false;
    let mut max_seams = DEFAULT_AGENT_BRIEF_MAX_SEAMS;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--diff" => {
                i += 1;
                set_working_set(
                    &mut working_set,
                    WorkingSetCandidate::Diff(PathBuf::from(expect_value(args, i, "--diff")?)),
                )?;
            }
            "--base" => {
                i += 1;
                set_working_set(
                    &mut working_set,
                    WorkingSetCandidate::Base(expect_value(args, i, "--base")?.to_string()),
                )?;
            }
            "--files" => {
                i += 1;
                set_working_set(
                    &mut working_set,
                    WorkingSetCandidate::Files(parse_files_value(expect_value(
                        args, i, "--files",
                    )?)?),
                )?;
            }
            "--seam-id" => {
                i += 1;
                set_working_set(
                    &mut working_set,
                    WorkingSetCandidate::SeamId(expect_value(args, i, "--seam-id")?.to_string()),
                )?;
            }
            "--json" => json = true,
            "--max-seams" => {
                i += 1;
                max_seams = parse_max_seams(expect_value(args, i, "--max-seams")?)?;
            }
            other => return Err(format!("unknown agent brief argument {other:?}")),
        }
        i += 1;
    }

    if !json {
        return Err("agent brief requires --json until human output is implemented".to_string());
    }

    let working_set = working_set
        .ok_or_else(|| {
            "agent brief requires exactly one of --diff, --base, --files, or --seam-id".to_string()
        })?
        .into_working_set();

    Ok(AgentBriefOptions {
        root,
        working_set,
        json,
        max_seams,
    })
}

pub(super) fn parse_agent_packet_options(args: &[String]) -> Result<AgentPacketOptions, String> {
    let mut root = PathBuf::from(".");
    let mut seam_id: Option<String> = None;
    let mut json = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--seam-id" => {
                i += 1;
                let value = expect_value(args, i, "--seam-id")?;
                if value.trim().is_empty() {
                    return Err("agent packet --seam-id requires a non-empty ID".to_string());
                }
                seam_id = Some(value.to_string());
            }
            "--json" => json = true,
            other => return Err(format!("unknown agent packet argument {other:?}")),
        }
        i += 1;
    }

    if !json {
        return Err("agent packet requires --json until human output is implemented".to_string());
    }

    Ok(AgentPacketOptions {
        root,
        seam_id: seam_id.ok_or_else(|| "agent packet requires --seam-id".to_string())?,
        json,
    })
}

pub(super) fn parse_agent_verify_options(args: &[String]) -> Result<AgentVerifyOptions, String> {
    let mut root = PathBuf::from(".");
    let mut before: Option<PathBuf> = None;
    let mut after: Option<PathBuf> = None;
    let mut json = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--before" => {
                i += 1;
                before = Some(PathBuf::from(expect_value(args, i, "--before")?));
            }
            "--after" => {
                i += 1;
                after = Some(PathBuf::from(expect_value(args, i, "--after")?));
            }
            "--json" => json = true,
            other => return Err(format!("unknown agent verify argument {other:?}")),
        }
        i += 1;
    }

    if !json {
        return Err("agent verify requires --json until human output is implemented".to_string());
    }

    Ok(AgentVerifyOptions {
        root,
        before: before.ok_or_else(|| "agent verify requires --before <path>".to_string())?,
        after: after.ok_or_else(|| "agent verify requires --after <path>".to_string())?,
        json,
    })
}

pub(super) fn parse_agent_receipt_options(args: &[String]) -> Result<AgentReceiptOptions, String> {
    let mut root = PathBuf::from(".");
    let mut verify_json: Option<PathBuf> = None;
    let mut seam_id: Option<String> = None;
    let mut test_changed: Option<String> = None;
    let mut commands_run = Vec::new();
    let mut json = false;
    let mut out: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--verify-json" => {
                i += 1;
                verify_json = Some(PathBuf::from(expect_value(args, i, "--verify-json")?));
            }
            "--seam-id" => {
                i += 1;
                let value = expect_value(args, i, "--seam-id")?;
                if value.trim().is_empty() {
                    return Err("agent receipt --seam-id requires a non-empty ID".to_string());
                }
                seam_id = Some(value.to_string());
            }
            "--test" => {
                i += 1;
                let value = expect_value(args, i, "--test")?;
                if value.trim().is_empty() {
                    return Err("agent receipt --test requires a non-empty value".to_string());
                }
                test_changed = Some(value.to_string());
            }
            "--command" => {
                i += 1;
                let value = expect_value(args, i, "--command")?;
                if value.trim().is_empty() {
                    return Err("agent receipt --command requires a non-empty value".to_string());
                }
                commands_run.push(value.to_string());
            }
            "--json" => json = true,
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(expect_value(args, i, "--out")?));
            }
            other => return Err(format!("unknown agent receipt argument {other:?}")),
        }
        i += 1;
    }

    if !json {
        return Err("agent receipt requires --json until human output is implemented".to_string());
    }

    Ok(AgentReceiptOptions {
        root,
        verify_json: verify_json
            .ok_or_else(|| "agent receipt requires --verify-json <path>".to_string())?,
        seam_id: seam_id.ok_or_else(|| "agent receipt requires --seam-id".to_string())?,
        test_changed,
        commands_run,
        json,
        out,
    })
}

pub(super) fn parse_agent_status_options(args: &[String]) -> Result<AgentStatusOptions, String> {
    let mut root = PathBuf::from(".");
    let mut json = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--json" => json = true,
            other => return Err(format!("unknown agent status argument {other:?}")),
        }
        i += 1;
    }

    if !json {
        return Err("agent status requires --json until human output is implemented".to_string());
    }

    Ok(AgentStatusOptions { root, json })
}

fn set_working_set(
    current: &mut Option<WorkingSetCandidate>,
    next: WorkingSetCandidate,
) -> Result<(), String> {
    if current.is_some() {
        return Err(
            "agent brief requires exactly one of --diff, --base, --files, or --seam-id".to_string(),
        );
    }
    *current = Some(next);
    Ok(())
}

fn parse_files_value(value: &str) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for part in value.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return Err("agent brief --files requires non-empty paths".to_string());
        }
        paths.push(PathBuf::from(trimmed));
    }
    Ok(paths)
}

fn parse_max_seams(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|err| format!("invalid --max-seams: expected a positive integer: {err}"))?;
    if parsed == 0 {
        return Err("invalid --max-seams: expected a positive integer".to_string());
    }
    if parsed > AGENT_BRIEF_HARD_MAX_SEAMS {
        return Err(format!(
            "invalid --max-seams: maximum is {AGENT_BRIEF_HARD_MAX_SEAMS}"
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn agent_args_parse_help_and_brief_help() {
        assert_eq!(parse_agent_args(&args(&[])), Ok(AgentCommand::Help));
        assert_eq!(parse_agent_args(&args(&["--help"])), Ok(AgentCommand::Help));
        assert_eq!(
            parse_agent_args(&args(&["start", "--help"])),
            Ok(AgentCommand::StartHelp)
        );
        assert_eq!(
            parse_agent_args(&args(&["brief", "--help"])),
            Ok(AgentCommand::BriefHelp)
        );
        assert_eq!(
            parse_agent_args(&args(&["packet", "--help"])),
            Ok(AgentCommand::PacketHelp)
        );
        assert_eq!(
            parse_agent_args(&args(&["verify", "--help"])),
            Ok(AgentCommand::VerifyHelp)
        );
        assert_eq!(
            parse_agent_args(&args(&["receipt", "--help"])),
            Ok(AgentCommand::ReceiptHelp)
        );
        assert_eq!(
            parse_agent_args(&args(&["status", "--help"])),
            Ok(AgentCommand::StatusHelp)
        );
    }

    #[test]
    fn agent_args_reject_unknown_subcommand() {
        assert_eq!(
            parse_agent_args(&args(&["packet"])),
            Err("agent packet requires --json until human output is implemented".to_string())
        );
        assert_eq!(
            parse_agent_args(&args(&["other"])),
            Err(
                "unknown agent subcommand \"other\"; expected `start`, `brief`, `packet`, `verify`, `receipt`, or `status`"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_start_parses_workflow_request() {
        assert_eq!(
            parse_agent_args(&args(&[
                "start",
                "--root",
                "repo",
                "--seam-id",
                "67fc764ba37d77bd",
                "--out",
                "target/ripr/workflow",
            ])),
            Ok(AgentCommand::Start(AgentStartOptions {
                root: PathBuf::from("repo"),
                seam_id: "67fc764ba37d77bd".to_string(),
                out: PathBuf::from("target/ripr/workflow"),
            }))
        );
    }

    #[test]
    fn agent_start_requires_seam_id_and_rejects_unknown_arguments() {
        assert_eq!(
            parse_agent_start_options(&args(&["--root", "repo", "--out", "target/ripr/workflow"])),
            Err("agent start requires --seam-id".to_string())
        );
        assert_eq!(
            parse_agent_start_options(&args(&["--seam-id", "", "--out", "target/ripr/workflow",])),
            Err("agent start --seam-id requires a non-empty ID".to_string())
        );
        assert_eq!(
            parse_agent_start_options(&args(&["--seam-id", "abc", "--out", ""])),
            Err("agent start --out requires a non-empty path".to_string())
        );
        assert_eq!(
            parse_agent_start_options(&args(&["--seam-id", "abc", "--json"])),
            Err("unknown agent start argument \"--json\"".to_string())
        );
    }

    #[test]
    fn agent_args_parse_brief_request() {
        assert_eq!(
            parse_agent_args(&args(&["brief", "--diff", "change.diff", "--json"])),
            Ok(AgentCommand::Brief(AgentBriefOptions {
                root: PathBuf::from("."),
                working_set: AgentBriefWorkingSet::Diff(PathBuf::from("change.diff")),
                json: true,
                max_seams: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            }))
        );
    }

    #[test]
    fn agent_brief_parses_diff_scope() {
        assert_eq!(
            parse_agent_brief_options(&args(&[
                "--root",
                "repo",
                "--diff",
                "change.diff",
                "--json",
                "--max-seams",
                "2",
            ])),
            Ok(AgentBriefOptions {
                root: PathBuf::from("repo"),
                working_set: AgentBriefWorkingSet::Diff(PathBuf::from("change.diff")),
                json: true,
                max_seams: 2,
            })
        );
    }

    #[test]
    fn agent_brief_parses_base_scope() {
        assert_eq!(
            parse_agent_brief_options(&args(&["--base", "main", "--json"])),
            Ok(AgentBriefOptions {
                root: PathBuf::from("."),
                working_set: AgentBriefWorkingSet::Base("main".to_string()),
                json: true,
                max_seams: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            })
        );
    }

    #[test]
    fn agent_brief_parses_file_scope() {
        assert_eq!(
            parse_agent_brief_options(&args(&[
                "--files",
                "src/pricing.rs, tests/pricing.rs",
                "--json",
            ])),
            Ok(AgentBriefOptions {
                root: PathBuf::from("."),
                working_set: AgentBriefWorkingSet::Files(vec![
                    PathBuf::from("src/pricing.rs"),
                    PathBuf::from("tests/pricing.rs"),
                ]),
                json: true,
                max_seams: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            })
        );
    }

    #[test]
    fn agent_brief_parses_seam_id_scope() {
        assert_eq!(
            parse_agent_brief_options(&args(&["--seam-id", "f3c9e4d21a0b7c88", "--json",])),
            Ok(AgentBriefOptions {
                root: PathBuf::from("."),
                working_set: AgentBriefWorkingSet::SeamId("f3c9e4d21a0b7c88".to_string()),
                json: true,
                max_seams: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            })
        );
    }

    #[test]
    fn agent_brief_requires_json() {
        assert_eq!(
            parse_agent_brief_options(&args(&["--diff", "change.diff"])),
            Err("agent brief requires --json until human output is implemented".to_string())
        );
    }

    #[test]
    fn agent_brief_requires_exactly_one_working_set() {
        assert_eq!(
            parse_agent_brief_options(&args(&["--json"])),
            Err(
                "agent brief requires exactly one of --diff, --base, --files, or --seam-id"
                    .to_string()
            )
        );
        assert_eq!(
            parse_agent_brief_options(&args(&[
                "--diff",
                "change.diff",
                "--files",
                "src/lib.rs",
                "--json",
            ])),
            Err(
                "agent brief requires exactly one of --diff, --base, --files, or --seam-id"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_brief_requires_values_for_value_flags() {
        assert_eq!(
            parse_agent_brief_options(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&["--diff"])),
            Err("missing value for --diff".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&["--files"])),
            Err("missing value for --files".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&["--seam-id"])),
            Err("missing value for --seam-id".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&["--max-seams"])),
            Err("missing value for --max-seams".to_string())
        );
    }

    #[test]
    fn agent_brief_rejects_invalid_limits_and_file_lists() {
        assert_eq!(
            parse_agent_brief_options(&args(&[
                "--diff",
                "change.diff",
                "--json",
                "--max-seams",
                "many",
            ])),
            Err(
                "invalid --max-seams: expected a positive integer: invalid digit found in string"
                    .to_string()
            )
        );
        assert_eq!(
            parse_agent_brief_options(&args(&[
                "--diff",
                "change.diff",
                "--json",
                "--max-seams",
                "0",
            ])),
            Err("invalid --max-seams: expected a positive integer".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&[
                "--diff",
                "change.diff",
                "--json",
                "--max-seams",
                "11",
            ])),
            Err("invalid --max-seams: maximum is 10".to_string())
        );
        assert_eq!(
            parse_agent_brief_options(&args(&["--files", "src/lib.rs,", "--json"])),
            Err("agent brief --files requires non-empty paths".to_string())
        );
    }

    #[test]
    fn agent_brief_rejects_unknown_arguments() {
        assert_eq!(
            parse_agent_brief_options(&args(&["--diff", "change.diff", "--xml"])),
            Err("unknown agent brief argument \"--xml\"".to_string())
        );
    }

    #[test]
    fn agent_packet_parses_seam_id_request() {
        assert_eq!(
            parse_agent_packet_options(&args(&[
                "--root",
                "repo",
                "--seam-id",
                "f3c9e4d21a0b7c88",
                "--json",
            ])),
            Ok(AgentPacketOptions {
                root: PathBuf::from("repo"),
                seam_id: "f3c9e4d21a0b7c88".to_string(),
                json: true,
            })
        );
    }

    #[test]
    fn agent_packet_requires_json_and_seam_id() {
        assert_eq!(
            parse_agent_packet_options(&args(&["--seam-id", "abc"])),
            Err("agent packet requires --json until human output is implemented".to_string())
        );
        assert_eq!(
            parse_agent_packet_options(&args(&["--json"])),
            Err("agent packet requires --seam-id".to_string())
        );
        assert_eq!(
            parse_agent_packet_options(&args(&["--seam-id", "", "--json"])),
            Err("agent packet --seam-id requires a non-empty ID".to_string())
        );
    }

    #[test]
    fn agent_packet_rejects_unknown_arguments() {
        assert_eq!(
            parse_agent_packet_options(&args(&[
                "--seam-id",
                "f3c9e4d21a0b7c88",
                "--json",
                "--xml",
            ])),
            Err("unknown agent packet argument \"--xml\"".to_string())
        );
    }

    #[test]
    fn agent_verify_parses_before_after_request() {
        assert_eq!(
            parse_agent_verify_options(&args(&[
                "--root",
                "repo",
                "--before",
                "target/ripr/workflow/before.repo-exposure.json",
                "--after",
                "target/ripr/workflow/after.repo-exposure.json",
                "--json",
            ])),
            Ok(AgentVerifyOptions {
                root: PathBuf::from("repo"),
                before: PathBuf::from("target/ripr/workflow/before.repo-exposure.json"),
                after: PathBuf::from("target/ripr/workflow/after.repo-exposure.json"),
                json: true,
            })
        );
    }

    #[test]
    fn agent_verify_requires_json_before_and_after() {
        assert_eq!(
            parse_agent_verify_options(&args(&[
                "--before",
                "before.json",
                "--after",
                "after.json",
            ])),
            Err("agent verify requires --json until human output is implemented".to_string())
        );
        assert_eq!(
            parse_agent_verify_options(&args(&["--after", "after.json", "--json"])),
            Err("agent verify requires --before <path>".to_string())
        );
        assert_eq!(
            parse_agent_verify_options(&args(&["--before", "before.json", "--json"])),
            Err("agent verify requires --after <path>".to_string())
        );
    }

    #[test]
    fn agent_verify_rejects_unknown_arguments() {
        assert_eq!(
            parse_agent_verify_options(&args(&[
                "--before",
                "before.json",
                "--after",
                "after.json",
                "--json",
                "--format",
                "md",
            ])),
            Err("unknown agent verify argument \"--format\"".to_string())
        );
    }

    #[test]
    fn agent_receipt_parses_verify_json_request() {
        assert_eq!(
            parse_agent_receipt_options(&args(&[
                "--root",
                "repo",
                "--verify-json",
                "target/ripr/workflow/agent-verify.json",
                "--seam-id",
                "f3c9e4d21a0b7c88",
                "--test",
                "pricing_boundary",
                "--command",
                "cargo test pricing_boundary",
                "--command",
                "cargo run -p ripr -- agent verify --json",
                "--json",
                "--out",
                "target/ripr/reports/agent-receipt.json",
            ])),
            Ok(AgentReceiptOptions {
                root: PathBuf::from("repo"),
                verify_json: PathBuf::from("target/ripr/workflow/agent-verify.json"),
                seam_id: "f3c9e4d21a0b7c88".to_string(),
                test_changed: Some("pricing_boundary".to_string()),
                commands_run: vec![
                    "cargo test pricing_boundary".to_string(),
                    "cargo run -p ripr -- agent verify --json".to_string(),
                ],
                json: true,
                out: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
            })
        );
    }

    #[test]
    fn agent_receipt_requires_json_verify_json_and_seam_id() {
        assert_eq!(
            parse_agent_receipt_options(&args(&[
                "--verify-json",
                "agent-verify.json",
                "--seam-id",
                "abc",
            ])),
            Err("agent receipt requires --json until human output is implemented".to_string())
        );
        assert_eq!(
            parse_agent_receipt_options(&args(&["--seam-id", "abc", "--json"])),
            Err("agent receipt requires --verify-json <path>".to_string())
        );
        assert_eq!(
            parse_agent_receipt_options(&args(&["--verify-json", "agent-verify.json", "--json"])),
            Err("agent receipt requires --seam-id".to_string())
        );
        assert_eq!(
            parse_agent_receipt_options(&args(&[
                "--verify-json",
                "agent-verify.json",
                "--seam-id",
                "",
                "--json",
            ])),
            Err("agent receipt --seam-id requires a non-empty ID".to_string())
        );
    }

    #[test]
    fn agent_receipt_rejects_unknown_arguments_and_empty_metadata() {
        assert_eq!(
            parse_agent_receipt_options(&args(&[
                "--verify-json",
                "agent-verify.json",
                "--seam-id",
                "abc",
                "--json",
                "--format",
                "md",
            ])),
            Err("unknown agent receipt argument \"--format\"".to_string())
        );
        assert_eq!(
            parse_agent_receipt_options(&args(&[
                "--verify-json",
                "agent-verify.json",
                "--seam-id",
                "abc",
                "--test",
                "",
                "--json",
            ])),
            Err("agent receipt --test requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_agent_receipt_options(&args(&[
                "--verify-json",
                "agent-verify.json",
                "--seam-id",
                "abc",
                "--command",
                "",
                "--json",
            ])),
            Err("agent receipt --command requires a non-empty value".to_string())
        );
    }

    #[test]
    fn agent_status_parses_root_and_json() {
        assert_eq!(
            parse_agent_status_options(&args(&["--root", "repo", "--json"])),
            Ok(AgentStatusOptions {
                root: PathBuf::from("repo"),
                json: true,
            })
        );
        assert_eq!(
            parse_agent_args(&args(&["status", "--root", "repo", "--json"])),
            Ok(AgentCommand::Status(AgentStatusOptions {
                root: PathBuf::from("repo"),
                json: true,
            }))
        );
    }

    #[test]
    fn agent_status_requires_json_and_rejects_unknown_arguments() {
        assert_eq!(
            parse_agent_status_options(&args(&[])),
            Err("agent status requires --json until human output is implemented".to_string())
        );
        assert_eq!(
            parse_agent_status_options(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            parse_agent_status_options(&args(&["--json", "--xml"])),
            Err("unknown agent status argument \"--xml\"".to_string())
        );
    }
}
