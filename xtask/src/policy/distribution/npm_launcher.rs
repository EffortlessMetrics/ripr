use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::contract::{DistributionContract, TargetContract};

pub(super) const MANIFEST_PATH: &str = "packaging/npm/launcher/package.json";
pub(super) const BIN_PATH: &str = "packaging/npm/launcher/bin/ripr.cjs";
pub(super) const LIBRARY_PATH: &str = "packaging/npm/launcher/lib/launcher.cjs";
pub(super) const TEST_PATH: &str = "packaging/npm/launcher/test/launcher.test.cjs";
pub(super) const TEST_RUNNER_PATH: &str = "packaging/npm/launcher/test/run-tests.test.cjs";
pub(super) const TEST_RUNNER_TEXT: &str =
    include_str!("../../../../packaging/npm/launcher/test/run-tests.test.cjs");
pub(super) const LICENSE_APACHE_PATH: &str = "packaging/npm/launcher/LICENSE-APACHE";
pub(super) const LICENSE_MIT_PATH: &str = "packaging/npm/launcher/LICENSE-MIT";

const EXPECTED_FILES: &[&str] = &[
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
    "bin/ripr.cjs",
    "lib/launcher.cjs",
];
const FORBIDDEN_LIFECYCLE_SCRIPTS: &[&str] = &[
    "preinstall",
    "install",
    "postinstall",
    "preprepare",
    "prepare",
    "postprepare",
];

#[derive(Debug, Deserialize)]
struct LauncherManifest {
    name: String,
    version: String,
    license: String,
    #[serde(rename = "type")]
    module_type: String,
    bin: BTreeMap<String, String>,
    files: Vec<String>,
    engines: BTreeMap<String, String>,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
    #[serde(rename = "publishConfig")]
    publish_config: PublishConfig,
    #[serde(rename = "optionalDependencies")]
    optional_dependencies: BTreeMap<String, String>,
    ripr: LauncherMetadata,
}

#[derive(Debug, Deserialize)]
struct PublishConfig {
    access: String,
}

