//! Shared "did you mean" support for unrecognized CLI arguments.
//!
//! `ripr` already suggested the nearest command for an unknown *command*
//! (`ripr chekc` -> "Did you mean `check`?"), but an unknown *flag* produced a
//! bare `unknown check argument "--forma"` with no suggestion and no pointer to
//! the command's help. A mistyped flag is the more common slip, because there
//! are far more flags than commands.
//!
//! The candidate flags are read out of the command's own help text rather than
//! from a hand-maintained list, so a suggestion can never name a flag that
//! `ripr <command> --help` does not document, and adding a flag to help makes
//! it suggestible with no second edit.

use crate::cli::help;

/// Build the error for an unrecognized argument to `command`.
///
/// `command` is the space-separated command path as the user would type it
/// (`"check"`, `"agent brief"`, `"policy readiness"`), which is also what the
/// help pointer names. Every `ripr` command path accepts `--help` directly, so
/// the pointer is uniform.
pub(crate) fn unknown_argument(command: &str, arg: &str) -> String {
    // `--version` is a process-level identity query scoped to the top level
    // (`parse.rs`); a subcommand that does not implement its own version
    // contract rejects it. Naming the spelling that works (#4318) beats
    // pointing only at the command's help, and no flag suggestion competes:
    // the user asked for the version, not a similar flag.
    if matches!(arg, "--version" | "-V") && !implements_local_version(command) {
        return format!(
            "unknown {command} argument {arg:?}. `ripr --version` reports the binary version. Run `ripr {command} --help`."
        );
    }
    match closest_flag(command, arg) {
        Some(suggestion) => format!(
            "unknown {command} argument {arg:?}. Did you mean `{suggestion}`? Run `ripr {command} --help`."
        ),
        None => format!("unknown {command} argument {arg:?}. Run `ripr {command} --help`."),
    }
}

/// Commands whose `--version` is a command-local contract, so the top-level
/// hint would be wrong. `ripr lsp --version` reports the sidecar server's
/// own version and is accepted by that command's parser.
fn implements_local_version(command: &str) -> bool {
    command == "lsp"
}

/// Build the error for an unrecognized enum-style *value* (`--format yaml`).
///
/// Enumerates the accepted values and, when one is close enough, proposes it
/// as a near-miss suggestion, so the same mistake names the fix on every
/// command instead of only where a parser happens to inline the list.
pub(in crate::cli) fn unknown_value(label: &str, value: &str, accepted: &[&str]) -> String {
    match closest(value, accepted.iter().copied()) {
        Some(suggestion) => format!(
            "unknown {label} {value:?}. Did you mean `{suggestion}`? Accepted: {}.",
            accepted.join(", ")
        ),
        None => format!(
            "unknown {label} {value:?}. Accepted: {}.",
            accepted.join(", ")
        ),
    }
}

fn closest_flag(command: &str, arg: &str) -> Option<String> {
    // Only flag-shaped input gets a flag suggestion. A stray positional value
    // is a different mistake, and proposing `--out` for `report.json` would be
    // noise.
    if !arg.starts_with('-') {
        return None;
    }
    let help_text = help::help_text_for(command)?;
    // #2583 review: never answer a rejected flag with itself. Section scoping
    // removes the known way this happened (a shared help body offering a
    // sibling subcommand's flag), but "did you mean the thing you just typed?"
    // is nonsense from any source, so it is excluded structurally rather than
    // left to depend on the scoping tables staying correct.
    closest(
        arg,
        known_flags(command, help_text)
            .into_iter()
            .filter(|flag| *flag != arg),
    )
    .map(str::to_string)
}

