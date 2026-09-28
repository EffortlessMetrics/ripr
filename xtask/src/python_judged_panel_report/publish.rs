//! Rendering and publication: Markdown from the same derived Value, verified
//! byte-stability, staged temp siblings, and one-generation report writes
//! (RIPR-SPEC-0092).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::REPORT_RERUN;
use super::adjudication::acquire_path_lock;
use super::report::RenderedReport;

// ---------------------------------------------------------------------------
// Rendering (JSON from serde; Markdown from the same Value)
// ---------------------------------------------------------------------------

pub(super) fn verify_stored_bytes(path: &Path, expected: &str) -> Result<(), String> {
    let stored = fs::read_to_string(path).map_err(|error| {
        format!(
            "report file `{}` is missing or unreadable; run `{REPORT_RERUN}` first: {error}",
            path.display()
        )
    })?;
    if stored != expected {
        return Err(format!(
            "report file `{}` drifted from a fresh render; run `{REPORT_RERUN}` to refresh",
            path.display()
        ));
    }
    Ok(())
}

/// FIX f2TMb helper: one staged temp sibling, flushed to disk before the
/// caller renames it into place.
fn stage_temp_sibling(path: &Path, body: &str) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("no parent directory for `{}`", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create report output directory `{}`: {error}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("report");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(".{file_name}.tmp-{}-{nanos}", std::process::id()));
    let mut file =
        fs::File::create(&temp).map_err(|error| format!("stage `{}`: {error}", temp.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|error| format!("stage `{}`: {error}", temp.display()))?;
    file.sync_all()
        .map_err(|error| format!("flush `{}`: {error}", temp.display()))?;
    Ok(temp)
}

/// FIX f2TNz: report.json and report.md publish as one generation — both
/// files are staged completely as temp siblings first, then renamed into
/// place. If the second rename fails, the first publication is rolled back
/// from the prior bytes held in memory, so no half-updated pair survives.
/// Publish one report generation: both files staged, then renamed in — the
/// json first, the markdown second, with the json rolled back when the
/// markdown rename fails. Residual, disclosed (#3674 review round 5): a
/// process termination between the two renames can still leave a mixed pair
/// on disk; the one-generation guarantee holds for observable errors, not
/// for a crash mid-publication. A crashed pair self-heals on the next
/// successful publication, and the generation lock excludes concurrent
/// publishers.
pub(super) fn write_report_generation(
    json_path: &Path,
    markdown_path: &Path,
    json: &str,
    markdown: &str,
) -> Result<(), String> {
    // FIX fqNv (devin round 4): two concurrent report commands could
    // interleave their separate renames and publish a json from one
    // generation with markdown from another; one exclusive lock per output
    // directory serializes the pair.
    let out_parent = json_path
        .parent()
        .ok_or_else(|| format!("no parent directory for `{}`", json_path.display()))?;
    fs::create_dir_all(out_parent).map_err(|error| {
        format!(
            "create report output directory `{}`: {error}",
            out_parent.display()
        )
    })?;
    let _generation_lock = acquire_path_lock(
        out_parent.join(".report-generation.lock"),
        "report generation",
        |lock_path| {
            format!(
                "another report generation is publishing to this output directory (`{}` exists); if none is running, remove the stale lock",
                lock_path.display()
            )
        },
    )?;
    let json_temp = stage_temp_sibling(json_path, json)?;
    let markdown_temp = match stage_temp_sibling(markdown_path, markdown) {
        Ok(temp) => temp,
        Err(error) => {
            let _ = fs::remove_file(&json_temp);
            return Err(error);
        }
    };
    let prior_json = fs::read(json_path).ok();
    if let Err(error) = fs::rename(&json_temp, json_path) {
        let _ = fs::remove_file(&json_temp);
        let _ = fs::remove_file(&markdown_temp);
        return Err(format!("publish `{}`: {error}", json_path.display()));
    }
    if let Err(error) = fs::rename(&markdown_temp, markdown_path) {
        // Roll the first publication back so the pair stays one generation.
        // FIX fTNz (devin round 1): a failed restoration must be named, not
        // claimed — "restored" in the message would otherwise be a false
        // confidence surface when the restore write itself failed.
        let restored = match &prior_json {
            Some(bytes) => fs::write(json_path, bytes).is_ok(),
            None => match fs::remove_file(json_path) {
                Ok(()) => true,
                Err(remove_error) if remove_error.kind() == std::io::ErrorKind::NotFound => true,
                Err(_) => false,
            },
        };
        let _ = fs::remove_file(&markdown_temp);
        let restoration_note = if restored {
            format!(
                "the prior report.json generation was restored; retry or inspect `{}`",
                json_path.display()
            )
        } else {
            "RESTORATION FAILED: report.json may hold the new generation without its markdown pair — delete the mismatched pair and re-run".to_string()
        };
        return Err(format!(
            "publish `{}` failed after `{}` was replaced ({error}); {restoration_note}",
            markdown_path.display(),
            json_path.display()
        ));
    }
    Ok(())
}

