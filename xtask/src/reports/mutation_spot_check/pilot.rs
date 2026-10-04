//! Pilot top-recommendation precision for `cargo xtask mutation-spot-check`.
//!
//! `ripr pilot` sends a developer to its top-ranked seams first, so a wrong
//! recommendation costs a wasted trip. This scores each of pilot's top
//! `PILOT_MAX_SEAMS` recommendations against the same cargo-mutants outcomes
//! the verdict scoring uses:
//!
//! - `seam`: on a predicate or return seam whose expression occurs once on
//!   its line, viable operator mutants that start inside that expression. Any missed mutant confirms the
//!   recommendation; all caught refutes it.
//! - `line`: with no such mutant, viable non-`FnValue` mutants that start on
//!   the recommended line. Coarser: one can belong to another expression.
//! - `owner`: when the line has none, the `FnValue` mutants of the innermost
//!   function containing the line. Replacing the whole body is coarser than
//!   changing the seam, so each tier is counted separately.
//! - `unscored`: no tier has a viable mutant.
//!
//! Precision is confirmed over confirmed plus refuted. It speaks only for the
//! recorded revisions, cargo-mutants version and this join rule.

use super::{EXPOSURE_TIMEOUT, path_arg, read_json, run_text};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

/// Recommendations scored per repository. Pilot's default is 5; ten gives
/// the scoreboard enough scored recommendations on small crates.
pub(super) const PILOT_MAX_SEAMS: usize = 10;

pub(super) const CLAIM_BOUNDARY: &str = "A pilot recommendation is confirmed when a viable operator mutant of its predicate or return expression was missed (with none, a viable mutant on its line; with none there either, a whole-body mutant of its innermost function), refuted when every such mutant was caught, and unscored otherwise. Precision is confirmed over confirmed plus refuted, for the recorded revisions and cargo-mutants versions only.";

/// The 1-based column range `[start, end)` of `expression` on `line`, in
/// characters as cargo-mutants reports them, when it occurs exactly once.
fn expression_columns(line: &str, expression: &str) -> Option<(u64, u64)> {
    if expression.is_empty() {
        return None;
    }
    let mut occurrences = line.match_indices(expression);
    let (byte, _) = occurrences.next()?;
    if occurrences.next().is_some() {
        return None;
    }
    let start = line[..byte].chars().count() as u64 + 1;
    Some((start, start + expression.chars().count() as u64))
}

/// Run `ripr pilot` on `checkout` and return its ranked top seams.
pub(super) fn pilot_top_seams(
    binary: &Path,
    scratch: &Path,
    name: &str,
    checkout: &Path,
) -> Result<Vec<Value>, String> {
    let out_dir = scratch.join(format!("{name}.pilot"));
    run_text(
        &path_arg(binary),
        &[
            "pilot".to_string(),
            "--root".to_string(),
            path_arg(checkout),
            "--out".to_string(),
            path_arg(&out_dir),
            "--max-seams".to_string(),
            PILOT_MAX_SEAMS.to_string(),
            "--quiet".to_string(),
        ],
        EXPOSURE_TIMEOUT,
        "ripr pilot for spot check",
    )?;
    let summary = read_json(&out_dir.join("pilot-summary.json"))?;
    top_seams_from_summary(name, &summary)
}

/// Pilot exits 0 with a `partial` summary and no ranked seams when its own
/// budget runs out, so only a `complete` summary counts as a ranking;
/// anything else makes the repo unavailable rather than empty.
fn top_seams_from_summary(name: &str, summary: &Value) -> Result<Vec<Value>, String> {
    let status = summary
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if status != "complete" {
        return Err(format!(
            "pilot summary for `{name}` has status `{status}`, not `complete`"
        ));
    }
    summary
        .get("top_actionable_seams")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| format!("pilot summary for `{name}` has no top_actionable_seams array"))
}