/// Scan a help text for the flags it documents.
///
/// A flag is documented when it opens an Options line (e.g.
/// `  --format FORMAT    Output format. ...`) or appears on a usage line owned
/// by this command (`Usage: ripr explain [--base REV|--diff PATH] ...`). That
/// is the same documented-surface definition the flag/help parity gate's
/// `extract_flags` applies in `help.rs`, so a flag the gate counts as
/// documented is suggestible here with no second edit. Prose and examples
/// document nothing: a mention inside another option's essay (`--perl-facts`
/// inside check's `--write-artifact` entry) stays undiscoverable, in both
/// miners.
///
/// Usage candidates are scoped to the command's own usage lines so shared
/// help bodies (baseline, policy, reports, assistant-loop) keep sibling flags
/// out of each other's candidate sets: the parser rejects a sibling's flag,
/// so proposing it would be a wrong suggestion. The Options mining below
/// keeps its existing section scoping for the same reason.
fn known_flags<'a>(command: &str, help_text: &'a str) -> Vec<&'a str> {
    let section = option_section(command);
    let mut in_section = section.is_none();
    let mut in_usage_block = false;
    let mut flags = Vec::new();
    for line in help_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Usage:") {
            in_usage_block = true;
            collect_usage_flags(line, command, &mut flags);
            continue;
        }
        // Wrapped usage blocks list one `ripr <command>` line per sibling
        // until the first line that is neither — the same block discipline
        // the parity gate's extractor applies.
        if in_usage_block && trimmed.starts_with("ripr ") {
            collect_usage_flags(line, command, &mut flags);
            continue;
        }
        in_usage_block = false;
        if let Some(section) = section {
            if line == section {
                in_section = true;
                continue;
            }
            if in_section && line.ends_with(" options:") {
                break;
            }
        }
        if !in_section {
            continue;
        }
        if !line.starts_with(' ') {
            continue;
        }
        if !trimmed.starts_with("--") {
            continue;
        }
        let flag = match trimmed
            .split(|ch: char| ch.is_whitespace() || ch == '=' || ch == ',')
            .next()
        {
            Some(flag) => flag,
            None => continue,
        };
        // `--` alone is the end-of-options separator, not a suggestible flag.
        if flag.len() > 2 && !flags.contains(&flag) {
            flags.push(flag);
        }
    }
    flags
}

/// Collect `--flag` tokens from a usage-block line owned by `command`.
///
/// The line must name the command path as the user types it (`ripr explain`,
/// `ripr baseline create`) with a name boundary after it, so a shared body's
/// sibling usage lines (`ripr agent verify-execute` next to
/// `ripr agent verify`) do not leak flags into each other's candidate sets.
/// Token rules mirror the parity gate's usage scanner in `help.rs`: `--`
/// preceded by the line start, whitespace, `[`, `(`, or `|`, so bracketed and
/// alternation forms like `[--base REV|--diff PATH]` are covered.
fn collect_usage_flags<'a>(line: &'a str, command: &str, flags: &mut Vec<&'a str>) {
    if !usage_line_belongs_to_command(line, command) {
        return;
    }
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        if bytes[index] != b'-' || bytes[index + 1] != b'-' {
            index += 1;
            continue;
        }
        let previous_ok =
            index == 0 || matches!(bytes[index - 1], b' ' | b'\t' | b'[' | b'(' | b'|');
        if !previous_ok {
            index += 1;
            continue;
        }
        let mut end = index + 2;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-' || bytes[end] == b'_')
        {
            end += 1;
        }
        let token = &line[index..end];
        // `--` alone ends options; it is not a suggestible flag.
        if token.len() > 2 && !flags.contains(&token) {
            flags.push(token);
        }
        index = end;
    }
}

/// Whether a usage-block line carries this command's own syntax.
///
/// Whole-body mining passes an empty command (a test aid that deliberately
/// sweeps every Options line); with no owner to scope to, no usage line
/// qualifies.
fn usage_line_belongs_to_command(line: &str, command: &str) -> bool {
    if command.is_empty() {
        return false;
    }
    let needle = format!("ripr {command}");
    let mut from = 0usize;
    while let Some(relative) = line[from..].find(&needle) {
        let after = from + relative + needle.len();
        let boundary_ok = after >= line.len()
            || line[after..]
                .chars()
                .next()
                .is_none_or(|next| !(next.is_alphanumeric() || next == '-' || next == '_'));
        if boundary_ok {
            return true;
        }
        from = after;
    }
    false
}

/// Return the options heading for help bodies shared by sibling commands.
///
/// The shared body is still useful for rendering `--help`, but mining the
/// whole body would let one sibling suggest another sibling's flag. Commands
/// with a single options section keep the existing full-body scan.
fn option_section(command: &str) -> Option<&'static str> {
    match command {
        "assistant-loop proof" => Some("Proof options:"),
        "assistant-loop health" => Some("Health options:"),
        "baseline create" => Some("Create options:"),
        "baseline diff" => Some("Diff options:"),
        "baseline update" => Some("Update options:"),
        "policy readiness" => Some("Readiness options:"),
        "policy operations" => Some("Operations options:"),
        "policy history" => Some("History options:"),
        "policy promote" => Some("Promotion options:"),
        "policy preview-promote" => Some("Preview promotion options:"),
        "policy waiver-aging" => Some("Waiver aging options:"),
        "policy suppression-health" => Some("Suppression health options:"),
        "reports index" => Some("Index options:"),
        "reports gap-ledger" => Some("Gap ledger options:"),
        "reports ts-limitations" => Some("TypeScript limitation options:"),
        "reports ts-false-actionable" => Some("TypeScript false-actionable audit options:"),
        _ => None,
    }
}

