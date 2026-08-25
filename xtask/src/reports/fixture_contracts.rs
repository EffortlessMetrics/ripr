use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const CORPUS_ROOT: &str = "fixtures/source_promotion_resolved_tree";
const RECEIPT_SCHEMA: &str = "ripr.source_promotion_resolved_tree_validation.v1";
const REQUIRED_ROOT_MEMBERS: &[&str] = &["SPEC.md", "expected"];
const REQUIRED_EXPECTED_MEMBERS: &[&str] = &[
    "rejected.json",
    "rejected.md",
    "validated.json",
    "validated.md",
];
const GENERIC_ANALYZER_TOKENS: &[&str] = &[
    "Given",
    "When",
    "Then",
    "Must Not",
    "diff.patch",
    "expected/check.json",
    "from-to-language",
    "language metadata",
];

pub(crate) fn check_fixture_contracts_report() -> Result<(), String> {
    match crate::check_fixture_contracts() {
        Ok(()) => {}
        Err(error) => {
            if let Some(remaining) = without_snapshot_category_errors(&error) {
                return Err(remaining);
            }
        }
    }

    let mut violations = Vec::new();
    validate_source_promotion_resolved_tree_corpus(Path::new(CORPUS_ROOT), &mut violations)?;
    if violations.is_empty() {
        println!("fixture contracts are valid");
        return Ok(());
    }
    Err(format!(
        "fixture contracts failed:\n- {}",
        violations.join("\n- ")
    ))
}

fn without_snapshot_category_errors(error: &str) -> Option<String> {
    let retained = error
        .lines()
        .filter(|line| !is_snapshot_category_error(line))
        .collect::<Vec<_>>();
    let has_retained_violation = retained.iter().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty()
            && !matches!(
                trimmed,
                "fixture contracts failed:" | "fixture contract check failed:"
            )
    });
    has_retained_violation.then(|| retained.join("\n"))
}

fn is_snapshot_category_error(line: &str) -> bool {
    line.contains(CORPUS_ROOT)
        && GENERIC_ANALYZER_TOKENS
            .iter()
            .any(|token| line.contains(token))
}

fn validate_source_promotion_resolved_tree_corpus(
    root: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !root.is_dir() {
        violations.push(format!(
            "source-promotion receipt corpus is missing {}",
            display(root)
        ));
        return Ok(());
    }

    require_exact_members(root, REQUIRED_ROOT_MEMBERS, violations)?;
    validate_spec(&root.join("SPEC.md"), violations)?;

    let expected = root.join("expected");
    if !expected.is_dir() {
        return Ok(());
    }
    require_exact_members(&expected, REQUIRED_EXPECTED_MEMBERS, violations)?;
    validate_receipt_pair(&expected, "validated", true, violations)?;
    validate_receipt_pair(&expected, "rejected", false, violations)
}

