//! `check-covered-by` — resolve test-valued `covered_by` claims in policy
//! ledgers against the workspace's actual test inventory (issue #3528), and
//! fail-close `policy/clippy-exceptions.toml` structure and expiry (issue
//! #3867).
//!
//! Scope: ledgers whose `covered_by` is not already enforced by another gate.
//! `policy/non-rust-allowlist.toml` is owned by `check-file-policy` (#3528 /
//! #3551 own that gate's enumeration), so it stays out of this ledger set.
//! Today this gate owns `policy/clippy-exceptions.toml`: required nonblank
//! fields, unique ids, optional ISO `expires` that are not in the past, and
//! test-valued `covered_by` resolution. It does not match every
//! `.ripr/allow-attributes.txt` count row, and it does not pay the live
//! exception.
//!
//! Only `cargo test ...` commands are statically resolvable, so only those
//! are validated here; other command shapes (`cargo xtask check-*`, npm
//! scripts) are out of scope and pass through untouched. Every failure names
//! the ledger file, the entry id and line, and the repair (update the
//! reference to the current test name, or drop the claim).

use std::collections::BTreeSet;
use std::path::Path;

use crate::{FixKind, PolicyReportSpec, finish_policy_report, is_cargo_test_command};

use super::test_inventory::{TestInventory, parse_cargo_test_command};

/// Policy ledgers validated by this gate: `(path, entry section header)`.
const COVERED_BY_LEDGERS: &[(&str, &str)] = &[("policy/clippy-exceptions.toml", "[[exception]]")];
const CLIPPY_EXCEPTIONS_PATH: &str = "policy/clippy-exceptions.toml";

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
    let exceptions_text = crate::read_text_lossy(Path::new(CLIPPY_EXCEPTIONS_PATH))?;
    let (entries, structural) =
        parse_and_check_exceptions(&exceptions_text, &crate::no_panic::today_date_string());
    violations.extend(structural);
    for entry in &entries {
        let covered = CoveredByEntry {
            id: Some(entry.id.clone()),
            id_line: Some(entry.block_line),
            covered_by_line: entry.block_line,
            commands: entry.commands.clone(),
        };
        validate_entry(
            CLIPPY_EXCEPTIONS_PATH,
            &covered,
            &inventory,
            &mut violations,
        );
    }
    for (path, section) in COVERED_BY_LEDGERS {
        if *path == CLIPPY_EXCEPTIONS_PATH {
            continue;
        }
        let entries = read_covered_by_entries(path, section)?;
        for entry in entries {
            validate_entry(path, &entry, &inventory, &mut violations);
        }
    }
    finish_policy_report(
        PolicyReportSpec {
            report_file: "covered-by.md",
            check: "check-covered-by",
            why_it_matters: "A `covered_by` entry that names a renamed or deleted test is a false-confidence receipt: nothing proves the suppressed surface is still exercised. An `expires` date or required field that the ledger records but no gate reads is the same class of unread contract.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Update `covered_by` to the current test name (enumerate with `cargo test -p <package> -- --list --format terse`).",
                "Drop the `covered_by` claim if the coverage no longer exists, and say so in the ledger reason.",
                "Renew, remove, or drop an exception whose `expires` date is in the past.",
                "Fill required [[exception]] fields (`id`, `lint`, `path`, `selector`, `owner`, `reason`, `covered_by`).",
            ],
            rerun_command: "cargo xtask check-covered-by",
            exception_template: Some(
                "ledger entry:\n[[exception]]\nid = \"...\"\ncovered_by = [\"cargo test -p <package> <current-test-filter>\"]",
            ),
        },
        &violations,
    )
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
struct ClippyExceptionsFile {
    schema_version: String,
    policy: String,
    owner: String,
    status: String,
    exception: Vec<ClippyExceptionRow>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
struct ClippyExceptionRow {
    id: String,
    lint: String,
    path: String,
    selector: String,
    owner: String,
    reason: String,
    covered_by: Option<ClippyExceptionCoveredBy>,
    expires: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum ClippyExceptionCoveredBy {
    One(String),
    Many(Vec<String>),
}

struct ClippyExceptionEntry {
    id: String,
    expires: Option<String>,
    block_line: usize,
    commands: Vec<String>,
}

fn covered_by_is_blank(value: &Option<ClippyExceptionCoveredBy>) -> bool {
    match value {
        None => true,
        Some(ClippyExceptionCoveredBy::One(command)) => command.trim().is_empty(),
        Some(ClippyExceptionCoveredBy::Many(commands)) => {
            commands.iter().all(|command| command.trim().is_empty())
        }
    }
}

fn covered_by_commands(value: &ClippyExceptionCoveredBy) -> Vec<String> {
    match value {
        ClippyExceptionCoveredBy::One(command) => {
            let command = command.trim();
            if command.is_empty() {
                Vec::new()
            } else {
                vec![command.to_string()]
            }
        }
        ClippyExceptionCoveredBy::Many(commands) => commands
            .iter()
            .map(|command| command.trim().to_string())
            .filter(|command| !command.is_empty())
            .collect(),
    }
}

fn next_exception_header_line(text: &str, after_line: usize) -> usize {
    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        if line_number <= after_line {
            continue;
        }
        if raw.trim() == "[[exception]]" {
            return line_number;
        }
    }
    0
}

fn id_line_in_block(text: &str, id: &str, block_line: usize) -> usize {
    if id.is_empty() {
        return block_line;
    }
    let double = format!("id = \"{id}\"");
    let single = format!("id = '{id}'");
    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        if line_number <= block_line {
            continue;
        }
        let line = raw.trim();
        if line == "[[exception]]" {
            break;
        }
        if line.starts_with(&double) || line.starts_with(&single) {
            return line_number;
        }
    }
    block_line
}

