use serde_json::{Value, json};

use super::command::{CommandOptions, evaluate_path};
use super::gate::evaluate;
use super::model::{
    CLAIM_BOUNDARY, ExpectedIdentity, GATE_KIND, GateInput, GateVerdict, RECEIPT_KIND,
    SelectionScope,
};
use super::parse::parse_receipt;
use super::render::{render_gate_json, render_gate_markdown, render_receipt_json};

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TREE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PACKAGE_HASH: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const PAYLOAD_HASH: &str =
    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const OTHER_HASH: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const STALE_COMMIT: &str = "ffffffffffffffffffffffffffffffffffffffff";

fn valid_receipt_value() -> Value {
    json!({
        "schema_version": "1.0",
        "kind": RECEIPT_KIND,
        "source": { "commit": COMMIT, "tree": TREE },
        "release_identity": { "product": "ripr", "version": "0.11.0" },
        "package": {
            "channel": "pypi",
            "name": "ripr-rs",
            "hash": PACKAGE_HASH
        },
        "payload": {
            "target": "x86_64-unknown-linux-gnu",
            "hash": PAYLOAD_HASH
        },
        "tools": {
            "runtime": "python 3.12.3",
            "package_manager": "pip 24.2"
        },
        "subjects": {
            "selected": 3,
            "executed": 3,
            "failed": 0,
            "skipped": 0
        },
        "selection_scope": "explicit_subset",
        "required_rows": [
            { "channel": "pypi", "target": "x86_64-unknown-linux-gnu" }
        ],
        "rows": [
            {
                "channel": "pypi",
                "target": "x86_64-unknown-linux-gnu",
                "status": "passed",
                "subject_count": 3,
                "package_hash": PACKAGE_HASH,
                "payload_hash": PAYLOAD_HASH,
                "executed_payload_hash": PAYLOAD_HASH,
                "steps": ["install_wheel", "run_installed_binary"],
                "limitations": [],
                "non_claims": []
            }
        ],
        "executed_steps": ["install_wheel", "run_installed_binary"],
        "elapsed": {
            "install_ms": 1200,
            "install_condition": "cold",
            "launcher_ms": 15
        },
        "cleanup": { "status": "passed" },
        "limitations": ["consumer journey is not executed by this receipt contract"],
        "non_claims": [
            "not a public registry publication",
            "not a support-tier promotion",
            "not a runtime mutation result",
            "not authorization to publish"
        ],
        "claim_boundary": CLAIM_BOUNDARY
    })
}

fn parse_value(value: &Value) -> Result<super::model::PackageQualificationReceipt, String> {
    parse_receipt(&value.to_string())
}

fn evaluate_value(value: &Value) -> Result<super::model::PackageQualificationGate, String> {
    let receipt = parse_value(value)?;
    Ok(evaluate(&GateInput {
        receipt: &receipt,
        expected: &ExpectedIdentity::default(),
        full_matrix_targets: &[
            "x86_64-unknown-linux-gnu".to_string(),
            "aarch64-unknown-linux-gnu".to_string(),
        ],
    }))
}

fn temp_dir(label: &str) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "ripr-package-qualification-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

#[test]
fn valid_receipt_round_trips_through_the_typed_parser() -> Result<(), String> {
    let receipt = parse_value(&valid_receipt_value())?;
    let rendered = render_receipt_json(&receipt)?;
    let again = parse_receipt(&rendered)?;
    assert_eq!(receipt, again);
    assert_eq!(receipt.kind, RECEIPT_KIND);
    assert_eq!(receipt.source.commit, COMMIT);
    assert_eq!(receipt.package.hash, PACKAGE_HASH);
    Ok(())
}

#[test]
fn passing_explicit_subset_gate_keeps_identity_and_counts() -> Result<(), String> {
    let gate = evaluate_value(&valid_receipt_value())?;
    assert_eq!(gate.verdict, GateVerdict::Passed);
    assert!(gate.failures.is_empty(), "{:#?}", gate.failures);
    assert_eq!(gate.source.commit, COMMIT);
    assert_eq!(gate.package.hash, PACKAGE_HASH);
    assert_eq!(gate.payload.hash, PAYLOAD_HASH);
    assert_eq!(gate.subjects.executed, 3);
    assert_eq!(gate.kind, GATE_KIND);
    match gate.selection_scope {
        SelectionScope::ExplicitSubset => Ok(()),
        SelectionScope::DeclaredFullMatrix => {
            Err("explicit subset rendered as a full matrix".to_string())
        }
    }
}