/// One viable runtime outcome with the location facts the judge needs.
struct Outcome<'a> {
    name: &'a str,
    genre: &'a str,
    file: &'a str,
    line: u64,
    column: u64,
    function_span: Option<(u64, u64)>,
    whole_body: bool,
    missed: bool,
}

/// Join cargo-mutants `mutants.json` (locations) to `outcomes.json`
/// (results) by mutant name, keeping only caught and missed mutants.
fn viable_outcomes<'a>(mutants: &'a Value, outcomes: &'a Value) -> Vec<Outcome<'a>> {
    let results: BTreeMap<&str, &str> = outcomes
        .get("outcomes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|record| {
            Some((
                record.pointer("/scenario/Mutant/name")?.as_str()?,
                record.get("summary")?.as_str()?,
            ))
        })
        .collect();
    mutants
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|mutant| {
            let missed = match *results.get(mutant.get("name")?.as_str()?)? {
                "MissedMutant" => true,
                "CaughtMutant" => false,
                _ => return None,
            };
            let span = |end: &str| {
                mutant
                    .pointer(&format!("/function/span/{end}/line"))
                    .and_then(Value::as_u64)
            };
            let name = mutant.get("name")?.as_str()?;
            let genre = mutant.get("genre").and_then(Value::as_str).unwrap_or("");
            Some(Outcome {
                name,
                genre,
                file: mutant.get("file")?.as_str()?,
                line: mutant.pointer("/span/start/line")?.as_u64()?,
                column: mutant
                    .pointer("/span/start/column")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                function_span: span("start").zip(span("end")),
                whole_body: genre == "FnValue",
                missed,
            })
        })
        .collect()
}

/// Judge each recommendation, in pilot's rank order.
///
/// `expressions` maps seam id to the seam's source expression (from the repo
/// exposure JSON) and `source_line` returns a checkout line's text. On a
/// predicate or return seam whose expression occurs exactly once on its line,
/// operator mutants that start inside that occurrence form the `seam` tier and
/// decide alone, so a mutant of another expression on the same line, even one
/// with the same operator, cannot grade the seam. Otherwise the coarser `line`
/// tier, then the `owner` tier, applies.
pub(super) fn judge_recommendations(
    top: &[Value],
    mutants: &Value,
    outcomes: &Value,
    expressions: &BTreeMap<&str, &str>,
    source_line: &dyn Fn(&str, u64) -> Option<String>,
) -> Vec<Value> {
    let viable = viable_outcomes(mutants, outcomes);
    top.iter()
        .enumerate()
        .map(|(index, seam)| {
            let file = seam.get("file").and_then(Value::as_str).unwrap_or("");
            let line = seam.get("line").and_then(Value::as_u64).unwrap_or(0);
            let on_line = viable
                .iter()
                .filter(|o| !o.whole_body && o.file == file && o.line == line)
                .collect::<Vec<_>>();
            let kind = seam.get("kind").and_then(Value::as_str).unwrap_or("");
            let expression = seam
                .get("seam_id")
                .and_then(Value::as_str)
                .and_then(|id| expressions.get(id))
                .copied()
                .unwrap_or("");
            let columns =
                source_line(file, line).and_then(|text| expression_columns(&text, expression));
            let precise = on_line
                .iter()
                .copied()
                .filter(|o| {
                    columns.is_some_and(|(start, end)| (start..end).contains(&o.column))
                        && super::pairing_for(o.genre, o.name, kind, expression) == "seam_precise"
                })
                .collect::<Vec<_>>();
            let (tier, joined) = if !precise.is_empty() {
                ("seam", precise)
            } else if on_line.is_empty() {
                let containing = viable
                    .iter()
                    .filter(|o| o.whole_body && o.file == file)
                    .filter_map(|o| o.function_span.map(|span| (span, o)))
                    .filter(|((start, end), _)| (*start..=*end).contains(&line))
                    .collect::<Vec<_>>();
                let innermost = containing.iter().map(|((start, _), _)| *start).max();
                let owner = containing
                    .into_iter()
                    .filter(|((start, _), _)| Some(*start) == innermost)
                    .map(|(_, outcome)| outcome)
                    .collect::<Vec<_>>();
                ("owner", owner)
            } else {
                ("line", on_line)
            };
            let missed = joined.iter().filter(|o| o.missed).count();
            let caught = joined.len() - missed;
            let (verdict, tier) = match (missed, caught) {
                (0, 0) => ("unscored", "none"),
                (0, _) => ("refuted", tier),
                _ => ("confirmed", tier),
            };
            let text = |key: &str| seam.get(key).cloned().unwrap_or(Value::Null);
            json!({
                "rank": index + 1,
                "seam_id": text("seam_id"),
                "file": file,
                "line": line,
                "kind": text("kind"),
                "grip_class": text("grip_class"),
                "verdict": verdict,
                "tier": tier,
                "caught": caught,
                "missed": missed,
            })
        })
        .collect()
}

