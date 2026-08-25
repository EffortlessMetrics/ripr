use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())
}

fn read(root: &Path, path: &str) -> Result<String, String> {
    fs::read_to_string(root.join(path)).map_err(|error| format!("read {path}: {error}"))
}

#[test]
fn source_promotion_output_contract_vocabulary_is_registered_and_checked() -> Result<(), String> {
    let root = repository_root()?;
    let registry = read(&root, "policy/output_contracts.txt")?;
    let checker = read(&root, "xtask/src/main.rs")?;
    let schema = read(&root, "docs/OUTPUT_SCHEMA.md")?;

    for kind in [
        "source_promotion_resolved_tree_schema",
        "source_promotion_resolved_tree_status",
        "source_promotion_command_state",
        "source_promotion_path_role",
    ] {
        if !registry
            .lines()
            .any(|line| line.starts_with(&format!("{kind}|")))
        {
            return Err(format!("output-contract registry is missing kind {kind}"));
        }
        if !checker.contains(&format!("\"{kind}\"")) {
            return Err(format!("output-contract checker does not own kind {kind}"));
        }
    }

    for value in [
        "ripr.source_promotion_resolved_tree_validation.v1",
        "rejected",
        "validated",
        "failed",
        "not_run",
        "passed",
        "unavailable",
        "os_temp_disposable_checkout",
        "source_checkout_regular_file",
    ] {
        if !schema.contains(value) {
            return Err(format!(
                "output schema does not document contract value {value}"
            ));
        }
    }

    Ok(())
}
