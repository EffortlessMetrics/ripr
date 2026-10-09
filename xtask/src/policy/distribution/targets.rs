use std::collections::{BTreeMap, BTreeSet};

use super::contract::TargetContract;

const EXPECTED_QUALIFICATION_ISSUE: u32 = 4489;

#[derive(Clone, Copy)]
struct ExpectedTarget {
    rust_target: &'static str,
    executable: &'static str,
    archive: &'static str,
    wheel_family: &'static str,
    npm_package: &'static str,
    npm_os: &'static str,
    npm_cpu: &'static str,
    npm_libc: Option<&'static str>,
}

const EXPECTED_TARGETS: &[ExpectedTarget] = &[
    ExpectedTarget {
        rust_target: "x86_64-unknown-linux-gnu",
        executable: "ripr",
        archive: "tar.gz",
        wheel_family: "manylinux",
        npm_package: "@effortlessmetrics/ripr-linux-x64-gnu",
        npm_os: "linux",
        npm_cpu: "x64",
        npm_libc: Some("glibc"),
    },
    ExpectedTarget {
        rust_target: "aarch64-unknown-linux-gnu",
        executable: "ripr",
        archive: "tar.gz",
        wheel_family: "manylinux",
        npm_package: "@effortlessmetrics/ripr-linux-arm64-gnu",
        npm_os: "linux",
        npm_cpu: "arm64",
        npm_libc: Some("glibc"),
    },
    ExpectedTarget {
        rust_target: "x86_64-apple-darwin",
        executable: "ripr",
        archive: "tar.gz",
        wheel_family: "macosx",
        npm_package: "@effortlessmetrics/ripr-darwin-x64",
        npm_os: "darwin",
        npm_cpu: "x64",
        npm_libc: None,
    },
    ExpectedTarget {
        rust_target: "aarch64-apple-darwin",
        executable: "ripr",
        archive: "tar.gz",
        wheel_family: "macosx",
        npm_package: "@effortlessmetrics/ripr-darwin-arm64",
        npm_os: "darwin",
        npm_cpu: "arm64",
        npm_libc: None,
    },
    ExpectedTarget {
        rust_target: "x86_64-pc-windows-msvc",
        executable: "ripr.exe",
        archive: "zip",
        wheel_family: "win",
        npm_package: "@effortlessmetrics/ripr-win32-x64-msvc",
        npm_os: "win32",
        npm_cpu: "x64",
        npm_libc: None,
    },
];

pub(super) fn validate_targets(
    path: &str,
    targets: &[TargetContract],
    violations: &mut Vec<String>,
) {
    if targets.len() != EXPECTED_TARGETS.len() {
        violations.push(format!(
            "{path}: expected {} distribution targets, found {}",
            EXPECTED_TARGETS.len(),
            targets.len()
        ));
    }

    let mut by_rust_target = BTreeMap::new();
    let mut npm_packages = BTreeSet::new();
    for target in targets {
        if by_rust_target
            .insert(target.rust_target.as_str(), target)
            .is_some()
        {
            violations.push(format!("{path}: duplicate target `{}`", target.rust_target));
        }
        if !npm_packages.insert(target.npm_package.as_str()) {
            violations.push(format!(
                "{path}: duplicate npm payload package `{}`",
                target.npm_package
            ));
        }
        if target.compatibility_state != "unqualified" {
            violations.push(format!(
                "{path}: target `{}` compatibility_state must remain `unqualified` until #4489 records measured proof",
                target.rust_target
            ));
        }
        if target.qualification_issue != EXPECTED_QUALIFICATION_ISSUE {
            violations.push(format!(
                "{path}: target `{}` qualification_issue must be {EXPECTED_QUALIFICATION_ISSUE}, got {}",
                target.rust_target, target.qualification_issue
            ));
        }
    }

    for expected in EXPECTED_TARGETS {
        let Some(actual) = by_rust_target.get(expected.rust_target) else {
            violations.push(format!("{path}: missing target `{}`", expected.rust_target));
            continue;
        };
        check_field(
            path,
            expected.rust_target,
            "executable",
            &actual.executable,
            expected.executable,
            violations,
        );
        check_field(
            path,
            expected.rust_target,
            "archive",
            &actual.archive,
            expected.archive,
            violations,
        );
        check_field(
            path,
            expected.rust_target,
            "wheel_family",
            &actual.wheel_family,
            expected.wheel_family,
            violations,
        );
        check_field(
            path,
            expected.rust_target,
            "npm_package",
            &actual.npm_package,
            expected.npm_package,
            violations,
        );
        check_field(
            path,
            expected.rust_target,
            "npm_os",
            &actual.npm_os,
            expected.npm_os,
            violations,
        );
        check_field(
            path,
            expected.rust_target,
            "npm_cpu",
            &actual.npm_cpu,
            expected.npm_cpu,
            violations,
        );
        if actual.npm_libc.as_deref() != expected.npm_libc {
            violations.push(format!(
                "{path}: target `{}` field npm_libc must be {:?}, got {:?}",
                expected.rust_target, expected.npm_libc, actual.npm_libc
            ));
        }
    }
}

fn check_field(
    path: &str,
    target: &str,
    field: &str,
    actual: &str,
    expected: &str,
    violations: &mut Vec<String>,
) {
    if actual != expected {
        violations.push(format!(
            "{path}: target `{target}` field {field} must be `{expected}`, got `{actual}`"
        ));
    }
}