/// Pool the judged recommendations of every repository.
pub(super) fn summarize(repos: &[(String, Result<Vec<Value>, String>)]) -> Value {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_tier: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_class: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (_, judged) in repos {
        for row in judged.iter().flatten() {
            let field = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or("");
            let verdict = match field("verdict") {
                "confirmed" => "confirmed",
                "refuted" => "refuted",
                _ => "unscored",
            };
            *counts.entry(verdict).or_default() += 1;
            *by_tier
                .entry(field("tier").to_string())
                .or_default()
                .entry(verdict.to_string())
                .or_default() += 1;
            *by_class
                .entry(field("grip_class").to_string())
                .or_default()
                .entry(verdict.to_string())
                .or_default() += 1;
        }
    }
    let confirmed = counts.get("confirmed").copied().unwrap_or(0);
    let scored = confirmed + counts.get("refuted").copied().unwrap_or(0);
    json!({
        "max_seams_per_repo": PILOT_MAX_SEAMS,
        "claim_boundary": CLAIM_BOUNDARY,
        "recommendations_total": counts.values().sum::<usize>(),
        "scored": scored,
        "counts": counts,
        "precision": super::rate(confirmed, scored),
        "by_tier": by_tier,
        "by_grip_class": by_class,
        "unavailable_repos": repos.iter().filter(|(_, judged)| judged.is_err()).count(),
        "repos": repos.iter().map(|(name, judged)| match judged {
            Ok(judged) => json!({"name": name, "recommendations": judged}),
            Err(reason) => json!({"name": name, "recommendations": [], "unavailable": reason}),
        }).collect::<Vec<_>>(),
    })
}

