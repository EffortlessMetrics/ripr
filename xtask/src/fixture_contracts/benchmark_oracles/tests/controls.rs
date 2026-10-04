//! Synthetic control-collection models; no execution receipts are manufactured for the corpus.

use super::*;

struct StaticModel {
    evidence: SyntheticEvidence,
    capture: Value,
    controls: Vec<Value>,
}

impl StaticModel {
    fn new() -> Result<Self, String> {
        let mut evidence = SyntheticEvidence::new()?;
        let executable =
            json!({"path": "/synthetic/bin/ripr", "bytes": 7, "sha256": "c".repeat(64)});
        let compiler =
            json!({"executable": executable["path"], "target": {"name": "ripr", "kind": ["bin"]}});
        let producer = json!({"head": "a".repeat(40), "tree": "d".repeat(40), "artifacts": {"ripr": {"head": "a".repeat(40), "tree": "d".repeat(40), "compiler_artifact": compiler, "files": [{"path": executable["path"], "sha256": executable["sha256"], "metadata": {"size": executable["bytes"]}}]}}});
        let producer_file = evidence.file(
            "static-producer.json",
            &serde_json::to_vec(&producer).map_err(|error| error.to_string())?,
        )?;
        let identity = json!({"executable": executable, "producer_receipt": producer_file});
        let diff = evidence.file(
            "historical.patch",
            b"Synthetic retained static diff model\n",
        )?;
        let mut runs = Vec::new();
        let mut observations = Vec::new();
        for (variant, native_index) in [("corrected", 0), ("original", 4), ("weak", 2)] {
            let inputs = evidence.input_inventory("fixed", variant)?;
            let inventory = evidence.file(
                &format!("static-{variant}-inputs.json"),
                &serde_json::to_vec(&inputs).map_err(|error| error.to_string())?,
            )?;
            let mut native_capture = evidence.capture(native_index)?;
            native_capture["full_workspace_inputs_before"] = inventory.clone();
            native_capture["full_workspace_inputs_after"] = inventory.clone();
            evidence.persist_capture(native_index, &native_capture)?;
            let report = json!({"root": "/synthetic/workspace", "summary": {"probes": 1}, "analysis_outcome": {"analysis_complete": true}, "findings": [{"classification": "exposed"}]});
            let stdout = evidence.file(
                &format!("static-{variant}.json"),
                &serde_json::to_vec(&report).map_err(|error| error.to_string())?,
            )?;
            let stderr = evidence.file(&format!("static-{variant}.stderr"), b"")?;
            runs.push(json!({"variant": variant, "root": "/synthetic/workspace", "argv": [executable["path"], "check", "--root", "/synthetic/workspace", "--diff", diff["path"], "--format", "json"], "timed_out": false, "resource_stop": null, "parsed_json": true, "exit_code": 0, "identity_before": identity, "identity_after": identity, "patch_before": diff, "patch_after": diff, "stdout": stdout, "stderr": stderr, "inputs_before": inventory, "inputs_after": inventory}));
            observations.push(json!({"variant": variant, "native_exit": 0, "json": stdout, "stderr": stderr, "inputs_before": inventory, "inputs_after": inventory}));
        }
        let execution = json!({"producer_head": producer["head"], "compiler_artifact": compiler, "identity_before": identity, "identity_after": identity, "status": "THREE_COMMANDS_TERMINAL", "observations": runs});
        let capture = json!({"kind": "historical_exact_ripr_observation", "status": "THREE_COMMANDS_COMPLETED", "case_id": evidence.case["id"], "normative_static_expectation": false, "test_source_path": "src/lib.rs", "production_source_path": "src/production.rs", "transforms": "Synthetic model only; no commands executed", "producer_head": producer["head"], "producer_tree": producer["tree"], "producer_receipt": producer_file, "execution_receipt": evidence.file("static-execution.json", &serde_json::to_vec(&execution).map_err(|error| error.to_string())?)?, "executable": executable, "diff": diff, "observations": observations});
        evidence.persist()?;
        evidence.accept_review()?;
        let mut corrected = evidence.case.clone();
        corrected["fixture_reference"] = json!("synthetic contract model");
        evidence.case["semantic_oracle"]["status"] = json!("invalid");
        evidence.case["semantic_oracle"]["variant"] = json!("original");
        // Keep the two independently reviewed verdict records as distinct bytes.
        corrected["semantic_oracle"]["independent_review"] = evidence.file(
            "corrected-review.json",
            &fs::read(evidence.root.join("review.json")).map_err(|error| error.to_string())?,
        )?;
        evidence.accept_review()?;
        let mut original = evidence.case.clone();
        original["fixture_reference"] = json!("synthetic contract model");
        let mut model = Self {
            evidence,
            capture,
            controls: vec![corrected, original],
        };
        model.persist_static()?;
        Ok(model)
    }