fn parse_clippy_exceptions_ledger(text: &str) -> (Vec<ClippyExceptionEntry>, Vec<String>) {
    let mut violations = Vec::new();
    if text.trim().is_empty() {
        return (Vec::new(), violations);
    }
    let parsed: ClippyExceptionsFile = match toml::from_str(text) {
        Ok(file) => file,
        Err(err) => {
            violations.push(format!("{CLIPPY_EXCEPTIONS_PATH}: {err}"));
            return (Vec::new(), violations);
        }
    };

    let mut entries = Vec::new();
    let mut last_block_line = 0;
    for row in parsed.exception {
        let entry_id = row.id.trim().to_string();
        let lint = row.lint.trim().to_string();
        let path = row.path.trim().to_string();
        let selector = row.selector.trim().to_string();
        let owner = row.owner.trim().to_string();
        let reason = row.reason.trim().to_string();
        let expires = row.expires.map(|value| value.trim().to_string());
        let block_line = next_exception_header_line(text, last_block_line);
        last_block_line = block_line;
        let id_line = id_line_in_block(text, &entry_id, block_line);
        let mut missing = Vec::new();
        if entry_id.is_empty() {
            missing.push("id");
        }
        if lint.is_empty() {
            missing.push("lint");
        }
        if path.is_empty() {
            missing.push("path");
        }
        if selector.is_empty() {
            missing.push("selector");
        }
        if owner.is_empty() {
            missing.push("owner");
        }
        if reason.is_empty() {
            missing.push("reason");
        }
        if covered_by_is_blank(&row.covered_by) {
            missing.push("covered_by");
        }
        if !missing.is_empty() {
            let label = if entry_id.is_empty() {
                format!("{CLIPPY_EXCEPTIONS_PATH}:{block_line}")
            } else {
                format!("{CLIPPY_EXCEPTIONS_PATH}:{id_line} `{entry_id}`")
            };
            violations.push(format!(
                "{label} missing required field{}: {}",
                if missing.len() == 1 { "" } else { "s" },
                missing.join(", ")
            ));
            continue;
        }
        entries.push(ClippyExceptionEntry {
            id: entry_id,
            expires,
            block_line: id_line,
            commands: row
                .covered_by
                .as_ref()
                .map(covered_by_commands)
                .unwrap_or_default(),
        });
    }
    (entries, violations)
}

