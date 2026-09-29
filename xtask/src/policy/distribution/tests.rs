use super::*;

const CONTRACT_TEXT: &str = include_str!("../../../../policy/distribution.toml");
const WORKSPACE_TEXT: &str = include_str!("../../../../Cargo.toml");
const CRATE_TEXT: &str = include_str!("../../../../crates/ripr/Cargo.toml");

fn parsed_contract(text: &str) -> Result<DistributionContract, String> {
    parse_distribution_contract(CONTRACT_PATH, text)
}

#[test]
fn valid_contract_matches_workspace_and_crate() -> Result<(), String> {
    let contract = parsed_contract(CONTRACT_TEXT)?;
    let violations = evaluate_contract(
        CONTRACT_PATH,
        &contract,
        WORKSPACE_MANIFEST_PATH,
        WORKSPACE_TEXT,
        CRATE_MANIFEST_PATH,
        CRATE_TEXT,
    );
    assert!(violations.is_empty(), "{violations:#?}");
    Ok(())
}

#[test]
fn duplicate_target_and_wrong_payload_are_rejected() -> Result<(), String> {
    let mutated = CONTRACT_TEXT
        .replace("aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu")
        .replace(
            "@effortlessmetrics/ripr-win32-x64-msvc",
            "@effortlessmetrics/ripr-win32-arm64-msvc",
        );
    let contract = parsed_contract(&mutated)?;
    let violations = evaluate_contract(
        CONTRACT_PATH,
        &contract,
        WORKSPACE_MANIFEST_PATH,
        WORKSPACE_TEXT,
        CRATE_MANIFEST_PATH,
        CRATE_TEXT,
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("duplicate target"))
    );
    assert!(violations.iter().any(|violation| {
        violation.contains("field npm_package") && violation.contains("ripr-win32-x64-msvc")
    }));
    Ok(())
}

#[test]
fn missing_target_is_rejected() -> Result<(), String> {
    let start = CONTRACT_TEXT
        .find("[[target]]\nrust_target = \"aarch64-apple-darwin\"")
        .ok_or_else(|| "missing macOS ARM64 fixture anchor".to_string())?;
    let end = CONTRACT_TEXT
        .find("[[target]]\nrust_target = \"x86_64-pc-windows-msvc\"")
        .ok_or_else(|| "missing Windows x64 fixture anchor".to_string())?;
    let mutated = format!("{}{}", &CONTRACT_TEXT[..start], &CONTRACT_TEXT[end..]);
    let contract = parsed_contract(&mutated)?;
    let violations = evaluate_contract(
        CONTRACT_PATH,
        &contract,
        WORKSPACE_MANIFEST_PATH,
        WORKSPACE_TEXT,
        CRATE_MANIFEST_PATH,
        CRATE_TEXT,
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("missing target `aarch64-apple-darwin`"))
    );
    Ok(())
}

#[test]
fn stale_feature_contract_is_rejected() -> Result<(), String> {
    let mutated = CONTRACT_TEXT.replace(
        "features = [\"lang-python\", \"lang-rust\", \"lang-typescript\"]",
        "features = [\"lang-rust\"]",
    );
    let contract = parsed_contract(&mutated)?;
    let violations = evaluate_contract(
        CONTRACT_PATH,
        &contract,
        WORKSPACE_MANIFEST_PATH,
        WORKSPACE_TEXT,
        CRATE_MANIFEST_PATH,
        CRATE_TEXT,
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("product.features"))
    );
    Ok(())
}

#[test]
fn independent_crate_version_is_rejected() -> Result<(), String> {
    let contract = parsed_contract(CONTRACT_TEXT)?;
    let mutated_crate = CRATE_TEXT.replace("version.workspace = true", "version = \"9.9.9\"");
    let violations = evaluate_contract(
        CONTRACT_PATH,
        &contract,
        WORKSPACE_MANIFEST_PATH,
        WORKSPACE_TEXT,
        CRATE_MANIFEST_PATH,
        &mutated_crate,
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("package.version.workspace must be true"))
    );
    Ok(())
}