    fn persist_static(&mut self) -> Result<(), String> {
        let descriptor = self.evidence.file(
            "static-capture.json",
            &serde_json::to_vec(&self.capture).map_err(|error| error.to_string())?,
        )?;
        for control in &mut self.controls {
            control["observed_static"] = descriptor.clone();
        }
        Ok(())
    }

    fn check(&self) -> (crate::PolicyDisclosure, Vec<String>) {
        let mut violations = Vec::new();
        let disclosure = append_controls_disclosure(
            &self.evidence.root,
            &json!({"cases": [], "semantic_oracle_controls": self.controls}),
            Summary::default().disclosure(),
            &mut violations,
        );
        (disclosure, violations)
    }
}

#[test]
fn benchmark_semantic_controls_require_explicit_complete_views() -> Result<(), String> {
    let model = StaticModel::new()?;
    assert!(model.check().1.is_empty());
    for field in [
        "id",
        "fixture_reference",
        "semantic_oracle",
        "observed_static",
    ] {
        let mut model = StaticModel::new()?;
        model.controls[0][field] = Value::Null;
        assert!(!model.check().1.is_empty(), "accepted missing {field}");
    }
    for controls in [
        Value::Null,
        json!({}),
        json!([null]),
        json!([{"id": "legacy"}]),
    ] {
        let mut violations = Vec::new();
        let _ = append_controls_disclosure(
            Path::new("."),
            &json!({"semantic_oracle_controls": controls}),
            Summary::default().disclosure(),
            &mut violations,
        );
        assert!(!violations.is_empty());
    }
    let before = Summary::default().disclosure();
    let mut violations = Vec::new();
    let after = append_controls_disclosure(
        Path::new("."),
        &json!({}),
        Summary::default().disclosure(),
        &mut violations,
    );
    assert!(violations.is_empty());
    assert_eq!(before.items, after.items);
    Ok(())
}

