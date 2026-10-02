use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::policy::distribution::{DistributionContract, load_distribution_contract};
use crate::run::run_output_owned;

use super::payload::FinalNativePayloadIdentity;
use super::sha256_file;

const PRODUCER_SCHEMA_VERSION: u32 = 1;
const PRODUCER_KIND: &str = "ripr_final_native_payload_producer_build";
pub(crate) const PRODUCER_RECEIPT_FILE: &str = "producer-build.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProducerBuildReceipt {
    pub(crate) schema_version: u32,
    pub(crate) kind: String,
    pub(crate) product: String,
    pub(crate) version: String,
    pub(crate) target: String,
    pub(crate) executable: String,
    pub(crate) source_commit: String,
    pub(crate) source_tree: String,
    pub(crate) cargo_lock_sha256: String,
    pub(crate) cargo_arguments: Vec<String>,
    pub(crate) default_features: bool,
    pub(crate) features: Vec<String>,
    pub(crate) rustc_verbose_version: String,
    pub(crate) cargo_verbose_version: String,
    pub(crate) executable_sha256: String,
    pub(crate) executable_version_line: String,
    pub(crate) cargo_fresh: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    #[serde(default)]
    target: Option<CargoTarget>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    executable: Option<String>,
    #[serde(default)]
    fresh: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CargoBinaryArtifact {
    executable: PathBuf,
    features: Vec<String>,
    fresh: bool,
}

pub(crate) fn build_release_executable(
    version: &str,
    target: &str,
    executable: &str,
) -> Result<ProducerBuildReceipt, String> {
    let contract = load_distribution_contract()?;
    validate_requested_build(&contract, target, executable)?;
    let source_commit = git_rev_parse("HEAD")?;
    let source_tree = git_rev_parse("HEAD^{tree}")?;
    let cargo_lock_sha256 = sha256_file(Path::new("Cargo.lock"))?;
    let cargo_arguments = vec![
        "build".to_string(),
        "--locked".to_string(),
        "--color=never".to_string(),
        "-p".to_string(),
        contract.product.cargo_package.clone(),
        "--release".to_string(),
        "--target".to_string(),
        target.to_string(),
        "--message-format=json-render-diagnostics".to_string(),
    ];
    let output = run_output_owned("cargo", &cargo_arguments)?;
    let expected_executable = Path::new("target")
        .join(target)
        .join("release")
        .join(executable);
    let artifact =
        select_binary_artifact(&output, &expected_executable, &contract.product.features)?;
    let executable_version_line = run_output_owned(
        &artifact.executable.to_string_lossy(),
        &["--version".to_string()],
    )?
    .trim()
    .to_string();
    validate_version_line(&executable_version_line, version, &source_commit)?;

    Ok(ProducerBuildReceipt {
        schema_version: PRODUCER_SCHEMA_VERSION,
        kind: PRODUCER_KIND.to_string(),
        product: contract.product.name,
        version: version.to_string(),
        target: target.to_string(),
        executable: executable.to_string(),
        source_commit,
        source_tree,
        cargo_lock_sha256,
        cargo_arguments,
        default_features: contract.product.default_features,
        features: artifact.features,
        rustc_verbose_version: checked_output("rustc", &["-vV"], "rustc identity")?,
        cargo_verbose_version: checked_output("cargo", &["-vV"], "cargo identity")?,
        executable_sha256: sha256_file(&artifact.executable)?,
        executable_version_line,
        cargo_fresh: artifact.fresh,
    })
}

pub(crate) fn bind_to_payload_identity(
    producer: &ProducerBuildReceipt,
    identity: &FinalNativePayloadIdentity,
) -> Result<(), String> {
    let mut violations = Vec::new();
    compare(
        "product",
        &producer.product,
        &identity.product,
        &mut violations,
    );
    compare(
        "version",
        &producer.version,
        &identity.version,
        &mut violations,
    );
    compare(
        "target",
        &producer.target,
        &identity.target,
        &mut violations,
    );
    compare(
        "executable",
        &producer.executable,
        &identity.executable,
        &mut violations,
    );
    compare(
        "source commit",
        &producer.source_commit,
        &identity.source.commit_sha,
        &mut violations,
    );
    compare(
        "source tree",
        &producer.source_tree,
        &identity.source.tree_sha,
        &mut violations,
    );
    compare(
        "Cargo.lock SHA-256",
        &producer.cargo_lock_sha256,
        &identity.cargo_lock_sha256,
        &mut violations,
    );
    if producer.features != identity.selected_features {
        violations.push(format!(
            "producer features {:?} do not match payload features {:?}",
            producer.features, identity.selected_features
        ));
    }
    compare(
        "rustc identity",
        &producer.rustc_verbose_version,
        &identity.toolchain.rustc_verbose_version,
        &mut violations,
    );
    compare(
        "cargo identity",
        &producer.cargo_verbose_version,
        &identity.toolchain.cargo_verbose_version,
        &mut violations,
    );
    let executable_rows = identity
        .files
        .iter()
        .filter(|file| file.path == identity.executable)
        .collect::<Vec<_>>();
    if executable_rows.len() != 1 {
        violations.push(format!(
            "payload identity must contain exactly one executable row, found {}",
            executable_rows.len()
        ));
    } else if executable_rows[0].sha256 != producer.executable_sha256 {
        violations.push(format!(
            "producer executable SHA-256 {} does not match staged payload SHA-256 {}",
            producer.executable_sha256, executable_rows[0].sha256
        ));
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "producer build does not bind the staged final payload:\n- {}",
            violations.join("\n- ")
        ))
    }
}

