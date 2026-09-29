//! Arg-parsing and dispatch for `ripr feedback record / export`.

use crate::app::feedback::{
    ExportFeedbackOptions, RecordFeedbackOptions, RecordStatus, export_feedback_join,
    record_feedback, record_result_json,
};
use crate::cli::parse::expect_value;
use crate::cli::suggest::unknown_argument;
use crate::domain::{
    ActorKind, FeedbackJudgment, FeedbackPayload, FeedbackReason, ResultIdentity, ReviewStatus,
};
use crate::output::path;
use std::path::PathBuf;

pub(in crate::cli) fn run_feedback(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h") => {
            print!("{FEEDBACK_HELP}");
            Ok(())
        }
        Some("record") => run_record(&args[1..]),
        Some("export") => run_export(&args[1..]),
        Some(other) => Err(format!(
            "unknown feedback subcommand {other:?}; expected `record` or `export`"
        )),
    }
}

fn run_record(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{FEEDBACK_RECORD_HELP}");
        return Ok(());
    }
    let (options, json) = parse_record_options(args)?;
    let recorded = record_feedback(&options)?;
    if json {
        print!("{}", record_result_json(&recorded)?);
    } else {
        let verb = match recorded.status {
            RecordStatus::Created => "Recorded",
            RecordStatus::AlreadyRecorded => "Reused existing",
        };
        println!(
            "{verb} usefulness feedback {} at {}. Diagnostics, classification, baseline, suppressions, gates, and gap closure were not changed.",
            recorded.receipt.feedback_id,
            path::display_path(&recorded.path)
        );
    }
    Ok(())
}

fn run_export(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{FEEDBACK_EXPORT_HELP}");
        return Ok(());
    }
    let (options, json, out) = parse_export_options(args)?;
    let (rendered, _) = export_feedback_join(&options)?;
    if let Some(path) = out {
        crate::atomic_file::write(&path, rendered.as_bytes(), "usefulness-feedback join")?;
        if json {
            print!("{rendered}");
        } else {
            println!(
                "Wrote usefulness-feedback join {} without changing diagnostics, classification, baseline, suppressions, gates, or gap closure.",
                crate::output::path::display_path(&path)
            );
        }
    } else {
        print!("{rendered}");
    }
    Ok(())
}