/// FIX f2XZZ: the adjudication record replacement is atomic — serialize,
/// stage a unique temp sibling in the same directory, flush, then rename over
/// the destination. `std::fs::rename` replaces an existing destination on
/// Windows (MoveFileEx with MOVEFILE_REPLACE_EXISTING). FIX fqm (devin round
/// 2): there is deliberately no copy fallback — `fs::copy` truncates the
/// destination before writing, so a mid-copy failure would destroy the prior
/// record. A rename failure therefore fails the command and leaves the prior
/// record byte-identical. Any failure before the rename removes the staged
pub(super) fn print_report_summary(report: &RenderedReport) {
    let value = serde_json::from_str::<Value>(&report.json).unwrap_or(Value::Null);
    let count = |key: &str| value["counts"][key].as_u64().unwrap_or(0);
    println!(
        "Python judged PR panel report: selected={} replayed={} not_run={} adjudicated={} inconclusive={} disputed={} pending_second_role={} stale_row={} stale={} anchor_stale={} mismatched={} comparison_unavailable={}",
        count("selected"),
        count("replayed"),
        count("not_run"),
        count("adjudicated"),
        count("inconclusive"),
        count("disputed"),
        count("pending_second_role"),
        count("stale_row"),
        count("stale"),
        count("anchor_stale"),
        count("mismatched"),
        count("comparison_unavailable"),
    );
    for key in ["false_actionable", "false_exposed"] {
        let rate = &value["rates"][key];
        let rate_text = rate["rate"]
            .as_f64()
            .map(|measured| format!("{measured:.3}"))
            .unwrap_or_else(|| "none: no denominator".to_string());
        println!(
            "{key}: {}/{} (rate {rate_text}, undecided {})",
            rate["numerator"].as_u64().unwrap_or(0),
            rate["denominator"].as_u64().unwrap_or(0),
            rate["undecided"].as_u64().unwrap_or(0),
        );
    }
}

