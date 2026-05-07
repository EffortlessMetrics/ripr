use crate::cli::parse::expect_value;
use std::path::PathBuf;

pub(super) const DEFAULT_AGENT_BRIEF_MAX_SEAMS: usize = 3;
pub(super) const AGENT_BRIEF_HARD_MAX_SEAMS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AgentCommand {
    Help,
    BriefHelp,
    Brief(AgentBriefOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentBriefOptions {
    pub(super) root: PathBuf,
    pub(super) working_set: AgentBriefWorkingSet,
    pub(super) json: bool,
    pub(super) max_seams: usize,
}

impl AgentBriefOptions {
    pub(super) fn parsed_summary(&self) -> String {
        format!(
            "root={}, working_set={}, max_seams={}, json={}",
            self.root.display(),
            self.working_set.summary(),
            self.max_seams,
            self.json
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AgentBriefWorkingSet {
    Diff(PathBuf),
    Base(String),
    Files(Vec<PathBuf>),
    SeamId(String),
}

impl AgentBriefWorkingSet {
    fn summary(&self) -> String {
        match self {
            Self::Diff(path) => format!("diff:{}", path.display()),
            Self::Base(base) => format!("base:{base}"),
            Self::Files(paths) => format!("files:{}", paths.len()),
            Self::SeamId(seam_id) => format!("seam-id:{seam_id}"),
        }
    }
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
        Some("brief") => parse_agent_brief_command(&args[1..]),
        Some(other) => Err(format!(
            "unknown agent subcommand {other:?}; expected `brief`"
        )),
    }
}

fn parse_agent_brief_command(args: &[String]) -> Result<AgentCommand, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(AgentCommand::BriefHelp);
    }
    parse_agent_brief_options(args).map(AgentCommand::Brief)
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
            parse_agent_args(&args(&["brief", "--help"])),
            Ok(AgentCommand::BriefHelp)
        );
    }

    #[test]
    fn agent_args_reject_unknown_subcommand() {
        assert_eq!(
            parse_agent_args(&args(&["packet"])),
            Err("unknown agent subcommand \"packet\"; expected `brief`".to_string())
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
    fn agent_brief_summary_covers_all_working_set_kinds() {
        for (working_set, expected) in [
            (
                AgentBriefWorkingSet::Diff(PathBuf::from("change.diff")),
                "root=., working_set=diff:change.diff, max_seams=3, json=true",
            ),
            (
                AgentBriefWorkingSet::Base("main".to_string()),
                "root=., working_set=base:main, max_seams=3, json=true",
            ),
            (
                AgentBriefWorkingSet::Files(vec![PathBuf::from("src/lib.rs")]),
                "root=., working_set=files:1, max_seams=3, json=true",
            ),
            (
                AgentBriefWorkingSet::SeamId("f3c9e4d21a0b7c88".to_string()),
                "root=., working_set=seam-id:f3c9e4d21a0b7c88, max_seams=3, json=true",
            ),
        ] {
            let options = AgentBriefOptions {
                root: PathBuf::from("."),
                working_set,
                json: true,
                max_seams: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            };
            assert_eq!(options.parsed_summary(), expected);
        }
    }
}
