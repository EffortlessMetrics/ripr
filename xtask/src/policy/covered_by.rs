//! `check-covered-by` — resolve test-valued `covered_by` claims in policy
//! ledgers against the workspace's actual test inventory (issue #3528).
//!
//! Scope: ledgers whose `covered_by` is not already enforced by another gate.
//! `policy/non-rust-allowlist.toml` is owned by `check-file-policy` (#3528 /
//! #3551 own that gate's enumeration), so it stays out of this ledger set.
//! Today this gate owns `policy/clippy-exceptions.toml`, whose `covered_by`
//! records were documentation-only before this check landed.
//!
//! Only `cargo test ...` commands are statically resolvable, so only those
//! are validated here; other command shapes (`cargo xtask check-*`, npm
//! scripts) are out of scope and pass through untouched. Every failure names
//! the ledger file, the entry id and line, and the repair (update the
//! reference to the current test name, or drop the claim).

use std::path::Path;

use crate::{FixKind, PolicyReportSpec, finish_policy_report, is_cargo_test_command};

use super::test_inventory::{TestInventory, parse_cargo_test_command};

/// Policy ledgers validated by this gate: `(path, entry section header)`.
const COVERED_BY_LEDGERS: &[(&str, &str)] = &[("policy/clippy-exceptions.toml", "[[exception]]")];

pub(crate) fn check_covered_by() -> Result<(), String> {
    let inventory = TestInventory::scan_workspace(Path::new("."))
        .map_err(|error| format!("test-valued `covered_by` enumeration failed: {error}"))?;
    if inventory.is_empty() {
        return Err(
            "test-valued `covered_by` enumeration failed: the static scan found no tests in any \
             workspace package; refusing to validate `covered_by` claims against an empty \
             inventory (check that the checkout has member `src/` trees present)"
                .to_string(),
        );
    }
    let mut violations = Vec::new();
    for (path, section) in COVERED_BY_LEDGERS {
        let entries = read_covered_by_entries(path, section)?;
        for entry in entries {
            validate_entry(path, &entry, &inventory, &mut violations);
        }
    }
    finish_policy_report(
        PolicyReportSpec {
            report_file: "covered-by.md",
            check: "check-covered-by",
            why_it_matters: "A `covered_by` entry that names a renamed or deleted test is a false-confidence receipt: nothing proves the suppressed surface is still exercised, so the claim must resolve against the tests that actually exist.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Update `covered_by` to the current test name (enumerate with `cargo test -p <package> -- --list --format terse`).",
                "Drop the `covered_by` claim if the coverage no longer exists, and say so in the ledger reason.",
            ],
            rerun_command: "cargo xtask check-covered-by",
            exception_template: Some(
                "ledger entry:\n[[exception]]\nid = \"...\"\ncovered_by = [\"cargo test -p <package> <current-test-filter>\"]",
            ),
        },
        &violations,
    )
}

/// One `covered_by` command occurrence with the ledger identity needed to
/// diagnose it: the owning entry's id (when present) and the line of the
/// `covered_by` key.
struct CoveredByEntry {
    id: Option<String>,
    id_line: Option<usize>,
    covered_by_line: usize,
    commands: Vec<String>,
}

fn validate_entry(
    path: &str,
    entry: &CoveredByEntry,
    inventory: &TestInventory,
    violations: &mut Vec<String>,
) {
    for command in &entry.commands {
        if !is_cargo_test_command(command) {
            continue;
        }
        let where_at = entry_identity(path, entry);
        let selection = match parse_cargo_test_command(command) {
            Ok(selection) => selection,
            Err(reason) => {
                violations.push(format!(
                    "{where_at} unsupported test-valued `covered_by`: `{command}`\n  {reason}\n  preferred: keep the command to `cargo test [-p <package>] [flags] <filter>` or cite the check that exercises the surface"
                ));
                continue;
            }
        };
        if let Err(reason) = inventory.resolve(&selection) {
            violations.push(format!(
                "{where_at} test-valued `covered_by` reference could not be resolved: `{command}`\n  {reason}\n  preferred: update `covered_by` to the current test name (enumerate with `cargo test -p <package> -- --list --format terse`) or drop the claim if the coverage no longer exists"
            ));
        }
    }
}

