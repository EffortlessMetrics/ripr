//! Synthetic contract models only. No native commands run here, and these
//! temporary objects must never be retained as real upstream observations.

use std::path::PathBuf;

use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;

mod controls;
mod evidence_graph;
mod retained_support;

const TEST_ID: &str = "synthetic::expected_empty";
const CORRECTED: &str = "fn synthetic() {\n    assert!(boundary());\n    assert!(ascii_boundary());\n    assert!(neighbor());\n}\n";
const ORIGINAL: &str = "fn synthetic() {\n    assert!(!boundary());\n    assert!(!ascii_boundary());\n    assert!(neighbor());\n}\n";
const WEAK: &str = "fn synthetic() {\n    assert!(neighbor());\n}\n";

struct SyntheticEvidence {
    root: PathBuf,
    case: Value,
    key: Value,
    pairing: Value,
}

impl Drop for SyntheticEvidence {
    fn drop(&mut self) {
        if let Ok(()) = fs::remove_dir_all(&self.root) {}
    }
}

impl SyntheticEvidence {
    fn file(&self, name: &str, bytes: &[u8]) -> Result<Value, String> {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, bytes).map_err(|error| error.to_string())?;
        Ok(
            json!({"path": name, "bytes": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))}),
        )
    }

    fn persist(&mut self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.key).map_err(|error| error.to_string())?;
        self.case["semantic_oracle"]["answer_key"] = self.file("key.json", &bytes)?;
        self.pairing["answer_key_sha256"] =
            self.case["semantic_oracle"]["answer_key"]["sha256"].clone();
        self.persist_pairing()
    }

    fn persist_pairing(&mut self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.pairing).map_err(|error| error.to_string())?;
        self.case["semantic_oracle"]["native_pairing"] = self.file("pairing.json", &bytes)?;
        Ok(())
    }

    fn accept_review(&mut self) -> Result<(), String> {
        let review = json!({
            "disposition": "accepted", "reviewer": "synthetic reviewer",
            "rationale": "Synthetic expected-value and historical capture review; not real execution",
            "reviewed_subject": {
                "case_id": self.case["id"],
                "status": self.case["semantic_oracle"]["status"],
                "variant": self.case["semantic_oracle"]["variant"],
                "answer_key": self.case["semantic_oracle"]["answer_key"],
                "native_pairing": self.case["semantic_oracle"]["native_pairing"]
            }
        });
        self.case["semantic_oracle"]["independent_review"] = self.file(
            "review.json",
            &serde_json::to_vec(&review).map_err(|error| error.to_string())?,
        )?;
        Ok(())
    }

    fn capture(&self, index: usize) -> Result<Value, String> {
        retained_json(&self.root, &self.pairing["observations"][index]["capture"])
    }

    fn persist_capture(&mut self, index: usize, capture: &Value) -> Result<(), String> {
        let name = format!("capture-{index}.json");
        self.pairing["observations"][index]["capture"] = self.file(
            &name,
            &serde_json::to_vec(capture).map_err(|error| error.to_string())?,
        )?;
        self.persist_pairing()
    }

    fn input_inventory(&self, production: &str, test: &str) -> Result<Value, String> {
        let manifest = self.file(
            "manifest-model.toml",
            b"[package]\nname = 'synthetic'\nversion = '0.0.0'\n",
        )?;
        Ok(json!([
            {"path": self.key["production_source_path"], "kind": "file", "bytes": self.key["sources"][production]["bytes"], "sha256": self.key["sources"][production]["sha256"]},
            {"path": self.key["test_source_path"], "kind": "file", "bytes": self.key["tests"][test]["bytes"], "sha256": self.key["tests"][test]["sha256"]},
            {"path": self.key["package_manifest_path"], "kind": "file", "bytes": manifest["bytes"], "sha256": manifest["sha256"]},
            {"path": "Cargo.lock", "kind": "file", "bytes": self.pairing["lock"]["bytes"], "sha256": self.pairing["lock"]["sha256"]}
        ]))
    }

    fn refresh_capture(&mut self, index: usize) -> Result<(), String> {
        let mut row = self.pairing["observations"][index].clone();
        if let Some(value) = row.as_object_mut() {
            let _ = value.remove("capture");
        }
        let variant = text(&row, "variant")?;
        let (_, production, test, _) = PAIRS
            .iter()
            .find(|(name, _, _, _)| *name == variant)
            .ok_or("unknown synthetic native variant")?;
        let inputs = self.input_inventory(production, test)?;
        let inventory_bytes = serde_json::to_vec(&inputs).map_err(|error| error.to_string())?;
        let inventory_before = self.file(
            &format!("native-inputs-{index}-before.json"),
            &inventory_bytes,
        )?;
        let inventory_after = self.file(
            &format!("native-inputs-{index}-after.json"),
            &inventory_bytes,
        )?;
        let artifact = self.file(
            "artifact-model.bin",
            b"Synthetic executable byte model; never executed",
        )?;
        let executable = "/synthetic/retained/library-test";
        let verification = json!({"kind": "post_capture_frozen_executable_rehash", "observed_utc": "2000-01-01T00:00:00Z", "artifact": {"path": executable, "bytes": artifact["bytes"], "sha256": artifact["sha256"]}});
        let capture = json!({
            "kind": "retained_native_capture", "case_id": self.key["case_id"], "observation": row,
            "full_workspace_inputs_before": inventory_before, "full_workspace_inputs_after": inventory_after,
            "compiler_artifact": {"reason": "compiler-artifact", "package_id": "path+file:///synthetic/workspace#synthetic@0.0.0", "manifest_path": "/synthetic/workspace/Cargo.toml", "target": {"kind": ["lib"], "name": "synthetic", "src_path": "/synthetic/workspace/src/lib.rs"}, "profile": {"test": true}, "executable": "/synthetic/build/library-test"},
            "artifact": {"compiled_path": "/synthetic/build/library-test", "bytes": artifact["bytes"], "sha256": artifact["sha256"], "retained": {"path": executable, "bytes": artifact["bytes"], "sha256": artifact["sha256"]}, "custody": {"status": "external", "locator": "synthetic external task evidence; absent here"}},
            "retained_verification": self.file(&format!("verification-{index}.json"), &serde_json::to_vec(&verification).map_err(|error| error.to_string())?)?,
            "discovery": {"phase": "completed", "timed_out": false, "process_failed": false, "exit_code": 0, "argv": [executable, TEST_ID, "--exact", "--list"], "stdout": self.file(&format!("listing-{index}.stdout"), format!("{TEST_ID}: test\n\n1 test, 0 benchmarks\n").as_bytes())?, "stderr": self.file(&format!("listing-{index}.stderr"), b"")?},
            "artifact_execution": {"phase": "completed", "timed_out": false, "process_failed": false, "exit_code": row["exit_code"], "argv": [executable, TEST_ID, "--exact"], "executable_sha256": artifact["sha256"], "stdout": row["stdout"], "stderr": row["stderr"]}
        });
        self.persist_capture(index, &capture)
    }

    fn new() -> Result<Self, String> {
        let mut fixture = Self {
            root: crate::tests::temp_dir("synthetic-semantic-oracle-contract"),
            case: json!({"id": "synthetic-oracle-contract", "semantic_oracle": {"status": "valid", "variant": "corrected"}}),
            key: json!({}),
            pairing: json!({}),
        };
        fixture.key = json!({
            "case_id": "synthetic-oracle-contract", "claim": "Synthetic independent contract model",
            "production_source_path": "src/production.rs",
            "test_id": TEST_ID, "package": "synthetic", "package_version": "0.0.0", "library_target": "synthetic", "test_source_path": "src/lib.rs", "package_manifest_path": "Cargo.toml", "library_source_path": "src/lib.rs",
            "basis": [{"kind": "source_document", "url": "https://example.invalid/synthetic-contract", "artifact": fixture.file("basis.txt", b"Synthetic independent expected behavior")?}],
            "sources": {"fixed": fixture.file("fixed.rs", b"fn boundary() -> bool { true }")?, "broken": fixture.file("broken.rs", b"fn boundary() -> bool { false }")?},
            "tests": {"corrected": fixture.file("corrected.rs", CORRECTED.as_bytes())?, "original": fixture.file("original.rs", ORIGINAL.as_bytes())?, "weak": fixture.file("weak.rs", WEAK.as_bytes())?},
            "boundary_assertions": [
                {"corrected": "assert!(boundary());", "original": "assert!(!boundary());"},
                {"corrected": "assert!(ascii_boundary());", "original": "assert!(!ascii_boundary());"}
            ],
            "failure_lines": {"broken_corrected": 2, "fixed_original": 2}
        });
        let lock = fixture.file("Cargo.lock", b"# Synthetic lock model\nversion = 4\n")?;
        let artifact = fixture.file(
            "artifact-model.bin",
            b"Synthetic executable byte model; never executed",
        )?;
        let mut observations = Vec::new();
        for (variant, source, test, passes) in PAIRS {
            let verdict = if *passes { "ok" } else { "FAILED" };
            let failure = if *passes {
                String::new()
            } else {
                let expression = if *test == "original" {
                    "!boundary()"
                } else {
                    "boundary()"
                };
                format!(
                    "failures:\n---- {TEST_ID} stdout ----\nthread '{TEST_ID}' panicked at src/lib.rs:2:5:\nassertion failed: {expression}\n"
                )
            };
            let stdout = format!(
                "running 1 test\ntest {TEST_ID} ... {verdict}\n{failure}test result: {verdict}. {} passed; {} failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n",
                u64::from(*passes),
                u64::from(!*passes)
            );
            let source_sha = &fixture.key["sources"][*source]["sha256"];
            let test_sha = &fixture.key["tests"][*test]["sha256"];
            observations.push(json!({
                "variant": variant, "phase": "completed", "timed_out": false, "compile_failed": false, "process_failed": false,
                "source_sha256": source_sha, "source_sha256_after": source_sha,
                "test_sha256": test_sha, "test_sha256_after": test_sha,
                "lock_sha256": lock["sha256"], "lock_sha256_after": lock["sha256"],
                "counts": {"intended": 1, "discovered": 1, "selected": 1, "executed": 1, "passed": u64::from(*passes), "failed": u64::from(!*passes), "ignored": 0},
                "test_ids": [TEST_ID], "exit_code": if *passes { 0 } else { 101 },
                "runner": {"version": "synthetic cargo", "compiler_version": "synthetic rustc", "cwd": "/synthetic/workspace", "target_dir": "/synthetic/target", "build_dir": "/synthetic/build", "artifact_sha256": artifact["sha256"], "executable": "/synthetic/bin/cargo", "executable_sha256": "b".repeat(64), "argv": ["/synthetic/bin/cargo", "test", "--locked", "--offline", "--manifest-path", "Cargo.toml", "-p", "synthetic", "--lib", TEST_ID, "--", "--exact"]},
                "stdout": fixture.file(&format!("{variant}.stdout"), stdout.as_bytes())?,
                "stderr": fixture.file(&format!("{variant}.stderr"), b"")?
            }));
        }
        fixture.pairing = json!({"case_id": "synthetic-oracle-contract", "lock": lock, "observations": observations});
        for index in 0..PAIRS.len() {
            fixture.refresh_capture(index)?;
        }
        fixture.persist()?;
        fixture.accept_review()?;
        Ok(fixture)
    }

    fn validate(&self) -> Result<&'static str, String> {
        validate_case(&self.root, &self.case)
    }
}