#[test]
fn json_and_markdown_share_the_same_gate_dto() -> Result<(), String> {
    let gate = evaluate_value(&valid_receipt_value())?;
    let json = render_gate_json(&gate)?;
    let markdown = render_gate_markdown(&gate);
    for needle in [
        COMMIT,
        TREE,
        PACKAGE_HASH,
        PAYLOAD_HASH,
        "passed",
        "explicit_subset",
        "not a public registry publication",
    ] {
        if !json.contains(needle) {
            return Err(format!("JSON missing {needle}: {json}"));
        }
        if !markdown.contains(needle) {
            return Err(format!("Markdown missing {needle}: {markdown}"));
        }
    }
    if !json.contains("\"selected\": 3") {
        return Err(format!("JSON missing selected count: {json}"));
    }
    if !markdown.contains("selected=3") {
        return Err(format!("Markdown missing selected count: {markdown}"));
    }
    let parsed: super::model::PackageQualificationGate = serde_json::from_str(&json)
        .map_err(|error| format!("gate JSON must parse as the same DTO: {error}"))?;
    assert_eq!(parsed, gate);
    Ok(())
}

fn require_err<T>(result: Result<T, String>, context: &str) -> Result<String, String> {
    match result {
        Ok(_) => Err(format!("{context}: expected a contract error")),
        Err(error) => Ok(error),
    }
}

#[test]
fn unknown_field_is_rejected() -> Result<(), String> {
    let mut value = valid_receipt_value();
    let object = value
        .as_object_mut()
        .ok_or_else(|| "receipt object".to_string())?;
    object.insert("token".to_string(), json!("unused"));
    let error = require_err(parse_value(&value), "unknown fields must fail closed")?;
    if error.contains("typed contract") {
        Ok(())
    } else {
        Err(format!(
            "unknown field must fail as a contract error, got {error}"
        ))
    }
}

#[test]
fn skipped_is_not_a_row_status() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["status"] = json!("skipped");
    let error = require_err(parse_value(&value), "skipped is not a valid status")?;
    if error.contains("typed contract") {
        Ok(())
    } else {
        Err(format!(
            "skipped must not be accepted as a row status, got {error}"
        ))
    }
}

#[test]
fn secret_material_cannot_enter_the_receipt() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["limitations"] = json!(["contact ghp_exampletoken for access"]);
    let error = require_err(parse_value(&value), "secrets must fail parse")?;
    if error.contains("credential or secret material") {
        Ok(())
    } else {
        Err(format!(
            "secret tripwire must name the refusal, got {error}"
        ))
    }
}

#[test]
fn pass_requires_exact_source_package_and_payload_identity() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["source"]["commit"] = json!("deadbeef");
    let error = require_err(parse_value(&value), "short SHA must fail")?;
    if !error.contains("source.commit") {
        return Err(error);
    }

    let mut value = valid_receipt_value();
    value["package"]["hash"] =
        json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    let error = require_err(parse_value(&value), "bare hash must fail")?;
    if error.contains("sha256:") {
        Ok(())
    } else {
        Err(error)
    }
}

#[test]
fn zero_subject_passed_row_cannot_parse() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["subject_count"] = json!(0);
    let error = require_err(parse_value(&value), "zero-subject pass must fail parse")?;
    if error.contains("zero-subject") {
        Ok(())
    } else {
        Err(error)
    }
}

#[test]
fn planted_executable_hash_cannot_turn_the_row_green() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["executed_payload_hash"] = json!(OTHER_HASH);
    let error = require_err(parse_value(&value), "planted payload must fail")?;
    if error.contains("planted or substituted executable") {
        Ok(())
    } else {
        Err(error)
    }
}

#[test]
fn passed_row_with_empty_steps_cannot_parse() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["steps"] = json!([]);
    let error = require_err(parse_value(&value), "empty steps must fail parse")?;
    if error.contains("steps must be nonempty") {
        Ok(())
    } else {
        Err(error)
    }
}

#[test]
fn passed_row_with_substituted_package_hash_cannot_parse() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["package_hash"] = json!(OTHER_HASH);
    let error = require_err(parse_value(&value), "row package hash must match")?;
    if error.contains("package_hash does not match package.hash") {
        Ok(())
    } else {
        Err(error)
    }
}

