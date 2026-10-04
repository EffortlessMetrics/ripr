//! Retained upstream Python evidence, checked by the existing corpus route.
//! These are historical execution records, not current analyzer goldens.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::retained_files::{local_path, read_json, verify_file};

const VARIANTS: &[&str] = &[
    "fixed_full",
    "broken_full",
    "broken_assertion_removed",
    "fixed_assertion_removed",
    "fixed_upstream_file",
    "fixed_positive_controls",
    "broken_positive_controls",
];

fn validate_observation(row: &Value) -> Result<(), String> {
    let variant = row["variant"].as_str().unwrap_or_default();
    let tests = match variant {
        "fixed_full" | "broken_full" | "broken_assertion_removed" | "fixed_assertion_removed" => 1,
        "fixed_upstream_file" => 5,
        "fixed_positive_controls" | "broken_positive_controls" => 3,
        _ => return Err(format!("unknown upstream oracle variant: {variant}")),
    };
    let failures = u64::from(variant == "broken_full");
    for (field, expected) in [
        ("selected_and_executed", tests),
        ("failures", failures),
        ("exit_code", failures),
        ("errors", 0),
        ("skipped", 0),
    ] {
        if row[field].as_u64() != Some(expected) {
            return Err(format!("{variant}: {field} must be {expected}"));
        }
    }
    let ids = row["test_ids"]
        .as_array()
        .ok_or_else(|| format!("{variant}: missing executed test identities"))?;
    if ids.len() as u64 != tests {
        return Err(format!("{variant}: executed test identity count mismatch"));
    }
    let (classname, expected_names): (&str, Vec<String>) = if variant.ends_with("positive_controls")
    {
        (
            "tests.sansio.test_cookie_controls",
            (0..3)
                .map(|index| {
                    format!(
                        "test_single_cookie_and_unrelated_headers[headers{index}-expected{index}]"
                    )
                })
                .collect(),
        )
    } else if variant == "fixed_upstream_file" {
        (
            "tests.sansio.test_request",
            [
                "test_content_length[headers0-None]",
                "test_content_length[headers1-6]",
                "test_content_length[headers2-6]",
                "test_content_length[headers3-None]",
                "test_cookies",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        )
    } else {
        (
            "tests.sansio.test_request",
            vec!["test_cookies".to_string()],
        )
    };
    let xml = row["junit_xml"].as_str().unwrap_or_default();
    if !xml.contains("<testsuites ")
        || !xml.ends_with("</testsuites>")
        || xml.matches("<testsuite ").count() != 1
        || xml.matches("<testcase ").count() as u64 != tests
        || xml.matches("<failure ").count() as u64 != failures
        || xml.contains("<error")
        || xml.contains("<skipped")
    {
        return Err(format!(
            "{variant}: missing or contradictory JUnit subjects"
        ));
    }
    for (id, expected_name) in ids.iter().zip(&expected_names) {
        if id["classname"].as_str() != Some(classname)
            || id["name"].as_str() != Some(expected_name.as_str())
            || !xml.contains(&format!(
                "classname=\"{classname}\" name=\"{expected_name}\""
            ))
        {
            return Err(format!("{variant}: wrong executed test identity"));
        }
    }
    for (field, expected) in [
        ("tests", tests),
        ("failures", failures),
        ("errors", 0),
        ("skipped", 0),
    ] {
        if !xml.contains(&format!("{field}=\"{expected}\"")) {
            return Err(format!("{variant}: JUnit lacks {field}={expected}"));
        }
    }
    if variant == "broken_full" {
        let stdout = row["stdout"].as_str().unwrap_or_default();
        if !stdout.contains("assert req.cookies.getlist(\"a\") == [\"b\", \"c\"]")
            || !stdout.contains("['b'] == ['b', 'c']")
            || !stdout.contains("AssertionError")
        {
            return Err("broken_full: missing intended list-assertion failure".to_string());
        }
    }
    Ok(())
}

fn semantic_identity(row: &Value) -> Vec<Value> {
    let tests: Vec<Value> = row["test_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|test| serde_json::json!({"classname": test["classname"], "name": test["name"]}))
        .collect();
    vec![
        row["production_sha256"].clone(),
        row["test_sha256"].clone(),
        row["conftest_sha256"].clone(),
        Value::Array(tests),
    ]
}

fn verify_observation_sources(root: &Path, row: &Value) -> Result<(), String> {
    let variant = row["variant"].as_str().unwrap_or_default();
    let source_path = root.join("input/src/werkzeug/sansio/request.py");
    let test_path = root.join("input/tests/sansio/test_request.py");
    let mut source = fs::read_to_string(&source_path).map_err(|error| error.to_string())?;
    let mut test = fs::read_to_string(&test_path).map_err(|error| error.to_string())?;
    if variant.starts_with("broken") {
        let fixed = "        wsgi_combined_cookie = \";\".join(self.headers.getlist(\"Cookie\"))\n";
        let argument = "            wsgi_combined_cookie,\n";
        if source.matches(fixed).count() != 1 || source.matches(argument).count() != 1 {
            return Err("upstream source no longer contains the exact cookie fix".to_string());
        }
        source = source
            .replace(fixed, "")
            .replace(argument, "            self.headers.get(\"Cookie\"),\n");
    }
    if variant.ends_with("assertion_removed") {
        let assertion = "    assert req.cookies.getlist(\"a\") == [\"b\", \"c\"]\n";
        if test.matches(assertion).count() != 1 {
            return Err("upstream test no longer contains the exact list assertion".to_string());
        }
        test = test.replace(assertion, "");
    }
    for (field, bytes) in [
        ("production_sha256", source.into_bytes()),
        ("test_sha256", test.into_bytes()),
        (
            "conftest_sha256",
            fs::read(root.join("input/tests/conftest.py")).map_err(|error| error.to_string())?,
        ),
        (
            "driver_sha256",
            fs::read(root.join("test_replay.py")).map_err(|error| error.to_string())?,
        ),
        (
            "requirements_lock_sha256",
            fs::read(root.join("requirements.lock")).map_err(|error| error.to_string())?,
        ),
    ] {
        let digest = format!("{:x}", Sha256::digest(bytes));
        if row[field].as_str() != Some(digest.as_str()) {
            return Err(format!("{variant}: stale observation {field}"));
        }
    }
    Ok(())
}

fn validate_manifest(root: &Path, manifest: &Value) -> Result<(), String> {
    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| "upstream evidence manifest lacks files".to_string())?;
    let mut names = BTreeSet::new();
    for entry in files {
        verify_file(root, entry)?;
        if !names.insert(entry["path"].as_str().unwrap_or_default()) {
            return Err("duplicate upstream evidence file".to_string());
        }
    }
    for required in [
        "input/LICENSE.rst",
        "input/src/werkzeug/sansio/request.py",
        "input/tests/sansio/test_request.py",
        "input/tests/conftest.py",
        "upstream/fix.patch",
        "upstream/retained-files.json",
        "production.patch",
        "oracle.json",
        "test_replay.py",
        "requirements.lock",
        "evidence/setup.json",
        "evidence/effective-check.json",
        "evidence/ineffective-check.json",
    ] {
        if !names.contains(required) {
            return Err(format!("upstream evidence manifest lacks {required}"));
        }
    }
    let retained = read_json(&root.join("upstream/retained-files.json"))?;
    let retained_files = retained
        .as_array()
        .ok_or_else(|| "missing retained upstream file inventory".to_string())?;
    if retained_files.len() != 33 {
        return Err("Werkzeug retained upstream inventory must contain 33 files".to_string());
    }
    let mut retained_names = BTreeSet::new();
    for entry in retained_files {
        verify_file(&root.join("input"), entry)?;
        let relative = entry["path"].as_str().unwrap_or_default();
        if !retained_names.insert(relative) || !names.contains(format!("input/{relative}").as_str())
        {
            return Err(format!(
                "missing or duplicate retained upstream module: {relative}"
            ));
        }
    }
    let mut original = BTreeMap::new();
    for scope in ["original", "retained"] {
        for variant in VARIANTS {
            let path = format!("evidence/{scope}/{variant}.json");
            if !names.contains(path.as_str()) {
                return Err(format!("upstream evidence manifest lacks {path}"));
            }
            let row = read_json(&root.join(path))?;
            if row["variant"].as_str() != Some(*variant) {
                return Err(format!("{scope}: wrong variant identity for {variant}"));
            }
            validate_observation(&row)?;
            verify_observation_sources(root, &row)?;
            let identity = semantic_identity(&row);
            if scope == "original" {
                original.insert(*variant, identity);
            } else if original.get(variant) != Some(&identity) {
                return Err(format!(
                    "{variant}: retained source changed the original oracle identity"
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_werkzeug_upstream_evidence(corpus: &Path) -> Result<(), String> {
    let data = read_json(corpus)?;
    let root = corpus
        .parent()
        .ok_or_else(|| "Python corpus has no parent".to_string())?;
    let rows = data["static_limit_cases"]
        .as_array()
        .ok_or_else(|| "Python corpus lacks static-limit records".to_string())?;
    let row = rows
        .iter()
        .find(|row| {
            row["id"].as_str() == Some("werkzeug_multiple_cookie_headers_2065_historical_limit")
        })
        .ok_or_else(|| "Python corpus lacks the retained Werkzeug upstream case".to_string())?;
    if row["source_kind"].as_str() != Some("external_repo") {
        return Err("Werkzeug must retain its external_repo provenance".to_string());
    }
    let evidence = &row["upstream_evidence"];
    let path = evidence["manifest"]
        .as_str()
        .filter(|path| local_path(path))
        .ok_or_else(|| "Werkzeug case lacks contained upstream manifest".to_string())?;
    let manifest_path = root.join(path);
    let bytes = fs::read(&manifest_path).map_err(|error| format!("{path}: {error}"))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if evidence["sha256"].as_str() != Some(digest.as_str()) {
        return Err(format!("upstream manifest digest mismatch: {path}"));
    }
    let manifest = read_json(&manifest_path)?;
    let case_root = manifest_path
        .parent()
        .ok_or_else(|| "upstream manifest has no parent".to_string())?;
    validate_manifest(case_root, &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/python-real-repo-evals/werkzeug-multiple-cookie")
    }

    #[test]
    fn upstream_python_retained_evidence_is_bound() -> Result<(), String> {
        validate_werkzeug_upstream_evidence(&case_root().join("../corpus.json"))
    }

    #[test]
    fn upstream_python_rejects_zero_setup_failure_and_wrong_oracle() -> Result<(), String> {
        let original = read_json(&case_root().join("evidence/retained/broken_full.json"))?;
        for (key, value) in [
            ("selected_and_executed", Value::from(0)),
            ("errors", Value::from(1)),
            ("skipped", Value::from(1)),
            ("exit_code", Value::from(0)),
            ("stdout", Value::from("ImportError: no xprocess")),
            ("test_ids", serde_json::json!([{}])),
            (
                "junit_xml",
                Value::from("tests=\"1\" failures=\"1\" errors=\"0\" skipped=\"0\""),
            ),
        ] {
            let mut wrong = original.clone();
            wrong[key] = value;
            assert!(validate_observation(&wrong).is_err(), "accepted {key}");
        }
        Ok(())
    }

    #[test]
    fn upstream_python_ignores_volatile_test_timing() -> Result<(), String> {
        let original = read_json(&case_root().join("evidence/retained/broken_full.json"))?;
        let mut later = original.clone();
        later["test_ids"][0]["time"] = Value::from("123.456");
        assert_eq!(semantic_identity(&original), semantic_identity(&later));
        later["test_ids"][0]["name"] = Value::from("wrong_test");
        assert_ne!(semantic_identity(&original), semantic_identity(&later));
        later = original;
        later["test_sha256"] = Value::from("stale");
        assert!(verify_observation_sources(&case_root(), &later).is_err());
        Ok(())
    }

    #[test]
    fn upstream_python_rejects_omitted_dependency_module() -> Result<(), String> {
        let mut manifest = read_json(&case_root().join("manifest.json"))?;
        let files = manifest["files"]
            .as_array_mut()
            .ok_or_else(|| "missing fixture inventory".to_string())?;
        files.retain(|entry| entry["path"].as_str() != Some("input/src/werkzeug/http.py"));
        assert!(validate_manifest(&case_root(), &manifest).is_err());
        Ok(())
    }

    #[test]
    fn upstream_python_rejects_digest_drift_and_escaping_paths() -> Result<(), String> {
        let manifest = read_json(&case_root().join("manifest.json"))?;
        let mut entry = manifest["files"][0].clone();
        entry["sha256"] = Value::from("wrong");
        assert!(verify_file(&case_root(), &entry).is_err());
        for path in ["../elsewhere", "/absolute", "a/../b", "a\\b", ""] {
            assert!(!local_path(path), "accepted {path}");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn upstream_python_manifest_preserves_relocation_and_refuses_outside_links()
    -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let sandbox = crate::tests::temp_dir("upstream-python-contained-evidence");
        let result = (|| {
            for mode in ["file", "parent"] {
                let root = sandbox.join(mode);
                crate::tests::copy_dir_recursive(&case_root(), &root)?;
                let manifest = read_json(&root.join("manifest.json"))?;
                validate_manifest(&root, &manifest)?;
                let relocated = sandbox.join(format!("{mode}-relocated"));
                fs::rename(&root, &relocated).map_err(|error| error.to_string())?;
                validate_manifest(&relocated, &manifest)?;
                let outside = sandbox.join(format!("{mode}-outside"));
                fs::create_dir(&outside).map_err(|error| error.to_string())?;
                let upstream = relocated.join("upstream");
                if mode == "file" {
                    fs::rename(upstream.join("fix.patch"), outside.join("fix.patch"))
                        .map_err(|error| error.to_string())?;
                    symlink(outside.join("fix.patch"), upstream.join("fix.patch"))
                        .map_err(|error| error.to_string())?;
                } else {
                    fs::rename(&upstream, outside.join("upstream"))
                        .map_err(|error| error.to_string())?;
                    symlink(outside.join("upstream"), &upstream)
                        .map_err(|error| error.to_string())?;
                }
                let error = validate_manifest(&relocated, &manifest)
                    .err()
                    .ok_or_else(|| format!("accepted Python evidence outside {mode} link"))?;
                assert!(error.contains("escapes fixture root"), "{mode}: {error}");
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&sandbox).map_err(|error| error.to_string());
        result.and(cleanup)
    }
}
