//! Fault injection in copied retained evidence. These are not new native runs.

use std::collections::BTreeMap;

use super::*;

fn real_case() -> Result<SyntheticEvidence, String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let root = crate::tests::temp_dir("retained-evidence-graph");
    crate::tests::copy_dir_recursive(&repo.join("fixtures/evidence-quality-benchmark"), &root)?;
    let corpus = read_json(&root.join("corpus.json"))?;
    let case = corpus["semantic_oracle_controls"][0].clone();
    let key = retained_json(&root, &case["semantic_oracle"]["answer_key"])?;
    let pairing = retained_json(&root, &case["semantic_oracle"]["native_pairing"])?;
    Ok(SyntheticEvidence {
        root,
        case,
        key,
        pairing,
    })
}

fn rewrite_capture_inputs(
    fixture: &mut SyntheticEvidence,
    index: usize,
    before: &Value,
    after: &Value,
) -> Result<(), String> {
    let mut capture = fixture.capture(index)?;
    for (field, value) in [
        ("full_workspace_inputs_before", before),
        ("full_workspace_inputs_after", after),
    ] {
        capture[field] = fixture.file(
            &format!("mutated-{index}-{field}.json"),
            &serde_json::to_vec(value).map_err(|error| error.to_string())?,
        )?;
    }
    fixture.persist_capture(index, &capture)?;
    fixture.accept_review()
}

fn matcher(fixture: &SyntheticEvidence) -> Result<Value, String> {
    retained_json(&fixture.root, &fixture.key["basis"][2]["artifact"])
}

fn rewrite_matcher(fixture: &mut SyntheticEvidence, witness: &Value) -> Result<(), String> {
    fixture.key["basis"][2]["artifact"] = fixture.file(
        "mutated-matcher.json",
        &serde_json::to_vec(witness).map_err(|error| error.to_string())?,
    )?;
    fixture.persist()?;
    fixture.accept_review()
}

fn expect_rejection(actual: Result<&str, String>, label: &str, failures: &mut Vec<String>) {
    if actual.is_ok() {
        failures.push(format!("accepted {label}: {actual:?}"));
    }
}