#[test]
fn benchmark_semantic_oracle_legacy_absent_is_unreviewed() -> Result<(), String> {
    assert_eq!(
        validate_case(Path::new("."), &json!({"id": "legacy"}))?,
        "unreviewed"
    );
    assert_eq!(
        validate_case(
            Path::new("."),
            &json!({"semantic_oracle": {"status": "unreviewed"}})
        )?,
        "unreviewed"
    );
    for value in [
        Value::Null,
        json!({}),
        json!({"status": "unexpected"}),
        json!({"status": "valid"}),
    ] {
        let actual = validate_case(Path::new("."), &json!({"semantic_oracle": value}));
        assert!(
            actual.is_err(),
            "accepted malformed semantic metadata: {actual:?}"
        );
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_polarity_is_separate_from_discrimination() -> Result<(), String> {
    let mut fixture = SyntheticEvidence::new()?;
    assert_eq!(fixture.validate()?, "valid");
    fixture.case["semantic_oracle"]["variant"] = json!("original");
    let actual = fixture.validate();
    assert!(
        actual.is_err(),
        "accepted original test as valid: {actual:?}"
    );
    fixture.case["semantic_oracle"]["status"] = json!("invalid");
    assert!(
        fixture.validate().is_err(),
        "old verdict review must not carry forward"
    );
    fixture.accept_review()?;
    assert_eq!(fixture.validate()?, "invalid");
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_requires_independent_basis_and_exact_removal() -> Result<(), String> {
    for field in [
        "basis",
        "self_basis",
        "review",
        "polarity",
        "weak_neighbor",
        "failure_line",
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        match field {
            "basis" => fixture.key["basis"] = json!([]),
            "self_basis" => {
                fixture.key["basis"][0]["artifact"] = fixture.key["tests"]["corrected"].clone()
            }
            "review" => {
                let mut review = retained_json(
                    &fixture.root,
                    &fixture.case["semantic_oracle"]["independent_review"],
                )?;
                review["rationale"] = json!(" ");
                fixture.case["semantic_oracle"]["independent_review"] = fixture.file(
                    "review.json",
                    &serde_json::to_vec(&review).map_err(|error| error.to_string())?,
                )?;
            }
            "polarity" => {
                fixture.key["boundary_assertions"][0]["original"] = json!("assert!(boundary());")
            }
            "weak_neighbor" => {
                fixture.key["tests"]["weak"] = fixture.file("weak.rs", b"fn synthetic() {}\n")?
            }
            _ => fixture.key["failure_lines"]["broken_corrected"] = json!(4),
        }
        fixture.persist()?;
        assert!(fixture.validate().is_err(), "accepted {field}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_rejects_missing_repeated_and_wrong_pairing() -> Result<(), String> {
    for field in ["missing", "duplicate", "wrong_case", "wrong_answer_key"] {
        let mut fixture = SyntheticEvidence::new()?;
        match field {
            "missing" => {
                if let Some(rows) = fixture.pairing["observations"].as_array_mut() {
                    let _ = rows.pop();
                }
            }
            "duplicate" => {
                fixture.pairing["observations"][5] = fixture.pairing["observations"][0].clone()
            }
            "wrong_case" => fixture.pairing["case_id"] = json!("neighbor"),
            _ => {}
        }
        fixture.persist()?;
        if field == "wrong_answer_key" {
            fixture.pairing["answer_key_sha256"] = json!("f".repeat(64));
            fixture.persist_pairing()?;
        }
        assert!(fixture.validate().is_err(), "accepted {field}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_rejects_zero_subjects_instrument_and_stale_inputs()
-> Result<(), String> {
    for (field, value) in [
        ("phase", json!("NOT_RUN")),
        ("timed_out", json!(true)),
        ("compile_failed", json!(true)),
        ("process_failed", json!(true)),
        ("exit_code", json!(1)),
        ("test_ids", json!(["neighbor"])),
        ("source_sha256_after", json!("stale")),
        ("test_sha256", json!("stale")),
        ("lock_sha256_after", json!("stale")),
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        fixture.pairing["observations"][1][field] = value;
        fixture.persist()?;
        assert!(fixture.validate().is_err(), "accepted {field}");
    }
    for field in ["intended", "discovered", "selected", "executed"] {
        let mut fixture = SyntheticEvidence::new()?;
        fixture.pairing["observations"][0]["counts"][field] = json!(0);
        fixture.persist()?;
        assert!(fixture.validate().is_err(), "accepted zero {field}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_rejects_wrong_failure_and_incomplete_output() -> Result<(), String> {
    for output in [
        "setup failed",
        "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n",
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        fixture.pairing["observations"][1]["stdout"] =
            fixture.file("bad.stdout", output.as_bytes())?;
        fixture.persist()?;
        let actual = fixture.validate();
        assert!(
            actual.is_err(),
            "accepted incomplete native output: {actual:?}"
        );
    }
    for (from, to) in [
        (
            "assertion failed: boundary()",
            "assertion failed: neighbor()",
        ),
        ("src/lib.rs:2:", "src/lib.rs:4:"),
        ("finished in 0.01s", "unfinished"),
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        let original = retained_text(&fixture.root, &fixture.pairing["observations"][1]["stdout"])?;
        fixture.pairing["observations"][1]["stdout"] =
            fixture.file("bad.stdout", original.replace(from, to).as_bytes())?;
        fixture.persist()?;
        assert!(fixture.validate().is_err(), "accepted {from}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_rejects_artifact_drift_and_escaping_paths() -> Result<(), String> {
    let fixture = SyntheticEvidence::new()?;
    fs::write(fixture.root.join("basis.txt"), "changed basis")
        .map_err(|error| error.to_string())?;
    let actual = fixture.validate();
    assert!(actual.is_err(), "accepted changed basis bytes: {actual:?}");
    for kind in ["parent", "absolute", "parent_component", "backslash"] {
        let mut fixture = SyntheticEvidence::new()?;
        assert_eq!(fixture.validate()?, "valid");
        let original = fixture.case["semantic_oracle"]["answer_key"].clone();
        let bytes = fs::read(fixture.root.join("key.json")).map_err(|error| error.to_string())?;
        let path = match kind {
            "parent" => {
                fs::create_dir(fixture.root.join("nested")).map_err(|error| error.to_string())?;
                "nested/../../".to_string()
                    + fixture
                        .root
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| "invalid fixture name".to_string())?
                    + "/key.json"
            }
            "absolute" => fixture.root.join("key.json").to_string_lossy().to_string(),
            "parent_component" => {
                fs::create_dir(fixture.root.join("nested")).map_err(|error| error.to_string())?;
                "nested/../key.json".to_string()
            }
            _ => {
                let name = "nested\\outside";
                fs::create_dir(fixture.root.join("nested")).map_err(|error| error.to_string())?;
                fs::write(fixture.root.join(name), &bytes).map_err(|error| error.to_string())?;
                name.to_string()
            }
        };
        fixture.case["semantic_oracle"]["answer_key"]["path"] = json!(path);
        fixture.accept_review()?;
        assert_eq!(
            fixture.case["semantic_oracle"]["answer_key"]["sha256"],
            original["sha256"]
        );
        assert!(fixture.validate().is_err(), "accepted {kind}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_command_discloses_absent_corpus() -> Result<(), String> {
    crate::tests::with_temp_cwd("semantic-oracle-command-disclosure", |root| {
        crate::fixture_contracts::check_fixture_contracts()?;
        let report = fs::read_to_string(root.join("target/ripr/reports/fixture-contracts.md"))
            .map_err(|error| error.to_string())?;
        assert!(report.contains("Benchmark semantic-oracle scope"));
        assert!(report.contains("NOT_ESTABLISHED"));
        assert!(!report.contains("valid=0"));
        Ok(())
    })
}

#[test]
fn benchmark_semantic_oracle_unavailable_corpus_has_no_zero_count() -> Result<(), String> {
    let fixture = SyntheticEvidence::new()?;
    let mut violations = Vec::new();
    let disclosure =
        super::super::general_validators::validate_evidence_quality_benchmark_fixture_corpus_at(
            &fixture.root.join("missing-corpus.json"),
            &mut violations,
        )?;
    assert!(!violations.is_empty());
    assert!(disclosure.intro.contains("NOT_ESTABLISHED"));
    assert!(disclosure.items.is_empty());
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_production_reader_discloses_rejected_and_unreviewed()
-> Result<(), String> {
    let mut fixture = SyntheticEvidence::new()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut corpus = read_json(&repo.join("fixtures/evidence-quality-benchmark/corpus.json"))?;
    // This temporary reader scenario isolates legacy cases. Real historical
    // controls and their assets have separate production-route coverage.
    if let Some(object) = corpus.as_object_mut() {
        let _ = object.remove("semantic_oracle_controls");
    }
    let rows = corpus["cases"]
        .as_array_mut()
        .ok_or_else(|| "missing cases".to_string())?;
    let total = rows.len();
    let first = rows.first_mut().ok_or_else(|| "no cases".to_string())?;
    first["id"] = fixture.case["id"].clone();
    first["semantic_oracle"] = fixture.case["semantic_oracle"].clone();
    let path = fixture.root.join("corpus.json");
    for valid in [true, false] {
        if !valid {
            fixture.pairing["observations"][1]["counts"]["executed"] = json!(0);
            fixture.persist()?;
            corpus["cases"][0]["semantic_oracle"] = fixture.case["semantic_oracle"].clone();
        }
        fs::write(
            &path,
            serde_json::to_vec(&corpus).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let mut violations = Vec::new();
        let disclosure = super::super::general_validators::validate_evidence_quality_benchmark_fixture_corpus_at(&path, &mut violations)?;
        assert_eq!(violations.is_empty(), valid);
        let body = crate::policy_report_body(
            &crate::PolicyReportSpec {
                report_file: "fixture-contracts.md",
                check: "check-fixture-contracts",
                why_it_matters: "Synthetic fixture acceptance contract",
                fix_kind: crate::FixKind::AuthorDecisionRequired,
                recommended_fixes: &[],
                rerun_command: "cargo xtask check-fixture-contracts",
                exception_template: None,
            },
            &violations,
            &[disclosure],
        );
        assert!(body.contains("Benchmark semantic-oracle scope"));
        assert!(body.contains(&format!(
            "unreviewed={} (legacy absent={})",
            total - 1,
            total - 1
        )));
        assert!(body.contains(if valid {
            "valid=1, invalid=0"
        } else {
            "valid=0, invalid=0"
        }));
        assert!(body.contains(if valid { "rejected=0" } else { "rejected=1" }));
        assert!(body.contains("do not change static discrimination"));
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_rejects_stale_reviewed_subject() -> Result<(), String> {
    for field in ["basis", "claim", "source", "test", "capture", "verdict"] {
        let mut fixture = SyntheticEvidence::new()?;
        assert_eq!(fixture.validate()?, "valid");
        let review = fixture.case["semantic_oracle"]["independent_review"].clone();
        match field {
            "basis" => {
                fixture.key["basis"][0]["artifact"] =
                    fixture.file("basis.txt", b"Changed synthetic independent basis")?
            }
            "claim" => fixture.key["claim"] = json!("Changed synthetic expected-behavior claim"),
            "source" => {
                fixture.key["sources"]["fixed"] = fixture.file(
                    "fixed.rs",
                    b"fn boundary() -> bool { true }\n// reviewed source changed\n",
                )?;
                for index in [0, 2, 4] {
                    for name in ["source_sha256", "source_sha256_after"] {
                        fixture.pairing["observations"][index][name] =
                            fixture.key["sources"]["fixed"]["sha256"].clone();
                    }
                    fixture.refresh_capture(index)?;
                }
            }
            "test" => {
                for (name, original) in [
                    ("corrected", CORRECTED),
                    ("original", ORIGINAL),
                    ("weak", WEAK),
                ] {
                    fixture.key["tests"][name] = fixture.file(
                        &format!("{name}.rs"),
                        format!("{original}// retained neighbor comment changed\n").as_bytes(),
                    )?;
                }
                for (index, (_, _, test, _)) in PAIRS.iter().enumerate() {
                    for name in ["test_sha256", "test_sha256_after"] {
                        fixture.pairing["observations"][index][name] =
                            fixture.key["tests"][*test]["sha256"].clone();
                    }
                    fixture.refresh_capture(index)?;
                }
            }
            "capture" => {
                fixture.pairing["observations"][0]["runner"]["version"] =
                    json!("changed synthetic cargo capture");
                fixture.refresh_capture(0)?;
            }
            _ => {
                fixture.case["semantic_oracle"]["status"] = json!("invalid");
                fixture.case["semantic_oracle"]["variant"] = json!("original");
            }
        }
        fixture.persist()?;
        assert_eq!(
            fixture.case["semantic_oracle"]["independent_review"],
            review
        );
        let error = fixture
            .validate()
            .err()
            .ok_or_else(|| format!("accepted stale {field} review"))?;
        assert!(error.contains("review has a stale"), "{field}: {error}");
        fixture.accept_review()?;
        assert!(
            fixture.validate().is_ok(),
            "the changed {field} model must otherwise be well-formed"
        );
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_rejects_unbound_or_swapped_native_capture() -> Result<(), String> {
    for field in [
        "missing",
        "swapped",
        "duplicate",
        "bare_hash",
        "case",
        "package",
        "target",
        "executable",
        "artifact_hash",
        "source",
        "test",
        "lock",
        "test_id",
        "discovery",
        "replay",
        "timeout",
        "missing_verification",
        "stale_verification",
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        assert_eq!(fixture.validate()?, "valid");
        let mut capture = fixture.capture(0)?;
        match field {
            "case" => capture["case_id"] = json!("another-case"),
            "package" => {
                capture["compiler_artifact"]["package_id"] =
                    json!("path+file:///synthetic/workspace#neighbor@0.0.0")
            }
            "target" => capture["compiler_artifact"]["target"]["name"] = json!("neighbor"),
            "executable" => {
                capture["compiler_artifact"]["executable"] = json!("/synthetic/build/neighbor")
            }
            "artifact_hash" => capture["artifact"]["sha256"] = json!("f".repeat(64)),
            "source" => capture["observation"]["source_sha256"] = json!("stale"),
            "test" => capture["observation"]["test_sha256"] = json!("stale"),
            "lock" => capture["observation"]["lock_sha256_after"] = json!("stale"),
            "test_id" => capture["observation"]["test_ids"] = json!(["neighbor"]),
            "discovery" => {
                capture["discovery"]["stdout"] =
                    fixture.file("bad-list.stdout", b"0 tests, 0 benchmarks\n")?
            }
            "replay" => capture["artifact_execution"]["argv"][1] = json!("neighbor"),
            "timeout" => capture["artifact_execution"]["timed_out"] = json!(true),
            "missing_verification" => capture["retained_verification"] = Value::Null,
            "stale_verification" => {
                let mut verification =
                    retained_json(&fixture.root, &capture["retained_verification"])?;
                verification["artifact"]["sha256"] = json!("f".repeat(64));
                capture["retained_verification"] = fixture.file(
                    "stale-verification.json",
                    &serde_json::to_vec(&verification).map_err(|error| error.to_string())?,
                )?;
            }
            _ => {}
        }
        fixture.persist_capture(0, &capture)?;
        match field {
            "missing" => fs::remove_file(fixture.root.join("capture-0.json"))
                .map_err(|error| error.to_string())?,
            "swapped" => {
                let first = fixture.pairing["observations"][0]["capture"].clone();
                fixture.pairing["observations"][0]["capture"] =
                    fixture.pairing["observations"][1]["capture"].clone();
                fixture.pairing["observations"][1]["capture"] = first;
            }
            "duplicate" => {
                fixture.pairing["observations"][1]["capture"] =
                    fixture.pairing["observations"][0]["capture"].clone()
            }
            "bare_hash" => fixture.pairing["observations"][0]["capture"] = Value::Null,
            _ => {}
        }
        fixture.persist_pairing()?;
        fixture.accept_review()?;
        assert!(
            fixture.validate().is_err(),
            "accepted malformed {field} capture even with a refreshed synthetic review"
        );
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_local_custody_never_falls_back() -> Result<(), String> {
    for mode in [
        "valid",
        "missing",
        "corrupt",
        "wrong_artifact",
        "external_fallback",
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        assert_eq!(fixture.validate()?, "valid");
        let mut capture = fixture.capture(0)?;
        let file = fixture.file(
            "local-artifact.bin",
            b"Synthetic executable byte model; never executed",
        )?;
        capture["artifact"]["custody"] = json!({"status": "local", "file": file});
        match mode {
            "missing" => fs::remove_file(fixture.root.join("local-artifact.bin"))
                .map_err(|error| error.to_string())?,
            "corrupt" => fs::write(fixture.root.join("local-artifact.bin"), b"corrupt")
                .map_err(|error| error.to_string())?,
            "wrong_artifact" => {
                capture["artifact"]["custody"]["file"] =
                    fixture.file("wrong-artifact.bin", b"Different synthetic executable")?
            }
            "external_fallback" => {
                capture["artifact"]["custody"]["status"] = json!("external");
                capture["artifact"]["custody"]["locator"] = json!("claimed fallback");
            }
            _ => {}
        }
        fixture.persist_capture(0, &capture)?;
        fixture.accept_review()?;
        assert_eq!(fixture.validate().is_ok(), mode == "valid", "{mode}");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn benchmark_semantic_oracle_local_custody_enforces_resolved_containment() -> Result<(), String> {
    use std::os::unix::fs::symlink;

    for mode in [
        "inside_file",
        "inside_parent",
        "outside_file",
        "outside_parent",
    ] {
        let mut fixture = SyntheticEvidence::new()?;
        let outside = SyntheticEvidence::new()?;
        let bytes = b"Synthetic executable byte model; never executed";
        let mut capture = fixture.capture(0)?;
        capture["artifact"]["custody"] =
            json!({"status": "local", "file": fixture.file("linked/artifact.bin", bytes)?});
        fixture.persist_capture(0, &capture)?;
        fixture.accept_review()?;
        let (status, custody) = validate_declaration(&fixture.root, &fixture.case)?;
        assert_eq!(status, "valid");
        assert_eq!((custody.local, custody.external), (1, 5));

        let is_inside = mode.starts_with("inside");
        let target_root = if is_inside {
            fixture.root.join("retained")
        } else {
            outside.root.join("retained")
        };
        fs::create_dir(&target_root).map_err(|error| error.to_string())?;
        fs::write(target_root.join("artifact.bin"), bytes).map_err(|error| error.to_string())?;
        let linked = fixture.root.join("linked");
        if mode.ends_with("file") {
            fs::remove_file(linked.join("artifact.bin")).map_err(|error| error.to_string())?;
            symlink(
                target_root.join("artifact.bin"),
                linked.join("artifact.bin"),
            )
            .map_err(|error| error.to_string())?;
        } else {
            fs::remove_dir_all(&linked).map_err(|error| error.to_string())?;
            symlink(&target_root, &linked).map_err(|error| error.to_string())?;
        }

        let mut summary = Summary::default();
        let mut violations = Vec::new();
        summary.observe(&fixture.root, &fixture.case, &mut violations);
        assert_eq!(violations.is_empty(), is_inside, "{mode}: {violations:?}");
        assert_eq!(summary.valid, usize::from(is_inside));
        assert_eq!(summary.rejected, usize::from(!is_inside));
        assert_eq!(summary.local_artifacts, usize::from(is_inside));
        if !is_inside {
            assert_eq!(summary.external_artifacts, 0);
            assert!(
                violations
                    .iter()
                    .any(|error| error.contains("escapes fixture root"))
            );
        }
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_production_command_discloses_custody_and_rejection()
-> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    crate::tests::with_temp_cwd("semantic-oracle-production-custody", |root| {
        crate::tests::copy_dir_recursive(&repo.join("fixtures"), &root.join("fixtures"))?;
        let corpus_root = root.join("fixtures/evidence-quality-benchmark");
        let original = read_json(&corpus_root.join("corpus.json"))?;
        for mode in ["external", "local", "rejected"] {
            let mut fixture = SyntheticEvidence::new()?;
            if mode == "local" {
                for index in 0..PAIRS.len() {
                    let mut capture = fixture.capture(index)?;
                    capture["artifact"]["custody"] = json!({"status": "local", "file": fixture.file("artifact-model.bin", b"Synthetic executable byte model; never executed")?});
                    fixture.persist_capture(index, &capture)?;
                }
                fixture.accept_review()?;
            } else if mode == "rejected" {
                fixture.pairing["observations"][0]["counts"]["executed"] = json!(0);
                fixture.persist()?;
            }
            crate::tests::copy_dir_recursive(&fixture.root, &corpus_root)?;
            let mut corpus = original.clone();
            corpus["cases"][0]["id"] = fixture.case["id"].clone();
            corpus["cases"][0]["semantic_oracle"] = fixture.case["semantic_oracle"].clone();
            fs::write(
                corpus_root.join("corpus.json"),
                serde_json::to_vec(&corpus).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let result = crate::fixture_contracts::check_fixture_contracts();
            assert_eq!(result.is_ok(), mode != "rejected", "{mode}: {result:?}");
            let report = fs::read_to_string(root.join("target/ripr/reports/fixture-contracts.md"))
                .map_err(|error| error.to_string())?;
            assert!(
                report.contains("Retained capture and review identities checked")
                    || mode == "rejected"
            );
            assert!(report.contains(match mode {
                "external" => "6 externally retained artifact references NOT_REVERIFIED",
                "local" =>
                    "6 local artifact byte checks; 0 externally retained artifact references",
                _ => "valid=0, invalid=0",
            }));
            assert!(report.contains(if mode == "rejected" {
                "rejected=1"
            } else {
                "rejected=0"
            }));
        }
        Ok(())
    })
}
