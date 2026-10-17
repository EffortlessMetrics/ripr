//! Exercise Cargo's inclusion rules, including automatic nested-package omission.

use crate::run::capture_output_in_dir;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn cargo_package_retains_exact_source_role_fixture_closure() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest has no workspace parent".to_string())?;
    let args: Vec<String> = ["package", "-p", "ripr", "--list", "--locked", "--offline"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let output = capture_output_in_dir(
        env!("CARGO"),
        &args,
        root,
        "list the real ripr Cargo package",
    )?;
    if !output.status.success() {
        return Err(format!(
            "cargo package --list failed with {}: {}",
            output.status, output.stderr
        ));
    }
    let retained: BTreeSet<&str> = output
        .stdout
        .lines()
        .filter_map(|path| path.strip_prefix("tests/data/source-role-corpus/cases/"))
        .collect();
    let required = BTreeSet::from([
        "cfg_variants/Cargo.toml.fixture",
        "cfg_variants/src/lib.rs",
        "explicit_test_target/Cargo.toml.fixture",
        "explicit_test_target/src/lib.rs",
        "explicit_test_target/tests/custom/smoke_case.rs",
        "naming_lookalike/Cargo.toml.fixture",
        "naming_lookalike/src/lib.rs",
        "naming_lookalike/src/price_test.rs",
        "production_lib/Cargo.toml.fixture",
        "production_lib/src/lib.rs",
    ]);
    assert_eq!(
        retained, required,
        "Cargo must retain exactly the ten corpus inputs, including manifests, without case-local build residue"
    );
    Ok(())
}
