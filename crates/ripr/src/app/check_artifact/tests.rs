//! Tests for the check-artifact write/load/identity gate (RIPR-SPEC-0140).
//!
//! These tests never spawn processes: the diff identity flow is exercised
//! through `--diff` file sources only, and every fail-closed path is
//! asserted on the typed error string naming the mismatched field.

use super::*;
use crate::app::check_workspace_with_config;
use crate::app::{Mode, collect_context_from_artifact, explain_finding_from_artifact};
use crate::config::RiprConfig;
use crate::domain::{OracleKind, OracleStrength, RelatedTest, RelationConfidence, RelationReason};
use std::path::{Path, PathBuf};

const SAMPLE_SELECTOR: &str = "probe:crates_ripr_examples_sample_src_lib.rs:error_path:a776c683";

fn sample_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample")
}

fn sample_input() -> CheckInput {
    let root = sample_root();
    CheckInput {
        root: root.clone(),
        diff_file: Some(root.join("example.diff")),
        mode: Mode::Draft,
        ..CheckInput::default()
    }
}

fn unique_temp_dir(label: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("clock failed: {err}"))?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ripr-check-artifact-test-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    Ok(dir)
}

/// Run the real sample analysis and write its artifact to `dir/artifact.json`.
fn write_sample_artifact(
    dir: &Path,
) -> Result<(PathBuf, CheckInput, RiprConfig, Vec<Finding>), String> {
    let input = sample_input();
    let config = RiprConfig::default();
    let output = check_workspace_with_config(input.clone(), &config)?;
    if output.findings.is_empty() {
        return Err("sample diff must produce findings".to_string());
    }
    let path = dir.join("artifact.json");
    write_check_artifact(&path, &input, &config, &output.findings)?;
    Ok((path, input, config, output.findings))
}

fn require_err_containing(
    result: Result<Vec<Finding>, String>,
    needles: &[&str],
) -> Result<(), String> {
    match result {
        Ok(_) => Err(format!(
            "expected a fail-closed error containing {needles:?}, got Ok"
        )),
        Err(err) => {
            for needle in needles {
                if !err.contains(needle) {
                    return Err(format!("expected error to contain {needle:?}, got: {err}"));
                }
            }
            Ok(())
        }
    }
}

