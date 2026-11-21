//! Arg-parsing and dispatch for `ripr context`.
//!
//! This is the CLI adapter layer only. Context collection semantics live in
//! `crate::app`. This module owns argv parsing, output destination selection,
//! and exit mapping for the context command family.

use crate::app::{self, CheckInput, OutputFormat};
use crate::cli::help;
use crate::cli::parse::{expect_value, parse_mode};
use crate::config::{CheckInputExplicit, apply_to_check_input, load_for_root};
use std::path::PathBuf;

pub(in crate::cli) fn context(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput {
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let mut explicit = CheckInputExplicit::default();
    let mut selector: Option<String> = None;
    let mut max_tests = crate::config::DEFAULT_CONTEXT_RELATED_TESTS;
    let mut explicit_max_tests = false;
    // RIPR-SPEC-0140: `--from` loads a previously written check artifact
    // instead of re-running the pipeline. Scope flags passed alongside it
    // are assertions verified against the recording, not overrides.
    // `--mode` and `--no-unchanged-tests` feed the identity recomputation:
    // an artifact recorded with non-default values is only consumable when
    // the same values resolve here (flag or config).
    let mut from_artifact: Option<PathBuf> = None;
    let mut base_explicitly_provided = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                input.base = Some(expect_value(args, i, "--base")?.to_string());
                base_explicitly_provided = true;
            }
            "--diff" => {
                i += 1;
                input.diff_file = Some(PathBuf::from(expect_value(args, i, "--diff")?));
            }
            "--from" => {
                i += 1;
                from_artifact = Some(PathBuf::from(expect_value(args, i, "--from")?));
            }
            "--mode" => {
                i += 1;
                input.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                explicit.mode = true;
            }
            "--no-unchanged-tests" => {
                input.include_unchanged_tests = false;
                explicit.include_unchanged_tests = true;
            }
            "--perl-facts" => {
                i += 1;
                input.perl_facts_path = Some(PathBuf::from(expect_value(args, i, "--perl-facts")?));
            }
            "--suppression-policy" => {
                i += 1;
                input.suppression_policy = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--suppression-policy",
                )?));
            }
            "--at" => {
                i += 1;
                selector = Some(expect_value(args, i, "--at")?.to_string());
            }
            "--finding" => {
                i += 1;
                selector = Some(expect_value(args, i, "--finding")?.to_string());
            }
            "--max-related-tests" => {
                i += 1;
                max_tests = expect_value(args, i, "--max-related-tests")?
                    .parse::<usize>()
                    .map_err(|err| format!("invalid --max-related-tests: {err}"))?;
                explicit_max_tests = true;
            }
            "--json" => input.format = OutputFormat::Json,
            "--help" | "-h" => {
                help::print_context_help();
                return Ok(());
            }
            other => return Err(crate::cli::suggest::unknown_argument("context", other)),
        }
        i += 1;
    }
    let selector = selector.ok_or_else(|| {
        "missing --at or --finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string()
    })?;
    let config = load_for_root(&input.root)?;
    apply_to_check_input(&mut input, &config, explicit);
    if !explicit_max_tests {
        max_tests = config.reports().max_related_tests();
    }
    let asserted_base = if base_explicitly_provided {
        input.base.clone()
    } else {
        None
    };
    let rendered = match from_artifact.as_deref() {
        Some(artifact_path) => app::collect_context_from_artifact(
            input,
            &selector,
            max_tests,
            &config,
            artifact_path,
            asserted_base.as_deref(),
        )?,
        None => app::collect_context_with_config(input, &selector, max_tests, &config)?,
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::args;
    use super::*;

    #[test]
    fn context_rejects_invalid_max_related_tests() {
        let result = context(&args(&[
            "--at",
            "probe:file.rs:1:predicate",
            "--max-related-tests",
            "many",
        ]));
        assert!(
            matches!(result, Err(message) if message.starts_with("invalid --max-related-tests:"))
        );
    }

    #[test]
    fn context_requires_selector() {
        assert_eq!(
            context(&args(&[])),
            Err("missing --at or --finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string())
        );
    }

    #[test]
    fn context_rejects_unknown_argument() {
        assert_eq!(
            context(&args(&["--unknown", "value"])),
            Err("unknown context argument \"--unknown\". Run `ripr context --help`.".to_string())
        );
    }

    /// Exercised through the parser, not `unknown_argument` directly: wiring
    /// the help lookup is worthless if the parser never consults it, and this
    /// arm previously returned a bare `format!` that bypassed the suggestion
    /// helper entirely.
    #[test]
    fn context_suggests_the_nearest_flag_for_a_typo() {
        assert_eq!(
            context(&args(&["--fromm", "artifact.json"])),
            Err(
                "unknown context argument \"--fromm\". Did you mean `--from`? \
                 Run `ripr context --help`."
                    .to_string()
            )
        );
    }

    #[test]
    fn context_requires_values_for_value_flags() {
        assert_eq!(
            context(&args(&["--at"])),
            Err("missing value for --at".to_string())
        );
        assert_eq!(
            context(&args(&["--finding"])),
            Err("missing value for --finding".to_string())
        );
        assert_eq!(
            context(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            context(&args(&["--perl-facts"])),
            Err("missing value for --perl-facts".to_string())
        );
        assert_eq!(
            context(&args(&["--suppression-policy"])),
            Err("missing value for --suppression-policy".to_string())
        );
    }
}
