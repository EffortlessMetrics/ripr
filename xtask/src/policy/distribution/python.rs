use std::collections::BTreeSet;

use toml::Value;

use super::DistributionContract;

const MATURIN_REQUIREMENT: &str = "maturin==1.14.1";
const REQUIRES_PYTHON: &str = ">=3.10";
const PACKAGE_README_CONTENT_TYPE: &str = "text/markdown";
const LICENSE_EXPRESSION: &str = "MIT OR Apache-2.0";

pub(super) struct PythonAdapterSources<'a> {
    pub(super) manifest_path: &'a str,
    pub(super) manifest_text: &'a str,
    pub(super) readme_path: &'a str,
    pub(super) readme_text: &'a str,
    pub(super) packaged_mit_path: &'a str,
    pub(super) packaged_mit_text: &'a str,
    pub(super) root_mit_path: &'a str,
    pub(super) root_mit_text: &'a str,
    pub(super) packaged_apache_path: &'a str,
    pub(super) packaged_apache_text: &'a str,
    pub(super) root_apache_path: &'a str,
    pub(super) root_apache_text: &'a str,
}

pub(super) fn validate_python_adapter(
    contract: &DistributionContract,
    sources: PythonAdapterSources<'_>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let value: Value = match toml::from_str(sources.manifest_text) {
        Ok(value) => value,
        Err(err) => {
            violations.push(format!(
                "{}: invalid Python package manifest: {err}",
                sources.manifest_path
            ));
            return violations;
        }
    };

    check_string(
        sources.manifest_path,
        &value,
        &["build-system", "build-backend"],
        "maturin",
        &mut violations,
    );
    check_exact_string_array(
        sources.manifest_path,
        &value,
        &["build-system", "requires"],
        &[MATURIN_REQUIREMENT],
        &mut violations,
    );
    check_string(
        sources.manifest_path,
        &value,
        &["project", "name"],
        &contract.python.distribution,
        &mut violations,
    );
    check_exact_string_array(
        sources.manifest_path,
        &value,
        &["project", "dynamic"],
        &["version"],
        &mut violations,
    );
    if value_at(&value, &["project", "version"]).is_some() {
        violations.push(format!(
            "{}: project.version must be absent; Cargo.toml remains the sole product version source",
            sources.manifest_path
        ));
    }

    check_string(
        sources.manifest_path,
        &value,
        &["project", "requires-python"],
        REQUIRES_PYTHON,
        &mut violations,
    );
    check_string(
        sources.manifest_path,
        &value,
        &["project", "license"],
        LICENSE_EXPRESSION,
        &mut violations,
    );
    check_exact_string_array(
        sources.manifest_path,
        &value,
        &["project", "license-files"],
        &["LICENSE-APACHE", "LICENSE-MIT"],
        &mut violations,
    );
    check_string(
        sources.manifest_path,
        &value,
        &["project", "readme", "file"],
        "README.md",
        &mut violations,
    );
    check_string(
        sources.manifest_path,
        &value,
        &["project", "readme", "content-type"],
        PACKAGE_README_CONTENT_TYPE,
        &mut violations,
    );
    check_string(
        sources.manifest_path,
        &value,
        &["project", "urls", "Repository"],
        "https://github.com/EffortlessMetrics/ripr",
        &mut violations,
    );

    reject_non_empty_array(
        sources.manifest_path,
        &value,
        &["project", "dependencies"],
        "runtime dependencies",
        &mut violations,
    );
    if value_at(&value, &["project", "optional-dependencies"]).is_some() {
        violations.push(format!(
            "{}: project.optional-dependencies must be absent for the native CLI wheel",
            sources.manifest_path
        ));
    }

    check_string(
        sources.manifest_path,
        &value,
        &["tool", "maturin", "bindings"],
        &contract.python.binding,
        &mut violations,
    );
    check_single_maturin_target(
        sources.manifest_path,
        &value,
        &contract.product.binary,
        &mut violations,
    );
    check_string(
        sources.manifest_path,
        &value,
        &["tool", "maturin", "manifest-path"],
        "../../crates/ripr/Cargo.toml",
        &mut violations,
    );
    check_bool(
        sources.manifest_path,
        &value,
        &["tool", "maturin", "locked"],
        true,
        &mut violations,
    );
    check_bool(
        sources.manifest_path,
        &value,
        &["tool", "maturin", "strip"],
        true,
        &mut violations,
    );
    check_string_set(
        sources.manifest_path,
        &value,
        &["tool", "maturin", "features"],
        &contract.product.features,
        &mut violations,
    );

    for unsupported in ["compatibility", "manylinux", "module-name", "python-source"] {
        if value_at(&value, &["tool", "maturin", unsupported]).is_some() {
            violations.push(format!(
                "{}: tool.maturin.{unsupported} must remain absent; #4489 owns compatibility claims and this adapter is a native binary, not a Python module",
                sources.manifest_path
            ));
        }
    }

    validate_package_readme(sources.readme_path, sources.readme_text, &mut violations);
    validate_license_copy(
        sources.packaged_mit_path,
        sources.packaged_mit_text,
        sources.root_mit_path,
        sources.root_mit_text,
        &mut violations,
    );
    validate_license_copy(
        sources.packaged_apache_path,
        sources.packaged_apache_text,
        sources.root_apache_path,
        sources.root_apache_text,
        &mut violations,
    );

    violations
}