fn collect_clippy_exception_row_violations(
    entries: &[ClippyExceptionEntry],
    today: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut seen_ids = BTreeSet::new();
    for entry in entries {
        if !seen_ids.insert(entry.id.clone()) {
            violations.push(format!(
                "{CLIPPY_EXCEPTIONS_PATH}:{} duplicate exception id `{}`.",
                entry.block_line, entry.id
            ));
        }
        let Some(expires) = entry.expires.as_deref() else {
            continue;
        };
        if expires.is_empty() || !crate::no_panic::is_valid_iso_date(expires) {
            violations.push(format!(
                "{CLIPPY_EXCEPTIONS_PATH}:{} `{}` has malformed expires `{expires}`; expected YYYY-MM-DD.",
                entry.block_line, entry.id
            ));
        } else if expires < today {
            violations.push(format!(
                "{CLIPPY_EXCEPTIONS_PATH}:{} `{}` has expires `{expires}` before today `{today}`. Renew, remove, or drop the exception.",
                entry.block_line, entry.id
            ));
        }
    }
    violations
}

fn parse_and_check_exceptions(text: &str, today: &str) -> (Vec<ClippyExceptionEntry>, Vec<String>) {
    let (entries, mut violations) = parse_clippy_exceptions_ledger(text);
    violations.extend(collect_clippy_exception_row_violations(&entries, today));
    (entries, violations)
}

#[cfg(test)]
fn collect_clippy_exception_violations(text: &str, today: &str) -> Vec<String> {
    parse_and_check_exceptions(text, today).1
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
    use super::{
        CoveredByEntry, collect_clippy_exception_violations, parse_and_check_exceptions,
        read_covered_by_entries, validate_entry,
    };
    use crate::policy::test_inventory::TestInventory;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    const TODAY: &str = "2026-09-22";
    const LIVE_SHAPED: &str = r#"
schema_version = "1.0"
policy = "clippy-exceptions"
owner = "core/rust"
status = "active"

[[exception]]
id = "clippy-exception-0001"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "V2 grows with exact-identity snippet/count fields; boxing is a post-0.5.1 refactor"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"
"#;

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

    #[test]
    fn check_covered_by_parses_clippy_exceptions_and_rejects_stale_or_invalid_rows() {
        assert!(
            collect_clippy_exception_violations(LIVE_SHAPED, TODAY).is_empty(),
            "live-shaped exception row must pass"
        );
        assert!(
            collect_clippy_exception_violations("", TODAY).is_empty(),
            "empty exceptions file must still be allowed"
        );

        let omitted_expires = r#"
[[exception]]
id = "clippy-exception-0002"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "expires is optional"
covered_by = "cargo test -p xtask no_panic"
"#;
        assert!(
            collect_clippy_exception_violations(omitted_expires, TODAY).is_empty(),
            "omitted expires must pass"
        );

        let today_expires = r#"
[[exception]]
id = "clippy-exception-0003"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "expires equal to today still valid"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-09-22"
"#;
        assert!(
            collect_clippy_exception_violations(today_expires, TODAY).is_empty(),
            "expires == today must pass"
        );

        let array_covered_by = r#"
[[exception]]
id = "clippy-exception-0011"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "array covered_by is valid"
covered_by = ["cargo test -p xtask no_panic"]
expires = "2026-12-01"
"#;
        assert!(
            collect_clippy_exception_violations(array_covered_by, TODAY).is_empty(),
            "array covered_by must pass structural parse"
        );

        let missing = r#"
[[exception]]
id = "clippy-exception-0004"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
"#;
        let violations = collect_clippy_exception_violations(missing, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| row.contains("clippy-exception-0004")
                    && row.contains("missing required field")
                    && row.contains("selector")
                    && row.contains("owner")
                    && row.contains("reason")
                    && row.contains("covered_by")),
            "missing required fields must fail: {violations:?}"
        );

        let duplicate = r#"
[[exception]]
id = "clippy-exception-0001"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "first copy"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"

[[exception]]
id = "clippy-exception-0001"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "second copy"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"
"#;
        let violations = collect_clippy_exception_violations(duplicate, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| { row.contains("duplicate exception id `clippy-exception-0001`") }),
            "duplicate id must fail: {violations:?}"
        );

        let past = r#"
[[exception]]
id = "clippy-exception-0001"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "expired"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-01-01"
"#;
        let violations = collect_clippy_exception_violations(past, TODAY);
        assert!(
            violations.iter().any(|row| {
                row.contains("clippy-exception-0001")
                    && row.contains("expires `2026-01-01`")
                    && row.contains("before today `2026-09-22`")
            }),
            "past expires must fail: {violations:?}"
        );

        let misspelled_table = r#"
