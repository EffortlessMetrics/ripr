//! Ambient declarations (parsed `declare ...` statements and `.d.ts` files)
//! are type-only, so a change to them produces no probe (2026-09-27 re-walk:
//! they produced a `predicate` finding reading `no_static_path`).

use super::*;
use crate::analysis::diff::ChangedLine;
use std::path::PathBuf;

fn changed(path: &str, lines: &[(usize, &str)]) -> ChangedFile {
    ChangedFile {
        path: PathBuf::from(path),
        added_lines: lines
            .iter()
            .map(|(line, text)| ChangedLine {
                line: *line,
                text: (*text).to_string(),
                new_side_line: *line,
            })
            .collect(),
        removed_lines: Vec::new(),
    }
}

fn analyze(
    label: &str,
    files: &[(&str, &str)],
    change: ChangedFile,
) -> Result<LanguageDiffResult, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ripr-ts-ambient-{label}-{nanos}"));
    for (path, source) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::write(&path, source).map_err(|err| err.to_string())?;
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
    let result = TypeScriptAdapter.analyze_diff(&options, &OraclePolicy::default(), &[change]);
    let _ = std::fs::remove_dir_all(&root);
    result
}

#[test]
fn declaration_file_change_produces_no_probe_but_is_counted() -> Result<(), String> {
    let root_files = [(
        "src/types.d.ts",
        "export declare function typed(x: number): string;\nexport function plain(x: number): string;\n",
    )];
    let result = analyze(
        "dts",
        &root_files,
        changed(
            "src/types.d.ts",
            &[
                (1, "export declare function typed(x: number): string;"),
                (2, "export function plain(x: number): string;"),
            ],
        ),
    )?;
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert_eq!(
        result.changed_files, 1,
        "the declaration file stays in the denominator"
    );
    Ok(())
}

#[test]
fn declare_line_in_source_file_is_ignored_but_runtime_line_is_probed() -> Result<(), String> {
    let source = "export declare function typed(x: number): string;\nexport function run(limit: number): number {\n  return limit + 1;\n}\n";
    let findings = analyze(
        "declare-line",
        &[("src/run.ts", source)],
        changed(
            "src/run.ts",
            &[
                (1, "export declare function typed(x: number): string;"),
                (3, "  return limit + 1;"),
            ],
        ),
    )?
    .findings;
    let lines: Vec<usize> = findings
        .iter()
        .map(|finding| finding.probe.location.line)
        .collect();
    assert_eq!(
        lines,
        vec![3],
        "only the runtime line is probed: {findings:?}"
    );
    Ok(())
}

#[test]
fn only_parsed_ambient_declarations_are_skipped() -> Result<(), String> {
    // `declare` is also a runtime identifier, and a template literal line can
    // start with `declare `: both stay probed. The body of `declare module`
    // is ambient even on lines without the keyword.
    let source = "declare module 'x' {\n  export function f(): number;\n}\nexport function emit(): string {\n  return `\ndeclare module y;\n`;\n}\nexport function reset(): number {\n  let declare = 0;\n  declare = compute();\n  return declare;\n}\nfunction compute(): number {\n  return 1;\n}\n";
    let findings = analyze(
        "declare-identifier",
        &[("src/emit.ts", source)],
        changed(
            "src/emit.ts",
            &[
                (2, "  export function f(): number;"),
                (6, "declare module y;"),
                (11, "  declare = compute();"),
            ],
        ),
    )?
    .findings;
    let lines: Vec<usize> = findings
        .iter()
        .map(|finding| finding.probe.location.line)
        .collect();
    assert_eq!(
        lines,
        vec![6, 11],
        "the ambient module body is skipped, runtime lines are probed: {findings:?}"
    );
    Ok(())
}