pub(crate) fn write_receipt(
    directory: &Path,
    receipt: &ProducerBuildReceipt,
) -> Result<PathBuf, String> {
    validate_receipt_shape(receipt)?;
    let path = directory.join(PRODUCER_RECEIPT_FILE);
    let rendered = serde_json::to_string_pretty(receipt)
        .map_err(|err| format!("failed to render producer build receipt: {err}"))?;
    fs::write(&path, format!("{rendered}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(path)
}

fn validate_requested_build(
    contract: &DistributionContract,
    target: &str,
    executable: &str,
) -> Result<(), String> {
    if !contract.product.default_features {
        return Err("distribution contract must use the default release features".to_string());
    }
    let target_contract = contract
        .target
        .iter()
        .find(|candidate| candidate.rust_target == target)
        .ok_or_else(|| format!("target `{target}` is absent from policy/distribution.toml"))?;
    if target_contract.executable != executable {
        return Err(format!(
            "target `{target}` requires executable `{}`, got `{executable}`",
            target_contract.executable
        ));
    }
    Ok(())
}

fn select_binary_artifact(
    output: &str,
    expected_executable: &Path,
    expected_features: &[String],
) -> Result<CargoBinaryArtifact, String> {
    let expected = fs::canonicalize(expected_executable).map_err(|err| {
        format!(
            "canonical release executable {} is unavailable after Cargo build: {err}",
            expected_executable.display()
        )
    })?;
    let mut matches = Vec::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(message) = serde_json::from_str::<CargoMessage>(line) else {
            continue;
        };
        if message.reason != "compiler-artifact" {
            continue;
        }
        let Some(target) = message.target else {
            continue;
        };
        if target.name != "ripr" || !target.kind.iter().any(|kind| kind == "bin") {
            continue;
        }
        let Some(executable) = message.executable else {
            continue;
        };
        let executable = PathBuf::from(executable);
        let canonical = fs::canonicalize(&executable).map_err(|err| {
            format!(
                "Cargo reported executable {} that cannot be canonicalized: {err}",
                executable.display()
            )
        })?;
        if canonical != expected {
            continue;
        }
        let features = normalized_release_features(&message.features, expected_features)?;
        matches.push(CargoBinaryArtifact {
            executable: canonical,
            features,
            fresh: message.fresh,
        });
    }
    match matches.as_slice() {
        [artifact] => Ok(artifact.clone()),
        [] => Err(format!(
            "Cargo build did not report the canonical ripr binary artifact `{}`",
            expected.display()
        )),
        many => Err(format!(
            "Cargo build reported {} canonical ripr binary artifacts; expected exactly one",
            many.len()
        )),
    }
}

fn normalized_release_features(
    actual: &[String],
    expected: &[String],
) -> Result<Vec<String>, String> {
    let mut actual = actual
        .iter()
        .filter(|feature| feature.as_str() != "default")
        .cloned()
        .collect::<Vec<_>>();
    actual.sort();
    actual.dedup();
    let mut expected = expected.to_vec();
    expected.sort();
    expected.dedup();
    if actual != expected {
        return Err(format!(
            "Cargo built ripr with features {actual:?}; canonical release features are {expected:?}"
        ));
    }
    Ok(actual)
}

fn validate_version_line(actual: &str, version: &str, commit: &str) -> Result<(), String> {
    let expected = format!("ripr {version} ({commit})");
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "canonical release executable identity mismatch: expected `{expected}`, got `{actual}`"
        ))
    }
}

fn validate_receipt_shape(receipt: &ProducerBuildReceipt) -> Result<(), String> {
    let mut violations = Vec::new();
    if receipt.schema_version != PRODUCER_SCHEMA_VERSION {
        violations.push(format!(
            "schema_version must be {PRODUCER_SCHEMA_VERSION}, got {}",
            receipt.schema_version
        ));
    }
    if receipt.kind != PRODUCER_KIND {
        violations.push(format!(
            "kind must be `{PRODUCER_KIND}`, got `{}`",
            receipt.kind
        ));
    }
    if receipt.product != "ripr" {
        violations.push(format!("product must be `ripr`, got `{}`", receipt.product));
    }
    if receipt.cargo_arguments
        != [
            "build",
            "--locked",
            "--color=never",
            "-p",
            "ripr",
            "--release",
            "--target",
            receipt.target.as_str(),
            "--message-format=json-render-diagnostics",
        ]
        .map(str::to_string)
    {
        violations.push("cargo_arguments are not the canonical locked release build".to_string());
    }
    if !receipt.default_features {
        violations.push("default_features must be true".to_string());
    }
    for (label, value, length) in [
        ("source_commit", receipt.source_commit.as_str(), 40),
        ("source_tree", receipt.source_tree.as_str(), 40),
        ("cargo_lock_sha256", receipt.cargo_lock_sha256.as_str(), 64),
        ("executable_sha256", receipt.executable_sha256.as_str(), 64),
    ] {
        if !is_lower_hex(value, length) {
            violations.push(format!(
                "{label} must be {length} lowercase hexadecimal characters"
            ));
        }
    }
    if receipt.features.is_empty()
        || receipt.rustc_verbose_version.trim().is_empty()
        || receipt.cargo_verbose_version.trim().is_empty()
        || receipt.executable_version_line.trim().is_empty()
    {
        violations.push("producer build identity fields must not be empty".to_string());
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "invalid producer build receipt:\n- {}",
            violations.join("\n- ")
        ))
    }
}