/// Both renderings walk the same derived Value: the two surfaces can never
/// disagree.
pub(super) fn render_markdown(report: &Value) -> String {
    let count = |key: &str| report["counts"][key].as_u64().unwrap_or(0);
    let mut out = String::new();
    out.push_str("# Python Judged PR Panel Report (RIPR-SPEC-0092)\n\nschema ");
    out.push_str(report["schema_version"].as_str().unwrap_or("?"));
    out.push_str(" — authority boundary: ");
    out.push_str(report["authority_boundary"].as_str().unwrap_or("?"));
    out.push_str("\n\n## As-of identity\n\n");
    out.push_str(&format!(
        "- panel digest: `{}` ({} envelope file(s))\n",
        report["as_of"]["panel_digest"].as_str().unwrap_or("?"),
        report["inputs"]["inventory"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
    ));
    for identity in report["inputs"]["inventory"]
        .as_array()
        .into_iter()
        .flatten()
    {
        out.push_str(&format!(
            "  - `{}` sha256 `{}`\n",
            identity["path"].as_str().unwrap_or("?"),
            identity["sha256"].as_str().unwrap_or("?"),
        ));
    }
    match report["as_of"]["replay_binary"]["version"].as_str() {
        Some(version) => out.push_str(&format!(
            "- replay records: `{}` — binary `{version}` sha256 `{}` (record schema {})\n",
            report["inputs"]["records_dir"].as_str().unwrap_or("?"),
            report["as_of"]["replay_binary"]["sha256"]
                .as_str()
                .unwrap_or("?"),
            report["as_of"]["replay_record_schema_version"]
                .as_str()
                .unwrap_or("unknown"),
        )),
        None => out.push_str(&format!(
            "- replay records: `{}` — no records read\n",
            report["inputs"]["records_dir"].as_str().unwrap_or("?"),
        )),
    }
    out.push_str(&format!(
        "- adjudications: `{}`\n",
        report["inputs"]["adjudications_dir"]
            .as_str()
            .unwrap_or("?"),
    ));

    out.push_str("\n## Counts\n\n");
    for key in [
        "selected",
        "replayed",
        "not_run",
        "adjudicated",
        "pending_second_role",
        "disputed",
        "inconclusive",
        "stale_row",
        "stale",
        "anchor_stale",
        "mismatched",
        "comparison_unavailable",
        "unjudged",
        "no_replay_record",
    ] {
        out.push_str(&format!("- {key}: {}\n", count(key)));
    }

    out.push_str("\n## Coverage\n");
    for (title, key) in [
        ("By direction", "by_direction"),
        ("By repository", "by_repository"),
        (
            "By behavior family (a row counts under each of its shapes)",
            "by_behavior_family",
        ),
        (
            "By oracle alignment (`unrecorded` for rows without an observed alignment)",
            "by_oracle_alignment",
        ),
        (
            "By limitation kind (`none` for rows that name no static limit)",
            "by_limitation_kind",
        ),
    ] {
        out.push_str(&format!(
            "\n### {title}\n\n| value | selected | replayed | adjudicated |\n| --- | --- | --- | --- |\n"
        ));
        for (value, cell) in report["coverage"][key].as_object().into_iter().flatten() {
            out.push_str(&format!(
                "| {value} | {} | {} | {} |\n",
                cell[0].as_u64().unwrap_or(0),
                cell[1].as_u64().unwrap_or(0),
                cell[2].as_u64().unwrap_or(0),
            ));
        }
    }
    out.push_str(&format!(
        "- relation basis: unavailable — {}\n",
        report["coverage"]["relation_basis"]["reason"]
            .as_str()
            .unwrap_or("?"),
    ));

    out.push_str("\n## Separate error rates (two-error lattice; no combined score)\n\n");
    for key in ["false_actionable", "false_exposed"] {
        let rate = &report["rates"][key];
        out.push_str(&format!(
            "- {key}: numerator {} / denominator {}",
            rate["numerator"].as_u64().unwrap_or(0),
            rate["denominator"].as_u64().unwrap_or(0),
        ));
        match rate["rate"].as_f64() {
            Some(measured) => out.push_str(&format!(" — rate {measured:.3}")),
            None => out.push_str(" — rate not disclosed: no denominator"),
        }
        let cases = rate["denominator_case_ids"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .map(|id| id.as_str().unwrap_or("?"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        out.push_str(&format!(
            " (undecided {})\n  - coverage boundary: {}\n  - denominator cases: {}\n",
            rate["undecided"].as_u64().unwrap_or(0),
            rate["coverage_boundary"].as_str().unwrap_or("?"),
            if cases.is_empty() {
                "none"
            } else {
                cases.as_str()
            },
        ));
        match rate["as_of_basis"].as_str() {
            // FIX f2TMb: the cited identity is the denominator cases' own;
            // without one common identity the rate discloses instead of
            // fabricating the directory-wide identity.
            Some("denominator_case_records") => match rate["as_of"]["binary_version"].as_str() {
                Some(version) => out.push_str(&format!(
                    "  - as-of: binary `{version}` sha256 `{}` (bound by the denominator cases' own replay records)\n",
                    rate["as_of"]["binary_sha256"].as_str().unwrap_or("?"),
                )),
                None => out.push_str("  - as-of: no replay binary identity bound\n"),
            },
            Some("no_common_binary_identity") => out.push_str(
                "  - as-of: not disclosed — no common binary identity across the denominator cases' own replay records\n",
            ),
            _ => out.push_str("  - as-of: no denominator\n"),
        }
    }
    for key in ["wrong_target", "invalid_command"] {
        let axis = &report["rates"][key];
        out.push_str(&format!(
            "- {key}: flagged {} of assessed {} (unassessed {}, disputed_axis {})\n",
            axis["flagged"].as_u64().unwrap_or(0),
            axis["assessed"].as_u64().unwrap_or(0),
            axis["unassessed"].as_u64().unwrap_or(0),
            axis["disputed_axis"].as_u64().unwrap_or(0),
        ));
    }
    let limitation = &report["rates"]["limitation_correctness"];
    let count_at = |key: &str| limitation[key].as_u64().unwrap_or(0);
    out.push_str(&format!(
        "- limitation_correctness (adjudicated `should_limit` rows): precise {}, imprecise {}, wrong_kind {}, over_limited {}, undecided {}, disputed_axis {}, not_adjudicated {}\n",
        count_at("precise"),
        count_at("imprecise"),
        count_at("wrong_kind"),
        count_at("over_limited"),
        count_at("undecided"),
        count_at("disputed_axis"),
        count_at("not_adjudicated"),
    ));

    if let Some(thresholds) = report.get("thresholds") {
        out.push_str("\n## Threshold evaluation (explicit, non-authoritative)\n\n- policy: `");
        out.push_str(thresholds["policy"]["path"].as_str().unwrap_or("?"));
        out.push_str("`\n- rationale (echoed from the policy file): ");
        out.push_str(thresholds["policy"]["rationale"].as_str().unwrap_or("?"));
        out.push('\n');
        if let Some(authority) = thresholds["policy"]["authority"].as_str() {
            out.push_str(&format!("- authority (echoed): {authority}\n"));
        }
        out.push_str(
            "\n| metric | operator | threshold | measured | result | reason |\n| --- | --- | --- | --- | --- | --- |\n",
        );
        for evaluation in thresholds["evaluations"].as_array().into_iter().flatten() {
            // FIX (#3686): count metrics render integral values without
            // decimals (adjudicated_count shows 2, not 2.000), matching the
            // threshold column. Rates always keep three decimals even when
            // they land exactly on 0 or 1 (the common zero-tolerance case) —
            // the fixed precision is part of the rate's format, not noise
            // (round-2 review: fract() alone would misclassify them).
            let metric = evaluation["metric"].as_str().unwrap_or("?");
            let measured = match evaluation["measured"].as_f64() {
                Some(measured) if metric.ends_with("_count") && measured.fract() == 0.0 => {
                    format!("{measured}")
                }
                Some(measured) => format!("{measured:.3}"),
                None => "n/a".to_string(),
            };
            let threshold = evaluation["threshold_value"].as_f64().unwrap_or(0.0);
            out.push_str(&format!(
                "| {} | {} | {} | {measured} | {} | {} |\n",
                evaluation["metric"].as_str().unwrap_or("?"),
                evaluation["operator"].as_str().unwrap_or("?"),
                if threshold == 0.0 {
                    "0".to_string()
                } else {
                    format!("{threshold}")
                },
                evaluation["result"].as_str().unwrap_or("?"),
                evaluation["reason"].as_str().unwrap_or("?"),
            ));
        }
        out.push_str(&format!(
            "\n{}\n",
            thresholds["authority_note"].as_str().unwrap_or("?")
        ));
    }

    out.push_str(
        "\n## Cases\n\n| case | direction | row kind | replay outcome | candidate | mismatches | adjudication | roles |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for case in report["cases"].as_array().into_iter().flatten() {
        let replay = &case["replay"];
        let (outcome, candidate, mismatches) = match replay.as_object() {
            // A candidate classification exists only where a comparison was
            // actually available; not_run and unavailable outcomes must not
            // read as quiet.
            Some(replay) if replay["comparison_unavailable"].as_bool() != Some(true) => (
                replay["outcome"].as_str().unwrap_or("?").to_string(),
                replay["candidate_classification"]
                    .as_str()
                    .unwrap_or("quiet")
                    .to_string(),
                match replay["mismatch_kinds"].as_array() {
                    Some(kinds) if !kinds.is_empty() => kinds
                        .iter()
                        .map(|kind| kind.as_str().unwrap_or("?"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => "none".to_string(),
                },
            ),
            Some(replay) => (
                replay["outcome"].as_str().unwrap_or("?").to_string(),
                "n/a".to_string(),
                "n/a".to_string(),
            ),
            None => (
                "no_record".to_string(),
                "n/a".to_string(),
                "n/a".to_string(),
            ),
        };
        let adjudication = &case["adjudication"];
        let (state, roles) = match adjudication.as_object() {
            Some(adjudication) => (
                adjudication["state"].as_str().unwrap_or("?").to_string(),
                match adjudication["roles"].as_array() {
                    Some(roles) if !roles.is_empty() => roles
                        .iter()
                        .map(|role| role.as_str().unwrap_or("?"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => "n/a".to_string(),
                },
            ),
            None => ("unjudged".to_string(), "n/a".to_string()),
        };
        // FIX #3685-review round 2: the anchor-staleness reason must reach
        // the human surface, not only the JSON — a count without a visible
        // reason is a false-confidence surface.
        let anchor_note = replay["anchor_stale_reason"]
            .as_str()
            .map(|reason| format!(" (anchor: {reason})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "| {} | {} | {} | {outcome}{anchor_note} | {candidate} | {mismatches} | {state} | {roles} |\n",
            case["case_id"].as_str().unwrap_or("?"),
            case["expected_direction"].as_str().unwrap_or("?"),
            case["row_kind"].as_str().unwrap_or("?"),
        ));
    }

    out.push_str("\n## Notes\n\n");
    for note in report["notes"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", note.as_str().unwrap_or("?")));
    }
    out
}
