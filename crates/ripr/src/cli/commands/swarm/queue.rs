use crate::cli::commands_context::ensure_command_root;
use crate::cli::commands_numeric::parse_positive_usize;
use crate::cli::parse::expect_value;
use crate::output;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Options {
    pub(super) root: PathBuf,
    pub(super) gap_ledger: PathBuf,
    pub(super) language: String,
    pub(super) top: usize,
}

pub(super) fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut root = PathBuf::from(".");
    let mut gap_ledger = PathBuf::from("target/ripr/reports/gap-decision-ledger.json");
    let mut language = "python".to_string();
    let mut top = 10usize;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                let value = expect_value(args, i, "--root")?;
                if value.trim().is_empty() {
                    return Err("swarm queue --root requires a non-empty path".to_string());
                }
                root = PathBuf::from(value);
            }
            "--gap-ledger" => {
                i += 1;
                let value = expect_value(args, i, "--gap-ledger")?;
                if value.trim().is_empty() {
                    return Err("swarm queue --gap-ledger requires a non-empty path".to_string());
                }
                gap_ledger = PathBuf::from(value);
            }
            "--language" => {
                i += 1;
                let value = expect_value(args, i, "--language")?;
                if value.trim().is_empty() {
                    return Err("swarm queue --language requires a non-empty language".to_string());
                }
                language = value.to_string();
            }
            "--top" => {
                i += 1;
                top = parse_positive_usize(expect_value(args, i, "--top")?, "swarm queue --top")?;
            }
            "--format" => {
                i += 1;
                let value = expect_value(args, i, "--format")?;
                if value != "json" {
                    return Err(format!(
                        "unknown swarm queue format {value:?}; expected `json`"
                    ));
                }
            }
            "--json" => {}
            other => return Err(format!("unknown swarm queue argument {other:?}")),
        }
        i += 1;
    }

    Ok(Options {
        root,
        gap_ledger,
        language,
        top,
    })
}

pub(super) fn run(options: Options) -> Result<(), String> {
    ensure_command_root(&options.root, "swarm queue")?;
    let contents = std::fs::read_to_string(&options.gap_ledger).map_err(|err| {
        format!(
            "swarm queue --gap-ledger {} is invalid: read failed: {err}",
            options.gap_ledger.display()
        )
    })?;
    let rendered = render_from_gap_ledger_contents(&options, &contents)?;
    print!("{rendered}");
    Ok(())
}