/// Markdown section for the pilot scoring, empty when the report has none.
pub(super) fn markdown(report: &Value) -> String {
    let Some(section) = report.get("pilot_top_recommendations") else {
        return String::new();
    };
    let count = |key: &str| {
        section
            .pointer(&format!("/counts/{key}"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let mut out = format!(
        "\n## Pilot top recommendations\n\n{CLAIM_BOUNDARY}\n\nTop {} per repository: {} confirmed, {} refuted, {} unscored; precision {}.\n\n| Repo | Rank | Seam | Kind | Grip class | Verdict | Tier | Caught | Missed |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
        section
            .get("max_seams_per_repo")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        count("confirmed"),
        count("refuted"),
        count("unscored"),
        section
            .get("precision")
            .and_then(Value::as_f64)
            .map_or_else(|| "n/a".to_string(), |rate| format!("{:.1}%", rate * 100.0)),
    );
    for repo in section
        .get("repos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = repo.get("name").and_then(Value::as_str).unwrap_or("");
        if let Some(reason) = repo.get("unavailable").and_then(Value::as_str) {
            let reason = reason.lines().next().unwrap_or("").replace('|', "\\|");
            out.push_str(&format!(
                "| {name} | - | pilot unavailable: {reason} | | | unavailable | | | |\n"
            ));
        }
        for row in repo
            .get("recommendations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or("");
            let number = |key: &str| row.get(key).and_then(Value::as_u64).unwrap_or(0);
            out.push_str(&format!(
                "| {name} | {} | `{}:{}` | {} | {} | {} | {} | {} | {} |\n",
                number("rank"),
                text("file"),
                number("line"),
                text("kind"),
                text("grip_class"),
                text("verdict"),
                text("tier"),
                number("caught"),
                number("missed"),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mutant(name: &str, line: u64, genre: &str, function: (u64, u64)) -> Value {
        // Operator mutant names carry their column (`src/a.rs:3:22: ...`).
        let column = name
            .split(':')
            .nth(2)
            .and_then(|column| column.parse::<u64>().ok())
            .unwrap_or(1);
        json!({
            "name": name,
            "file": "src/a.rs",
            "genre": genre,
            "span": {"start": {"line": line, "column": column}, "end": {"line": line, "column": column + 1}},
            "function": {"span": {"start": {"line": function.0}, "end": {"line": function.1}}},
        })
    }

    fn outcome(name: &str, summary: &str) -> Value {
        json!({"scenario": {"Mutant": {"name": name}}, "summary": summary})
    }

    fn seam(line: u64) -> Value {
        json!({"seam_id": format!("s{line}"), "file": "src/a.rs", "line": line, "kind": "call_presence", "grip_class": "weakly_gripped"})
    }

    #[test]
    fn line_mutants_decide_before_whole_body_mutants() {
        let mutants = json!([
            mutant("op-caught", 3, "BinaryOperator", (1, 9)),
            mutant("op-missed", 4, "BinaryOperator", (1, 9)),
            mutant("op-unviable", 5, "BinaryOperator", (1, 9)),
            mutant("body-missed", 2, "FnValue", (1, 9)),
        ]);
        let outcomes = json!({"outcomes": [
            outcome("op-caught", "CaughtMutant"),
            outcome("op-missed", "MissedMutant"),
            outcome("op-unviable", "Unviable"),
            outcome("body-missed", "MissedMutant"),
        ]});
        let judged = judge_recommendations(
            &[seam(3), seam(4), seam(5)],
            &mutants,
            &outcomes,
            &BTreeMap::new(),
            &|_, _| None,
        );
        let verdicts = judged
            .iter()
            .map(|row| (row["verdict"].as_str(), row["tier"].as_str()))
            .collect::<Vec<_>>();
        // A caught operator on line 3 refutes it even though the body
        // mutant was missed; line 5's only line mutant is unviable, so the
        // missed whole-body mutant decides it.
        assert_eq!(
            verdicts,
            vec![
                (Some("refuted"), Some("line")),
                (Some("confirmed"), Some("line")),
                (Some("confirmed"), Some("owner")),
            ]
        );
    }

    #[test]
    fn seam_expression_mutants_decide_over_other_expressions_on_the_line() {
        // Line 3 is `    let a = x > 0; if y > 0 {`: the `>` at column 15
        // belongs to `x > 0`, the `>` at column 25 to the `y > 0` seam. The
        // missed mutant of the other expression must not confirm the seam.
        let text = "    let a = x > 0; if y > 0 {";
        assert_eq!(text.find("y > 0").map(|byte| byte + 1), Some(23));
        let mutants = json!([
            mutant(
                "src/a.rs:3:15: replace > with >= in gate",
                3,
                "BinaryOperator",
                (1, 9)
            ),
            mutant(
                "src/a.rs:3:25: replace > with >= in gate",
                3,
                "BinaryOperator",
                (1, 9)
            ),
        ]);
        let outcomes = json!({"outcomes": [
            outcome("src/a.rs:3:15: replace > with >= in gate", "MissedMutant"),
            outcome("src/a.rs:3:25: replace > with >= in gate", "CaughtMutant"),
        ]});
        let mut predicate = seam(3);
        predicate["kind"] = json!("predicate_boundary");
        let source =
            |file: &str, line: u64| (file == "src/a.rs" && line == 3).then(|| text.to_string());
        let expressions = BTreeMap::from([("s3", "y > 0")]);
        let judged = judge_recommendations(
            std::slice::from_ref(&predicate),
            &mutants,
            &outcomes,
            &expressions,
            &source,
        );
        assert_eq!(judged[0]["verdict"], "refuted");
        assert_eq!(judged[0]["tier"], "seam");
        // An expression that occurs twice on the line cannot be located, so
        // the coarse line tier reads both mutants.
        let twice = BTreeMap::from([("s3", "> 0")]);
        let judged = judge_recommendations(
            std::slice::from_ref(&predicate),
            &mutants,
            &outcomes,
            &twice,
            &source,
        );
        assert_eq!(judged[0]["verdict"], "confirmed");
        assert_eq!(judged[0]["tier"], "line");
    }

    #[test]
    fn owner_tier_reads_only_the_innermost_function() {
        let mutants = json!([
            mutant("outer-missed", 1, "FnValue", (1, 20)),
            mutant("inner-caught", 6, "FnValue", (5, 8)),
        ]);
        let outcomes = json!({"outcomes": [
            outcome("outer-missed", "MissedMutant"),
            outcome("inner-caught", "CaughtMutant"),
        ]});
        let judged = judge_recommendations(
            &[seam(7), seam(12), seam(30)],
            &mutants,
            &outcomes,
            &BTreeMap::new(),
            &|_, _| None,
        );
        let verdicts = judged
            .iter()
            .map(|row| row["verdict"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            verdicts,
            vec![Some("refuted"), Some("confirmed"), Some("unscored")]
        );
    }

    #[test]
    fn summary_precision_excludes_unscored_recommendations() {
        let judged = vec![
            json!({"verdict": "confirmed", "tier": "line", "grip_class": "ungripped"}),
            json!({"verdict": "refuted", "tier": "owner", "grip_class": "weakly_gripped"}),
            json!({"verdict": "refuted", "tier": "line", "grip_class": "weakly_gripped"}),
            json!({"verdict": "unscored", "tier": "none", "grip_class": "ungripped"}),
        ];
        let summary = summarize(&[("a".to_string(), Ok(judged))]);
        assert_eq!(summary["scored"], 3);
        assert_eq!(summary["recommendations_total"], 4);
        assert_eq!(summary["precision"], json!(0.333));
        assert_eq!(summary["by_grip_class"]["weakly_gripped"]["refuted"], 2);
        assert!(
            markdown(&json!({"pilot_top_recommendations": summary})).contains("precision 33.3%")
        );
        assert_eq!(markdown(&json!({})), "");
    }

    #[test]
    fn unavailable_pilot_is_reported_without_dropping_other_repos() {
        let judged =
            vec![json!({"verdict": "confirmed", "tier": "line", "grip_class": "ungripped"})];
        let summary = summarize(&[
            ("a".to_string(), Ok(judged)),
            (
                "b".to_string(),
                Err("ripr pilot timed out | after 600s\nmore".to_string()),
            ),
        ]);
        assert_eq!(summary["scored"], 1);
        assert_eq!(summary["unavailable_repos"], 1);
        assert_eq!(
            summary["repos"][1]["unavailable"],
            "ripr pilot timed out | after 600s\nmore"
        );
        let rendered = markdown(&json!({"pilot_top_recommendations": summary}));
        assert!(rendered.contains(
            "| b | - | pilot unavailable: ripr pilot timed out \\| after 600s | | | unavailable |"
        ));
    }

    #[test]
    fn only_a_complete_pilot_summary_counts_as_a_ranking() {
        let seams = json!([{"file": "src/lib.rs", "line": 3}]);
        let complete = json!({"status": "complete", "top_actionable_seams": seams});
        assert_eq!(
            top_seams_from_summary("a", &complete).map(|top| top.len()),
            Ok(1)
        );
        for summary in [
            json!({"status": "partial", "top_actionable_seams": []}),
            json!({"top_actionable_seams": seams}),
        ] {
            assert!(
                top_seams_from_summary("a", &summary)
                    .is_err_and(|err| err.contains("not `complete`"))
            );
        }
    }
}