pub(in crate::cli) fn parse_record_options(
    args: &[String],
) -> Result<(RecordFeedbackOptions, bool), String> {
    let mut snapshot_id: Option<String> = None;
    let mut canonical_item: Option<String> = None;
    let mut route_digest: Option<String> = None;
    let mut attempt_id: Option<String> = None;
    let mut receipt_id: Option<String> = None;
    let mut actor_kind = ActorKind::Unknown;
    let mut review_status = ReviewStatus::Unreviewed;
    let mut review_actor_kind: Option<ActorKind> = None;
    let mut reason: Option<FeedbackReason> = None;
    let mut judgment_override: Option<FeedbackJudgment> = None;
    let mut note: Option<String> = None;
    let mut idempotency_key: Option<String> = None;
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--snapshot" => {
                i += 1;
                snapshot_id = Some(expect_value(args, i, "--snapshot")?.to_string());
            }
            "--item" => {
                i += 1;
                canonical_item = Some(expect_value(args, i, "--item")?.to_string());
            }
            "--route-digest" => {
                i += 1;
                route_digest = Some(expect_value(args, i, "--route-digest")?.to_string());
            }
            "--attempt" => {
                i += 1;
                attempt_id = Some(expect_value(args, i, "--attempt")?.to_string());
            }
            "--receipt" => {
                i += 1;
                receipt_id = Some(expect_value(args, i, "--receipt")?.to_string());
            }
            "--actor" => {
                i += 1;
                actor_kind = ActorKind::parse(expect_value(args, i, "--actor")?)?;
            }
            "--review" => {
                i += 1;
                review_status = ReviewStatus::parse(expect_value(args, i, "--review")?)?;
            }
            "--review-actor" => {
                i += 1;
                review_actor_kind =
                    Some(ActorKind::parse(expect_value(args, i, "--review-actor")?)?);
            }
            "--reason" => {
                i += 1;
                reason = Some(FeedbackReason::parse(expect_value(args, i, "--reason")?)?);
            }
            "--judgment" => {
                i += 1;
                judgment_override = Some(FeedbackJudgment::parse(expect_value(
                    args,
                    i,
                    "--judgment",
                )?)?);
            }
            "--note" => {
                i += 1;
                note = Some(expect_value(args, i, "--note")?.to_string());
            }
            "--idempotency-key" => {
                i += 1;
                idempotency_key = Some(expect_value(args, i, "--idempotency-key")?.to_string());
            }
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--json" => json = true,
            "--file" | "--line" => {
                return Err(
                    "feedback identity is --snapshot and optional --item, not file/line; a current file/line cannot attach an old judgment to a new result"
                        .to_string(),
                );
            }
            other => return Err(unknown_argument("feedback record", other)),
        }
        i += 1;
    }
    let snapshot_id = snapshot_id.ok_or_else(|| {
        "feedback record requires --snapshot; file and line are not a result identity".to_string()
    })?;
    let reason = reason.ok_or_else(|| "feedback record requires --reason".to_string())?;
    let payload = FeedbackPayload {
        identity: ResultIdentity {
            snapshot_id,
            canonical_item,
            route_digest,
            attempt_id,
            receipt_id,
        },
        actor_kind,
        review_status,
        review_actor_kind,
        reason,
        judgment_override,
        note,
    };
    Ok((
        RecordFeedbackOptions {
            root,
            payload,
            idempotency_key,
            recorded_at: None,
            live_identity: None,
        },
        json,
    ))
}

pub(in crate::cli) fn parse_export_options(
    args: &[String],
) -> Result<(ExportFeedbackOptions, bool, Option<PathBuf>), String> {
    let mut root = PathBuf::from(".");
    let mut route_quality = None;
    let mut out = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--route-quality" => {
                i += 1;
                route_quality = Some(PathBuf::from(expect_value(args, i, "--route-quality")?));
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(expect_value(args, i, "--out")?));
            }
            "--json" => json = true,
            other => return Err(unknown_argument("feedback export", other)),
        }
        i += 1;
    }
    Ok((
        ExportFeedbackOptions {
            root,
            route_quality,
            live_identity: None,
        },
        json,
        out,
    ))
}

pub(in crate::cli) const FEEDBACK_HELP: &str = r#"Record local usefulness feedback against an immutable analysis result.

Usage:
  ripr feedback record --snapshot ID --reason CODE [--item ID] [--root PATH] [--json]
  ripr feedback export [--root PATH] [--route-quality PATH] [--out PATH] [--json]

`ripr feedback record` writes one versioned receipt under
`target/ripr/feedback/`. Recording is explicit: analyze, hover, and packet
paths never record feedback implicitly. Export joins stored receipts onto
existing route-quality rows without creating a second attempt ledger.

Writes:
  target/ripr/feedback/<idempotency-key>.json
  optional --out join document (export only)

Privacy:
  Stores references and reason codes, not source bodies, diffs, logs,
  environment values, or credentials. Notes are bounded and checked against
  known secret patterns (best-effort, not a guarantee). No network
  submission. Default storage is a local ignored artifact.

Exact output:
  JSON receipts use schema_version 0.1, kind usefulness_feedback_receipt.
  Export uses kind usefulness_feedback_join. Status is created or
  already_recorded. Conflict on the same idempotency key with a different
  payload exits non-zero.

Non-effects:
  Recording or aggregating feedback changes none of diagnostic visibility,
  analyzer classification, baseline/suppression/waiver policy, gap movement
  or closure, support tier, or CI gates. Agent feedback stays agent feedback
  until a human review actor is recorded. Silence is not a vote.

Run `ripr feedback record --help` or `ripr feedback export --help` for flags.
"#;