/// Pick the best candidate for `input`, or nothing when none is close enough.
///
/// Leading dashes are stripped before comparing. Every flag shares the `--`
/// prefix, so leaving it in inflates similarity between flags that have
/// nothing else in common — that is how `--wat` came out as "did you mean
/// `--base`?", which is worse than no suggestion at all.
///
/// A candidate the input is a prefix of always beats an edit-distance match,
/// because a typed prefix is almost always an abbreviation rather than a typo
/// (`--sea` means `--seam-id`, not `--base`). Among prefix matches the
/// shortest completion wins; among typo matches the smallest distance wins.
/// Ties break on the candidate name so the choice is deterministic.
fn closest<'a>(input: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let needle = input.trim_start_matches('-');
    if needle.is_empty() {
        return None;
    }
    let budget = typo_budget(needle);
    let mut best: Option<(u8, usize, &'a str)> = None;
    for candidate in candidates {
        let hay = candidate.trim_start_matches('-');
        let ranked = if hay.starts_with(needle) {
            // Tier 0: abbreviation. Rank by how much is left to type.
            (0u8, hay.chars().count() - needle.chars().count(), candidate)
        } else {
            let distance = edit_distance(needle, hay);
            if distance > budget {
                continue;
            }
            (1u8, distance, candidate)
        };
        if best.is_none_or(|current| ranked < current) {
            best = Some(ranked);
        }
    }
    best.map(|(_, _, candidate)| candidate)
}

/// How many edits to tolerate, scaled to the length actually being compared.
///
/// This runs on the dash-stripped name, so a three-character name like `wat`
/// gets one edit rather than the three a `--`-inclusive length would have
/// bought it.
fn typo_budget(name: &str) -> usize {
    match name.chars().count() {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    }
}