fn entry_identity(path: &str, entry: &CoveredByEntry) -> String {
    match (&entry.id, entry.id_line) {
        (Some(id), Some(line)) => format!("{path}:{line} (entry `{id}`)"),
        (Some(id), None) => format!("{path} (entry `{id}`)"),
        (None, _) => format!("{path}:{}", entry.covered_by_line),
    }
}

/// Read the `covered_by` claims of one ledger. The parser follows the repo's
/// hand-rolled ledger-reader style (see `parse_file_policy_allowlist`): a
/// bounded line scan over `[[section]]` blocks with single-line keys.
/// `covered_by` accepts the quoted-string form and the inline-array form;
/// multi-line arrays fail closed with an actionable message.
fn read_covered_by_entries(path: &str, section: &str) -> Result<Vec<CoveredByEntry>, String> {
    let text = crate::read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();
    let mut in_entry = false;
    let mut current = CoveredByEntry {
        id: None,
        id_line: None,
        covered_by_line: 0,
        commands: Vec::new(),
    };
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == section {
            if in_entry {
                entries.push(current);
            }
            current = CoveredByEntry {
                id: None,
                id_line: None,
                covered_by_line: 0,
                commands: Vec::new(),
            };
            in_entry = true;
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "id" {
            let id = unquote(value);
            if !id.is_empty() {
                current.id = Some(id);
                current.id_line = Some(line_number);
            }
            continue;
        }
        if key != "covered_by" {
            continue;
        }
        current.covered_by_line = line_number;
        if value.starts_with('[') {
            let closing = value.ends_with(']');
            let inner = value.trim_start_matches('[');
            let inner = inner.strip_suffix(']').unwrap_or(inner);
            if !closing {
                return Err(format!(
                    "{path}:{line_number} multi-line `covered_by` arrays are not supported by check-covered-by; keep the array on one line"
                ));
            }
            for item in inner.split(',') {
                let item = unquote(item.trim());
                if !item.is_empty() {
                    current.commands.push(item);
                }
            }
            continue;
        }
        let command = unquote(value);
        if command.is_empty() {
            return Err(format!(
                "{path}:{line_number} `covered_by` must be a non-empty command or array"
            ));
        }
        current.commands.push(command);
    }
    if in_entry {
        entries.push(current);
    }
    Ok(entries)
}