fn require_exact_members(
    root: &Path,
    required: &[&str],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !root.is_dir() {
        violations.push(format!("{} is missing", display(root)));
        return Ok(());
    }
    let observed = fs::read_dir(root)
        .map_err(|error| format!("failed to read {}: {error}", display(root)))?
        .map(|entry| {
            entry
                .map_err(|error| format!("failed to read fixture entry: {error}"))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let required = required
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    for missing in required.difference(&observed) {
        violations.push(format!("{} is missing {missing}", display(root)));
    }
    for unexpected in observed.difference(&required) {
        violations.push(format!(
            "{} has unexpected member {unexpected}",
            display(root)
        ));
    }
    Ok(())
}

fn validate_spec(path: &Path, violations: &mut Vec<String>) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", display(path)))?;
    for required in [
        "Spec: RIPR-SPEC-0150",
        "dedicated receipt-snapshot corpus",
        "dedicated validator",
        "not a Given/When/Then analyzer fixture",
    ] {
        if !text.contains(required) {
            violations.push(format!("{} is missing `{required}`", display(path)));
        }
    }
    Ok(())
}

fn validate_receipt_pair(
    expected: &Path,
    stem: &str,
    expected_admissible: bool,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let json_path = expected.join(format!("{stem}.json"));
    let markdown_path = expected.join(format!("{stem}.md"));
    if !json_path.is_file() || !markdown_path.is_file() {
        return Ok(());
    }

    let value = match read_json(&json_path) {
        Ok(value) => value,
        Err(error) => {
            violations.push(error);
            return Ok(());
        }
    };
    if value.get("schema").and_then(Value::as_str) != Some(RECEIPT_SCHEMA) {
        violations.push(format!("{} has wrong receipt schema", display(&json_path)));
    }
    if value.get("status").and_then(Value::as_str) != Some(stem) {
        violations.push(format!("{} must have status {stem}", display(&json_path)));
    }
    let admissible =
        super::source_promotion_validate_resolved_tree::resolved_tree_receipt_is_admissible(&value);
    if admissible != expected_admissible {
        violations.push(format!(
            "{} admission result was {admissible}, expected {expected_admissible}",
            display(&json_path)
        ));
    }

    let markdown = fs::read_to_string(&markdown_path)
        .map_err(|error| format!("failed to read {}: {error}", display(&markdown_path)))?;
    validate_markdown_mirror(&markdown_path, &markdown, stem, &value, violations)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", display(path)))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("{} is invalid JSON: {error}", display(path)))
}

fn validate_markdown_mirror(
    path: &Path,
    markdown: &str,
    status: &str,
    value: &Value,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let schema_line = format!("- Schema: `{RECEIPT_SCHEMA}`");
    let status_line = format!("- Status: **{status}**");
    if !markdown.contains(&schema_line) {
        violations.push(format!(
            "{} is missing canonical schema line",
            display(path)
        ));
    }
    if !markdown.contains(&status_line) {
        violations.push(format!(
            "{} is missing canonical status line",
            display(path)
        ));
    }

    let canonical = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", display(path)))?;
    let embedded = markdown
        .split_once("```json\n")
        .and_then(|(_, rest)| rest.rsplit_once("\n```\n"))
        .map(|(json, _)| json);
    if embedded != Some(canonical.as_str()) {
        violations.push(format!("{} does not embed canonical JSON", display(path)));
    }
    Ok(())
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{
        CORPUS_ROOT, validate_source_promotion_resolved_tree_corpus,
        without_snapshot_category_errors,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn create(label: &str) -> Result<Self, String> {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ripr-fixture-contract-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).map_err(|error| error.to_string())?;
            Ok(Self(path))
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn snapshot_category_filter_preserves_unrelated_violations() {
        let error = format!(
            "fixture contracts failed:\n- {CORPUS_ROOT}/SPEC.md is missing Given\n- fixtures/boundary_gap is missing diff.patch"
        );
        assert_eq!(
            without_snapshot_category_errors(&error).as_deref(),
            Some("fixture contracts failed:\n- fixtures/boundary_gap is missing diff.patch")
        );
    }

    #[test]
    fn checked_in_snapshot_corpus_is_valid() -> Result<(), String> {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest must have a parent".to_string())?;
        let mut violations = Vec::new();
        validate_source_promotion_resolved_tree_corpus(&repo.join(CORPUS_ROOT), &mut violations)?;
        assert_eq!(violations, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn snapshot_corpus_fails_closed_on_missing_and_malformed_members() -> Result<(), String> {
        let root = TempRoot::create("invalid")?;
        fs::create_dir(root.0.join("expected")).map_err(|error| error.to_string())?;
        fs::write(
            root.0.join("SPEC.md"),
            "Spec: RIPR-SPEC-0150\ndedicated receipt-snapshot corpus\ndedicated validator\nnot a Given/When/Then analyzer fixture\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(root.0.join("expected/validated.json"), "{\n")
            .map_err(|error| error.to_string())?;
        fs::write(root.0.join("expected/validated.md"), "invalid\n")
            .map_err(|error| error.to_string())?;
        fs::write(
            root.0.join("expected/rejected.json"),
            r#"{"schema":"wrong","status":"validated"}"#,
        )
        .map_err(|error| error.to_string())?;

        let mut violations = Vec::new();
        validate_source_promotion_resolved_tree_corpus(&root.0, &mut violations)?;
        let report = violations.join("\n");
        assert!(report.contains("missing rejected.md"));
        assert!(report.contains("invalid JSON"));
        assert!(report.contains("wrong receipt schema"));
        assert!(report.contains("must have status rejected"));
        Ok(())
    }
}