fn check_single_maturin_target(
    path: &str,
    value: &Value,
    expected_name: &str,
    violations: &mut Vec<String>,
) {
    let field_path = &["tool", "maturin", "targets"];
    let Some(targets) = value_at(value, field_path).and_then(Value::as_array) else {
        violations.push(format!(
            "{path}: {} must select exactly one Cargo target",
            field_path.join(".")
        ));
        return;
    };
    if targets.len() != 1 {
        violations.push(format!(
            "{path}: {} must contain exactly one target, got {}",
            field_path.join("."),
            targets.len()
        ));
        return;
    }
    let target = &targets[0];
    let name = target.get("name").and_then(Value::as_str);
    let kind = target.get("kind").and_then(Value::as_str);
    if name != Some(expected_name) || kind != Some("bin") {
        violations.push(format!(
            "{path}: {} must be [{{ name = `{expected_name}`, kind = `bin` }}], got name={name:?} kind={kind:?}",
            field_path.join(".")
        ));
    }
    let extra_fields = target
        .as_table()
        .map(|table| {
            table
                .keys()
                .filter(|key| key.as_str() != "name" && key.as_str() != "kind")
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !extra_fields.is_empty() {
        violations.push(format!(
            "{path}: {} target contains unsupported fields {extra_fields:?}",
            field_path.join(".")
        ));
    }
}

fn validate_package_readme(path: &str, text: &str, violations: &mut Vec<String>) {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "# ripr",
        "PyPI distribution name is `ripr-rs`",
        "uv tool install ripr-rs",
        "uvx --from ripr-rs ripr check",
        "does not provide `import ripr`",
        "wheel-only",
        "does not mean that a public PyPI release exists",
    ] {
        if !normalized.contains(required) {
            violations.push(format!("{path}: package README must contain `{required}`"));
        }
    }

    for forbidden in ["pip install ripr", "uvx ripr"] {
        if text.lines().any(|line| line.trim() == forbidden) {
            violations.push(format!(
                "{path}: package README must not advertise the unrelated `{forbidden}` invocation"
            ));
        }
    }
}

fn validate_license_copy(
    packaged_path: &str,
    packaged_text: &str,
    root_path: &str,
    root_text: &str,
    violations: &mut Vec<String>,
) {
    if packaged_text != root_text {
        violations.push(format!(
            "{packaged_path}: packaged license must be byte-identical to {root_path}"
        ));
    }
}

fn check_string(
    path: &str,
    value: &Value,
    field_path: &[&str],
    expected: &str,
    violations: &mut Vec<String>,
) {
    let actual = value_at(value, field_path).and_then(Value::as_str);
    if actual != Some(expected) {
        violations.push(format!(
            "{path}: {} must be `{expected}`, got {actual:?}",
            field_path.join(".")
        ));
    }
}

fn check_bool(
    path: &str,
    value: &Value,
    field_path: &[&str],
    expected: bool,
    violations: &mut Vec<String>,
) {
    let actual = value_at(value, field_path).and_then(Value::as_bool);
    if actual != Some(expected) {
        violations.push(format!(
            "{path}: {} must be `{expected}`, got {actual:?}",
            field_path.join(".")
        ));
    }
}

fn check_exact_string_array(
    path: &str,
    value: &Value,
    field_path: &[&str],
    expected: &[&str],
    violations: &mut Vec<String>,
) {
    let actual = string_array(value_at(value, field_path));
    let expected = expected
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    if actual.as_ref() != Some(&expected) {
        violations.push(format!(
            "{path}: {} must be {expected:?}, got {actual:?}",
            field_path.join(".")
        ));
    }
}

fn check_string_set(
    path: &str,
    value: &Value,
    field_path: &[&str],
    expected: &[String],
    violations: &mut Vec<String>,
) {
    let Some(actual) = string_array(value_at(value, field_path)) else {
        violations.push(format!(
            "{path}: {} must be an array of strings",
            field_path.join(".")
        ));
        return;
    };
    let actual_set = actual.iter().cloned().collect::<BTreeSet<_>>();
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    if actual_set.len() != actual.len() {
        violations.push(format!(
            "{path}: {} contains duplicate entries",
            field_path.join(".")
        ));
    }
    if actual_set != expected_set {
        violations.push(format!(
            "{path}: {} must be {expected_set:?}, got {actual_set:?}",
            field_path.join(".")
        ));
    }
}

fn reject_non_empty_array(
    path: &str,
    value: &Value,
    field_path: &[&str],
    label: &str,
    violations: &mut Vec<String>,
) {
    let Some(actual) = value_at(value, field_path) else {
        return;
    };
    if actual.as_array().is_none_or(|items| !items.is_empty()) {
        violations.push(format!(
            "{path}: {} must be absent or empty; the native CLI wheel has no {label}",
            field_path.join(".")
        ));
    }
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn value_at<'a>(value: &'a Value, field_path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for field in field_path {
        current = current.get(*field)?;
    }
    Some(current)
}