#[test]
fn benchmark_semantic_graph_requires_every_native_input_fence() -> Result<(), String> {
    let mut failures = Vec::new();
    for (index, pair) in PAIRS.iter().enumerate() {
        for field in [
            "full_workspace_inputs_before",
            "full_workspace_inputs_after",
        ] {
            for mode in ["missing", "corrupt", "missing_descriptor"] {
                let mut fixture = SyntheticEvidence::new()?;
                assert_eq!(fixture.validate()?, "valid");
                let mut capture = fixture.capture(index)?;
                if mode == "missing_descriptor" {
                    let _ = capture
                        .as_object_mut()
                        .ok_or("missing capture")?
                        .remove(field);
                    fixture.persist_capture(index, &capture)?;
                    fixture.accept_review()?;
                } else {
                    let path = fixture.root.join(text(&capture[field], "path")?);
                    if mode == "missing" {
                        fs::remove_file(path).map_err(|error| error.to_string())?;
                    } else {
                        fs::write(path, b"corrupt retained inventory")
                            .map_err(|error| error.to_string())?;
                    }
                }
                expect_rejection(
                    fixture.validate(),
                    &format!("{} {field} {mode}", pair.0),
                    &mut failures,
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "native inventory acceptance gaps: {failures:?}"
    );
    Ok(())
}

#[test]
fn benchmark_semantic_graph_rejects_partial_or_stale_workspace_inputs() -> Result<(), String> {
    let mut failures = Vec::new();
    for mode in [
        "shape",
        "duplicate",
        "escape",
        "kind",
        "source",
        "test",
        "lock",
        "before_after",
        "missing_neighbor",
        "changed_neighbor",
        "extra_neighbor",
        "three_rows",
    ] {
        let mut fixture = real_case()?;
        assert_eq!(fixture.validate()?, "valid");
        let index = fixture.pairing["observations"]
            .as_array()
            .ok_or("missing observations")?
            .iter()
            .position(|row| row["variant"] == "broken_original")
            .ok_or("missing broken-original")?;
        let capture = fixture.capture(index)?;
        let mut before = retained_json(&fixture.root, &capture["full_workspace_inputs_before"])?;
        let mut after = before.clone();
        if mode == "shape" {
            before = json!({});
        } else {
            let rows = before.as_array_mut().ok_or("missing input rows")?;
            let first = rows.first().ok_or("empty inputs")?.clone();
            match mode {
                "duplicate" => rows.push(first),
                "escape" => rows[0]["path"] = json!("../outside"),
                "kind" => rows[0]["kind"] = json!("directory"),
                "extra_neighbor" => {
                    let mut extra = first;
                    extra["path"] = json!("unrecorded-neighbor.txt");
                    rows.push(extra);
                }
                "missing_neighbor" => rows.retain(|row| row["path"] != "CHANGELOG.md"),
                "three_rows" => rows.retain(|row| {
                    [
                        "regex-syntax/src/hir/mod.rs",
                        "regex-syntax/src/hir/translate.rs",
                        "Cargo.lock",
                    ]
                    .iter()
                    .any(|path| row["path"] == *path)
                }),
                _ => {
                    let path = match mode {
                        "source" => "regex-syntax/src/hir/mod.rs",
                        "test" => "regex-syntax/src/hir/translate.rs",
                        "lock" => "Cargo.lock",
                        _ => "CHANGELOG.md",
                    };
                    let row = rows
                        .iter_mut()
                        .find(|row| row["path"] == path)
                        .ok_or("missing mutation subject")?;
                    row["sha256"] = json!("0".repeat(64));
                }
            }
        }
        if mode != "before_after" {
            after = before.clone();
        }
        rewrite_capture_inputs(&mut fixture, index, &before, &after)?;
        expect_rejection(fixture.validate(), mode, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "native footprint acceptance gaps: {failures:?}"
    );
    Ok(())
}

#[test]
fn benchmark_semantic_graph_requires_typed_matcher_basis() -> Result<(), String> {
    let mut failures = Vec::new();
    for mode in [
        "missing_kind",
        "null_kind",
        "unknown_kind",
        "wrong_native_variant",
        "package",
        "package_version",
        "library_target",
        "package_manifest_path",
        "library_source_path",
        "unrecorded_compiler_paths",
        "citation",
    ] {
        let mut fixture = real_case()?;
        assert_eq!(fixture.validate()?, "valid");
        match mode {
            "missing_kind" => {
                let _ = fixture.key["basis"][2]
                    .as_object_mut()
                    .ok_or("missing matcher basis")?
                    .remove("kind");
            }
            "null_kind" => fixture.key["basis"][2]["kind"] = Value::Null,
            "unknown_kind" => fixture.key["basis"][2]["kind"] = json!("unknown_witness"),
            "wrong_native_variant" => {
                fixture.key["basis"][2]["native_test_variant"] = json!("original")
            }
            "unrecorded_compiler_paths" => {
                fixture.key["basis"][2]["package_manifest_path"] = json!("unrecorded/Cargo.toml");
                fixture.key["basis"][2]["library_source_path"] = json!("unrecorded/lib.rs");
                let mut witness = matcher(&fixture)?;
                for observation in witness["observations"]
                    .as_array_mut()
                    .ok_or("missing matcher observations")?
                {
                    let variant = format!("{}_corrected", text(observation, "production")?);
                    let native = fixture.pairing["observations"]
                        .as_array()
                        .ok_or("missing native rows")?
                        .iter()
                        .find(|row| row["variant"] == variant)
                        .ok_or("missing matching native row")?;
                    let cwd = text(&native["runner"], "cwd")?;
                    observation["compiler_artifact"]["package_id"] =
                        json!(format!("path+file://{cwd}/unrecorded#regex@1.5.5"));
                    observation["compiler_artifact"]["manifest_path"] =
                        json!(format!("{cwd}/unrecorded/Cargo.toml"));
                    observation["compiler_artifact"]["target"]["src_path"] =
                        json!(format!("{cwd}/unrecorded/lib.rs"));
                }
                rewrite_matcher(&mut fixture, &witness)?;
            }
            "citation" => {
                let mut witness = matcher(&fixture)?;
                witness["source_of_nonconsumption_claim"] = fixture.file(
                    "different-citation.txt",
                    b"Different synthetic source document",
                )?;
                rewrite_matcher(&mut fixture, &witness)?;
            }
            field => fixture.key["basis"][2][field] = json!("wrong-subject"),
        }
        fixture.persist()?;
        fixture.accept_review()?;
        expect_rejection(fixture.validate(), mode, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "matcher type/subject acceptance gaps: {failures:?}"
    );
    Ok(())
}

#[test]
fn benchmark_semantic_graph_rejects_missing_or_inconsistent_matcher_inputs() -> Result<(), String> {
    let mut failures = Vec::new();
    for mode in [
        "missing_production",
        "duplicate_production",
        "missing_program_role",
        "program_identity",
        "input_variant",
        "input_partial",
        "argv",
        "exit",
        "custody",
        "artifact",
        "linked_library",
        "linked_metadata",
        "compiler_target",
        "observed_values",
        "observed_output",
    ] {
        let mut fixture = real_case()?;
        assert_eq!(fixture.validate()?, "valid");
        let mut witness = matcher(&fixture)?;
        match mode {
            "missing_production" => {
                let _ = witness["observations"]
                    .as_array_mut()
                    .ok_or("missing matcher observations")?
                    .pop();
            }
            "duplicate_production" => witness["observations"][1]["production"] = json!("fixed"),
            "missing_program_role" => witness["observations"][0]["original"] = Value::Null,
            "program_identity" => {
                witness["observations"][1]["original"]["program"] = fixture.file(
                    "different-program.txt",
                    b"Synthetic different matcher program",
                )?
            }
            "input_variant" => {
                let wrong = fixture.pairing["observations"]
                    .as_array()
                    .ok_or("missing native observations")?
                    .iter()
                    .find(|row| row["variant"] == "broken_original")
                    .ok_or("missing wrong variant")?;
                let capture = retained_json(&fixture.root, &wrong["capture"])?;
                witness["observations"][1]["input_inventory_before"] =
                    capture["full_workspace_inputs_before"].clone();
                witness["observations"][1]["input_inventory_after"] =
                    capture["full_workspace_inputs_after"].clone();
            }
            "input_partial" => {
                let mut inputs = retained_json(
                    &fixture.root,
                    &witness["observations"][0]["input_inventory_before"],
                )?;
                let _ = inputs.as_array_mut().ok_or("missing matcher inputs")?.pop();
                let descriptor = fixture.file(
                    "partial-matcher-inputs.json",
                    &serde_json::to_vec(&inputs).map_err(|error| error.to_string())?,
                )?;
                witness["observations"][0]["input_inventory_before"] = descriptor.clone();
                witness["observations"][0]["input_inventory_after"] = descriptor;
            }
            "argv" => witness["observations"][0]["original"]["argv"] = json!(["/wrong-executable"]),
            "exit" => witness["observations"][0]["original"]["exit_code"] = json!(101),
            "custody" => witness["observations"][0]["original"]["custody"] = json!("local"),
            "artifact" => witness["observations"][0]["original"]["artifact"]["bytes"] = json!(0),
            "linked_library" => {
                witness["observations"][0]["linked_library"]["path"] = json!("/not-selected.rlib")
            }
            "linked_metadata" => {
                let metadata = witness["observations"][0]["compiler_artifact"]["filenames"]
                    .as_array()
                    .ok_or("missing compiler files")?
                    .iter()
                    .find(|path| path.as_str().is_some_and(|path| path.ends_with(".rmeta")))
                    .ok_or("missing listed metadata output")?
                    .clone();
                witness["observations"][0]["linked_library"]["path"] = metadata;
            }
            "compiler_target" => {
                witness["observations"][0]["compiler_artifact"]["target"]["kind"] = json!(["bin"])
            }
            "observed_values" => {
                witness["observations"][0]["observed_values"]["word_boundary_empty_haystack"] =
                    json!(true)
            }
            _ => {
                witness["observations"][0]["observed"]["stdout"] =
                    fixture.file("wrong-matcher.stdout", b"wrong captured output\n")?
            }
        }
        rewrite_matcher(&mut fixture, &witness)?;
        expect_rejection(fixture.validate(), mode, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "matcher capture acceptance gaps: {failures:?}"
    );
    Ok(())
}

fn add(files: &mut BTreeMap<String, Value>, descriptor: &Value) -> Result<(), String> {
    let path = text(descriptor, "path")?.to_string();
    if let Some(previous) = files.insert(path, descriptor.clone()) {
        assert_eq!(previous, *descriptor, "conflicting retained descriptor");
    }
    Ok(())
}

fn required_graph(root: &Path, corpus: &Value) -> Result<BTreeMap<String, Value>, String> {
    let mut files = BTreeMap::new();
    let controls = corpus["semantic_oracle_controls"]
        .as_array()
        .ok_or("missing controls")?;
    for control in controls {
        for field in ["answer_key", "native_pairing", "independent_review"] {
            add(&mut files, &control["semantic_oracle"][field])?;
        }
        add(&mut files, &control["observed_static"])?;
    }
    let key = retained_json(root, &controls[0]["semantic_oracle"]["answer_key"])?;
    for group in ["sources", "tests"] {
        for descriptor in key[group].as_object().ok_or("missing key assets")?.values() {
            add(&mut files, descriptor)?;
        }
    }
    for basis in key["basis"].as_array().ok_or("missing basis")? {
        add(&mut files, &basis["artifact"])?;
        if basis["kind"] == "separate_matcher_semantic_witness" {
            let witness = retained_json(root, &basis["artifact"])?;
            add(&mut files, &witness["source_of_nonconsumption_claim"])?;
            for observation in witness["observations"]
                .as_array()
                .ok_or("missing matcher observations")?
            {
                for field in ["input_inventory_before", "input_inventory_after"] {
                    add(&mut files, &observation[field])?;
                }
                for role in ["original", "observed"] {
                    for field in ["program", "stdout", "stderr"] {
                        add(&mut files, &observation[role][field])?;
                    }
                }
            }
        }
    }
    let pairing = retained_json(root, &controls[0]["semantic_oracle"]["native_pairing"])?;
    for field in [
        "lock",
        "native_packet",
        "native_receipt",
        "resolution_receipt",
        "offline_setup_receipt",
        "capture_checker_interruption",
    ] {
        add(&mut files, &pairing[field])?;
    }
    for group in ["archives", "inventories", "provenance"] {
        for descriptor in pairing["original_workspace"][group]
            .as_object()
            .ok_or("missing workspace support")?
            .values()
        {
            add(&mut files, descriptor)?;
        }
    }
    for row in pairing["observations"]
        .as_array()
        .ok_or("missing native rows")?
    {
        for field in ["stdout", "stderr", "capture"] {
            add(&mut files, &row[field])?;
        }
        let capture = retained_json(root, &row["capture"])?;
        for field in [
            "retained_verification",
            "full_workspace_inputs_before",
            "full_workspace_inputs_after",
        ] {
            add(&mut files, &capture[field])?;
        }
        for run in ["discovery", "artifact_execution"] {
            for field in ["stdout", "stderr"] {
                add(&mut files, &capture[run][field])?;
            }
        }
    }
    let static_capture = retained_json(root, &controls[0]["observed_static"])?;
    for field in [
        "producer_receipt",
        "execution_receipt",
        "diff",
        "subject_alignment",
        "corrected_summary",
        "original_summary_before_correction",
    ] {
        add(&mut files, &static_capture[field])?;
    }
    add(&mut files, &static_capture["old_attempt"]["receipt"])?;
    for row in static_capture["observations"]
        .as_array()
        .ok_or("missing static observations")?
    {
        for field in ["json", "stderr", "inputs_before", "inputs_after"] {
            add(&mut files, &row[field])?;
        }
    }
    Ok(files)
}

fn check_corpus(path: &Path) -> Result<(crate::PolicyDisclosure, Vec<String>), String> {
    let mut violations = Vec::new();
    let report = super::super::super::general_validators::validate_evidence_quality_benchmark_fixture_corpus_at(path, &mut violations)?;
    Ok((report, violations))
}

#[test]
fn benchmark_semantic_graph_rejects_missing_or_corrupt_local_dependencies() -> Result<(), String> {
    let fixture = real_case()?;
    let path = fixture.root.join("corpus.json");
    let corpus = read_json(&path)?;
    let files = required_graph(&fixture.root, &corpus)?;
    assert_eq!(files.len(), 120, "complete bounded local graph");
    let (_, baseline) = check_corpus(&path)?;
    assert!(baseline.is_empty(), "invalid baseline: {baseline:?}");
    let mut failures = Vec::new();
    for (name, descriptor) in files {
        verify_file(&fixture.root, &descriptor)?;
        let file = fixture.root.join(&name);
        let bytes = fs::read(&file).map_err(|error| error.to_string())?;
        for mode in ["missing", "corrupt"] {
            if mode == "missing" {
                fs::remove_file(&file).map_err(|error| error.to_string())?;
            } else {
                let mut corrupt = bytes.clone();
                if let Some(first) = corrupt.first_mut() {
                    *first ^= 1;
                } else {
                    corrupt.push(b'X');
                }
                fs::write(&file, corrupt).map_err(|error| error.to_string())?;
            }
            let result = check_corpus(&path);
            fs::write(&file, &bytes).map_err(|error| error.to_string())?;
            verify_file(&fixture.root, &descriptor)?;
            let (report, violations) = result?;
            if violations.is_empty()
                || !report
                    .items
                    .iter()
                    .any(|item| item.contains("complete_cases=0, incomplete_cases=1"))
            {
                failures.push(format!("{name}/{mode}: {violations:?}"));
            }
        }
    }
    let (_, restored) = check_corpus(&path)?;
    assert!(restored.is_empty(), "restored graph failed: {restored:?}");
    assert!(
        failures.is_empty(),
        "required local graph acceptance gaps: {failures:?}"
    );
    Ok(())
}

#[test]
fn benchmark_semantic_graph_production_report_rejects_required_leaf_damage() -> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    crate::tests::with_temp_cwd("typed-evidence-graph-report", |root| {
        crate::tests::copy_dir_recursive(&repo.join("fixtures"), &root.join("fixtures"))?;
        let corpus_root = root.join("fixtures/evidence-quality-benchmark");
        let corpus = read_json(&corpus_root.join("corpus.json"))?;
        let graph = required_graph(&corpus_root, &corpus)?;
        crate::fixture_contracts::check_fixture_contracts()?;
        let mut failures = Vec::new();
        for name in [
            "generated-Cargo.lock",
            "inputs/broken_original-before.json",
            "basis/matcher-original.rs.txt",
            "outputs/matcher-fixed-observed.stdout",
            "inputs/matcher-broken-after.json",
            "static/subject-alignment-68770.json",
        ] {
            let relative = format!("regex-word-boundary-empty/{name}");
            let descriptor = graph.get(&relative).ok_or("missing real graph subject")?;
            let path = corpus_root.join(&relative);
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            fs::remove_file(&path).map_err(|error| error.to_string())?;
            let result = crate::fixture_contracts::check_fixture_contracts();
            let report = fs::read_to_string(root.join("target/ripr/reports/fixture-contracts.md"));
            fs::write(&path, &bytes).map_err(|error| error.to_string())?;
            verify_file(&corpus_root, descriptor)?;
            crate::fixture_contracts::check_fixture_contracts()?;
            let report = report.map_err(|error| error.to_string())?;
            if result.is_ok()
                || !report.contains(&relative)
                || !report.contains("complete_cases=0, incomplete_cases=1")
            {
                failures.push(format!("{relative}: {result:?}\n{report}"));
            }
        }
        assert!(
            failures.is_empty(),
            "required graph report acceptance gaps: {failures:?}"
        );
        Ok(())
    })
}
