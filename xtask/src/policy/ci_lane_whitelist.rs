use std::fs;
use std::path::Path;

const BUDGET_PATH: &str = "policy/ci-budget.toml";
const WHITELIST_PATH: &str = "policy/ci-lane-whitelist.toml";
const EXCEPTIONS_PATH: &str = "policy/ci-whitelist-exceptions.toml";
const RISK_PACKS_PATH: &str = "policy/ci-risk-packs.toml";

const SCHEMA_TOKEN: &str = "schema_version";

pub(crate) fn check_ci_lane_whitelist() -> Result<(), String> {
    let mut findings: Vec<String> = Vec::new();

    for path in [BUDGET_PATH, WHITELIST_PATH, EXCEPTIONS_PATH, RISK_PACKS_PATH] {
        verify_ledger(Path::new(path), &mut findings);
    }

    let lane_count = count_blocks(Path::new(WHITELIST_PATH), "[[lane]]")?;
    let pack_count = count_inline_blocks(Path::new(RISK_PACKS_PATH), "[risk_pack.")?;

    println!(
        "ci-lane-whitelist: lanes={lane_count}, risk_packs={pack_count}, files={files}",
        files = 4 - findings.iter().filter(|f| f.starts_with("missing:")).count()
    );

    if lane_count == 0 {
        findings.push(format!("{WHITELIST_PATH}: no [[lane]] entries found"));
    }
    if pack_count == 0 {
        findings.push(format!(
            "{RISK_PACKS_PATH}: no [risk_pack.<id>] entries found"
        ));
    }

    if findings.is_empty() {
        println!("ci-lane-whitelist: ok (advisory)");
        return Ok(());
    }

    for finding in &findings {
        println!("ci-lane-whitelist: {finding}");
    }
    println!(
        "ci-lane-whitelist: {} finding(s) (advisory; see docs/ci/ci-lane-whitelist.md)",
        findings.len()
    );
    Ok(())
}

fn verify_ledger(path: &Path, findings: &mut Vec<String>) {
    match fs::read_to_string(path) {
        Ok(text) => {
            if !text.contains(SCHEMA_TOKEN) {
                findings.push(format!(
                    "{}: missing `{SCHEMA_TOKEN}` declaration",
                    path.display()
                ));
            }
        }
        Err(_) => {
            findings.push(format!("missing: {}", path.display()));
        }
    }
}

fn count_blocks(path: &Path, marker: &str) -> Result<usize, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text
            .lines()
            .filter(|line| line.trim() == marker)
            .count()),
        Err(_) => Ok(0),
    }
}

fn count_inline_blocks(path: &Path, prefix: &str) -> Result<usize, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text
            .lines()
            .filter(|line| line.trim_start().starts_with(prefix))
            .count()),
        Err(_) => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_blocks_returns_zero_for_missing_file() {
        let result = count_blocks(Path::new("does-not-exist.toml"), "[[lane]]");
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn count_inline_blocks_returns_zero_for_missing_file() {
        let result = count_inline_blocks(Path::new("does-not-exist.toml"), "[risk_pack.");
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn check_runs_against_repo_ledgers() {
        let result = check_ci_lane_whitelist();
        assert!(result.is_ok(), "advisory check must not fail; got {result:?}");
    }
}
