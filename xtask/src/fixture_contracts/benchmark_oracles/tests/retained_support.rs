//! Synthetic support descriptors exercise the production pairing validator.

use super::*;

const RECEIPTS: &[&str] = &[
    "native_packet",
    "native_receipt",
    "resolution_receipt",
    "offline_setup_receipt",
    "capture_checker_interruption",
];

fn support_fixture() -> Result<SyntheticEvidence, String> {
    let mut fixture = SyntheticEvidence::new()?;
    fixture.pairing["original_workspace"] = json!({});
    for (group, names, payload) in [
        (
            "archives",
            &["parent", "fixed"][..],
            &b"synthetic archive bytes"[..],
        ),
        ("inventories", &["parent", "fixed"][..], &b"[]"[..]),
        (
            "provenance",
            &[
                "parent-git-commit.json",
                "fixed-git-commit.json",
                "parent-git-tree.json",
                "fixed-git-tree.json",
            ][..],
            &b"{}"[..],
        ),
    ] {
        for name in names {
            fixture.pairing["original_workspace"][group][*name] =
                fixture.file(&format!("support/{group}/{name}"), payload)?;
        }
    }
    for (side, production, test) in [
        ("parent", "broken", "original"),
        ("fixed", "fixed", "corrected"),
    ] {
        let mut parent = fixture.input_inventory(production, test)?;
        let rows = parent
            .as_array_mut()
            .ok_or("missing synthetic parent inventory")?;
        rows.retain(|row| row["path"] != "Cargo.lock");
        for row in rows {
            let kind = row
                .as_object_mut()
                .ok_or("missing synthetic parent row")?
                .remove("kind")
                .ok_or("missing synthetic input kind")?;
            row["type"] = kind;
        }
        fixture.pairing["original_workspace"]["inventories"][side] = fixture.file(
            &format!("support/inventories/{side}"),
            &serde_json::to_vec(&parent).map_err(|error| error.to_string())?,
        )?;
    }
    for field in RECEIPTS {
        fixture.pairing[*field] = fixture.file(&format!("support/{field}.json"), b"{}")?;
    }
    refresh_review(&mut fixture)?;
    Ok(fixture)
}

fn refresh_review(fixture: &mut SyntheticEvidence) -> Result<(), String> {
    fixture.persist_pairing()?;
    fixture.accept_review()
}

fn descriptors(pairing: &Value) -> Result<Vec<(String, Value)>, String> {
    let mut entries = Vec::new();
    for group in ["archives", "inventories", "provenance"] {
        for (name, descriptor) in pairing["original_workspace"][group]
            .as_object()
            .ok_or("missing synthetic support group")?
        {
            entries.push((
                format!("/original_workspace/{group}/{name}"),
                descriptor.clone(),
            ));
        }
    }
    entries.extend(
        RECEIPTS
            .iter()
            .map(|field| (format!("/{field}"), pairing[*field].clone())),
    );
    Ok(entries)
}

fn record_refusal(actual: Result<&str, String>, label: &str, unexpected: &mut Vec<String>) {
    match actual {
        Err(error) if error.contains("semantic oracle retained support") => {}
        other => unexpected.push(format!("{label}: expected support refusal, got {other:?}")),
    }
}

