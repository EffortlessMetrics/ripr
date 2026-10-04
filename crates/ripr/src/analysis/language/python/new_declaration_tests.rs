//! Tests for the new-declaration guard: the `def` header of a NEW function
//! whose body carries its own added lines is not a behavior probe.

use super::no_behavior::is_new_def_header_without_defaults;
use super::*;
use std::path::PathBuf;

const PRICING_PY: &str = "def loyalty_price(amount: int, years: int) -> int:\n    if years >= 5:\n        return amount - 100\n    return amount\n\n\ndef double(x): return x * 2\n";

/// Off-boundary oracle: neither call sits on the `years >= 5` boundary.
const PRICING_TEST_PY: &str = "from src.pricing import loyalty_price, double\n\n\ndef test_loyalty_price():\n    assert loyalty_price(1_000, 10) == 900\n    assert loyalty_price(1_000, 1) == 1_000\n\n\ndef test_double():\n    assert double(2) == 4\n";

fn line(number: usize, text: &str) -> crate::analysis::diff::ChangedLine {
    crate::analysis::diff::ChangedLine {
        line: number,
        new_side_line: number,
        text: text.to_string(),
    }
}

fn analyze(
    label: &str,
    source: &str,
    added: &[usize],
    removed: Vec<crate::analysis::diff::ChangedLine>,
) -> Result<Vec<Finding>, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system time: {err}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-new-decl-{label}-{}-{nanos}",
        std::process::id()
    ));
    for (path, contents) in [
        ("src/pricing.py", source),
        ("tests/test_pricing.py", PRICING_TEST_PY),
    ] {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create_dir_all({}): {err}", parent.display()))?;
        }
        std::fs::write(&file, contents)
            .map_err(|err| format!("write({}): {err}", file.display()))?;
    }
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: false,
        perl_facts_path: None,
        git_timeout: None,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
        open_rust_index_paths: Default::default(),
    };
    let mut added_lines = Vec::new();
    for number in added {
        let text = source
            .lines()
            .nth(number - 1)
            .ok_or_else(|| format!("fixture has no line {number}"))?;
        added_lines.push(line(*number, text));
    }
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/pricing.py"),
        added_lines,
        removed_lines: removed,
    }];
    let result = PythonAdapter.analyze_diff(&options, &OraclePolicy::default(), &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;
    assert_eq!(result.changed_files, 1, "the changed file must be analyzed");
    Ok(result.findings)
}

fn classes_on(findings: &[Finding], number: usize) -> Vec<ExposureClass> {
    findings
        .iter()
        .filter(|finding| finding.probe.location.line == number)
        .map(|finding| finding.class.clone())
        .collect()
}

/// A new function with an off-boundary test: the `def` header yields no
/// probe (previously it claimed unearned `exposed` credit), while the real
/// `if years >= 5` predicate stays weakly exposed. Removing the guard in
/// `classify.rs` makes the first assertion fail with an `exposed` header.
#[test]
fn new_def_header_is_not_probed_when_body_is_added() -> Result<(), String> {
    let findings = analyze("new-def", PRICING_PY, &[1, 2, 3, 4], Vec::new())?;
    assert_eq!(
        classes_on(&findings, 1),
        Vec::<ExposureClass>::new(),
        "the header of a new function has no behavior of its own"
    );
    assert_eq!(
        classes_on(&findings, 2),
        vec![ExposureClass::WeaklyExposed],
        "the off-boundary test leaves the body predicate weakly exposed"
    );
    Ok(())
}

/// A changed default value on an existing `def` is runtime behavior: the
/// paired header keeps its probe even though body lines were added too.
#[test]
fn changed_default_value_on_def_keeps_probe() -> Result<(), String> {
    let source = PRICING_PY.replacen("years: int)", "years: int = 3)", 1);
    let findings = analyze(
        "changed-default",
        &source,
        &[1, 2],
        vec![line(
            1,
            "def loyalty_price(amount: int, years: int = 5) -> int:",
        )],
    )?;
    assert_eq!(classes_on(&findings, 1).len(), 1);
    Ok(())
}

/// A NEW `def` whose header carries a default value keeps its probe.
#[test]
fn new_def_with_default_value_keeps_probe() -> Result<(), String> {
    let source = PRICING_PY.replacen("years: int)", "years: int = 3)", 1);
    let findings = analyze("new-default", &source, &[1, 2, 3, 4], Vec::new())?;
    assert_eq!(classes_on(&findings, 1).len(), 1);
    Ok(())
}

/// A one-line `def` carries its body on the header line.
#[test]
fn one_line_def_keeps_probe() -> Result<(), String> {
    let findings = analyze("one-line", PRICING_PY, &[7], Vec::new())?;
    assert_eq!(classes_on(&findings, 7).len(), 1);
    Ok(())
}

#[test]
fn new_def_header_shapes() {
    for header in [
        "def f(a, b):",
        "def loyalty_price(amount: int, years: int) -> int:",
        "    async def f(self, *args, **kwargs):",
        "def f(a: list[int] | None, *rest: str, **kw: typing.Any) -> dict[str, int]:",
        "def f(a: \"Forward\") -> tuple[int, ...]:",
    ] {
        assert!(
            is_new_def_header_without_defaults(header),
            "expected a bare header: `{header}`"
        );
    }
    for behavior in [
        "def f(x): return x",
        "def f(x=1):",
        "def f(*, key=None):",
        "def f(",
        "if x >= 5:",
        // Annotations run at definition time without postponed evaluation.
        "def f(x: record_event()):",
        "def f(x) -> make_type():",
        "def f(*args: (lambda: int)()):",
    ] {
        assert!(
            !is_new_def_header_without_defaults(behavior),
            "expected a behavior-bearing line: `{behavior}`"
        );
    }
}

/// Review of #4428: a `def` made `async` while a helper is inserted above it.
/// Git can pair the removed old header with the inserted helper line, so the
/// header has no in-place removed partner; the old `def` of the same name
/// still marks it as a changed signature that keeps its probe.
#[test]
fn changed_header_paired_elsewhere_by_git_keeps_probe() -> Result<(), String> {
    let source = PRICING_PY.replacen("def loyalty_price", "async def loyalty_price", 1);
    let mut removed = line(7, "def loyalty_price(amount: int, years: int) -> int:");
    removed.new_side_line = 7;
    let findings = analyze("paired-elsewhere", &source, &[1, 2, 3, 4], vec![removed])?;
    assert_eq!(classes_on(&findings, 1).len(), 1, "{findings:?}");
    Ok(())
}