pub(in crate::cli) const FEEDBACK_RECORD_HELP: &str = r#"Record one local usefulness-feedback receipt bound to an immutable result.

Usage: ripr feedback record --snapshot ID --reason CODE [--item ID]
                            [--route-digest DIGEST] [--attempt ID] [--receipt ID]
                            [--actor human|agent|unknown]
                            [--review unreviewed|reviewed_accepted|reviewed_rejected]
                            [--review-actor human|agent|unknown]
                            [--judgment useful|incorrect|unclear|expensive|intentional_no_action]
                            [--note TEXT] [--idempotency-key KEY]
                            [--root PATH] [--json]

Options:
  --snapshot ID         Immutable analysis snapshot or result identity. Required.
  --item ID             Optional canonical item / gap id. Omit for useful
                        limitations and intentional no-action without a gap.
  --route-digest DIGEST Optional repair-route digest or kind used by join.
  --attempt ID          Optional RepairAttempt id. Historical after a later attempt.
  --receipt ID          Optional verify/receipt identity.
  --actor KIND          Who recorded this: human, agent, or unknown. Default unknown.
  --review STATUS       unreviewed (default), reviewed_accepted, or reviewed_rejected.
  --review-actor KIND   Required when --review is not unreviewed.
  --reason CODE         Closed taxonomy (useful_actionable, useful_limitation,
                        wrong_target, wrong_discriminator, false_actionable,
                        unclear_explanation, too_slow, intentional_defensive_code,
                        other, ...). `other` requires --note and --judgment.
  --judgment CLASS      Required for --reason other. Must match the reason class
                        otherwise.
  --note TEXT           Optional bounded note (1024 bytes). Fail-closed on known
                        secret patterns. Best-effort, not a secret detector.
  --idempotency-key KEY Repeat of the same key and payload reuses one record.
                        A different payload with the same key is a conflict.
  --root PATH           Repository root. Default `.`. Writes
                        target/ripr/feedback/<key>.json.
  --json                Print the receipt JSON, including policy_effects unchanged.
  --file PATH           Rejected. Identity is --snapshot plus optional --item,
                        not a current file/line.
  --line N              Rejected. Identity is --snapshot plus optional --item,
                        not a current file/line.

Non-effects: does not suppress findings, change classification, rewrite
baselines, waive gates, close gaps, or contact a network. File/line is not
accepted as identity.
"#;

pub(in crate::cli) const FEEDBACK_EXPORT_HELP: &str = r#"Join local usefulness-feedback receipts onto existing route-quality rows.

Usage: ripr feedback export [--root PATH] [--route-quality PATH] [--out PATH] [--json]

