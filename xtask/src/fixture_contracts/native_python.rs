//! Native test understanding is a separate authority from analyzer verdicts.

use super::retained_files::{read_json, verify_file};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

const FIXED: &str = "c30678d19e37011890e2374cca04f7789e101793";
const BROKEN: &str = "1a9b8d1f9968eb248bda69f39d373a6330b692ba";

pub(super) fn validate_native_case_registration(root: &Path) -> Result<(), String> {
    let corpus = read_json(&root.join("corpus.json"))?;
    let cases = corpus["native_cases"]
        .as_array()
        .ok_or("missing native corpus bucket")?;
    let case = cases
        .iter()
        .find(|case| case["id"] == "itsdangerous_future_age_native_test_understanding")
        .ok_or("missing ItsDangerous native registration")?;
    if case["analyzer_execution"] != "not_established"
        || case["metrics_eligible"] != false
        || case["source_kind"] != "external_repo"
        || case["manifest"] != "itsdangerous-future-age/manifest.json"
    {
        return Err("native registration must remain outside analyzer metrics".to_string());
    }
    validate_itsdangerous(&root.join("itsdangerous-future-age"))
}

pub(super) fn validate_itsdangerous(root: &Path) -> Result<(), String> {
    let manifest = read_json(&root.join("manifest.json"))?;
    if manifest["fixed"] != FIXED
        || manifest["broken"] != BROKEN
        || manifest["license"] != "BSD-3-Clause"
        || manifest["analyzer_execution"] != "not_established"
    {
        return Err("ItsDangerous case lost its pins, license or analyzer boundary".to_string());
    }
    let mut names = BTreeSet::new();
    for entry in manifest["files"]
        .as_array()
        .ok_or("missing native artifact inventory")?
    {
        verify_file(root, entry)?;
        if !names.insert(entry["path"].as_str().unwrap_or_default()) {
            return Err("duplicate native artifact identity".to_string());
        }
    }
    for required in [
        "input/LICENSE.rst",
        "input/src/itsdangerous/timed.py",
        "input/tests/test_itsdangerous/test_timed.py",
        "input/tests/test_itsdangerous/test_signer.py",
        "input/tests/test_itsdangerous/test_serializer.py",
        "upstream/broken-timed.py",
        "retained-files.json",
        "oracle.json",
        "replay.py",
        "requirements.lock",
        "evidence/native.json",
    ] {
        if !names.contains(required) {
            return Err(format!("native case inventory lacks {required}"));
        }
    }
    let retained = read_json(&root.join("retained-files.json"))?;
    let files = retained
        .as_array()
        .ok_or("missing retained source inventory")?;
    if files.len() != 17 {
        return Err("ItsDangerous must retain all 17 selected upstream files".to_string());
    }
    for entry in files {
        verify_file(&root.join("input"), entry)?;
        let path = entry["path"].as_str().unwrap_or_default();
        if !names.contains(format!("input/{path}").as_str()) {
            return Err(format!(
                "native artifact inventory omits upstream file {path}"
            ));
        }
    }
    let receipt = read_json(&root.join("evidence/native.json"))?;
    if receipt["fixed"] != FIXED
        || receipt["broken"] != BROKEN
        || receipt["analyzer_execution"] != "not_established"
    {
        return Err("native receipt changed its subject or analyzer boundary".to_string());
    }
    let rows = receipt["rows"]
        .as_array()
        .ok_or("missing native observations")?;
    let lock_text =
        fs::read_to_string(root.join("requirements.lock")).map_err(|error| error.to_string())?;
    validate_environment(&receipt, &lock_text)?;
    if rows.len() != 13 {
        return Err("native receipt needs all 13 control rows".to_string());
    }
    let driver = hash_file(&root.join("replay.py"))?;
    let lock = hash_file(&root.join("requirements.lock"))?;
    let mut subjects = BTreeSet::new();
    for row in rows {
        validate_row(row)?;
        let implementation = row["implementation"].as_str().unwrap_or_default();
        let variant = row["variant"].as_str().unwrap_or_default();
        if !subjects.insert((implementation, variant)) {
            return Err("duplicate native control row".to_string());
        }
        let production = if implementation == "fixed" {
            root.join("input/src/itsdangerous/timed.py")
        } else {
            root.join("upstream/broken-timed.py")
        };
        if row["production_sha256"].as_str() != Some(hash_file(&production)?.as_str())
            || row["driver_sha256"].as_str() != Some(driver.as_str())
            || row["requirements_lock_sha256"].as_str() != Some(lock.as_str())
        {
            return Err(format!(
                "stale native source/driver/lock identity: {implementation}/{variant}"
            ));
        }
        let original = fs::read_to_string(root.join("input/tests/test_itsdangerous/test_timed.py"))
            .map_err(|error| error.to_string())?;
        let marker = "    def test_future_age(self, signer):";
        let test = match variant {
            "weak" => original.replace(
                "            with pytest.raises(SignatureExpired):\n                signer.unsign(signed, max_age=10)",
                "            assert signer.unsign(signed) == b\"value\"",
            ),
            "skip-method" => original.replace(marker, &format!("    @pytest.mark.skip(reason='activation control')\n{marker}")),
            "xfail" => original.replace(marker, &format!("    @pytest.mark.xfail(strict=False, reason='activation control')\n{marker}")),
            "skip-class" => original.replace("class TestTimestampSigner(",
                "@pytest.mark.skip(reason='activation control')\nclass TestTimestampSigner("),
            _ => original,
        };
        if row["test_sha256"].as_str()
            != Some(format!("{:x}", Sha256::digest(test.as_bytes())).as_str())
        {
            return Err(format!(
                "stale native test variant: {implementation}/{variant}"
            ));
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let raw = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(raw)))
}