fn compare(label: &str, actual: &str, expected: &str, violations: &mut Vec<String>) {
    if actual != expected {
        violations.push(format!(
            "{label} mismatch: producer `{actual}`, payload `{expected}`"
        ));
    }
}

fn git_rev_parse(revision: &str) -> Result<String, String> {
    let value = checked_output("git", &["rev-parse", revision], "git source identity")?;
    if is_lower_hex(&value, 40) {
        Ok(value)
    } else {
        Err(format!(
            "git rev-parse {revision} returned invalid object identity `{value}`"
        ))
    }
}

fn checked_output(program: &str, args: &[&str], label: &str) -> Result<String, String> {
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let output = run_output_owned(program, &args)?;
    let output = output.replace("\r\n", "\n").trim().to_string();
    if output.is_empty() {
        Err(format!("{label} produced empty output"))
    } else {
        Ok(output)
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_feature_set_rejects_wrong_feature_build() -> Result<(), String> {
        let error = require_error(
            normalized_release_features(
                &["default".to_string(), "lang-rust".to_string()],
                &[
                    "lang-python".to_string(),
                    "lang-rust".to_string(),
                    "lang-typescript".to_string(),
                ],
            ),
            "wrong feature set",
        )?;
        if !error.contains("canonical release features") {
            return Err(format!("unexpected feature-set error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn version_line_rejects_stale_and_dirty_binary() -> Result<(), String> {
        let commit = "a".repeat(40);
        let stale = require_error(
            validate_version_line("ripr 0.11.0 (old)", "0.11.0", &commit),
            "stale binary version",
        )?;
        if !stale.contains("identity mismatch") {
            return Err(format!("unexpected stale-version error: {stale}"));
        }
        let dirty = require_error(
            validate_version_line(&format!("ripr 0.11.0 ({commit}-dirty)"), "0.11.0", &commit),
            "dirty binary version",
        )?;
        if !dirty.contains("identity mismatch") {
            return Err(format!("unexpected dirty-version error: {dirty}"));
        }
        validate_version_line(&format!("ripr 0.11.0 ({commit})"), "0.11.0", &commit)
    }

    #[test]
    fn receipt_rejects_noncanonical_build_arguments() -> Result<(), String> {
        let mut receipt = fixture_receipt();
        receipt
            .cargo_arguments
            .push("--no-default-features".to_string());
        let error = require_error(
            validate_receipt_shape(&receipt),
            "noncanonical build arguments",
        )?;
        if !error.contains("cargo_arguments") {
            return Err(format!("unexpected receipt error: {error}"));
        }
        Ok(())
    }

    fn require_error<T>(result: Result<T, String>, label: &str) -> Result<String, String> {
        let Err(error) = result else {
            return Err(format!("{label} unexpectedly succeeded"));
        };
        Ok(error)
    }

    fn fixture_receipt() -> ProducerBuildReceipt {
        ProducerBuildReceipt {
            schema_version: PRODUCER_SCHEMA_VERSION,
            kind: PRODUCER_KIND.to_string(),
            product: "ripr".to_string(),
            version: "0.11.0".to_string(),
            target: "x86_64-unknown-linux-gnu".to_string(),
            executable: "ripr".to_string(),
            source_commit: "a".repeat(40),
            source_tree: "b".repeat(40),
            cargo_lock_sha256: "c".repeat(64),
            cargo_arguments: [
                "build",
                "--locked",
                "--color=never",
                "-p",
                "ripr",
                "--release",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--message-format=json-render-diagnostics",
            ]
            .map(str::to_string)
            .to_vec(),
            default_features: true,
            features: vec![
                "lang-python".to_string(),
                "lang-rust".to_string(),
                "lang-typescript".to_string(),
            ],
            rustc_verbose_version: "rustc 1.95.0".to_string(),
            cargo_verbose_version: "cargo 1.95.0".to_string(),
            executable_sha256: "d".repeat(64),
            executable_version_line: format!("ripr 0.11.0 ({})", "a".repeat(40)),
            cargo_fresh: false,
        }
    }
}