/// Strip one layer of TOML quoting from a scalar value.
fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::{CoveredByEntry, read_covered_by_entries, validate_entry};
    use crate::policy::test_inventory::TestInventory;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("mkdir failed: {error}"))?;
        }
        std::fs::write(path, text).map_err(|error| format!("write failed: {error}"))
    }

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ripr-xtask-check-covered-by-{label}-{}",
            std::process::id()
        ));
        let created = std::fs::create_dir_all(&dir);
        assert!(created.is_ok(), "failed to create temp dir: {created:?}");
        dir
    }

    fn inventory_with(package: &str, names: &[&str]) -> TestInventory {
        let mut packages = BTreeMap::new();
        packages.insert(
            package.to_string(),
            names.iter().map(|name| name.to_string()).collect(),
        );
        TestInventory::from_parts(packages, vec![package.to_string()])
    }

    #[test]
    fn reads_string_and_array_covered_by_with_entry_ids() -> Result<(), String> {
        let path = temp_root("ledger").join("clippy-exceptions.toml");
        write(
            &path,
            "# ledgers\n[[exception]]\nid = \"one\"\ncovered_by = \"cargo test -p alpha one_case\"\n\n[[exception]]\nid = \"two\"\ncovered_by = [\"cargo xtask check-doc-index\", \"cargo test -p alpha two_case\"]\n",
        )?;
        let entries = read_covered_by_entries(&path.to_string_lossy(), "[[exception]]")?;
        let _ = std::fs::remove_file(&path);
        if entries.len() != 2 {
            return Err(format!("expected 2 entries, found {}", entries.len()));
        }
        if entries[0].id.as_deref() != Some("one") || entries[0].commands.len() != 1 {
            return Err("string-form entry was misread".to_string());
        }
        if entries[1].commands.len() != 2 || entries[1].covered_by_line != 8 {
            return Err(format!(
                "array-form entry was misread: {:?} at line {}",
                entries[1].commands, entries[1].covered_by_line
            ));
        }
        Ok(())
    }

    #[test]
    fn existing_reference_passes_and_stale_reference_is_named() -> Result<(), String> {
        let inventory = inventory_with("alpha", &["tests::current_case"]);
        let mut violations = Vec::new();
        let good = entry_with_id(&["cargo test -p alpha tests::current_case"]);
        validate_entry("ledger.toml", &good, &inventory, &mut violations);
        if !violations.is_empty() {
            return Err(format!("valid claim was flagged: {violations:?}"));
        }

        let stale = entry_with_id(&["cargo test -p alpha renamed_case"]);
        validate_entry("ledger.toml", &stale, &inventory, &mut violations);
        if violations.len() != 1 {
            return Err(format!(
                "stale claim was not flagged exactly once: {violations:?}"
            ));
        }
        let violation = &violations[0];
        for needle in [
            "ledger.toml",
            "entry `stale-one`",
            "renamed_case",
            "cargo test -p alpha renamed_case",
            "update `covered_by`",
        ] {
            if !violation.contains(needle) {
                return Err(format!("violation is missing `{needle}`: {violation}"));
            }
        }
        Ok(())
    }

    #[test]
    fn non_test_valued_commands_are_skipped() -> Result<(), String> {
        let inventory = inventory_with("alpha", &[]);
        let entry = entry_with_id(&[
            "cargo xtask check-doc-index",
            "cd editors/vscode && npm run compile",
        ]);
        let mut violations = Vec::new();
        validate_entry("ledger.toml", &entry, &inventory, &mut violations);
        if !violations.is_empty() {
            return Err(format!("non-test commands were flagged: {violations:?}"));
        }
        Ok(())
    }

    #[test]
    fn unsupported_test_command_fails_with_distinct_reason() -> Result<(), String> {
        let inventory = inventory_with("alpha", &["tests::a"]);
        let entry = entry_with_id(&["cargo test --frobnicate a"]);
        let mut violations = Vec::new();
        validate_entry("ledger.toml", &entry, &inventory, &mut violations);
        if violations.len() != 1 || !violations[0].contains("unsupported test-valued") {
            return Err(format!(
                "unsupported command not diagnosed distinctly: {violations:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn multi_line_arrays_fail_closed() -> Result<(), String> {
        let path = temp_root("multiline").join("clippy-exceptions.toml");
        write(
            &path,
            "[[exception]]\nid = \"split\"\ncovered_by = [\n  \"cargo test -p alpha one_case\"\n]\n",
        )?;
        let parsed = read_covered_by_entries(&path.to_string_lossy(), "[[exception]]");
        let _ = std::fs::remove_file(&path);
        let error = match parsed {
            Ok(_) => return Err("multi-line array was accepted".to_string()),
            Err(error) => error,
        };
        if !error.contains("multi-line") {
            return Err(format!("multi-line failure was not actionable: {error}"));
        }
        Ok(())
    }

    /// Build a `CoveredByEntry` from inline commands so validation tests can
    /// name ids and lines the way the reader would.
    fn entry_with_id(commands: &[&str]) -> CoveredByEntry {
        CoveredByEntry {
            id: Some("stale-one".to_string()),
            id_line: Some(41),
            covered_by_line: 42,
            commands: commands.iter().map(|command| command.to_string()).collect(),
        }
    }
}
