#!/usr/bin/env python3
from pathlib import Path

EXPECTED_ANCESTOR = "3e26d62d8d34452a147f5d464443233185cb2104"


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one marker, found {count}: {old[:80]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def patch_main() -> None:
    path = Path("xtask/src/main.rs")
    text = path.read_text(encoding="utf-8")

    marker = '                    | "perl_lsp_facts_exporter"\n'
    insertion = marker + '                    | "source_promotion_resolved_tree"\n'
    if '                    | "source_promotion_resolved_tree"\n' not in text:
        if text.count(marker) != 1:
            raise SystemExit("manifest-only fixture marker drifted")
        text = text.replace(marker, insertion, 1)

    call_marker = "    validate_perl_lsp_facts_exporter_fixture_corpus(&mut violations)?;\n"
    call = call_marker + "    validate_source_promotion_resolved_tree_fixture_corpus(&mut violations)?;\n"
    if "validate_source_promotion_resolved_tree_fixture_corpus(&mut violations)?;" not in text:
        if text.count(call_marker) != 1:
            raise SystemExit("fixture validator call marker drifted")
        text = text.replace(call_marker, call, 1)

    function_marker = "fn validate_perl_lsp_facts_exporter_fixture_corpus(\n"
    if "fn validate_source_promotion_resolved_tree_fixture_corpus(" not in text:
        if text.count(function_marker) != 1:
            raise SystemExit("fixture validator insertion marker drifted")
        helper = r'''const SOURCE_PROMOTION_RESOLVED_TREE_FIXTURE_ROOT: &str =
    "fixtures/source_promotion_resolved_tree";
const SOURCE_PROMOTION_RESOLVED_TREE_REQUIRED_EXPECTED: &[&str] = &[
    "rejected.json",
    "rejected.md",
    "validated.json",
    "validated.md",
];

fn validate_source_promotion_resolved_tree_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    validate_source_promotion_resolved_tree_fixture_corpus_at(
        Path::new(SOURCE_PROMOTION_RESOLVED_TREE_FIXTURE_ROOT),
        violations,
    )
}

fn validate_source_promotion_resolved_tree_fixture_corpus_at(
    root: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !root.is_dir() {
        violations.push(format!(
            "source-promotion resolved-tree fixture corpus is missing {}",
            normalize_path(root)
        ));
        return Ok(());
    }

    let spec = root.join("SPEC.md");
    if !spec.is_file() {
        violations.push(format!("{} is missing SPEC.md", normalize_path(root)));
    } else {
        let text = read_text_lossy(&spec)?;
        if !text.lines().any(|line| line == "Spec: RIPR-SPEC-0150") {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0150`",
                normalize_path(&spec)
            ));
        }
        for required in [
            "dedicated receipt-snapshot corpus",
            "dedicated validator",
            "not a Given/When/Then analyzer fixture",
        ] {
            if !text.contains(required) {
                violations.push(format!(
                    "{} is missing dedicated-corpus statement `{required}`",
                    normalize_path(&spec)
                ));
            }
        }
    }

    let expected_dir = root.join("expected");
    if !expected_dir.is_dir() {
        violations.push(format!(
            "{} is missing expected directory",
            normalize_path(root)
        ));
        return Ok(());
    }

    let mut observed = fs::read_dir(&expected_dir)
        .map_err(|err| format!("failed to read {}: {err}", normalize_path(&expected_dir)))?
        .map(|entry| {
            entry
                .map_err(|err| format!("failed to read fixture entry: {err}"))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let required = SOURCE_PROMOTION_RESOLVED_TREE_REQUIRED_EXPECTED
        .iter()
        .map(|name| (*name).to_string())
        .collect::<std::collections::BTreeSet<_>>();
    for missing in required.difference(&observed) {
        violations.push(format!(
            "{} is missing expected/{missing}",
            normalize_path(root)
        ));
    }
    for unexpected in observed.difference(&required) {
        violations.push(format!(
            "{} has unexpected expected/{unexpected}",
            normalize_path(root)
        ));
    }
    observed.clear();

    for (stem, expected_status, expected_admissible) in [
        ("validated", "validated", true),
        ("rejected", "rejected", false),
    ] {
        let json_path = expected_dir.join(format!("{stem}.json"));
        if !json_path.is_file() {
            continue;
        }
        let value = match read_json_value(&json_path) {
            Ok(value) => value,
            Err(error) => {
                violations.push(format!(
                    "{} is invalid JSON: {error}",
                    normalize_path(&json_path)
                ));
                continue;
            }
        };
        if value.get("schema").and_then(Value::as_str)
            != Some("ripr.source_promotion_resolved_tree_validation.v1")
        {
            violations.push(format!(
                "{} has wrong resolved-tree receipt schema",
                normalize_path(&json_path)
            ));
        }
        if value.get("status").and_then(Value::as_str) != Some(expected_status) {
            violations.push(format!(
                "{} must have status {expected_status}",
                normalize_path(&json_path)
            ));
        }
        let admissible = reports::resolved_tree_receipt_is_admissible(&value);
        if admissible != expected_admissible {
            violations.push(format!(
                "{} admission result was {admissible}, expected {expected_admissible}",
                normalize_path(&json_path)
            ));
        }

        let markdown_path = expected_dir.join(format!("{stem}.md"));
        if !markdown_path.is_file() {
            continue;
        }
        let expected_markdown =
            reports::render_source_promotion_resolved_tree_markdown(&value)?;
        let observed_markdown = read_text_lossy(&markdown_path)?;
        if observed_markdown != expected_markdown {
            violations.push(format!(
                "{} does not mirror the canonical {stem} JSON receipt",
                normalize_path(&markdown_path)
            ));
        }
    }

    Ok(())
}

'''
        text = text.replace(function_marker, helper + function_marker, 1)

    test_marker = "    #[test]\n    fn evidence_quality_benchmark_is_manifest_only_fixture_dir() -> Result<(), String> {\n"
    if "fn source_promotion_resolved_tree_fixture_corpus_is_valid" not in text:
        if text.count(test_marker) != 1:
            raise SystemExit("fixture test insertion marker drifted")
        tests = r'''    #[test]
    fn source_promotion_resolved_tree_fixture_corpus_is_valid() -> Result<(), String> {
        assert!(super::is_manifest_only_fixture_dir(Path::new(
            "fixtures/source_promotion_resolved_tree"
        )));
        with_repo_cwd(|| {
            let mut violations = Vec::new();
            super::validate_source_promotion_resolved_tree_fixture_corpus(&mut violations)?;
            assert_eq!(violations, Vec::<String>::new());
            Ok(())
        })
    }

    #[test]
    fn source_promotion_resolved_tree_fixture_guard_fails_closed() -> Result<(), String> {
        let root = temp_dir("source-promotion-resolved-tree-invalid");
        fs::create_dir_all(root.join("expected")).map_err(|err| err.to_string())?;
        write(
            &root.join("SPEC.md"),
            "Spec: RIPR-SPEC-0150\n\ndedicated receipt-snapshot corpus\ndedicated validator\nnot a Given/When/Then analyzer fixture\n",
        );
        write(&root.join("expected/validated.json"), "{\n");
        write(&root.join("expected/validated.md"), "invalid\n");
        write(
            &root.join("expected/rejected.json"),
            r#"{"schema":"wrong","status":"validated"}"#,
        );
        let mut violations = Vec::new();
        super::validate_source_promotion_resolved_tree_fixture_corpus_at(
            &root,
            &mut violations,
        )?;
        let report = violations.join("\n");
        assert!(report.contains("missing expected/rejected.md"));
        assert!(report.contains("invalid JSON"));
        assert!(report.contains("wrong resolved-tree receipt schema"));
        assert!(report.contains("must have status rejected"));
        assert!(report.contains("admission result was false, expected true"));
        Ok(())
    }

'''
        text = text.replace(test_marker, tests + test_marker, 1)

    path.write_text(text, encoding="utf-8")


def patch_validator_exports() -> None:
    io = Path("xtask/src/reports/source_promotion_validate_resolved_tree/io.rs")
    replace_once(io, "fn render_markdown(report: &Value) -> Result<String, String> {", "pub(crate) fn render_markdown(report: &Value) -> Result<String, String> {")

    reports = Path("xtask/src/reports/mod.rs")
    old = '''pub(crate) use source_promotion_validate_resolved_tree::{
    SOURCE_PROMOTION_VALIDATE_RESOLVED_TREE_SUBCOMMAND, source_promotion_validate_resolved_tree,
};'''
    new = '''pub(crate) use source_promotion_validate_resolved_tree::{
    SOURCE_PROMOTION_VALIDATE_RESOLVED_TREE_SUBCOMMAND,
    render_markdown as render_source_promotion_resolved_tree_markdown,
    resolved_tree_receipt_is_admissible, source_promotion_validate_resolved_tree,
};'''
    replace_once(reports, old, new)


def patch_spec() -> None:
    path = Path("fixtures/source_promotion_resolved_tree/SPEC.md")
    path.write_text(
        """# Source-promotion resolved-tree fixture

Spec: RIPR-SPEC-0150

This is a dedicated receipt-snapshot corpus for
`ripr.source_promotion_resolved_tree_validation.v1`. Its dedicated validator
owns the required JSON and Markdown members, schema/status checks, semantic
admission checks, and canonical Markdown mirrors. It is not a Given/When/Then
analyzer fixture and therefore has no `diff.patch` or `expected/check.json`.

The J5 final-tree behavioral corpus lives in
`xtask/tests/source_promotion_resolved_tree.rs` and executes the production
`check-network-policy` command against a Git-tracked temporary repository.
""",
        encoding="utf-8",
    )


def main() -> None:
    patch_main()
    patch_validator_exports()
    patch_spec()


if __name__ == "__main__":
    main()