#[test]
fn benchmark_semantic_controls_reject_conflicting_views() -> Result<(), String> {
    for mode in [
        "duplicate",
        "wrong_variant",
        "unreviewed",
        "same_case_subject",
        "stale_review",
    ] {
        let mut model = StaticModel::new()?;
        assert!(model.check().1.is_empty());
        match mode {
            "duplicate" => model.controls.push(model.controls[0].clone()),
            "wrong_variant" => model.controls[0]["semantic_oracle"]["variant"] = json!("weak"),
            "unreviewed" => model.controls[0]["semantic_oracle"]["status"] = json!("unreviewed"),
            "same_case_subject" => {
                model.controls[1]["observed_static"]["path"] = json!("other-static.json")
            }
            _ => {
                model.controls[0]["semantic_oracle"]["independent_review"] =
                    model.controls[1]["semantic_oracle"]["independent_review"].clone()
            }
        }
        assert!(!model.check().1.is_empty(), "accepted {mode}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_controls_reject_stale_static_capture() -> Result<(), String> {
    for mode in [
        "variant",
        "input",
        "producer",
        "report_corrupt",
        "report_missing",
        "normative",
    ] {
        let mut model = StaticModel::new()?;
        assert!(model.check().1.is_empty());
        match mode {
            "variant" => model.capture["observations"][0]["variant"] = json!("wrong"),
            "input" => {
                model.capture["observations"][0]["inputs_before"] =
                    model.capture["observations"][1]["inputs_before"].clone()
            }
            "producer" => model.capture["producer_head"] = json!("f".repeat(40)),
            "report_corrupt" => fs::write(
                model.evidence.root.join("static-corrected.json"),
                b"changed report",
            )
            .map_err(|error| error.to_string())?,
            "report_missing" => fs::remove_file(model.evidence.root.join("static-corrected.json"))
                .map_err(|error| error.to_string())?,
            _ => model.capture["normative_static_expectation"] = json!(true),
        }
        model.persist_static()?;
        assert!(!model.check().1.is_empty(), "accepted {mode}");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_controls_reject_wrong_selected_producer() -> Result<(), String> {
    for mode in [
        "artifact_head",
        "artifact_tree",
        "target_name",
        "target_kind",
    ] {
        let mut model = StaticModel::new()?;
        assert!(model.check().1.is_empty());
        let mut producer = retained_json(&model.evidence.root, &model.capture["producer_receipt"])?;
        let mut execution =
            retained_json(&model.evidence.root, &model.capture["execution_receipt"])?;
        match mode {
            "artifact_head" => producer["artifacts"]["ripr"]["head"] = json!("f".repeat(40)),
            "artifact_tree" => producer["artifacts"]["ripr"]["tree"] = json!("f".repeat(40)),
            "target_name" => {
                producer["artifacts"]["ripr"]["compiler_artifact"]["target"]["name"] =
                    json!("xtask")
            }
            _ => {
                producer["artifacts"]["ripr"]["compiler_artifact"]["target"]["kind"] =
                    json!(["lib"])
            }
        }
        // Refresh every enclosing descriptor and repeated identity. Only the
        // selected artifact's own source or target identity remains wrong.
        let descriptor = model.evidence.file(
            "static-producer.json",
            &serde_json::to_vec(&producer).map_err(|error| error.to_string())?,
        )?;
        model.capture["producer_receipt"] = descriptor.clone();
        execution["compiler_artifact"] = producer["artifacts"]["ripr"]["compiler_artifact"].clone();
        for field in ["identity_before", "identity_after"] {
            execution[field]["producer_receipt"] = descriptor.clone();
            for run in execution["observations"]
                .as_array_mut()
                .ok_or("missing observations")?
            {
                run[field]["producer_receipt"] = descriptor.clone();
            }
        }
        model.capture["execution_receipt"] = model.evidence.file(
            "static-execution.json",
            &serde_json::to_vec(&execution).map_err(|error| error.to_string())?,
        )?;
        model.persist_static()?;
        let violations = model.check().1;
        assert_eq!(
            violations.len(),
            3,
            "unexpected rejection count for {mode}: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .filter(|message| message.ends_with("historical static producer identity differs"))
                .count()
                == 2,
            "wrong refusal for {mode}: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|message| message.contains("incomplete reviewed role pair"))
        );
    }
    Ok(())
}

#[test]
fn benchmark_semantic_controls_require_both_reviewed_roles() -> Result<(), String> {
    for mode in [
        "original_pair",
        "omit_corrected",
        "omit_original",
        "duplicate_corrected",
    ] {
        let mut model = StaticModel::new()?;
        assert!(model.check().1.is_empty());
        match mode {
            "omit_corrected" => {
                let _ = model.controls.remove(0);
            }
            "omit_original" => {
                let _ = model.controls.remove(1);
            }
            "duplicate_corrected" => model.controls[1] = model.controls[0].clone(),
            _ => {}
        }
        let (disclosure, violations) = model.check();
        let report = disclosure.items.join("\n");
        if mode == "original_pair" {
            assert!(violations.is_empty());
            assert!(report.contains("historical_cases=1, valid=1, invalid=1, rejected=0"));
            assert!(report.contains("complete_cases=1, incomplete_cases=0"));
        } else {
            assert!(
                violations
                    .iter()
                    .any(|message| message.contains("incomplete reviewed role pair")),
                "{mode}: {violations:?}"
            );
            assert!(report.contains("historical_cases=0"));
            assert!(report.contains("complete_cases=0, incomplete_cases=1"));
            assert!(report.contains(if mode == "omit_corrected" {
                "valid=0, invalid=1"
            } else {
                "valid=1, invalid=0"
            }));
            if mode == "duplicate_corrected" {
                assert!(
                    violations.iter().any(
                        |message| message.contains("duplicate historical case/variant control")
                    )
                );
            }
        }
    }
    Ok(())
}

#[test]
fn benchmark_semantic_controls_reject_individually_valid_mismatched_subjects() -> Result<(), String>
{
    for mode in ["key_source", "native_pairing"] {
        let mut model = StaticModel::new()?;
        assert!(model.check().1.is_empty());
        let mut alternate = model.controls[1].clone();
        let mut key = model.evidence.key.clone();
        let mut pairing = model.evidence.pairing.clone();
        if mode == "key_source" {
            // A separately retained source identity can have identical bytes.
            // Re-review this alternate subject instead of leaving stale hashes.
            let source = retained_text(&model.evidence.root, &key["sources"]["fixed"])?;
            key["sources"]["fixed"] = model
                .evidence
                .file("alternate-fixed.rs", source.as_bytes())?;
            alternate["semantic_oracle"]["answer_key"] = model.evidence.file(
                "alternate-key.json",
                &serde_json::to_vec(&key).map_err(|error| error.to_string())?,
            )?;
            pairing["answer_key_sha256"] =
                alternate["semantic_oracle"]["answer_key"]["sha256"].clone();
        } else {
            let mut row = pairing["observations"][0].clone();
            let mut capture = retained_json(&model.evidence.root, &row["capture"])?;
            row["runner"]["version"] = json!("alternate synthetic capture version");
            let mut observation = row.clone();
            let _ = observation
                .as_object_mut()
                .ok_or("missing observation")?
                .remove("capture");
            capture["observation"] = observation;
            row["capture"] = model.evidence.file(
                "alternate-native-capture.json",
                &serde_json::to_vec(&capture).map_err(|error| error.to_string())?,
            )?;
            pairing["observations"][0] = row;
        }
        alternate["semantic_oracle"]["native_pairing"] = model.evidence.file(
            "alternate-pairing.json",
            &serde_json::to_vec(&pairing).map_err(|error| error.to_string())?,
        )?;
        let mut review = retained_json(
            &model.evidence.root,
            &alternate["semantic_oracle"]["independent_review"],
        )?;
        for field in ["answer_key", "native_pairing"] {
            review["reviewed_subject"][field] = alternate["semantic_oracle"][field].clone();
        }
        alternate["semantic_oracle"]["independent_review"] = model.evidence.file(
            "alternate-review.json",
            &serde_json::to_vec(&review).map_err(|error| error.to_string())?,
        )?;
        assert_eq!(
            validate_declaration(&model.evidence.root, &alternate)?.0,
            "invalid"
        );
        observed_static::validate(
            &model.evidence.root,
            &key,
            &pairing,
            &alternate["observed_static"],
        )?;
        model.controls[1] = alternate;
        let (disclosure, violations) = model.check();
        assert!(
            violations
                .iter()
                .any(|message| message.contains("same-case controls disagree")),
            "{mode}: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|message| message.contains("incomplete reviewed role pair"))
        );
        let report = disclosure.items.join("\n");
        assert!(report.contains("historical_cases=0, valid=1, invalid=1, rejected=0"));
        assert!(report.contains("complete_cases=0, incomplete_cases=1"));
    }
    Ok(())
}

#[test]
fn benchmark_semantic_controls_real_corpus_identity_and_disclosure() -> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let path = repo.join("fixtures/evidence-quality-benchmark/corpus.json");
    let corpus = read_json(&path)?;
    let cases = corpus["cases"]
        .as_array()
        .ok_or_else(|| "missing cases".to_string())?;
    let controls = corpus["semantic_oracle_controls"]
        .as_array()
        .ok_or_else(|| "locked semantic controls missing".to_string())?;
    assert_eq!(cases.len(), 82);
    assert_eq!(controls.len(), 2);
    let variants = controls
        .iter()
        .map(|control| {
            (
                control["id"].as_str(),
                control["semantic_oracle"]["variant"].as_str(),
                control["semantic_oracle"]["status"].as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        variants,
        BTreeSet::from([
            (
                Some("regex-word-boundary-empty"),
                Some("corrected"),
                Some("valid")
            ),
            (
                Some("regex-word-boundary-empty"),
                Some("original"),
                Some("invalid")
            )
        ])
    );
    let root = path
        .parent()
        .ok_or_else(|| "corpus has no parent".to_string())?;
    let key = retained_json(root, &controls[0]["semantic_oracle"]["answer_key"])?;
    let pairing = retained_json(root, &controls[0]["semantic_oracle"]["native_pairing"])?;
    assert_eq!(key["production_source_path"], "regex-syntax/src/hir/mod.rs");
    assert_eq!(key["basis"][0]["kind"], "source_document");
    assert_eq!(key["basis"][1]["kind"], "source_document");
    for (field, expected) in [
        ("kind", "separate_matcher_semantic_witness"),
        ("native_test_variant", "corrected"),
        ("package", "regex"),
        ("package_version", "1.5.5"),
        ("library_target", "regex"),
        ("package_manifest_path", "Cargo.toml"),
        ("library_source_path", "src/lib.rs"),
    ] {
        assert_eq!(key["basis"][2][field], expected, "reviewed matcher {field}");
    }
    // Documentary/package preservation does not grant runtime semantic authority.
    for descriptor in [
        json!({"path": "regex-word-boundary-empty/LICENSE-APACHE", "bytes": 10847, "sha256": "a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2"}),
        json!({"path": "regex-word-boundary-empty/LICENSE-MIT", "bytes": 1071, "sha256": "6485b8ed310d3f0340bf1ad1f47645069ce4069dcc6bb46c7d5c6faf41de1fdb"}),
        json!({"path": "regex-word-boundary-empty/LICENSE-UNICODE", "bytes": 2847, "sha256": "74db5baf44a41b1000312c673544b3374e4198af5605c7f9080a402cec42cfa3"}),
        json!({"path": "regex-word-boundary-empty/README.md", "bytes": 10293, "sha256": "141017c9f8e602432e88d19dd8e4601b699b49768f89f808f787c413fa745630"}),
        json!({"path": "regex-word-boundary-empty/patches/corrected-test.patch", "bytes": 1313, "sha256": "44c33c77f0edc41ebfc5e7a4824ae0c44a99df27ab65b054b40aa19d298c4eba"}),
        json!({"path": "regex-word-boundary-empty/patches/remove-word-boundary-assertions.patch", "bytes": 422, "sha256": "bd70932756c4ed2eb686a06884f2ad1383a3a2624935ddd80b6f26465db3a5f2"}),
        json!({"path": "regex-word-boundary-empty/patches/upstream-full.patch", "bytes": 4543, "sha256": "200029a68acec06196eb51702ea448571a7cd0d9f0ec48287a9acb06d73ae927"}),
        json!({"path": "regex-word-boundary-empty/reviews/native-evidence.md", "bytes": 6642, "sha256": "ff22efa72e6bfe2f95644b10f87e349020e7c16b822b6b37c286558e4b88a759"}),
    ] {
        verify_file(root, &descriptor)?;
    }
    let retained_support = [
        (
            "/original_workspace/archives/parent",
            json!({"path": "regex-word-boundary-empty/upstream/regex-72f09f1.tar.gz", "bytes": 3317228, "sha256": "8d73a0ebb84927de0fb6df939720a8cc7328cbe7972449ac06097db7af7d4dc2"}),
        ),
        (
            "/original_workspace/archives/fixed",
            json!({"path": "regex-word-boundary-empty/upstream/regex-88a2a62.tar.gz", "bytes": 3317664, "sha256": "4baf4cec952ef02a9eaa66a3c7d72514cd9cfef7cba36cf9fcda08a89559db6b"}),
        ),
        (
            "/original_workspace/inventories/parent",
            json!({"path": "regex-word-boundary-empty/upstream/parent-archive-files.json", "bytes": 64504, "sha256": "81029ea5e7fdc20c842a782547841247a22b76c0d66cbed100c025e34dc0222f"}),
        ),
        (
            "/original_workspace/inventories/fixed",
            json!({"path": "regex-word-boundary-empty/upstream/fixed-archive-files.json", "bytes": 64504, "sha256": "be21d5bf82723f6819d83f1a11518d1ff786c89230e72af582d0cd7d63b1de0a"}),
        ),
        (
            "/original_workspace/provenance/parent-git-commit.json",
            json!({"path": "regex-word-boundary-empty/upstream/parent-git-commit.json", "bytes": 2001, "sha256": "4d3cd0f4900c7ecf59b3d016f8e98746dd9e13a2f406cca3d1943585a51bbe4c"}),
        ),
        (
            "/original_workspace/provenance/fixed-git-commit.json",
            json!({"path": "regex-word-boundary-empty/upstream/fixed-git-commit.json", "bytes": 1992, "sha256": "bcf8d60dbc1e805f5368a26959fa92e6176b3353dd692d9a6ceb71d2e950db1e"}),
        ),
        (
            "/original_workspace/provenance/parent-git-tree.json",
            json!({"path": "regex-word-boundary-empty/upstream/parent-git-tree.json", "bytes": 81990, "sha256": "0cb0fae21a0344e7b5d24da852d99147d4bfbefeab5f7b0e45b19de6b4022bfa"}),
        ),
        (
            "/original_workspace/provenance/fixed-git-tree.json",
            json!({"path": "regex-word-boundary-empty/upstream/fixed-git-tree.json", "bytes": 81990, "sha256": "e6ae330a4b938b1e536ce3aa34d7e1e587e84f4377cb32b7d2d58d55853193e7"}),
        ),
        (
            "/native_packet",
            json!({"path": "regex-word-boundary-empty/capture/original-native-result-packet.json", "bytes": 50067, "sha256": "b910ec9fafcdca9e1783218df877f8900313362a7741c51716d1c950a9045a0b"}),
        ),
        (
            "/native_receipt",
            json!({"path": "regex-word-boundary-empty/capture/original-native-receipt.json", "bytes": 87202, "sha256": "8dca1d09ece7506e6f32302e310c55f0efc0eb6dcacb105e5cbdff88d74fe6b8"}),
        ),
        (
            "/resolution_receipt",
            json!({"path": "regex-word-boundary-empty/capture/resolution-and-offline-compile-receipt.json", "bytes": 7123, "sha256": "53b23722085f90ded5647dac2a9bb6c13796087139c80191c303e3d72ecfad04"}),
        ),
        (
            "/offline_setup_receipt",
            json!({"path": "regex-word-boundary-empty/capture/offline-setup-receipt.json", "bytes": 6454, "sha256": "03bbc87f7105db3376605c05ebfdf014ce8ac5bdde0299250b52c795e0979ec6"}),
        ),
        (
            "/capture_checker_interruption",
            json!({"path": "regex-word-boundary-empty/capture/checker-interruption-receipt.json", "bytes": 23673, "sha256": "a47592ed1dde8e45ca8d8a45fc16a53c85e1c3dc9845df329f8c0da5622966bd"}),
        ),
    ];
    for (pointer, expected) in retained_support {
        assert_eq!(
            pairing.pointer(pointer),
            Some(&expected),
            "retained support {pointer}"
        );
    }
    assert_eq!(
        key["test_id"],
        "hir::translate::tests::analysis_is_match_empty"
    );
    assert_eq!(
        pairing["original_workspace"]["parent_commit"],
        "72f09f1aeb0ff3f703b1afdbdd21f5ff63162fb4"
    );
    assert_eq!(
        pairing["original_workspace"]["fixed_commit"],
        "88a2a62d861d189faae539990f63cb9cf195bd8c"
    );
    assert_eq!(
        key["sources"]["fixed"]["sha256"],
        "51f1642b75e298b0847855d7f490eca2e8f445c9039dd3aa91491f533ec83f15"
    );
    assert_eq!(
        key["sources"]["broken"]["sha256"],
        "325dc1e42eb8fb9daeb7a8a5e7f967fdee745a7a7c5e26c20dec0b6c66109ad7"
    );
    assert_eq!(
        key["tests"]["corrected"]["sha256"],
        "de10ee2928001567f80c6ab602de0e280a8e3a1e615cf73def91d2dcc4f9f199"
    );
    assert_eq!(
        key["tests"]["original"]["sha256"],
        "cee557e068927ef028fdd0c8b673b948aa52397ed7bd5500f1a38a51bc29ec55"
    );
    assert_eq!(
        key["tests"]["weak"]["sha256"],
        "9150ff57bf007f523ef1dc8f4c9680ea2edfd3fe55fca47f83b36b1b5834e169"
    );
    let mut violations = Vec::new();
    let disclosure = super::super::super::general_validators::validate_evidence_quality_benchmark_fixture_corpus_at(&path, &mut violations)?;
    assert!(violations.is_empty(), "{violations:?}");
    assert!(disclosure.items.iter().any(|line| line.contains("Benchmark cases: 82. Historical semantic controls: views=2, historical_cases=1, valid=1, invalid=1, rejected=0")));
    assert!(
        disclosure
            .items
            .iter()
            .any(|line| line.contains("unreviewed=82 (legacy absent=82)"))
    );
    Ok(())
}

#[test]
fn benchmark_semantic_controls_production_command_keeps_separate_counts() -> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    crate::tests::with_temp_cwd("historical-controls-production-report", |root| {
        crate::tests::copy_dir_recursive(&repo.join("fixtures"), &root.join("fixtures"))?;
        crate::fixture_contracts::check_fixture_contracts()?;
        let report_path = root.join("target/ripr/reports/fixture-contracts.md");
        let report = fs::read_to_string(&report_path).map_err(|error| error.to_string())?;
        assert!(report.contains("Benchmark cases: 82. Historical semantic controls: views=2, historical_cases=1, valid=1, invalid=1, rejected=0"));
        assert!(report.contains("Semantic review accepts only its exact answer-key/native-pairing subject; it does not accept the attached static analysis"));
        assert!(report.contains("NOT_REVERIFIED"));
        let path = root.join("fixtures/evidence-quality-benchmark/corpus.json");
        let original = read_json(&path)?;
        for mode in [
            "malformed",
            "omit_corrected",
            "omit_original",
            "duplicate_corrected",
        ] {
            let mut corpus = original.clone();
            match mode {
                "omit_corrected" => {
                    let _ = corpus["semantic_oracle_controls"]
                        .as_array_mut()
                        .ok_or("missing controls")?
                        .remove(0);
                }
                "omit_original" => {
                    let _ = corpus["semantic_oracle_controls"]
                        .as_array_mut()
                        .ok_or("missing controls")?
                        .remove(1);
                }
                "duplicate_corrected" => {
                    corpus["semantic_oracle_controls"][1] =
                        corpus["semantic_oracle_controls"][0].clone()
                }
                _ => corpus["semantic_oracle_controls"][0]["semantic_oracle"] = Value::Null,
            }
            fs::write(
                &path,
                serde_json::to_vec(&corpus).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            assert!(
                crate::fixture_contracts::check_fixture_contracts().is_err(),
                "accepted {mode}"
            );
            let report = fs::read_to_string(&report_path).map_err(|error| error.to_string())?;
            assert!(report.contains("historical_cases=0"));
            assert!(report.contains("complete_cases=0, incomplete_cases=1"));
            assert!(report.contains("incomplete reviewed role pair"));
        }
        Ok(())
    })
}