Options:
  --root PATH             Repository root. Default `.`. Reads
                          target/ripr/feedback/*.json.
  --route-quality PATH    Existing route-quality.json. Default
                          target/ripr/reports/route-quality.json. Missing input
                          still reports unmatched receipts; it does not invent
                          objective movement.
  --out PATH              Write the join JSON. Export is explicit and
                          previewable: omitting --out prints the document.
  --json                  Print JSON even when --out is set.

The join keeps objective route-quality counts separate from subjective
usefulness. Unreviewed, stale, unmatched, and missing-feedback states are
counts, not success percentages. Agent feedback is never relabelled
human-approved. No network, process, or policy mutation.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::feedback::FEEDBACK_DIRECTORY;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ripr-feedback-cli-{label}-{}-{stamp}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).map_err(|error| format!("create: {error}"))?;
        Ok(path)
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn public_cli_records_idempotently_and_rejects_file_line_identity() -> Result<(), String> {
        let root = unique_root("cli-record")?;
        let root_s = root.to_string_lossy().into_owned();
        run_feedback(&args(&[
            "record",
            "--root",
            &root_s,
            "--snapshot",
            "snap-1",
            "--reason",
            "useful_limitation",
            "--idempotency-key",
            "cli-key",
            "--actor",
            "human",
        ]))?;
        run_feedback(&args(&[
            "record",
            "--root",
            &root_s,
            "--snapshot",
            "snap-1",
            "--reason",
            "useful_limitation",
            "--idempotency-key",
            "cli-key",
            "--actor",
            "human",
        ]))?;
        let files: Vec<_> = fs::read_dir(root.join(FEEDBACK_DIRECTORY))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        assert_eq!(files.len(), 1);

        let file_line = run_feedback(&args(&[
            "record",
            "--file",
            "src/lib.rs",
            "--line",
            "12",
            "--reason",
            "useful_actionable",
        ]))
        .err()
        .ok_or_else(|| "file/line".to_string())?;
        assert!(file_line.contains("not file/line"));

        let mismatch = run_feedback(&args(&[
            "record",
            "--root",
            &root_s,
            "--snapshot",
            "snap-2",
            "--reason",
            "wrong_target",
            "--idempotency-key",
            "cli-key",
            "--actor",
            "human",
        ]))
        .err()
        .ok_or_else(|| "conflict".to_string())?;
        assert!(mismatch.contains("idempotency conflict"));
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn public_cli_export_joins_without_network_or_policy_verbs() -> Result<(), String> {
        let root = unique_root("cli-export")?;
        let root_s = root.to_string_lossy().into_owned();
        run_feedback(&args(&[
            "record",
            "--root",
            &root_s,
            "--snapshot",
            "snap-1",
            "--item",
            "gap:alpha",
            "--reason",
            "intentional_defensive_code",
            "--actor",
            "human",
            "--idempotency-key",
            "no-action",
        ]))?;
        let out = root.join("join.json");
        let out_s = out.to_string_lossy().into_owned();
        run_feedback(&args(&[
            "export", "--root", &root_s, "--out", &out_s, "--json",
        ]))?;
        let join: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&out).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        assert_eq!(join["kind"], "usefulness_feedback_join");
        assert_eq!(join["denominators"]["receipts_total"], 1);
        assert!(join["reviewed_human_useful_rate"].is_null());
        assert!(!FEEDBACK_HELP.contains("http://"));
        assert!(!FEEDBACK_HELP.contains("https://"));
        assert!(FEEDBACK_HELP.contains("No network"));
        assert!(FEEDBACK_HELP.contains("Non-effects"));
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn public_cli_rejects_other_without_judgment_and_mismatched_class() -> Result<(), String> {
        let root = unique_root("cli-other")?;
        let root_s = root.to_string_lossy().into_owned();
        let missing_judgment = run_feedback(&args(&[
            "record",
            "--root",
            &root_s,
            "--snapshot",
            "snap-1",
            "--reason",
            "other",
            "--note",
            "custom",
        ]))
        .err()
        .ok_or_else(|| "other needs judgment".to_string())?;
        assert!(missing_judgment.contains("--judgment"));

        let mismatch = run_feedback(&args(&[
            "record",
            "--root",
            &root_s,
            "--snapshot",
            "snap-1",
            "--reason",
            "useful_limitation",
            "--judgment",
            "incorrect",
        ]))
        .err()
        .ok_or_else(|| "class mismatch".to_string())?;
        assert!(mismatch.contains("--judgment"));
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn help_states_writes_privacy_output_and_non_effects() {
        for body in [FEEDBACK_HELP, FEEDBACK_RECORD_HELP, FEEDBACK_EXPORT_HELP] {
            assert!(body.contains("ripr feedback"));
            assert!(body.contains("target/ripr/feedback"));
        }
        assert!(FEEDBACK_HELP.contains("schema_version 0.1"));
        assert!(FEEDBACK_HELP.contains("usefulness_feedback_receipt"));
        assert!(FEEDBACK_RECORD_HELP.contains("file/line"));
        assert!(FEEDBACK_EXPORT_HELP.contains("success percentages"));
        assert!(FEEDBACK_RECORD_HELP.contains("does not suppress"));
        assert!(FEEDBACK_RECORD_HELP.contains("--file PATH"));
        assert!(FEEDBACK_RECORD_HELP.contains("--line N"));
    }
}
