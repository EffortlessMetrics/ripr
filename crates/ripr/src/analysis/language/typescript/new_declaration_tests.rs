//! Tests for the new-declaration guard: the opening line of a NEW function
//! whose body carries its own added lines is not a behavior probe.

use super::*;

const PRICE_TS: &str = "export function loyaltyPrice(amount: number, years: number): number {\n  if (years >= 5) {\n    return amount - 100;\n  }\n  return amount;\n}\nexport const bump = (x: number) => x + 1;\n";

const PRICE_TEST_TS: &str = "import { loyaltyPrice, bump } from '../src/price';\ntest('loyalty boundary', () => {\n  expect(loyaltyPrice(1000, 5)).toBe(900);\n  expect(loyaltyPrice(1000, 4)).toBe(1000);\n});\ntest('bump', () => {\n  expect(bump(1)).toBe(2);\n});\n";

fn line(number: usize, text: &str) -> crate::analysis::diff::ChangedLine {
    crate::analysis::diff::ChangedLine {
        line: number,
        new_side_line: number,
        text: text.to_string(),
    }
}

fn source_line(source: &str, number: usize) -> Result<&str, String> {
    source
        .lines()
        .nth(number - 1)
        .ok_or_else(|| format!("fixture has no line {number}"))
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
        "ripr-ts-new-decl-{label}-{}-{nanos}",
        std::process::id()
    ));
    for (path, contents) in [
        ("src/price.ts", source),
        ("tests/price.test.ts", PRICE_TEST_TS),
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
    };
    let mut added_lines = Vec::new();
    for number in added {
        added_lines.push(line(*number, source_line(source, *number)?));
    }
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/price.ts"),
        added_lines,
        removed_lines: removed,
    }];
    let result = TypeScriptAdapter.analyze_diff(&options, &OraclePolicy::default(), &changed_files);
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

/// A new function with an exact boundary test: the declaration line yields no
/// probe, and the body predicate is exposed. Removing the guard in `mod.rs`
/// makes the first assertion fail with a weakly exposed declaration probe.
#[test]
fn new_function_declaration_line_is_not_probed_when_body_is_added() -> Result<(), String> {
    let findings = analyze("new-fn", PRICE_TS, &[1, 2, 3, 4, 5, 6], Vec::new())?;
    assert_eq!(
        classes_on(&findings, 1),
        Vec::<ExposureClass>::new(),
        "the declaration line of a new function has no behavior of its own"
    );
    assert_eq!(
        classes_on(&findings, 2),
        vec![ExposureClass::Exposed],
        "the body predicate carries the behavior and is exposed by the boundary test"
    );
    assert_eq!(classes_on(&findings, 3), vec![ExposureClass::Exposed]);
    Ok(())
}

/// A changed default value is runtime behavior: the paired signature line
/// keeps its probe even though body lines were added too.
#[test]
fn changed_default_value_on_signature_keeps_probe() -> Result<(), String> {
    let source = PRICE_TS.replacen("years: number)", "years = 3)", 1);
    let findings = analyze(
        "changed-default",
        &source,
        &[1, 2, 3],
        vec![line(
            1,
            "export function loyaltyPrice(amount: number, years = 5): number {",
        )],
    )?;
    assert_eq!(classes_on(&findings, 1).len(), 1);
    Ok(())
}

/// A NEW function whose signature carries a default value keeps its probe:
/// the default is behavior a test can discriminate.
#[test]
fn new_function_with_default_value_keeps_probe() -> Result<(), String> {
    let source = PRICE_TS.replacen("years: number)", "years = 3)", 1);
    let findings = analyze("new-default", &source, &[1, 2, 3, 4, 5, 6], Vec::new())?;
    assert_eq!(classes_on(&findings, 1).len(), 1);
    Ok(())
}

/// A one-line arrow function carries its body on the declaration line.
#[test]
fn one_line_arrow_function_keeps_probe() -> Result<(), String> {
    let findings = analyze("arrow", PRICE_TS, &[7], Vec::new())?;
    assert_eq!(classes_on(&findings, 7).len(), 1);
    Ok(())
}

#[test]
fn signature_opening_line_shapes() {
    let file = Path::new("src/t.ts");
    for opening in [
        "export function f(a: number, b: number): number {",
        "function f(a) {",
        "export default async function f(a: string): Promise<void> {",
        "export const f = (a: number): number => {",
        "const f = async (a) => {",
        "const f = function (a) {",
        "total(a: number): number {",
        "get total(): number {",
        "function f(...rest: number[]) {",
    ] {
        assert!(
            is_signature_opening_line(file, opening),
            "expected an opening line: `{opening}`"
        );
    }
    for behavior in [
        "export const f = (x: number) => x + 1;",
        "function f(a = 1) {",
        "function f({ a = 1 }) {",
        "function f(a) { return a;",
        "export const f = (a) =>",
        "constructor(private readonly a: number) {",
        "[key](a) {",
        "@decorate() total(a) {",
        "if (a > 1) {",
        "export function f(",
    ] {
        assert!(
            !is_signature_opening_line(file, behavior),
            "expected a behavior-bearing line: `{behavior}`"
        );
    }
}

/// Review of #4428: a function made `async` while a helper is inserted above
/// it. Git can pair the removed old signature with the inserted helper, so
/// the signature has no in-place removed partner; the old line naming the
/// same function still marks it as a changed signature that keeps its probe.
#[test]
fn changed_signature_paired_elsewhere_by_git_keeps_probe() -> Result<(), String> {
    let source = PRICE_TS.replacen("export function", "export async function", 1);
    let mut removed = line(
        7,
        "export function loyaltyPrice(amount: number, years: number): number {",
    );
    removed.new_side_line = 7;
    let findings = analyze("paired-elsewhere", &source, &[1, 2, 3], vec![removed])?;
    assert_eq!(classes_on(&findings, 1).len(), 1, "{findings:?}");
    Ok(())
}