#[test]
fn artifact_round_trip_preserves_full_fidelity_findings() -> Result<(), String> {
    let dir = unique_temp_dir("round-trip")?;
    let result = (|| {
        let (path, input, config, findings) = write_sample_artifact(&dir)?;
        let loaded = load_findings_for_reuse(&path, &input, &config, None)?;
        // Full-fidelity proof: the loaded set equals the computed set
        // exactly, including uncapped related-tests lists and probe owners
        // (the fields the lossy `check --json` projection drops).
        if loaded != findings {
            return Err("loaded findings differ from the computed finding set".to_string());
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn language_metadata_serde_uses_documented_wire_casing() -> Result<(), String> {
    // Enum-level pin for the same wire vocabulary (kept out of
    // domain/language.rs: the domain layer must not reference serde_json —
    // policy/architecture.txt).
    use crate::domain::{LanguageId, LanguageStatus, OwnerKind, StaticLimitKind};
    for (json, expected) in [
        (
            serde_json::to_string(&LanguageId::TypeScript),
            "\"typescript\"",
        ),
        (
            serde_json::to_string(&LanguageStatus::Preview),
            "\"preview\"",
        ),
        (
            serde_json::to_string(&OwnerKind::ClassMethod),
            "\"class_method\"",
        ),
        (
            serde_json::to_string(&StaticLimitKind::MissingImportGraph),
            "\"missing_import_graph\"",
        ),
    ] {
        let json = json.map_err(|err| format!("serialize failed: {err}"))?;
        if json != expected {
            return Err(format!("wire casing must be {expected}, got {json}"));
        }
    }
    Ok(())
}

#[test]
fn artifact_wire_form_uses_documented_snake_case_vocabulary() -> Result<(), String> {
    // The envelope is a wire contract: the domain enums it serializes must
    // use the same lowercase/snake_case spellings OUTPUT_SCHEMA.md
    // documents for the same data in `check --json` — not serde's default
    // PascalCase variant names. Assertions are scoped to the typed JSON
    // paths: a whole-text scan could match free-form evidence text or pass
    // while the enum field itself is wrong.
    let dir = unique_temp_dir("wire-form")?;
    let result = (|| {
        let (path, _, _, _) = write_sample_artifact(&dir)?;
        let text =
            std::fs::read_to_string(&path).map_err(|err| format!("read artifact failed: {err}"))?;
        let artifact: serde_json::Value = serde_json::from_str(&text)
            .map_err(|err| format!("artifact must parse as JSON: {err}"))?;
        let findings = artifact["findings"]
            .as_array()
            .ok_or_else(|| "artifact findings must be an array".to_string())?;
        if findings.is_empty() {
            return Err("sample artifact must carry at least one finding".to_string());
        }
        let mut language_checked = false;
        for (index, finding) in findings.iter().enumerate() {
            // Closed documented vocabularies (ProbeFamily / DeltaKind /
            // LanguageId), not character shape: a drifted or arbitrary
            // lowercase value fails.
            let family = finding["probe"]["family"]
                .as_str()
                .ok_or_else(|| format!("findings[{index}].probe.family must be a string"))?;
            if ![
                "predicate",
                "return_value",
                "error_path",
                "call_deletion",
                "field_construction",
                "side_effect",
                "match_arm",
                "static_unknown",
            ]
            .contains(&family)
            {
                return Err(format!(
                    "findings[{index}].probe.family must serialize in the documented ProbeFamily vocabulary, got {family:?}"
                ));
            }
            let delta = finding["probe"]["delta"]
                .as_str()
                .ok_or_else(|| format!("findings[{index}].probe.delta must be a string"))?;
            if !["value", "control", "effect", "unknown"].contains(&delta) {
                return Err(format!(
                    "findings[{index}].probe.delta must serialize in the documented DeltaKind vocabulary, got {delta:?}"
                ));
            }
            if let Some(language) = finding["language"].as_str() {
                language_checked = true;
                if !["rust", "python", "typescript", "javascript", "perl"].contains(&language) {
                    return Err(format!(
                        "findings[{index}].language must serialize in the documented LanguageId vocabulary, got {language:?}"
                    ));
                }
            }
        }
        if !language_checked {
            return Err(
                "sample artifact must carry at least one finding with a language".to_string(),
            );
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn explain_from_artifact_is_byte_identical_to_fresh_explain() -> Result<(), String> {
    let dir = unique_temp_dir("explain-identical")?;
    let result = (|| {
        let (path, input, config, _) = write_sample_artifact(&dir)?;
        let fresh =
            crate::app::explain_finding_with_config(input.clone(), SAMPLE_SELECTOR, &config)?;
        let reused = explain_finding_from_artifact(input, SAMPLE_SELECTOR, &config, &path, None)?;
        if fresh != reused {
            return Err(format!(
                "reused explain output differs from fresh output\nfresh:\n{fresh}\nreused:\n{reused}"
            ));
        }
        if !reused.contains("Static exposure") {
            return Err("explain output is missing its exposure section".to_string());
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn context_from_artifact_is_byte_identical_to_fresh_context() -> Result<(), String> {
    let dir = unique_temp_dir("context-identical")?;
    let result = (|| {
        let (path, input, config, _) = write_sample_artifact(&dir)?;
        let fresh =
            crate::app::collect_context_with_config(input.clone(), SAMPLE_SELECTOR, 2, &config)?;
        let reused =
            collect_context_from_artifact(input, SAMPLE_SELECTOR, 2, &config, &path, None)?;
        if fresh != reused {
            return Err(format!(
                "reused context output differs from fresh output\nfresh:\n{fresh}\nreused:\n{reused}"
            ));
        }
        if !reused.contains("\"missing_discriminators\"") {
            return Err("context packet is missing discriminator guidance".to_string());
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn context_from_artifact_honors_max_related_tests_beyond_json_render_cap() -> Result<(), String> {
    let dir = unique_temp_dir("uncapped-related")?;
    let result = (|| {
        let (path, input, config, findings) = write_sample_artifact(&dir)?;
        let mut finding = findings
            .first()
            .cloned()
            .ok_or("sample diff must produce findings")?;
        // The `check --json` render caps related tests at 8; the artifact
        // stores the uncapped list, so `context --from` must honor a
        // render-time bound beyond that cap.
        finding.related_tests = (0..12)
            .map(|n| RelatedTest {
                name: format!("reuse_cap_test_{n}"),
                file: PathBuf::from("tests/sample.rs"),
                line: n + 1,
                oracle: Some("assert_eq!(actual, expected)".to_string()),
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                relation_reason: Some(RelationReason::DirectOwnerCall),
                relation_confidence: Some(RelationConfidence::High),
            })
            .collect();
        write_check_artifact(&path, &input, &config, &[finding.clone()])?;
        let packet = collect_context_from_artifact(input, &finding.id, 12, &config, &path, None)?;
        if !packet.contains("reuse_cap_test_11") {
            return Err(
                "context --from did not honor --max-related-tests beyond the JSON render cap"
                    .to_string(),
            );
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_mode_mismatch() -> Result<(), String> {
    let dir = unique_temp_dir("mode-mismatch")?;
    let result = (|| {
        let (path, mut input, config, _) = write_sample_artifact(&dir)?;
        input.mode = Mode::Ready;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["cannot be reused", "identity mismatch", "mode"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_root_mismatch() -> Result<(), String> {
    let dir = unique_temp_dir("root-mismatch")?;
    let other_root = unique_temp_dir("root-mismatch-other")?;
    let result = (|| {
        let (path, mut input, config, _) = write_sample_artifact(&dir)?;
        input.root = other_root.clone();
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["cannot be reused", "identity mismatch", "root"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other_root);
    result
}

#[test]
fn load_fails_closed_on_include_unchanged_tests_mismatch() -> Result<(), String> {
    let dir = unique_temp_dir("unchanged-mismatch")?;
    let result = (|| {
        let (path, mut input, config, _) = write_sample_artifact(&dir)?;
        input.include_unchanged_tests = !input.include_unchanged_tests;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &[
                "cannot be reused",
                "identity mismatch",
                "analysis_options.include_unchanged_tests",
            ],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_diff_bytes_change() -> Result<(), String> {
    let dir = unique_temp_dir("diff-change")?;
    let result = (|| {
        // Copy the sample diff so the test can mutate the recorded source
        // without touching the repo.
        let diff_copy = dir.join("example.diff");
        std::fs::copy(sample_root().join("example.diff"), &diff_copy)
            .map_err(|err| format!("copy diff: {err}"))?;
        let mut input = sample_input();
        input.diff_file = Some(diff_copy.clone());
        let config = RiprConfig::default();
        let output = check_workspace_with_config(input.clone(), &config)?;
        let path = dir.join("artifact.json");
        write_check_artifact(&path, &input, &config, &output.findings)?;

        let mut bytes =
            std::fs::read_to_string(&diff_copy).map_err(|err| format!("read: {err}"))?;
        bytes.push_str("\n# test-local mutation of the recorded diff\n");
        std::fs::write(&diff_copy, bytes).map_err(|err| format!("write: {err}"))?;

        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["cannot be reused", "identity mismatch", "diff_bytes_hash"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_when_recorded_diff_source_is_missing() -> Result<(), String> {
    let dir = unique_temp_dir("diff-missing")?;
    let result = (|| {
        let diff_copy = dir.join("example.diff");
        std::fs::copy(sample_root().join("example.diff"), &diff_copy)
            .map_err(|err| format!("copy diff: {err}"))?;
        let mut input = sample_input();
        input.diff_file = Some(diff_copy.clone());
        let config = RiprConfig::default();
        let output = check_workspace_with_config(input.clone(), &config)?;
        let path = dir.join("artifact.json");
        write_check_artifact(&path, &input, &config, &output.findings)?;

        std::fs::remove_file(&diff_copy).map_err(|err| format!("remove diff: {err}"))?;

        // The consumer passes no scope flags: the recorded diff source is
        // re-resolved from the recording and its absence fails closed.
        input.diff_file = None;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["cannot be reused", "recorded diff file", "no longer exists"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_config_identity_mismatch() -> Result<(), String> {
    let dir = unique_temp_dir("config-mismatch")?;
    let result = (|| {
        let (path, input, _, _) = write_sample_artifact(&dir)?;
        let changed =
            crate::config::tests_only_parse("[oracles]\nsnapshot_strength = \"strong\"\n")?;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &changed, None),
            &[
                "cannot be reused",
                "identity mismatch",
                "config_identity_hash",
            ],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_analyzer_version_mismatch() -> Result<(), String> {
    let dir = unique_temp_dir("version-mismatch")?;
    let result = (|| {
        let (path, input, config, _) = write_sample_artifact(&dir)?;
        let text = std::fs::read_to_string(&path).map_err(|err| format!("read: {err}"))?;
        let mut value: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| format!("parse: {err}"))?;
        value["analyzer_version"] = serde_json::Value::String("0.0.0-test".to_string());
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
        )
        .map_err(|err| format!("write: {err}"))?;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["cannot be reused", "identity mismatch", "analyzer_version"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_enabled_languages_mismatch() -> Result<(), String> {
    let dir = unique_temp_dir("languages-mismatch")?;
    let result = (|| {
        let (path, input, config, _) = write_sample_artifact(&dir)?;
        let text = std::fs::read_to_string(&path).map_err(|err| format!("read: {err}"))?;
        let mut value: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| format!("parse: {err}"))?;
        value["identity"]["enabled_languages"] = serde_json::json!(["python", "rust"]);
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
        )
        .map_err(|err| format!("write: {err}"))?;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["cannot be reused", "identity mismatch", "enabled_languages"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn load_fails_closed_on_wrong_schema_version_and_malformed_input() -> Result<(), String> {
    let dir = unique_temp_dir("schema-mismatch")?;
    let result = (|| {
        let (path, input, config, _) = write_sample_artifact(&dir)?;

        let missing = dir.join("missing.json");
        require_err_containing(
            load_findings_for_reuse(&missing, &input, &config, None),
            &["not found or unreadable"],
        )?;

        let malformed = dir.join("malformed.json");
        std::fs::write(&malformed, "{ not json").map_err(|err| format!("write: {err}"))?;
        require_err_containing(
            load_findings_for_reuse(&malformed, &input, &config, None),
            &["malformed", "invalid JSON"],
        )?;

        let text = std::fs::read_to_string(&path).map_err(|err| format!("read: {err}"))?;
        let stale = text.replace(CHECK_ARTIFACT_SCHEMA_VERSION, "ripr-check-artifact-v0");
        let stale_path = dir.join("stale.json");
        std::fs::write(&stale_path, stale).map_err(|err| format!("write: {err}"))?;
        require_err_containing(
            load_findings_for_reuse(&stale_path, &input, &config, None),
            &["unsupported schema_version", "ripr-check-artifact-v0"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn scope_flags_passed_alongside_from_are_assertions() -> Result<(), String> {
    let dir = unique_temp_dir("scope-assertions")?;
    let result = (|| {
        let (path, mut input, config, _) = write_sample_artifact(&dir)?;

        // A matching --diff assertion is accepted.
        let loaded = load_findings_for_reuse(&path, &input, &config, None)?;
        if loaded.is_empty() {
            return Err("matching assertion must load findings".to_string());
        }

        // A mismatched --diff assertion fails closed naming diff_source.
        let other_diff = dir.join("other.diff");
        std::fs::copy(sample_root().join("example.diff"), &other_diff)
            .map_err(|err| format!("copy diff: {err}"))?;
        input.diff_file = Some(other_diff);
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &["asserted scope does not match", "diff_source"],
        )?;

        // A --base assertion against a --diff recording fails closed.
        input.diff_file = Some(sample_root().join("example.diff"));
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, Some("origin/main")),
            &["asserted scope does not match", "diff_source.base"],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn perl_facts_packet_content_is_part_of_the_identity() -> Result<(), String> {
    let dir = unique_temp_dir("perl-facts")?;
    let result = (|| {
        let packet = dir.join("packet.json");
        std::fs::write(&packet, "{\"schema_version\":\"ripr-perl-facts-v1\"}")
            .map_err(|err| format!("write packet: {err}"))?;
        let mut input = sample_input();
        input.perl_facts_path = Some(packet.clone());
        let config = RiprConfig::default();
        let path = dir.join("artifact.json");
        // Write directly: the artifact write path does not run the analysis,
        // so a synthetic packet exercises the identity without a Perl adapter.
        write_check_artifact(&path, &input, &config, &[])?;

        let loaded = load_findings_for_reuse(&path, &input, &config, None)?;
        if !loaded.is_empty() {
            return Err("expected the empty synthetic finding set".to_string());
        }

        std::fs::write(
            &packet,
            "{\"schema_version\":\"ripr-perl-facts-v1\",\"changed\":true}",
        )
        .map_err(|err| format!("rewrite packet: {err}"))?;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &[
                "cannot be reused",
                "identity mismatch",
                "analysis_options.perl_facts_content_hash",
            ],
        )?;

        std::fs::remove_file(&packet).map_err(|err| format!("remove packet: {err}"))?;
        require_err_containing(
            load_findings_for_reuse(&path, &input, &config, None),
            &[
                "cannot be reused",
                "recorded Perl facts packet",
                "no longer exists",
            ],
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn repeated_write_replaces_artifact_atomically() -> Result<(), String> {
    let dir = unique_temp_dir("rewrite")?;
    let result = (|| {
        let (path, input, config, findings) = write_sample_artifact(&dir)?;
        write_check_artifact(&path, &input, &config, &[])?;
        let loaded = load_findings_for_reuse(&path, &input, &config, None)?;
        if !loaded.is_empty() {
            return Err("last writer must win on a repeated --write-artifact".to_string());
        }
        write_check_artifact(&path, &input, &config, &findings)?;
        let loaded = load_findings_for_reuse(&path, &input, &config, None)?;
        if loaded != findings {
            return Err("re-written artifact must round-trip the finding set".to_string());
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn concurrent_writers_never_leave_a_torn_artifact() -> Result<(), String> {
    let dir = unique_temp_dir("concurrent")?;
    let result = (|| {
        let input = sample_input();
        let config = RiprConfig::default();
        let output = check_workspace_with_config(input.clone(), &config)?;
        let path = dir.join("artifact.json");
        let mut last_error: Option<String> = None;
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for writer in 0..2usize {
                let path = path.clone();
                let input = input.clone();
                let config = config.clone();
                let findings = if writer == 0 {
                    Vec::new()
                } else {
                    output.findings.clone()
                };
                handles.push(scope.spawn(move || -> Result<(), String> {
                    for _ in 0..10 {
                        write_check_artifact(&path, &input, &config, &findings)?;
                    }
                    Ok(())
                }));
            }
            for handle in handles {
                match handle.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => last_error = Some(err),
                    Err(_) => last_error = Some("writer thread panicked".to_string()),
                }
            }
        });
        if let Some(err) = last_error {
            return Err(format!("concurrent writer failed: {err}"));
        }
        // The final artifact parses as one complete envelope from one writer.
        let text = std::fs::read_to_string(&path).map_err(|err| format!("read: {err}"))?;
        let artifact: CheckArtifactV1 =
            serde_json::from_str(&text).map_err(|err| format!("torn artifact: {err}"))?;
        if !artifact.findings.is_empty() && artifact.findings.len() != output.findings.len() {
            return Err(format!(
                "artifact holds a torn finding set of {} entries",
                artifact.findings.len()
            ));
        }
        // No temp files are left behind.
        let leftovers = std::fs::read_dir(&dir)
            .map_err(|err| format!("read_dir: {err}"))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        if leftovers != 0 {
            return Err(format!("{leftovers} temp file(s) left behind"));
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}
