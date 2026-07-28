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
pub(in crate::cli) fn unknown_argument(command: &str, arg: &str) -> String {
    match closest_flag(command, arg) {
        Some(suggestion) => format!(
            "unknown {command} argument {arg:?}. Did you mean `{suggestion}`? Run `ripr {command} --help`."
        ),
        None => format!("unknown {command} argument {arg:?}. Run `ripr {command} --help`."),
    }
}

#[cfg(test)]
fn unknown_value(label: &str, value: &str, accepted: &[&str]) -> String {
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
/// Help bodies list options as indented `--flag` lines, e.g.
/// `  --format FORMAT    Output format. ...`. Usage lines are indented too but
/// bracket their flags (`[--diff PATH]`), so requiring the token to start the
/// trimmed line keeps this to the option list.
fn known_flags<'a>(command: &str, help_text: &'a str) -> Vec<&'a str> {
    let section = option_section(command);
    let mut in_section = section.is_none();
    let mut flags = Vec::new();
    for line in help_text.lines() {
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
        let trimmed = line.trim_start();
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
    fn known_flags_reads_the_option_list_and_skips_usage_brackets() {
        let help_text = "Summary line.\n\nUsage: ripr thing [--diff PATH]\n\nOptions:\n  --root PATH    Workspace root.\n  --json         Shortcut.\n  --            End of options.\n";
        assert_eq!(known_flags("thing", help_text), vec!["--root", "--json"]);
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