#[test]
fn missing_required_row_fails_the_aggregate() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["required_rows"] = json!([
        { "channel": "pypi", "target": "x86_64-unknown-linux-gnu" },
        { "channel": "pypi", "target": "aarch64-unknown-linux-gnu" }
    ]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("aarch64-unknown-linux-gnu")
                && failure.contains("missing")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn extra_passed_row_cannot_hide_a_missing_required_row() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["required_rows"] = json!([
        { "channel": "npm", "target": "x86_64-unknown-linux-gnu" }
    ]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("npm/x86_64-unknown-linux-gnu")
                && failure.contains("missing")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn not_run_required_row_fails_the_aggregate() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["status"] = json!("not_run");
    value["rows"][0]["subject_count"] = json!(0);
    value["rows"][0]["executed_payload_hash"] = Value::Null;
    value["rows"][0]["steps"] = json!([]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("not_run")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn planted_hash_on_a_not_run_required_row_still_fails_as_not_run() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["status"] = json!("not_run");
    value["rows"][0]["subject_count"] = json!(0);
    value["rows"][0]["executed_payload_hash"] = json!(OTHER_HASH);
    value["rows"][0]["steps"] = json!([]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    if !gate
        .failures
        .iter()
        .any(|failure| failure.contains("not_run"))
    {
        return Err(format!(
            "expected not_run failure, got {:#?}",
            gate.failures
        ));
    }
    if gate
        .failures
        .iter()
        .any(|failure| failure.contains("executed a different payload"))
    {
        return Err(
            "a not_run row must fail as not_run, not as a planted-payload pass check".to_string(),
        );
    }
    Ok(())
}

#[test]
fn unsupported_required_row_fails_the_aggregate() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["rows"][0]["status"] = json!("unsupported_by_contract");
    value["rows"][0]["subject_count"] = json!(0);
    value["rows"][0]["executed_payload_hash"] = Value::Null;
    value["rows"][0]["steps"] = json!([]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("unsupported_by_contract")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn receipt_level_zero_subjects_fail_the_aggregate() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["subjects"] = json!({
        "selected": 0,
        "executed": 0,
        "failed": 0,
        "skipped": 0
    });
    value["rows"][0]["status"] = json!("failed");
    value["rows"][0]["subject_count"] = json!(0);
    value["rows"][0]["executed_payload_hash"] = Value::Null;
    value["rows"][0]["steps"] = json!([]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("zero-subject receipt")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn stale_expected_commit_fails_even_when_the_receipt_is_internally_consistent() -> Result<(), String>
{
    let receipt = parse_value(&valid_receipt_value())?;
    let expected = ExpectedIdentity {
        commit: Some(STALE_COMMIT.to_string()),
        ..ExpectedIdentity::default()
    };
    let gate = evaluate(&GateInput {
        receipt: &receipt,
        expected: &expected,
        full_matrix_targets: &[],
    });
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("stale source commit")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn mismatched_expected_payload_fails_the_aggregate() -> Result<(), String> {
    let receipt = parse_value(&valid_receipt_value())?;
    let expected = ExpectedIdentity {
        payload_hash: Some(OTHER_HASH.to_string()),
        ..ExpectedIdentity::default()
    };
    let gate = evaluate(&GateInput {
        receipt: &receipt,
        expected: &expected,
        full_matrix_targets: &[],
    });
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("mismatched payload hash")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn declared_full_matrix_cannot_pass_with_a_partial_required_set() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["selection_scope"] = json!("declared_full_matrix");
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("complete channel/target matrix")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn declared_full_matrix_without_an_explicit_target_list_fails_closed() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["selection_scope"] = json!("declared_full_matrix");
    let receipt = parse_value(&value)?;
    let gate = evaluate(&GateInput {
        receipt: &receipt,
        expected: &ExpectedIdentity::default(),
        full_matrix_targets: &[],
    });
    assert_eq!(gate.verdict, GateVerdict::Failed);
    if !gate
        .failures
        .iter()
        .any(|failure| failure.contains("complete channel/target matrix"))
    {
        return Err(format!(
            "missing matrix-coverage failure: {:#?}",
            gate.failures
        ));
    }
    if !gate
        .failures
        .iter()
        .any(|failure| failure.contains("without an explicit target matrix"))
    {
        return Err(format!(
            "missing empty-matrix failure: {:#?}",
            gate.failures
        ));
    }
    Ok(())
}

#[test]
fn declared_full_matrix_passes_only_when_every_required_target_is_present() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["selection_scope"] = json!("declared_full_matrix");
    value["required_rows"] = json!([
        { "channel": "pypi", "target": "x86_64-unknown-linux-gnu" },
        { "channel": "pypi", "target": "aarch64-unknown-linux-gnu" }
    ]);
    value["rows"] = json!([
        {
            "channel": "pypi",
            "target": "x86_64-unknown-linux-gnu",
            "status": "passed",
            "subject_count": 2,
            "package_hash": PACKAGE_HASH,
            "payload_hash": PAYLOAD_HASH,
            "executed_payload_hash": PAYLOAD_HASH,
            "steps": ["install_wheel"],
            "limitations": [],
            "non_claims": []
        },
        {
            "channel": "pypi",
            "target": "aarch64-unknown-linux-gnu",
            "status": "passed",
            "subject_count": 1,
            "package_hash": PACKAGE_HASH,
            "payload_hash": PAYLOAD_HASH,
            "executed_payload_hash": PAYLOAD_HASH,
            "steps": ["install_wheel"],
            "limitations": [],
            "non_claims": []
        }
    ]);
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Passed, "{:#?}", gate.failures);
    match gate.selection_scope {
        SelectionScope::DeclaredFullMatrix => Ok(()),
        SelectionScope::ExplicitSubset => {
            Err("full matrix rendered as an explicit subset".to_string())
        }
    }
}

#[test]
fn cleanup_not_run_fails_the_aggregate() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["cleanup"]["status"] = json!("not_run");
    let gate = evaluate_value(&value)?;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert!(
        gate.failures
            .iter()
            .any(|failure| failure.contains("cleanup")),
        "{:#?}",
        gate.failures
    );
    Ok(())
}

#[test]
fn command_path_writes_shared_json_and_markdown_and_fails_closed_on_stale_sha() -> Result<(), String>
{
    let dir = temp_dir("command")?;
    let receipt_path = dir.join("receipt.json");
    std::fs::write(&receipt_path, valid_receipt_value().to_string())
        .map_err(|error| error.to_string())?;
    let out = dir.join("out");
    let (_receipt, passed) = evaluate_path(&CommandOptions {
        receipt: receipt_path.clone(),
        expected: ExpectedIdentity::default(),
        out: out.clone(),
        full_matrix_targets: Some(vec!["x86_64-unknown-linux-gnu".to_string()]),
    })?;
    assert_eq!(passed.verdict, GateVerdict::Passed);

    let (_receipt, failed) = evaluate_path(&CommandOptions {
        receipt: receipt_path,
        expected: ExpectedIdentity {
            commit: Some(STALE_COMMIT.to_string()),
            ..ExpectedIdentity::default()
        },
        out,
        full_matrix_targets: Some(vec!["x86_64-unknown-linux-gnu".to_string()]),
    })?;
    assert_eq!(failed.verdict, GateVerdict::Failed);
    assert!(
        failed
            .failures
            .iter()
            .any(|failure| failure.contains("stale source commit")),
        "{:#?}",
        failed.failures
    );
    Ok(())
}

#[test]
fn standing_non_claims_are_mandatory() -> Result<(), String> {
    let mut value = valid_receipt_value();
    value["non_claims"] = json!(["not a public registry publication"]);
    let error = require_err(parse_value(&value), "partial non-claims must fail")?;
    if error.contains("not a support-tier promotion") {
        Ok(())
    } else {
        Err(error)
    }
}

#[test]
fn command_run_writes_shared_reports_and_fails_closed_on_stale_sha() -> Result<(), String> {
    let dir = temp_dir("run")?;
    let receipt_path = dir.join("receipt.json");
    std::fs::write(&receipt_path, valid_receipt_value().to_string())
        .map_err(|error| error.to_string())?;
    let out = dir.join("out");
    super::run(&[
        "--receipt".into(),
        receipt_path.to_string_lossy().into_owned(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
    ])?;
    let json_path = out.join("package-qualification-gate.json");
    let md_path = out.join("package-qualification-gate.md");
    let json = std::fs::read_to_string(&json_path).map_err(|error| error.to_string())?;
    let markdown = std::fs::read_to_string(&md_path).map_err(|error| error.to_string())?;
    if !json.contains(COMMIT) || !markdown.contains(PACKAGE_HASH) {
        return Err("command reports must render the same identity".to_string());
    }
    let error = require_err(
        super::run(&[
            "--receipt".into(),
            receipt_path.to_string_lossy().into_owned(),
            "--expected-commit".into(),
            STALE_COMMIT.into(),
            "--out".into(),
            out.to_string_lossy().into_owned(),
        ]),
        "stale SHA must fail the command",
    )?;
    if error.contains("stale source commit") {
        Ok(())
    } else {
        Err(error)
    }
}