pub(in crate::cli) fn edit_distance(left: &str, right: &str) -> usize {
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
    use super::*;

    #[test]
    fn unknown_argument_suggests_the_nearest_documented_flag() {
        let message = unknown_argument("check", "--forma");
        assert_eq!(
            message,
            "unknown check argument \"--forma\". Did you mean `--format`? Run `ripr check --help`."
        );
    }

    #[test]
    fn unknown_argument_still_points_at_help_without_a_near_match() {
        let message = unknown_argument("check", "--totally-unrelated-flag");
        assert_eq!(
            message,
            "unknown check argument \"--totally-unrelated-flag\". Run `ripr check --help`."
        );
    }

    /// #4318: version is a top-level query, so a subcommand rejecting it must
    /// name the spelling that works instead of only pointing at the
    /// subcommand's help.
    #[test]
    fn a_rejected_version_argument_points_at_the_top_level_version() {
        for arg in ["--version", "-V"] {
            let message = unknown_argument("check", arg);
            assert_eq!(
                message,
                format!(
                    "unknown check argument {arg:?}. `ripr --version` reports the binary version. Run `ripr check --help`."
                )
            );
        }
    }

    /// A command with a command-local version contract (`ripr lsp --version`)
    /// must not be sent to the top level.
    #[test]
    fn lsp_keeps_its_own_version_contract() {
        let message = unknown_argument("lsp", "--version");
        assert!(!message.contains("`ripr --version`"), "{message}");
    }

    /// A wrong suggestion is worse than none. `--wat` shares only the `--`
    /// with every flag `check` accepts, and an earlier revision answered
    /// "did you mean `--base`?" because the shared prefix inflated similarity.
    #[test]
    fn unknown_argument_does_not_invent_a_match_for_an_unrelated_short_flag() {
        let message = unknown_argument("check", "--wat");
        assert_eq!(
            message,
            "unknown check argument \"--wat\". Run `ripr check --help`."
        );
    }

    /// A typed prefix is an abbreviation, not a typo: `--sea` means
    /// `--seam-id`. Edit distance alone ranked `--base` (distance 2) above
    /// `--seam-id` (distance 4) here.
    #[test]
    fn unknown_argument_prefers_an_abbreviation_over_a_closer_edit_distance() {
        let message = unknown_argument("agent brief", "--sea");
        assert!(message.contains("Did you mean `--seam-id`?"), "{message}");
    }

    #[test]
    fn closest_prefers_the_shortest_completion_among_prefix_matches() {
        let candidates = ["--out", "--out-md", "--output-directory"];
        assert_eq!(closest("--out", candidates.into_iter()), Some("--out"));
        assert_eq!(closest("--out-", candidates.into_iter()), Some("--out-md"));
    }

    #[test]
    fn closest_scales_the_typo_budget_to_the_dash_stripped_name() {
        // Three characters buy one edit, so an unrelated four-letter flag is
        // out of range even though both are short.
        assert_eq!(closest("--wat", ["--base"].into_iter()), None);
        // A single transposition in a longer name still resolves.
        assert_eq!(closest("--corss", ["--cross"].into_iter()), Some("--cross"));
    }

    #[test]
    fn closest_ignores_a_bare_dash_run_with_no_name() {
        assert_eq!(closest("--", ["--base", "--root"].into_iter()), None);
    }

    /// A stray positional is not a mistyped flag, so it gets the pointer but no
    /// flag suggestion.
    #[test]
    fn unknown_argument_does_not_suggest_a_flag_for_a_positional() {
        let message = unknown_argument("check", "report.json");
        assert!(!message.contains("Did you mean"), "{message}");
        assert!(message.contains("Run `ripr check --help`."), "{message}");
    }

    /// Command paths whose flags live in a parent help body still resolve.
    #[test]
    fn unknown_argument_resolves_flags_for_nested_command_paths() {
        let message = unknown_argument("policy readiness", "--ou");
        assert!(
            message.contains("Run `ripr policy readiness --help`."),
            "{message}"
        );
    }

    #[test]
    fn unknown_argument_does_not_suggest_a_sibling_baseline_flag() {
        let message = unknown_argument("baseline create", "--out-md");
        assert!(!message.contains("Did you mean"), "{message}");
        assert!(message.contains("ripr baseline create --help"), "{message}");
    }

    #[test]
    fn unknown_argument_does_not_suggest_a_sibling_report_flag() {
        let message = unknown_argument("reports index", "--records");
        assert!(!message.contains("Did you mean"), "{message}");
        assert!(message.contains("ripr reports index --help"), "{message}");
    }

    #[test]
    fn unknown_value_lists_accepted_values_and_suggests() {
        let message = unknown_value("format", "jsonn", &["human", "json", "sarif"]);
        assert_eq!(
            message,
            "unknown format \"jsonn\". Did you mean `json`? Accepted: human, json, sarif."
        );
    }

    #[test]
    fn unknown_value_lists_accepted_values_without_a_near_match() {
        let message = unknown_value("format", "yaml", &["human", "json", "sarif"]);
        assert_eq!(
            message,
            "unknown format \"yaml\". Accepted: human, json, sarif."
        );
    }

    #[test]
    fn known_flags_reads_the_option_list_and_the_owning_usage_line() {
        let help_text = "Summary line.\n\nUsage: ripr thing [--diff PATH]\n\nOptions:\n  --root PATH    Workspace root.\n  --json         Shortcut.\n  --            End of options.\n";
        assert_eq!(
            known_flags("thing", help_text),
            vec!["--diff", "--root", "--json"]
        );
    }

    /// `explain` and `context` had documented help bodies but no entry in
    /// `help_text_for`, so `closest_flag` returned `None` at its very first
    /// step and every mistyped flag on the two drill-down commands fell to the
    /// bare no-suggestion branch. They are the commands the human digest tells
    /// the reader to run next, so the slip is likely there.
    ///
    /// The guard test below could not catch this: it iterates
    /// `REGISTERED_COMMAND_PATHS`, and these paths were missing from that list
    /// too, so the omission was invisible from both sides.
    #[test]
    fn explain_and_context_suggest_their_own_documented_flags() {
        for (command, typo, expected) in [
            ("explain", "--fromm", "--from"),
            ("explain", "--mod", "--mode"),
            ("context", "--fromm", "--from"),
            ("context", "--perl-fact", "--perl-facts"),
        ] {
            assert_eq!(
                unknown_argument(command, typo),
                format!(
                    "unknown {command} argument {typo:?}. \
                     Did you mean `{expected}`? Run `ripr {command} --help`."
                ),
            );
        }
    }

    /// The parity gate counts usage-line flags as documented, so they must be
    /// suggestible too. `--base` on `explain` and `--at` on `context` are
    /// documented only on the `Usage:` line — the Options list never mentions
    /// them — yet both are parsed, so a typo used to fall to the bare
    /// no-suggestion branch while the parity gate stayed green.
    #[test]
    fn usage_documented_flags_become_suggestible() {
        let explain_base = unknown_argument("explain", "--bas");
        assert_eq!(
            explain_base,
            "unknown explain argument \"--bas\". \
             Did you mean `--base`? Run `ripr explain --help`."
        );
        let context_at = unknown_argument("context", "--att");
        assert_eq!(
            context_at,
            "unknown context argument \"--att\". \
             Did you mean `--at`? Run `ripr context --help`."
        );
    }

    /// Usage candidates inherit the sibling scoping the Options mining
    /// already applies: `--previous-readiness` is documented only on the
    /// readiness sibling's usage line of the shared policy body, so
    /// `policy operations` must never propose it.
    #[test]
    fn usage_candidates_stay_scoped_to_the_owning_sibling() {
        let readiness = unknown_argument("policy readiness", "--prev");
        assert!(
            readiness.contains("Did you mean `--previous-readiness`?"),
            "{readiness}"
        );
        let operations = unknown_argument("policy operations", "--previous-readines");
        assert!(!operations.contains("Did you mean"), "{operations}");
    }

    /// `cache status` and `cache clear` already routed unknown flags through
    /// `unknown_argument`, but they had no help body in `help_text_for` and
    /// were missing from `REGISTERED_COMMAND_PATHS`, so every typo fell to the
    /// bare no-suggestion branch. The accepted flags are closed and known.
    #[test]
    fn cache_status_and_clear_suggest_their_own_documented_flags() {
        for (command, typo, expected) in [
            ("cache status", "--jsonn", "--json"),
            ("cache clear", "--dry-runn", "--dry-run"),
            ("cache clear", "--fors", "--force"),
        ] {
            assert_eq!(
                unknown_argument(command, typo),
                format!(
                    "unknown {command} argument {typo:?}. \
                     Did you mean `{expected}`? Run `ripr {command} --help`."
                ),
            );
        }
        let status_force = unknown_argument("cache status", "--force");
        assert!(
            !status_force.contains("Did you mean"),
            "cache status must not suggest a cache-clear flag: {status_force}"
        );
        let clear_json = unknown_argument("cache clear", "--json");
        assert!(
            !clear_json.contains("Did you mean"),
            "cache clear must not suggest a cache-status flag: {clear_json}"
        );
    }

    /// The six public commands in #3812 already rejected unknown flags, but
    /// they used command-local bare error strings and had no `help_text_for`
    /// body, so a typo never produced a scoped suggestion.
    #[test]
    fn remaining_public_flag_errors_suggest_their_own_documented_flags() {
        for (command, typo, expected) in [
            ("first-pr", "--gap-ledgr", "--gap-ledger"),
            ("first-pr", "--out-di", "--out-dir"),
            ("pr-summary", "--baselin", "--baseline"),
            ("annotations", "--commnts", "--comments"),
            ("pr-evidence", "--hea", "--head"),
            ("impacted-evidence", "--pr-evidenc", "--pr-evidence"),
            ("plus", "--gap-ledgr", "--gap-ledger"),
            ("plus", "--repo-exposure-sumary", "--repo-exposure-summary"),
        ] {
            assert_eq!(
                unknown_argument(command, typo),
                format!(
                    "unknown {command} argument {typo:?}. \
                     Did you mean `{expected}`? Run `ripr {command} --help`."
                ),
            );
        }
        let first_pr_json = unknown_argument("first-pr", "--json");
        assert!(
            !first_pr_json.contains("Did you mean"),
            "first-pr must not suggest a cache-status flag: {first_pr_json}"
        );
        let summary_comments = unknown_argument("pr-summary", "--comments");
        assert!(
            !summary_comments.contains("Did you mean"),
            "pr-summary must not suggest an annotations flag: {summary_comments}"
        );
        let annotations_baseline = unknown_argument("annotations", "--baseline");
        assert!(
            !annotations_baseline.contains("Did you mean"),
            "annotations must not suggest a pr-summary flag: {annotations_baseline}"
        );
        let evidence_comments = unknown_argument("pr-evidence", "--comments");
        assert!(
            !evidence_comments.contains("Did you mean"),
            "pr-evidence must not suggest an annotations flag: {evidence_comments}"
        );
        let impacted_baseline = unknown_argument("impacted-evidence", "--baseline");
        assert!(
            !impacted_baseline.contains("Did you mean"),
            "impacted-evidence must not suggest a pr-summary flag: {impacted_baseline}"
        );
        let plus_baseline = unknown_argument("plus", "--baseline");
        assert!(
            !plus_baseline.contains("Did you mean"),
            "plus must not suggest a pr-summary flag: {plus_baseline}"
        );
        let status_gap = unknown_argument("cache status", "--gap-ledger");
        assert!(
            !status_gap.contains("Did you mean"),
            "cache status must not inherit first-pr or plus flags: {status_gap}"
        );
    }

    /// Every command path the CLI reports errors for must resolve to a help
    /// body that documents at least one flag, otherwise `unknown_argument`
    /// silently degrades to the no-suggestion branch forever.
    #[test]
    fn every_registered_command_path_resolves_to_documented_flags() {
        for command in help::registered_command_paths() {
            let flags = match help::help_text_for(command) {
                Some(help_text) => known_flags(command, help_text),
                None => Vec::new(),
            };
            assert!(
                !flags.is_empty(),
                "no help text documenting flags is registered for {command:?}"
            );
        }
    }

    /// #2583 review: `option_section` and `help_text_for` each decide, on
    /// their own, which commands share a help body. If a new sibling is added
    /// to `help_text_for` but not to `option_section`, that command silently
    /// mines the whole shared body again — reintroducing exactly the
    /// sibling-flag leak this PR fixes, with no failing test.
    ///
    /// Binding the two by construction is a larger refactor; this pins the
    /// invariant instead: every command that shares a help body with another
    /// command must have an options section, and every command with an options
    /// section must be one that shares a help body.
    #[test]
    fn every_shared_help_body_command_has_an_options_section() {
        let mut bodies: Vec<(&'static str, &'static str)> = Vec::new();
        for command in help::registered_command_paths() {
            if let Some(help_text) = help::help_text_for(command) {
                bodies.push((command, help_text));
            }
        }
        for (command, help_text) in &bodies {
            let shares_body = bodies
                .iter()
                .any(|(other, other_text)| other != command && other_text == help_text);
            assert_eq!(
                shares_body,
                option_section(command).is_some(),
                "{command:?}: shares a help body with a sibling = {shares_body}, \
                 but option_section is {:?}. These must agree, or scoped \
                 suggestions silently fall back to the whole shared body.",
                option_section(command)
            );
        }
    }

    /// #2583 review: sweep every registered command against every flag its
    /// help body mentions anywhere — including the sibling sections a shared
    /// body carries — and assert none of them is ever answered with itself.
    /// This covers the whole surface rather than the two reported examples.
    #[test]
    fn no_registered_command_ever_suggests_the_rejected_flag_itself() {
        for command in help::registered_command_paths() {
            let Some(help_text) = help::help_text_for(command) else {
                continue;
            };
            // Deliberately mine the *whole* body, so sibling-section flags are
            // included as inputs even though scoping should exclude them as
            // candidates.
            for flag in known_flags("", help_text) {
                let message = unknown_argument(command, flag);
                assert!(
                    !message.contains(&format!("Did you mean `{flag}`?")),
                    "{command:?} answered {flag:?} with itself: {message}"
                );
            }
        }
    }

    /// Scoping must actively *select* the right sibling's flag, not merely
    /// suppress suggestions. The same input resolves differently under two
    /// baseline subcommands that share one help body.
    #[test]
    fn scoping_selects_the_subcommands_own_flag_for_a_near_miss() {
        // `--out` belongs to create; `--out-md` belongs to diff.
        let message = unknown_argument("baseline create", "--out-m");
        assert!(message.contains("Did you mean `--out`?"), "{message}");

        let message = unknown_argument("baseline diff", "--out-m");
        assert!(message.contains("Did you mean `--out-md`?"), "{message}");
    }

    /// A single-command help body keeps mining its full option list, because
    /// its usage line is often just `ripr check [OPTIONS]`.
    #[test]
    fn single_command_help_still_mines_the_full_option_list() {
        let flags = match help::help_text_for("check") {
            Some(help_text) => known_flags("check", help_text),
            None => Vec::new(),
        };
        assert!(flags.contains(&"--format"), "{flags:?}");
        assert!(flags.contains(&"--worktree"), "{flags:?}");
    }
}
