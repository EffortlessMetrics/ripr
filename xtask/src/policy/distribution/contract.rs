use std::collections::BTreeSet;

use serde::Deserialize;

const EXPECTED_SCHEMA_VERSION: &str = "1.0";
const EXPECTED_PRODUCT: &str = "ripr";
const EXPECTED_PYPI_DISTRIBUTION: &str = "ripr-rs";
const EXPECTED_NPM_SCOPE: &str = "@effortlessmetrics";
const EXPECTED_NPM_LAUNCHER: &str = "@effortlessmetrics/ripr";
const EXPECTED_CRATE_MANIFEST: &str = "crates/ripr/Cargo.toml";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct DistributionContract {
    pub(crate) schema_version: String,
    pub(crate) product: ProductContract,
    pub(crate) python: PythonContract,
    pub(crate) npm: NpmContract,
    pub(crate) target: Vec<TargetContract>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProductContract {
    pub(crate) name: String,
    pub(crate) cargo_package: String,
    pub(crate) cargo_manifest: String,
    pub(crate) binary: String,
    pub(crate) version_source: String,
    pub(crate) default_features: bool,
    pub(crate) features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonContract {
    pub(crate) distribution: String,
    pub(crate) executable: String,
    pub(crate) binding: String,
    pub(crate) version_scheme: String,
    pub(crate) source_distribution: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpmContract {
    pub(crate) scope: String,
    pub(crate) launcher: String,
    pub(crate) executable: String,
    pub(crate) dependency_policy: String,
    pub(crate) install_scripts: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetContract {
    pub(crate) rust_target: String,
    pub(crate) executable: String,
    pub(crate) archive: String,
    pub(crate) wheel_family: String,
    pub(crate) npm_package: String,
    pub(crate) npm_os: String,
    pub(crate) npm_cpu: String,
    pub(crate) npm_libc: Option<String>,
    pub(crate) compatibility_state: String,
    pub(crate) qualification_issue: u32,
}

pub(super) fn parse_distribution_contract(
    path: &str,
    text: &str,
) -> Result<DistributionContract, String> {
    toml::from_str(text).map_err(|err| format!("{path}: invalid distribution contract: {err}"))
}

pub(super) fn validate_contract_identity(
    path: &str,
    contract: &DistributionContract,
    violations: &mut Vec<String>,
) {
    check_equal(
        path,
        "schema_version",
        &contract.schema_version,
        EXPECTED_SCHEMA_VERSION,
        violations,
    );
    check_equal(
        path,
        "product.name",
        &contract.product.name,
        EXPECTED_PRODUCT,
        violations,
    );
    check_equal(
        path,
        "product.cargo_package",
        &contract.product.cargo_package,
        EXPECTED_PRODUCT,
        violations,
    );
    check_equal(
        path,
        "product.cargo_manifest",
        &contract.product.cargo_manifest,
        EXPECTED_CRATE_MANIFEST,
        violations,
    );
    check_equal(
        path,
        "product.binary",
        &contract.product.binary,
        EXPECTED_PRODUCT,
        violations,
    );
    check_equal(
        path,
        "product.version_source",
        &contract.product.version_source,
        "Cargo.toml:[workspace.package].version",
        violations,
    );
    if !contract.product.default_features {
        violations.push(format!(
            "{path}: product.default_features must be true for the release feature contract"
        ));
    }

    let expected_features = BTreeSet::from([
        "lang-python".to_string(),
        "lang-rust".to_string(),
        "lang-typescript".to_string(),
    ]);
    check_string_set(
        path,
        "product.features",
        &contract.product.features,
        &expected_features,
        violations,
    );

    check_equal(
        path,
        "python.distribution",
        &contract.python.distribution,
        EXPECTED_PYPI_DISTRIBUTION,
        violations,
    );
    check_equal(
        path,
        "python.executable",
        &contract.python.executable,
        EXPECTED_PRODUCT,
        violations,
    );
    check_equal(
        path,
        "python.binding",
        &contract.python.binding,
        "bin",
        violations,
    );
    check_equal(
        path,
        "python.version_scheme",
        &contract.python.version_scheme,
        "pep440-from-semver",
        violations,
    );
    if contract.python.source_distribution {
        violations.push(format!(
            "{path}: python.source_distribution must remain false until an extracted sdist build is qualified"
        ));
    }

    check_equal(
        path,
        "npm.scope",
        &contract.npm.scope,
        EXPECTED_NPM_SCOPE,
        violations,
    );
    check_equal(
        path,
        "npm.launcher",
        &contract.npm.launcher,
        EXPECTED_NPM_LAUNCHER,
        violations,
    );
    check_equal(
        path,
        "npm.executable",
        &contract.npm.executable,
        EXPECTED_PRODUCT,
        violations,
    );
    check_equal(
        path,
        "npm.dependency_policy",
        &contract.npm.dependency_policy,
        "exact",
        violations,
    );
    if contract.npm.install_scripts {
        violations.push(format!(
            "{path}: npm.install_scripts must be false; installation cannot depend on lifecycle scripts"
        ));
    }
}

pub(super) fn validate_crate_manifest(
    path: &str,
    text: &str,
    product: &ProductContract,
    violations: &mut Vec<String>,
) {
    let value: toml::Value = match toml::from_str(text) {
        Ok(value) => value,
        Err(err) => {
            violations.push(format!("{path}: invalid Cargo manifest: {err}"));
            return;
        }
    };

    let package = value.get("package");
    let package_name = package
        .and_then(|table| table.get("name"))
        .and_then(toml::Value::as_str);
    if package_name != Some(product.cargo_package.as_str()) {
        violations.push(format!(
            "{path}: package.name must be `{}`, got {:?}",
            product.cargo_package, package_name
        ));
    }

    let inherits_workspace_version = package
        .and_then(|table| table.get("version"))
        .and_then(|version| version.get("workspace"))
        .and_then(toml::Value::as_bool);
    if inherits_workspace_version != Some(true) {
        violations.push(format!(
            "{path}: package.version.workspace must be true so adapters cannot acquire an independent product version"
        ));
    }

    let binaries = value
        .get("bin")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let has_binary = binaries.iter().any(|binary| {
        binary.get("name").and_then(toml::Value::as_str) == Some(product.binary.as_str())
            && binary.get("path").and_then(toml::Value::as_str) == Some("src/main.rs")
    });
    if !has_binary {
        violations.push(format!(
            "{path}: missing [[bin]] name = `{}` path = `src/main.rs`",
            product.binary
        ));
    }

    let default_features = value
        .get("features")
        .and_then(|features| features.get("default"))
        .and_then(toml::Value::as_array)
        .map(|features| {
            features
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let expected_features = product.features.iter().cloned().collect::<BTreeSet<_>>();
    check_string_set(
        path,
        "features.default",
        &default_features,
        &expected_features,
        violations,
    );
}

pub(super) fn parse_workspace_version(path: &str, text: &str) -> Result<String, String> {
    let value: toml::Value =
        toml::from_str(text).map_err(|err| format!("{path}: invalid Cargo manifest: {err}"))?;
    value
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{path}: missing [workspace.package].version string"))
}

fn check_equal(
    path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    violations: &mut Vec<String>,
) {
    if actual != expected {
        violations.push(format!(
            "{path}: {field} must be `{expected}`, got `{actual}`"
        ));
    }
}

fn check_string_set(
    path: &str,
    field: &str,
    actual: &[String],
    expected: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let actual_set = actual.iter().cloned().collect::<BTreeSet<_>>();
    if actual_set.len() != actual.len() {
        violations.push(format!("{path}: {field} contains duplicate entries"));
    }
    if &actual_set != expected {
        violations.push(format!(
            "{path}: {field} must be {:?}, got {:?}",
            expected, actual_set
        ));
    }
}