#[derive(Debug, Deserialize)]
struct LauncherMetadata {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    product: String,
    executable: String,
    platforms: Vec<LauncherPlatform>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
struct LauncherPlatform {
    #[serde(rename = "nodePlatform")]
    node_platform: String,
    #[serde(rename = "nodeArch")]
    node_arch: String,
    libc: Option<String>,
    #[serde(rename = "rustTarget")]
    rust_target: String,
    package: String,
    executable: String,
}

pub(super) fn validate_launcher(
    path: &str,
    text: &str,
    workspace_version: &str,
    contract: &DistributionContract,
    violations: &mut Vec<String>,
) {
    let manifest: LauncherManifest = match serde_json::from_str(text) {
        Ok(manifest) => manifest,
        Err(err) => {
            violations.push(format!(
                "{path}: invalid npm launcher package manifest: {err}"
            ));
            return;
        }
    };

    check_equal(
        path,
        "name",
        &manifest.name,
        &contract.npm.launcher,
        violations,
    );
    check_equal(
        path,
        "version",
        &manifest.version,
        workspace_version,
        violations,
    );
    check_equal(
        path,
        "license",
        &manifest.license,
        "MIT OR Apache-2.0",
        violations,
    );
    check_equal(path, "type", &manifest.module_type, "commonjs", violations);
    check_equal(
        path,
        "bin.ripr",
        manifest
            .bin
            .get("ripr")
            .map(String::as_str)
            .unwrap_or_default(),
        "bin/ripr.cjs",
        violations,
    );
    if manifest.bin.len() != 1 {
        violations.push(format!(
            "{path}: bin must contain only the public `ripr` executable, got {:?}",
            manifest.bin.keys().collect::<Vec<_>>()
        ));
    }

    let actual_files = manifest
        .files
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_files = EXPECTED_FILES.iter().copied().collect::<BTreeSet<_>>();
    if actual_files.len() != manifest.files.len() {
        violations.push(format!("{path}: files contains duplicate entries"));
    }
    if actual_files != expected_files {
        violations.push(format!(
            "{path}: files must be the explicit launcher package set {:?}, got {:?}",
            expected_files, actual_files
        ));
    }

    check_equal(
        path,
        "engines.node",
        manifest
            .engines
            .get("node")
            .map(String::as_str)
            .unwrap_or_default(),
        ">=20",
        violations,
    );
    check_equal(
        path,
        "publishConfig.access",
        &manifest.publish_config.access,
        "public",
        violations,
    );
    for script in FORBIDDEN_LIFECYCLE_SCRIPTS {
        if manifest.scripts.contains_key(*script) {
            violations.push(format!(
                "{path}: scripts.{script} is forbidden; installation cannot depend on npm lifecycle scripts"
            ));
        }
    }
    check_equal(
        path,
        "scripts.test",
        manifest
            .scripts
            .get("test")
            .map(String::as_str)
            .unwrap_or_default(),
        "node test/run-tests.test.cjs",
        violations,
    );

    if manifest.ripr.schema_version != 1 {
        violations.push(format!(
            "{path}: ripr.schemaVersion must be 1, got {}",
            manifest.ripr.schema_version
        ));
    }
    check_equal(
        path,
        "ripr.product",
        &manifest.ripr.product,
        &contract.product.name,
        violations,
    );
    check_equal(
        path,
        "ripr.executable",
        &manifest.ripr.executable,
        &contract.npm.executable,
        violations,
    );

    let expected_dependencies = contract
        .target
        .iter()
        .map(|target| (target.npm_package.as_str(), workspace_version))
        .collect::<BTreeMap<_, _>>();
    let actual_dependencies = manifest
        .optional_dependencies
        .iter()
        .map(|(name, version)| (name.as_str(), version.as_str()))
        .collect::<BTreeMap<_, _>>();
    if actual_dependencies != expected_dependencies {
        violations.push(format!(
            "{path}: optionalDependencies must equal the five exact-version native packages {:?}, got {:?}",
            expected_dependencies, actual_dependencies
        ));
    }

    let mut expected_platforms = contract
        .target
        .iter()
        .map(expected_platform)
        .collect::<Vec<_>>();
    expected_platforms.sort();
    let mut actual_platforms = manifest.ripr.platforms.clone();
    actual_platforms.sort();
    if actual_platforms.windows(2).any(|rows| rows[0] == rows[1]) {
        violations.push(format!("{path}: ripr.platforms contains duplicate rows"));
    }
    if actual_platforms != expected_platforms {
        violations.push(format!(
            "{path}: ripr.platforms must project policy/distribution.toml exactly; expected {expected_platforms:?}, got {actual_platforms:?}"
        ));
    }
}

pub(super) fn validate_license_copy(
    root_path: &str,
    root_text: &str,
    package_path: &str,
    package_text: &str,
    violations: &mut Vec<String>,
) {
    if package_text != root_text {
        violations.push(format!(
            "{package_path}: license copy must be byte-identical to {root_path}"
        ));
    }
}

pub(super) fn validate_launcher_sources(
    bin_path: &str,
    bin_text: &str,
    library_path: &str,
    library_text: &str,
    test_path: &str,
    test_text: &str,
    violations: &mut Vec<String>,
) {
    if !bin_text.starts_with("#!/usr/bin/env node\n") {
        violations.push(format!(
            "{bin_path}: launcher bin must use the Node env shebang"
        ));
    }
    for (needle, explanation) in [
        ("shell: false", "spawn must disable shell interpretation"),
        (
            "stdio: \"inherit\"",
            "native stdio must be inherited without wrapper output",
        ),
        (
            "createRequire",
            "native packages must resolve relative to the launcher",
        ),
        (
            "realpathSync",
            "native package/executable confinement must use real paths",
        ),
        (
            "optional dependencies enabled",
            "missing-payload recovery must be actionable",
        ),
        (
            "process.exitCode = signalExitCode",
            "a consumed child signal must still produce a nonzero launcher exit",
        ),
        (
            "SIGNALS_TO_OBSERVE",
            "terminal process-group signals must be observed without duplicate forwarding",
        ),
        (
            "const effectiveSignal = receivedSignal || signal",
            "the first observed termination signal must survive a graceful child exit",
        ),
    ] {
        if !library_text.contains(needle) {
            violations.push(format!("{library_path}: {explanation}; missing `{needle}`"));
        }
    }
    for forbidden in ["curl ", "wget ", "https.get", "execSync(", "shell: true"] {
        for (path, text) in [(bin_path, bin_text), (library_path, library_text)] {
            if text.contains(forbidden) {
                violations.push(format!(
                    "{path}: launcher cannot download, shell out, or enable shell interpolation; found `{forbidden}`"
                ));
            }
        }
    }
    validate_test_runner(
        TEST_RUNNER_PATH,
        TEST_RUNNER_TEXT,
        test_path,
        test_text,
        violations,
    );
}

const REQUIRED_EXECUTED_TESTS: &[&str] = &[
    "rejects lifecycle scripts, version ranges, and dependency drift",
    "rejects missing, wrong-version, wrong-target, traversal, symlink, directory, and non-executable payloads",
    "forwards direct SIGTERM to native child exactly once and re-emits signal",
    "retains the first observed signal when a supervisor escalates SIGINT to SIGTERM",
    "observes terminal SIGINT and SIGHUP without forwarding duplicates to the native child",
    "source bin missing-package failure never falls back to PATH or writes stdout",
    "npm package contents are explicit and exclude tests and build residue",
];

pub(super) fn validate_test_runner(
    runner_path: &str,
    runner_text: &str,
    test_path: &str,
    test_text: &str,
    violations: &mut Vec<String>,
) {
    for (needle, explanation) in [
        (
            "--test-reporter=tap",
            "runner must consume a machine-readable test report",
        ),
        (
            "result.status !== 0",
            "runner must propagate the Node test process failure",
        ),
        (
            "missing.length > 0",
            "runner must fail when a required named control did not pass",
        ),
        (
            "SKIP|TODO",
            "runner must not count skipped or todo controls as passed",
        ),
    ] {
        if !runner_text.contains(needle) {
            violations.push(format!("{runner_path}: {explanation}; missing `{needle}`"));
        }
    }
    for required_test in REQUIRED_EXECUTED_TESTS {
        let quoted = format!("\"{required_test}\"");
        if !runner_text.contains(&quoted) {
            violations.push(format!(
                "{runner_path}: required executed-control admission is missing `{required_test}`"
            ));
        }
        if !test_text.contains(required_test) {
            violations.push(format!(
                "{test_path}: required launcher control is missing `{required_test}`"
            ));
        }
    }
}

fn expected_platform(target: &TargetContract) -> LauncherPlatform {
    LauncherPlatform {
        node_platform: target.npm_os.clone(),
        node_arch: target.npm_cpu.clone(),
        libc: target.npm_libc.clone(),
        rust_target: target.rust_target.clone(),
        package: target.npm_package.clone(),
        executable: format!("bin/{}", target.executable),
    }
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