[[exceptions]]
id = "clippy-exception-0001"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "typo table"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"
"#;
        let violations = collect_clippy_exception_violations(misspelled_table, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| row.contains("exceptions") && row.contains("unknown field")),
            "misspelled [[exceptions]] must fail closed: {violations:?}"
        );

        let malformed_header = r#"
[[exception
id = "clippy-exception-0005"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "malformed header"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"
"#;
        let violations = collect_clippy_exception_violations(malformed_header, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| row.contains("policy/clippy-exceptions.toml:")),
            "unclosed [[exception header must fail closed: {violations:?}"
        );

        let impossible_date = r#"
[[exception]]
id = "clippy-exception-0006"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "impossible calendar day"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-02-31"
"#;
        let violations = collect_clippy_exception_violations(impossible_date, TODAY);
        assert!(
            violations.iter().any(|row| {
                row.contains("clippy-exception-0006")
                    && row.contains("malformed expires `2026-02-31`")
            }),
            "impossible calendar date must fail: {violations:?}"
        );

        let whitespace_only = r#"
[[exception]]
id = "   "
lint = "clippy::large_enum_variant"
path = " "
selector = "	"
owner = " "
reason = "	"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"
"#;
        let violations = collect_clippy_exception_violations(whitespace_only, TODAY);
        assert!(
            violations.iter().any(|row| {
                row.contains("missing required field")
                    && row.contains("id")
                    && row.contains("path")
                    && row.contains("selector")
                    && row.contains("owner")
                    && row.contains("reason")
            }),
            "whitespace-only required fields must fail: {violations:?}"
        );

        let duplicate_expires = r#"
[[exception]]
id = "clippy-exception-0008"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "duplicate key"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-01-01"
expires = "2026-12-01"
"#;
        let violations = collect_clippy_exception_violations(duplicate_expires, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| row.contains("duplicate") && row.contains("expires")),
            "duplicate expires key must fail closed: {violations:?}"
        );

        let trailing_garbage = r#"
[[exception]]
id = "clippy-exception-0009"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "trailing garbage"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01" trailing garbage
"#;
        let violations = collect_clippy_exception_violations(trailing_garbage, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| row.contains("policy/clippy-exceptions.toml:")),
            "trailing garbage after a quoted value must fail closed: {violations:?}"
        );

        let unknown_field = r#"
[[exception]]
id = "clippy-exception-0010"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "unknown field"
covered_by = "cargo test -p xtask no_panic"
expires = "2026-12-01"
owners = "typo"
"#;
        let violations = collect_clippy_exception_violations(unknown_field, TODAY);
        assert!(
            violations
                .iter()
                .any(|row| row.contains("owners") && row.contains("unknown field")),
            "unknown field must fail closed: {violations:?}"
        );
    }

    #[test]
    fn toml_literal_covered_by_is_resolved_not_skipped() -> Result<(), String> {
        let text = r#"
[[exception]]
id = "clippy-exception-0012"
lint = "clippy::large_enum_variant"
path = "xtask/src/no_panic.rs"
selector = "enum PanicAllowEntryVersioned"
owner = "core/policy"
reason = "literal string must still resolve"
covered_by = 'cargo test -p alpha definitely_missing'
expires = "2026-12-01"
"#;
        let (entries, structural) = parse_and_check_exceptions(text, TODAY);
        if !structural.is_empty() {
            return Err(format!(
                "literal covered_by failed structural parse: {structural:?}"
            ));
        }
        if entries.len() != 1
            || entries[0].commands.as_slice() != ["cargo test -p alpha definitely_missing"]
        {
            return Err(format!(
                "literal covered_by was not decoded: {:?}",
                entries
                    .iter()
                    .map(|entry| entry.commands.clone())
                    .collect::<Vec<_>>()
            ));
        }
        let inventory = inventory_with("alpha", &["tests::current_case"]);
        let mut violations = Vec::new();
        let covered = CoveredByEntry {
            id: Some(entries[0].id.clone()),
            id_line: Some(entries[0].block_line),
            covered_by_line: entries[0].block_line,
            commands: entries[0].commands.clone(),
        };
        validate_entry(
            "policy/clippy-exceptions.toml",
            &covered,
            &inventory,
            &mut violations,
        );
        if violations.len() != 1 || !violations[0].contains("definitely_missing") {
            return Err(format!(
                "stale literal covered_by was not resolved: {violations:?}"
            ));
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