fn render_from_gap_ledger_contents(options: &Options, contents: &str) -> Result<String, String> {
    let source =
        output::gap_decision_ledger::parse_gap_record_source_json(contents).map_err(|err| {
            format!(
                "swarm queue --gap-ledger {} is invalid: {err}",
                options.gap_ledger.display()
            )
        })?;
    let root_display = output::outcome::display_path(&options.root);
    let gap_ledger_display = output::outcome::display_path(&options.gap_ledger);
    let rendered = match gap_ledger_root_status(&options.root, source.root.as_deref()) {
        GapLedgerRootStatus::Missing => {
            output::agent_seam_packets::render_agent_gap_record_queue_missing_root_json(
                &root_display,
                &gap_ledger_display,
                source.generated_at.as_deref(),
                &source.records,
                &options.language,
                options.top,
            )?
        }
        GapLedgerRootStatus::Mismatch { ledger_root, .. } => {
            output::agent_seam_packets::render_agent_gap_record_queue_wrong_root_json(
                &root_display,
                &gap_ledger_display,
                &ledger_root,
                source.generated_at.as_deref(),
                &source.records,
                &options.language,
                options.top,
            )?
        }
        GapLedgerRootStatus::Match => {
            output::agent_seam_packets::render_agent_gap_record_queue_json(
                &root_display,
                &gap_ledger_display,
                &source.records,
                &options.language,
                options.top,
            )?
        }
    };
    Ok(rendered)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum GapLedgerRootStatus {
    Match,
    Missing,
    Mismatch { ledger_root: String, reason: String },
}

fn gap_ledger_root_status(requested_root: &Path, ledger_root: Option<&str>) -> GapLedgerRootStatus {
    let Some(raw_ledger_root) = ledger_root.map(str::trim).filter(|root| !root.is_empty()) else {
        return GapLedgerRootStatus::Missing;
    };
    let requested_root_display = output::outcome::display_path(requested_root);
    let ledger_root_display = output::path::display_path_text(raw_ledger_root);
    if requested_root_display == ledger_root_display {
        return GapLedgerRootStatus::Match;
    }

    let requested_canonical = requested_root.canonicalize().ok();
    let ledger_root_path = Path::new(raw_ledger_root);
    let ledger_canonical = ledger_root_path.canonicalize().ok();
    if requested_canonical.is_some()
        && ledger_canonical.is_some()
        && requested_canonical == ledger_canonical
    {
        return GapLedgerRootStatus::Match;
    }

    GapLedgerRootStatus::Mismatch {
        ledger_root: ledger_root_display.clone(),
        reason: format!(
            "gap ledger root {ledger_root_display} does not match requested --root {requested_root_display}; regenerate the gap decision ledger for the selected root before assigning swarm work"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn unique_command_test_dir(name: &str) -> PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        std::env::temp_dir().join(format!("ripr-{name}-{nanos}"))
    }

    #[test]
    fn parses_gap_ledger_language_top_and_format() {
        assert_eq!(
            parse_options(&args(&[
                "--root",
                ".",
                "--gap-ledger",
                "target/ripr/reports/gap-decision-ledger.json",
                "--language",
                "python",
                "--top",
                "3",
                "--format",
                "json",
            ])),
            Ok(Options {
                root: PathBuf::from("."),
                gap_ledger: PathBuf::from("target/ripr/reports/gap-decision-ledger.json"),
                language: "python".to_string(),
                top: 3,
            })
        );
        assert_eq!(
            parse_options(&args(&["--format", "md"])),
            Err("unknown swarm queue format \"md\"; expected `json`".to_string())
        );
        assert_eq!(
            parse_options(&args(&["--top", "0"])),
            Err("invalid swarm queue --top: expected a positive integer".to_string())
        );
    }

    fn python_swarm_queue_gap_ledger(root: &Path) -> String {
        serde_json::json!({
            "root": output::outcome::display_path(root),
            "generated_at": "unix_ms:1778240100000",
            "records": [{
                "gap_id": "gap:python:pricing-boundary",
                "canonical_gap_id": "gap:python:src/pricing.py:calculate_discount:predicate_boundary",
                "kind": "MissingBoundaryAssertion",
                "language": "python",
                "language_status": "preview",
                "scope": "repo",
                "evidence_class": "predicate_boundary",
                "gap_state": "actionable",
                "policy_state": "new",
                "repairability": "repairable",
                "anchor": {
                    "file": "src/pricing.py",
                    "line": 7,
                    "owner": "calculate_discount"
                },
                "repair_route": {
                    "route_kind": "AddBoundaryAssertion",
                    "target_file": "tests/test_pricing.py",
                    "assertion_shape": "assert calculate_discount(100, 100) == 90",
                    "changed_behavior": "amount >= threshold"
                },
                "verification_commands": ["pytest tests/test_pricing.py"],
                "receipt_command": "ripr agent receipt --verify-json target/ripr/workflow/verify.json --seam-id gap:python:pricing-boundary --test-changed tests/test_pricing.py",
                "projection_eligibility": {
                    "agent_packet": {
                        "eligible": true,
                        "reason": "bounded repair route"
                    }
                }
            }]
        })
        .to_string()
    }

    #[test]
    fn render_blocks_gap_ledger_from_wrong_root() -> Result<(), String> {
        let temp = unique_command_test_dir("swarm-queue-render-wrong-root");
        let requested = temp.join("requested");
        let other = temp.join("other");
        std::fs::create_dir_all(&requested).map_err(|err| format!("create requested: {err}"))?;
        std::fs::create_dir_all(&other).map_err(|err| format!("create other: {err}"))?;
        let options = Options {
            root: requested.clone(),
            gap_ledger: requested.join("gap-ledger.json"),
            language: "python".to_string(),
            top: 10,
        };

        let json =
            render_from_gap_ledger_contents(&options, &python_swarm_queue_gap_ledger(&other))?;
        let value = serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|err| format!("queue JSON should parse: {err}"))?;
        assert_eq!(
            value.get("status").and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            value
                .get("blocker")
                .and_then(|blocker| blocker.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(|reason| reason.contains("does not match requested --root")),
            Some(true)
        );
        assert_eq!(
            value
                .get("packets")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );

        std::fs::remove_dir_all(&temp).map_err(|err| format!("remove temp: {err}"))?;
        Ok(())
    }

    #[test]
    fn render_blocks_gap_ledger_without_root_metadata() -> Result<(), String> {
        let root = unique_command_test_dir("swarm-queue-render-missing-root");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let options = Options {
            root: root.clone(),
            gap_ledger: root.join("gap-ledger.json"),
            language: "python".to_string(),
            top: 10,
        };
        let mut value =
            serde_json::from_str::<serde_json::Value>(&python_swarm_queue_gap_ledger(&root))
                .map_err(|err| format!("ledger JSON should parse: {err}"))?;
        let Some(object) = value.as_object_mut() else {
            return Err("ledger JSON should be an object".to_string());
        };
        object.remove("root");

        let json = render_from_gap_ledger_contents(&options, &value.to_string())?;
        let value = serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|err| format!("queue JSON should parse: {err}"))?;
        assert_eq!(
            value.get("status").and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            value
                .get("blocker")
                .and_then(|blocker| blocker.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(|reason| reason.contains("missing root metadata")),
            Some(true)
        );
        assert_eq!(
            value
                .get("packets")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn render_allows_matching_root() -> Result<(), String> {
        let root = unique_command_test_dir("swarm-queue-render-matching-root");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let options = Options {
            root: root.clone(),
            gap_ledger: root.join("gap-ledger.json"),
            language: "python".to_string(),
            top: 10,
        };

        let json =
            render_from_gap_ledger_contents(&options, &python_swarm_queue_gap_ledger(&root))?;
        let value = serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|err| format!("queue JSON should parse: {err}"))?;
        assert_eq!(
            value.get("status").and_then(serde_json::Value::as_str),
            Some("advisory")
        );
        assert_eq!(
            value
                .get("summary")
                .and_then(|summary| summary.get("returned"))
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            value
                .get("packets")
                .and_then(serde_json::Value::as_array)
                .and_then(|packets| packets.first())
                .and_then(|packet| packet.get("allowed_files"))
                .and_then(serde_json::Value::as_array)
                .and_then(|files| files.first())
                .and_then(serde_json::Value::as_str),
            Some("tests/test_pricing.py")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn root_status_detects_wrong_ledger_root() -> Result<(), String> {
        let temp = unique_command_test_dir("swarm-queue-root-status");
        let requested = temp.join("requested");
        let other = temp.join("other");
        std::fs::create_dir_all(&requested).map_err(|err| format!("create requested: {err}"))?;
        std::fs::create_dir_all(&other).map_err(|err| format!("create other: {err}"))?;

        assert_eq!(
            gap_ledger_root_status(&requested, Some(&requested.display().to_string())),
            GapLedgerRootStatus::Match
        );
        let mismatch = gap_ledger_root_status(&requested, Some(&other.display().to_string()));
        match mismatch {
            GapLedgerRootStatus::Mismatch {
                ledger_root,
                reason,
            } => {
                assert!(ledger_root.contains("other"));
                assert!(reason.contains("does not match requested --root"));
            }
            other_status => return Err(format!("expected mismatch, got {other_status:?}")),
        }
        assert_eq!(
            gap_ledger_root_status(&requested, None),
            GapLedgerRootStatus::Missing
        );

        std::fs::remove_dir_all(&temp).map_err(|err| format!("remove temp: {err}"))?;
        Ok(())
    }
}