#[test]
fn benchmark_semantic_oracle_retained_support_optional_and_complete() -> Result<(), String> {
    let full = support_fixture()?;
    assert_eq!(full.validate()?, "valid");
    assert_eq!(SyntheticEvidence::new()?.validate()?, "valid");
    for field in std::iter::once("original_workspace").chain(RECEIPTS.iter().copied()) {
        let mut fixture = SyntheticEvidence::new()?;
        crate::tests::copy_dir_recursive(
            &full.root.join("support"),
            &fixture.root.join("support"),
        )?;
        fixture.pairing[field] = full.pairing[field].clone();
        refresh_review(&mut fixture)?;
        assert_eq!(fixture.validate()?, "valid", "optional support {field}");
    }
    for (group, bytes) in [
        ("archives", &b"extra"[..]),
        ("inventories", &b"[]"[..]),
        ("provenance", &b"{}"[..]),
    ] {
        let mut fixture = support_fixture()?;
        fixture.pairing["original_workspace"][group]["extra"] = fixture.file("extra", bytes)?;
        refresh_review(&mut fixture)?;
        assert_eq!(fixture.validate()?, "valid", "extra {group} entry");
    }
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_retained_support_rejects_missing_or_corrupt_files()
-> Result<(), String> {
    let mut fixture = support_fixture()?;
    let mut entries = descriptors(&fixture.pairing)?;
    assert_eq!(entries.len(), 13);
    for (group, bytes) in [
        ("archives", &b"extra"[..]),
        ("inventories", &b"[]"[..]),
        ("provenance", &b"{}"[..]),
    ] {
        let descriptor = fixture.file(&format!("support/extra-{group}"), bytes)?;
        fixture.pairing["original_workspace"][group]["extra"] = descriptor.clone();
        entries.push((format!("extra {group}"), descriptor));
    }
    refresh_review(&mut fixture)?;
    assert_eq!(fixture.validate()?, "valid");
    let subject = fixture.case["semantic_oracle"].clone();
    let pairing = fs::read(fixture.root.join("pairing.json")).map_err(|error| error.to_string())?;
    let review = fs::read(fixture.root.join("review.json")).map_err(|error| error.to_string())?;
    let mut unexpected = Vec::new();
    for (label, descriptor) in entries {
        let path = fixture.root.join(text(&descriptor, "path")?);
        let original = fs::read(&path).map_err(|error| error.to_string())?;
        for mode in ["missing", "corrupt"] {
            if mode == "missing" {
                fs::remove_file(&path).map_err(|error| error.to_string())?;
            } else {
                let mut corrupt = original.clone();
                *corrupt.first_mut().ok_or("empty synthetic support")? ^= 1;
                fs::write(&path, corrupt).map_err(|error| error.to_string())?;
            }
            let actual = fixture.validate();
            fs::write(&path, &original).map_err(|error| error.to_string())?;
            assert_eq!(fixture.validate()?, "valid", "restore {label}/{mode}");
            record_refusal(actual, &format!("{label}/{mode}"), &mut unexpected);
            assert_eq!(fixture.case["semantic_oracle"], subject);
            assert_eq!(
                fs::read(fixture.root.join("pairing.json")).map_err(|error| error.to_string())?,
                pairing
            );
            assert_eq!(
                fs::read(fixture.root.join("review.json")).map_err(|error| error.to_string())?,
                review
            );
        }
    }
    assert!(
        unexpected.is_empty(),
        "accepted damaged support: {unexpected:?}"
    );
    Ok(())
}

fn replace_or_remove(
    fixture: &mut SyntheticEvidence,
    pointer: &str,
    value: Option<Value>,
) -> Result<(), String> {
    if let Some(value) = value {
        *fixture
            .pairing
            .pointer_mut(pointer)
            .ok_or("missing synthetic pointer")? = value;
    } else {
        let (parent, field) = pointer
            .rsplit_once('/')
            .ok_or("invalid synthetic pointer")?;
        let _ = fixture
            .pairing
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .ok_or("missing synthetic parent")?
            .remove(field);
    }
    refresh_review(fixture)
}

#[test]
fn benchmark_semantic_oracle_retained_support_rejects_malformed_declarations() -> Result<(), String>
{
    let full = support_fixture()?;
    let mut unexpected = Vec::new();
    let entries = descriptors(&full.pairing)?;
    let mut malformed = vec![
        ("/original_workspace".to_string(), Some(Value::Null)),
        ("/original_workspace".to_string(), Some(json!([]))),
    ];
    for group in ["archives", "inventories", "provenance"] {
        for value in [None, Some(Value::Null), Some(json!({})), Some(json!([]))] {
            malformed.push((format!("/original_workspace/{group}"), value));
        }
    }
    for (pointer, _) in &entries {
        for value in [Some(Value::Null), Some(json!({})), Some(json!([]))] {
            malformed.push((pointer.clone(), value));
        }
        if pointer.starts_with("/original_workspace/") {
            malformed.push((pointer.clone(), None));
        }
    }
    for (pointer, value) in malformed {
        let mut fixture = support_fixture()?;
        assert_eq!(fixture.validate()?, "valid");
        let label = format!("{pointer}={value:?}");
        replace_or_remove(&mut fixture, &pointer, value)?;
        record_refusal(fixture.validate(), &label, &mut unexpected);
    }
    for (pointer, _) in entries {
        if pointer.contains("/archives/") {
            continue;
        }
        let wrong_type = if pointer.contains("/inventories/") {
            &b"{}"[..]
        } else {
            &b"[]"[..]
        };
        for bytes in [wrong_type, &b"not JSON"[..]] {
            let mut fixture = support_fixture()?;
            assert_eq!(fixture.validate()?, "valid");
            let descriptor = fixture.file("wrong-payload.json", bytes)?;
            replace_or_remove(&mut fixture, &pointer, Some(descriptor))?;
            record_refusal(
                fixture.validate(),
                &format!("{pointer}: {bytes:?}"),
                &mut unexpected,
            );
        }
    }
    assert!(
        unexpected.is_empty(),
        "accepted malformed support: {unexpected:?}"
    );
    Ok(())
}

#[test]
fn benchmark_semantic_oracle_retained_support_production_report_rejects_damage()
-> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    crate::tests::with_temp_cwd("historical-support-production-report", |root| {
        crate::tests::copy_dir_recursive(&repo.join("fixtures"), &root.join("fixtures"))?;
        let corpus_root = root.join("fixtures/evidence-quality-benchmark");
        let corpus = read_json(&corpus_root.join("corpus.json"))?;
        let pairing = retained_json(
            &corpus_root,
            &corpus["semantic_oracle_controls"][0]["semantic_oracle"]["native_pairing"],
        )?;
        crate::fixture_contracts::check_fixture_contracts()?;
        let report_path = root.join("target/ripr/reports/fixture-contracts.md");
        let positive = fs::read_to_string(&report_path).map_err(|error| error.to_string())?;
        assert!(positive.contains("views=2, historical_cases=1, valid=1, invalid=1, rejected=0"));
        let mut unexpected = Vec::new();
        for pointer in [
            "/original_workspace/archives/parent",
            "/original_workspace/inventories/parent",
            "/original_workspace/provenance/parent-git-commit.json",
            "/native_packet",
            "/offline_setup_receipt",
        ] {
            let descriptor = pairing
                .pointer(pointer)
                .ok_or("missing real support descriptor")?;
            let path = corpus_root.join(text(descriptor, "path")?);
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            for mode in ["missing", "corrupt"] {
                if mode == "missing" {
                    fs::remove_file(&path).map_err(|error| error.to_string())?;
                } else {
                    let mut corrupt = bytes.clone();
                    *corrupt.first_mut().ok_or("empty real support")? ^= 1;
                    fs::write(&path, corrupt).map_err(|error| error.to_string())?;
                }
                let actual = crate::fixture_contracts::check_fixture_contracts();
                let report = fs::read_to_string(&report_path);
                fs::write(&path, &bytes).map_err(|error| error.to_string())?;
                crate::fixture_contracts::check_fixture_contracts()?;
                let report = report.map_err(|error| error.to_string())?;
                if actual.is_ok()
                    || !report.contains("valid=0, invalid=0, rejected=2")
                    || !report.contains("complete_cases=0, incomplete_cases=1")
                    || !report.contains("semantic oracle retained support")
                    || !report.contains(text(descriptor, "path")?)
                {
                    unexpected.push(format!("{pointer}/{mode}: {actual:?}\n{report}"));
                }
            }
        }
        assert!(
            unexpected.is_empty(),
            "accepted damaged support report: {unexpected:?}"
        );
        Ok(())
    })
}