fn validate_environment(receipt: &Value, lock: &str) -> Result<(), String> {
    let env = &receipt["environment"];
    if env["implementation"] != "CPython"
        || env["system"] != "Windows"
        || env["pointer_bits"] != 64
        || !env["python"]
            .as_str()
            .unwrap_or_default()
            .starts_with("3.14.")
        || env["python"] != receipt["python"]
    {
        return Err("native runner identity must be CPython 3.14 on Windows x64".to_string());
    }
    let mut expected = BTreeSet::new();
    for line in lock
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (name, version) = line.split_once("==").ok_or("invalid native lock entry")?;
        let version = version
            .split_whitespace()
            .next()
            .ok_or("empty native lock version")?;
        expected.insert(name);
        if env["distributions"][name].as_str() != Some(version) {
            return Err(format!("native installed {name} must match the lock"));
        }
    }
    if env["distributions"].as_object().map(|values| values.len()) != Some(expected.len()) {
        return Err("native runner must report every locked distribution".to_string());
    }
    for module in ["pytest", "freezegun"] {
        if env["imported_modules"][module]["version"] != env["distributions"][module]
            || env["imported_modules"][module]["init_sha256"]
                .as_str()
                .is_none_or(|value| {
                    value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
        {
            return Err(format!("native imported {module} identity is missing"));
        }
    }
    Ok(())
}

fn validate_row(row: &Value) -> Result<(), String> {
    let implementation = row["implementation"].as_str().unwrap_or_default();
    let variant = row["variant"].as_str().unwrap_or_default();
    if !matches!(implementation, "fixed" | "broken") {
        return Err("unknown native implementation".to_string());
    }
    let tests = match variant {
        "effective" | "weak" | "skip-method" | "skip-class" | "xfail" => 1,
        "neighbors" => 2,
        "full-file" if implementation == "fixed" => 97,
        _ => return Err("unknown native control variant".to_string()),
    };
    let intended_red = implementation == "broken" && variant == "effective";
    let suppressed = implementation == "broken" && variant == "xfail";
    let disabled = matches!(variant, "skip-method" | "skip-class");
    for (field, expected) in [
        ("registered", tests),
        ("executed", if disabled { 0 } else { tests }),
        ("failures", u64::from(intended_red)),
        ("exit_code", u64::from(intended_red)),
        ("errors", 0),
        ("skipped", u64::from(disabled || suppressed)),
    ] {
        if row[field].as_u64() != Some(expected) {
            return Err(format!(
                "{implementation}/{variant}: {field} must be {expected}"
            ));
        }
    }
    if intended_red && row["intended_failure"].as_bool() != Some(true) {
        return Err("native red must fail at the SignatureExpired assertion".to_string());
    }
    let ids = row["test_ids"]
        .as_array()
        .ok_or("missing native subjects")?;
    let xml = row["junit_xml"].as_str().ok_or("missing native JUnit")?;
    if intended_red
        && (!xml.contains("<failure message=\"Failed: DID NOT RAISE SignatureExpired\">")
            || !xml.contains("with pytest.raises(SignatureExpired):")
            || !xml.contains("tests\\test_itsdangerous\\test_timed.py:71: Failed</failure>"))
    {
        return Err(
            "native red failed outside the original SignatureExpired assertion".to_string(),
        );
    }
    if ids.len() as u64 != tests
        || xml.matches("<testcase ").count() as u64 != tests
        || xml.matches("<failure ").count() as u64 != u64::from(intended_red)
        || xml.matches("<skipped ").count() as u64 != u64::from(disabled || suppressed)
        || xml.contains("<error")
        || !xml.ends_with("</testsuites>")
    {
        return Err("native JUnit contradicts nonempty observations".to_string());
    }
    for (field, expected) in [
        ("tests", tests),
        ("errors", 0),
        ("failures", u64::from(intended_red)),
        ("skipped", u64::from(disabled || suppressed)),
    ] {
        if !xml.contains(&format!("{field}=\"{expected}\"")) {
            return Err(format!("native JUnit lacks {field}={expected}"));
        }
    }
    let mut subjects = BTreeSet::new();
    for id in ids {
        let class = id["classname"].as_str().unwrap_or_default();
        let name = id["name"].as_str().unwrap_or_default();
        let needle = format!(
            "classname=\"{}\" name=\"{}\"",
            xml_attribute(class),
            xml_attribute(name)
        );
        if !subjects.insert((class, name))
            || !class.starts_with("tests.test_itsdangerous.test_timed.")
            || !name.starts_with("test_")
            || !xml.contains(&needle)
        {
            return Err(format!("wrong native subject identity: {class}/{name}"));
        }
        if tests <= 2 && class != "tests.test_itsdangerous.test_timed.TestTimestampSigner" {
            return Err("native control selected the wrong upstream class".to_string());
        }
        if tests == 1 && name != "test_future_age" {
            return Err("native control must select the upstream future-age test".to_string());
        }
        if suppressed && id["reason"] != "pytest.xfail" {
            return Err("suppressed assertion must remain an expected failure".to_string());
        }
        let start = xml.find(&needle).ok_or("missing native test identity")?;
        let rest = &xml[start..];
        let case = rest.split("<testcase ").next().unwrap_or(rest);
        let outcome = if case.contains("<failure ") {
            "failure"
        } else if case.contains("<skipped ") {
            "skipped"
        } else {
            "pass"
        };
        if id["outcome"] != outcome {
            return Err("native subject outcome contradicts JUnit".to_string());
        }
    }
    if tests == 2
        && subjects
            .iter()
            .map(|(_, name)| *name)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from(["test_max_age", "test_return_timestamp"])
    {
        return Err("native neighbors must select original expiry and timestamp tests".to_string());
    }
    Ok(())
}

// The receipt stores decoded subjects alongside the retained pytest JUnit XML.
// Bind identities to that XML's double-quoted attribute serialization.
fn xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\t', "&#09;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_native_test_matrix_is_bound() -> Result<(), String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/python-real-repo-evals/itsdangerous-future-age");
        validate_itsdangerous(&root)
    }
    #[test]
    fn parameterized_junit_names_bind_to_decoded_subjects() -> Result<(), String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/python-real-repo-evals/itsdangerous-future-age");
        let receipt = read_json(&root.join("evidence/native.json"))?;
        let row = receipt["rows"]
            .as_array()
            .ok_or("missing native rows")?
            .iter()
            .find(|row| row["variant"] == "full-file")
            .ok_or("missing native full-file row")?;
        validate_row(row)?;
        let mut changed = row.clone();
        let subject = changed["test_ids"]
            .as_array_mut()
            .ok_or("missing native subjects")?
            .iter_mut()
            .find(|id| {
                id["name"]
                    .as_str()
                    .is_some_and(|name| name.contains("<lambda>"))
            })
            .ok_or("missing upstream parameterized subject")?;
        subject["name"] = serde_json::json!(
            subject["name"]
                .as_str()
                .ok_or("missing subject name")?
                .replace('<', "&lt;")
        );
        assert!(
            validate_row(&changed).is_err(),
            "encoded text is a different decoded subject"
        );
        Ok(())
    }
    #[test]
    fn zero_setup_error_wrong_subject_and_suppressed_failure_are_not_passes() -> Result<(), String>
    {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/python-real-repo-evals/itsdangerous-future-age");
        let receipt = read_json(&root.join("evidence/native.json"))?;
        let rows = receipt["rows"].as_array().ok_or("missing native rows")?;
        for (field, bad) in [
            ("registered", serde_json::json!(0)),
            ("errors", serde_json::json!(1)),
            ("exit_code", serde_json::json!(0)),
            ("intended_failure", serde_json::json!(false)),
        ] {
            let mut row = rows
                .iter()
                .find(|row| row["implementation"] == "broken" && row["variant"] == "effective")
                .ok_or("missing intended red")?
                .clone();
            row[field] = bad;
            assert!(validate_row(&row).is_err(), "{field}");
        }
        let mut row = rows[0].clone();
        row["test_ids"][0]["name"] = serde_json::json!("test_unrelated");
        assert!(validate_row(&row).is_err());
        let mut row = rows
            .iter()
            .find(|row| row["implementation"] == "broken" && row["variant"] == "xfail")
            .ok_or("missing expected-failure control")?
            .clone();
        row["skipped"] = serde_json::json!(0);
        assert!(validate_row(&row).is_err());
        let red = rows
            .iter()
            .find(|row| row["implementation"] == "broken" && row["variant"] == "effective")
            .ok_or("missing intended red")?;
        let mut row = red.clone();
        row["junit_xml"] =
            serde_json::json!(row["junit_xml"].as_str().ok_or("missing JUnit")?.replace(
                "DID NOT RAISE SignatureExpired",
                "unrelated assertion failure"
            ));
        assert!(validate_row(&row).is_err(), "wrong red payload");
        let mut row = rows
            .iter()
            .find(|row| row["variant"] == "neighbors")
            .ok_or("missing neighbors")?
            .clone();
        row["test_ids"][0]["name"] = serde_json::json!("test_orthogonal");
        assert!(validate_row(&row).is_err(), "wrong neighbor");
        let mut row = rows[0].clone();
        row["test_ids"][0]["outcome"] = serde_json::json!("failure");
        assert!(validate_row(&row).is_err(), "wrong subject outcome");
        let lock = fs::read_to_string(root.join("requirements.lock"))
            .map_err(|error| error.to_string())?;
        let mut stale_environment = receipt.clone();
        stale_environment["environment"]["distributions"]["pytest"] = serde_json::json!("0.0");
        assert!(validate_environment(&stale_environment, &lock).is_err());
        Ok(())
    }
}
