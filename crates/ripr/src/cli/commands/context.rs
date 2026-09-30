//! Arg-parsing and dispatch for `ripr context`.
//!
//! This is the CLI adapter layer only. Context collection semantics live in
//! `crate::app`. This module owns argv parsing, output destination selection,
//! and exit mapping for the context command family.

use crate::app::{self, CheckInput, OutputFormat};
use crate::cli::help;
use crate::cli::parse::{
    base_with_diff_conflict_error, disclose_attached_terminal_stdin_read, expect_value, parse_mode,
};
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
                // #4318 review: a cap is a render knob, and zero is valid —
                // the packet renders zero related tests, matching the config
                // surface. Only the count-style flags are positive-only.
                max_tests = crate::cli::commands_numeric::parse_non_negative_usize(
                    expect_value(args, i, "--max-related-tests")?,
                    "--max-related-tests",
                )?;
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
    // #4319: same parse-time `--base`+`--diff` conflict as `explain`. Only
    // the fresh path conflicts: beside `--from`, both flags are assertions
    // verified against the recording (RIPR-SPEC-0140), so that verification
    // path is intentionally left alone.
    if from_artifact.is_none() && base_explicitly_provided && input.diff_file.is_some() {
        return Err(base_with_diff_conflict_error("context"));
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
    // #4319: `--diff -` reads the diff from stdin. On an attached terminal
    // that blocks until EOF with no visible sign of why, so the cli adapter
    // discloses the read before dispatching; the analysis loader itself
    // stays silent for library callers.
    let rendered = match from_artifact.as_deref() {
        Some(artifact_path) => app::collect_context_from_artifact(
            input,
            &selector,
            max_tests,
            &config,
            artifact_path,
            asserted_base.as_deref(),
        )?,
        None => {
            disclose_attached_terminal_stdin_read(input.diff_file.as_deref());
            app::collect_context_with_config(input, &selector, max_tests, &config)?
        }
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
            matches!(result, Err(message) if message.starts_with("--max-related-tests requires a non-negative integer"))
        );
    }

    /// #4318 review: zero is a valid render cap (the packet renders zero
    /// related tests, matching `reports.max_related_tests = 0`), so it must
    /// pass parsing and fail later — here on the missing root — never with
    /// the numeric-shape message.
    #[test]
    fn context_accepts_zero_max_related_tests_at_parse_time() {
        let result = context(&args(&[
            "--root",
            "missing-ripr-root-for-context",
            "--at",
            "probe:file.rs:1:predicate",
            "--max-related-tests",
            "0",
        ]));
        assert!(
            matches!(
                &result,
                Err(message) if !message.contains("requires a non-negative integer")
            ),
            "zero is a valid cap and must not fail numeric parsing: {result:?}"
        );
        assert!(
            matches!(
                &result,
                Err(message) if message.contains("missing-ripr-root-for-context")
            ),
            "the accepted zero cap must reach the root-dependent failure: {result:?}"
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

    /// #4319: same parse-time `--base`+`--diff` conflict as `explain`. The
    /// loader gives `--diff` precedence and never validates `--base` beside
    /// it, so both flags on one command line silently analyzed the diff while
    /// appearing to assert the base. The conflict fails before any pipeline
    /// run and before the selector requirement. Message pinned verbatim.
    #[test]
    fn context_rejects_base_and_diff_together_at_parse_time() {
        let expected = Err(
            "context --base cannot be combined with --diff: --base and --diff are alternative diff sources; pass one"
                .to_string(),
        );
        assert_eq!(
            context(&args(&[
                "--diff",
                "sample.diff",
                "--base",
                "refs/heads/nope",
                "--at",
                "probe:src_lib.rs:error_path:abcd",
            ])),
            expected
        );
        assert_eq!(
            context(&args(&[
                "--base",
                "refs/heads/nope",
                "--diff",
                "sample.diff"
            ])),
            expected,
            "the conflict must not depend on flag order or selector presence"
        );
    }

    /// `--from` scope flags are assertions verified against the recording
    /// (RIPR-SPEC-0140, `app/check_artifact.rs::verify_scope_assertions`),
    /// not alternative diff sources, so the fresh-run conflict must not fire
    /// on the reuse path. The parse proceeds past the gate and fails later,
    /// on the missing artifact — never with the conflict message.
    #[test]
    fn context_keeps_base_and_diff_as_from_artifact_assertions() {
        let result = context(&args(&[
            "--from",
            "does-not-exist.json",
            "--diff",
            "sample.diff",
            "--base",
            "refs/heads/nope",
            "--at",
            "probe:src_lib.rs:error_path:abcd",
        ]));
        assert!(
            !matches!(&result, Err(message) if message.contains("cannot be combined with --diff")),
            "`--from` + `--base` + `--diff` is the reuse-verification path, not a diff-source conflict: {result:?}"
        );
    }
}
