//! `ripr reports ci-summary`: print the generated workflow's step summary.

use crate::cli::parse::expect_value;
use crate::cli::suggest::unknown_argument;
use crate::output::ci_summary::{CiSummaryInput, render_ci_summary};
use std::io::Write;
use std::path::PathBuf;

use super::non_empty_string_arg;

const COMMAND: &str = "reports ci-summary";

/// The flags carry the workflow's environment as values, so an unset
/// variable arrives as an empty string and keeps its shell meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CiSummaryOptions {
    pub(super) root: PathBuf,
    pub(super) base_ref: String,
    pub(super) upload_sarif: String,
    pub(super) gate_baseline: String,
    pub(super) comment_mode: String,
}

pub(super) fn ci_summary(args: &[String]) -> Result<(), String> {
    let options = parse_ci_summary_options(args)?;
    let input = CiSummaryInput {
        configured_languages: configured_languages(&options.root),
        root: options.root,
        base_ref: options.base_ref,
        upload_sarif: options.upload_sarif == "true",
        gate_baseline: !options.gate_baseline.is_empty(),
        comment_mode: options.comment_mode,
    };
    let summary = render_ci_summary(&input);
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&summary)
        .and_then(|()| stdout.flush())
        .map_err(|err| format!("{COMMAND} could not write the summary: {err}"))
}

/// The enabled languages `ripr doctor --json` reports; none when the
/// configuration does not load, as the doctor projection leaves them.
fn configured_languages(root: &std::path::Path) -> Vec<String> {
    crate::config::load_for_root(root)
        .map(|config| {
            config
                .languages()
                .enabled()
                .iter()
                .map(|language| language.as_str().to_string())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_ci_summary_options(args: &[String]) -> Result<CiSummaryOptions, String> {
    let mut options = CiSummaryOptions {
        root: PathBuf::from("."),
        base_ref: "main".to_string(),
        upload_sarif: String::new(),
        gate_baseline: String::new(),
        comment_mode: String::new(),
    };
    let mut i = 0usize;
    while i < args.len() {
        let flag = args[i].as_str();
        i += 1;
        match flag {
            "--root" => options.root = PathBuf::from(non_empty_string_arg(args, i, flag, COMMAND)?),
            "--base-ref" => options.base_ref = non_empty_string_arg(args, i, flag, COMMAND)?,
            "--upload-sarif" => options.upload_sarif = expect_value(args, i, flag)?.to_string(),
            "--gate-baseline" => options.gate_baseline = expect_value(args, i, flag)?.to_string(),
            "--comment-mode" => options.comment_mode = expect_value(args, i, flag)?.to_string(),
            other => return Err(unknown_argument(COMMAND, other)),
        }
        i += 1;
    }
    Ok(options)
}
