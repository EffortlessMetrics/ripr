use std::fs;
use std::path::Path;

#[path = "support/source_role_fixture.rs"]
mod fixture;

const REQUIRED_INPUTS: [&str; 10] = [
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
];

#[test]
fn source_role_corpus_retains_every_required_input() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/source-role-corpus/cases");
    let missing: Vec<_> = REQUIRED_INPUTS
        .iter()
        .filter(|path| !root.join(path).is_file())
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "{}/{} required source-role corpus inputs missing: {}",
            missing.len(),
            REQUIRED_INPUTS.len(),
            missing.join(", ")
        ));
    }
    for path in REQUIRED_INPUTS {
        let bytes = fs::read(root.join(path))
            .map_err(|error| format!("read required corpus input {path}: {error}"))?;
        assert!(!bytes.is_empty(), "required corpus input is empty: {path}");
    }
    Ok(())
}

#[test]
fn materialized_source_role_cases_preserve_the_required_inputs() -> Result<(), String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/source-role-corpus/cases");
    for name in [
        "cfg_variants",
        "explicit_test_target",
        "naming_lookalike",
        "production_lib",
    ] {
        let case = fixture::SourceRoleCase::load(name)?;
        let prefix = format!("{name}/");
        let mut checked = 0;
        for input in REQUIRED_INPUTS {
            let Some(relative) = input.strip_prefix(&prefix) else {
                continue;
            };
            let destination = if relative == "Cargo.toml.fixture" {
                "Cargo.toml"
            } else {
                relative
            };
            let expected = fs::read(source.join(input))
                .map_err(|error| format!("read retained input {input}: {error}"))?;
            let actual = fs::read(case.root().join(destination))
                .map_err(|error| format!("read materialized input {input}: {error}"))?;
            assert_eq!(actual, expected, "materialized bytes changed: {input}");
            checked += 1;
        }
        assert!(checked > 0, "case has no inspected inputs: {name}");
        assert_eq!(case.files().len(), checked, "case input closure: {name}");
        assert!(!case.root().join("Cargo.toml.fixture").exists());
        let root = case.root().to_path_buf();
        drop(case);
        assert!(!root.exists(), "materialized case was not removed: {name}");
    }
    Ok(())
}
