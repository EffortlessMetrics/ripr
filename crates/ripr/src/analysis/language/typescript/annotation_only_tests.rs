//! Tests for annotation-only TypeScript signature changes (#4282).

use super::*;

fn annotation_only(old: &str, new: &str) -> bool {
    is_annotation_only_signature_change(Path::new("src/t.ts"), old, new, false)
}

/// A decorator on the line above a method (`@Get()` then `find(id: string) {`)
/// makes its types runtime metadata under `emitDecoratorMetadata`, so in a
/// file with decorators a method line keeps its probe; functions do not take
/// decorators and are unaffected.
#[test]
fn decorated_file_keeps_method_lines_probed() {
    let file = Path::new("src/t.ts");
    let method = ("  find(id: string) {", "  find(id: number) {");
    assert!(is_annotation_only_signature_change(
        file, method.0, method.1, false
    ));
    assert!(!is_annotation_only_signature_change(
        file, method.0, method.1, true
    ));
    assert!(is_annotation_only_signature_change(
        file,
        "function f(a: string) {",
        "function f(a: number) {",
        true,
    ));
}

#[test]
fn type_only_signature_edits_are_annotation_only() {
    let cases = [
        // Return type (the re-walk sample, row T3).
        (
            "export function run(o: Opts): number {",
            "export function run(o: Opts): unknown {",
        ),
        // Parameter annotation, optional marker, generic parameter list.
        ("function f(a: string) {", "function f(a: number) {"),
        ("function f(a: string) {", "function f(a?: string) {"),
        ("function f<T>(a: T): T {", "function f<T, U>(a: T): U {"),
        // A default VALUE is untouched when only its annotation changes.
        ("function f(x: number = 2) {", "function f(x: bigint = 2) {"),
        // `this` parameter and rest annotation are erased.
        (
            "function f(...xs: number[]) {",
            "function f(this: Ctx, ...xs: string[]) {",
        ),
        // `export default`, async, one-line bodies.
        (
            "export default async function load(u: string): Promise<Data> {",
            "export default async function load(u: URL): Promise<unknown> {",
        ),
        (
            "function id(a: number): number { return a; }",
            "function id(a: string): string { return a; }",
        ),
        // Arrow functions and function expressions bound to a const.
        (
            "export const run = (o: Opts): number => {",
            "export const run = (o: Opts): unknown => {",
        ),
        (
            "const add = (a: number, b: number): number => a + b;",
            "const add = (a: bigint, b: bigint): bigint => a + b;",
        ),
        (
            "const f = function (a: string) {",
            "const f = function (a: unknown) {",
        ),
        // Class method.
        (
            "  total(items: Item[]): number {",
            "  total(items: Line[]): number {",
        ),
        // Variable annotation.
        ("const limit: number = 5;", "const limit: Limit = 5;"),
    ];
    for (old, new) in cases {
        assert!(
            annotation_only(old, new),
            "expected annotation-only: `{old}` -> `{new}`"
        );
    }
}

#[test]
fn runtime_signature_edits_keep_their_probe() {
    let cases = [
        // Default value (the #4282 negative case).
        ("function f(x: number = 2) {", "function f(x: number = 3) {"),
        // Parameter renamed, added, reordered into a rest parameter.
        ("function f(a: string) {", "function f(b: string) {"),
        (
            "function f(a: string) {",
            "function f(a: string, b: string) {",
        ),
        ("function f(a: string[]) {", "function f(...a: string[]) {"),
        // Destructuring shape.
        ("function f({ a }: Opts) {", "function f({ a, b }: Opts) {"),
        // Name, async-ness, generator-ness.
        ("function f(a: T) {", "function g(a: T) {"),
        ("function f(a: T) {", "async function f(a: T) {"),
        ("function f(a: T) {", "function* f(a: T) {"),
        // Export shape is runtime module structure.
        ("function f(a: T) {", "export function f(a: T) {"),
        // Body text on a one-line function.
        (
            "function id(a: number): number { return a; }",
            "function id(a: number): number { return a + 1; }",
        ),
        // Arrow expression body.
        (
            "const add = (a: number, b: number): number => a + b;",
            "const add = (a: number, b: number): number => a - b;",
        ),
        // Parameter property added: emits `this.repo = repo`.
        (
            "  constructor(repo: Repo) {",
            "  constructor(private repo: Repo) {",
        ),
        // Constructor parameter types feed decorator metadata on a decorated
        // class, which one line cannot rule out.
        (
            "  constructor(private readonly repo: Repo) {",
            "  constructor(private readonly repo: RepoLike) {",
        ),
        // Any decorator: `emitDecoratorMetadata` makes types runtime values.
        ("  @Get() find(id: string) {", "  @Get() find(id: number) {"),
        (
            "  find(@Param() id: string) {",
            "  find(@Param() id: number) {",
        ),
        // A one-line class declaration is not the synthetic method wrapper.
        (
            "class A { m(a: string) { return 1; } }",
            "class B { m(a: string) { return 1; } }",
        ),
        (
            "class A { m(a: string) { return 1; } }",
            "class A extends Base { m(a: string) { return 1; } }",
        ),
        (
            "abstract class A { abstract m(): void; }",
            "declare abstract class A { abstract m(): void; }",
        ),
        ("class A { m(a: string) {}", "class B { m(a: string) {}"),
        ("class A { m(a: string) {} }", "m(a: string) {}"),
        // Method kind and staticness.
        (
            "  total(items: Item[]): number {",
            "  static total(items: Item[]): number {",
        ),
        // Variable value and declaration kind.
        ("const limit: number = 5;", "const limit: number = 6;"),
        ("let limit: number = 5;", "const limit: number = 5;"),
        // Type syntax inside an expression is compared as text: fail closed.
        ("const v = raw as Opts;", "const v = raw as Other;"),
        // Not a signature or declaration: no claim.
        ("if (a < b) {", "if (a <= b) {"),
        ("return a as number;", "return a as bigint;"),
        // A multi-line signature fragment does not parse alone: no claim.
        ("): number {", "): unknown {"),
        // Identical lines are not a change.
        ("function f(a: T) {", "function f(a: T) {"),
    ];
    for (old, new) in cases {
        assert!(
            !annotation_only(old, new),
            "expected a runtime change: `{old}` -> `{new}`"
        );
    }
}

fn temp_workspace(label: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system time: {err}"))?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ripr-ts-annotation-{label}-{}-{nanos}",
        std::process::id()
    ));
    for (path, contents) in [
        (
            "src/t.ts",
            "export interface Opts { n: number }\n\nexport function run(o: Opts, scale = 3): unknown {\n  return o.n * scale;\n}\n",
        ),
        (
            "tests/t.test.ts",
            "import { run } from '../src/t';\ntest('runs', () => {\n  expect(run({ n: 2 })).toBeTruthy();\n});\n",
        ),
    ] {
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create_dir_all({}): {err}", parent.display()))?;
        }
        std::fs::write(&file, contents)
            .map_err(|err| format!("write({}): {err}", file.display()))?;
    }
    Ok(dir)
}

fn findings_on_line_3(old_line: &str, label: &str) -> Result<usize, String> {
    let root = temp_workspace(label)?;
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/t.ts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 3,
            new_side_line: 3,
            text: "export function run(o: Opts, scale = 3): unknown {".to_string(),
        }],
        removed_lines: vec![crate::analysis::diff::ChangedLine {
            line: 3,
            new_side_line: 3,
            text: old_line.to_string(),
        }],
    }];
    let result = TypeScriptAdapter.analyze_diff(&options, &OraclePolicy::default(), &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;
    assert_eq!(result.changed_files, 1, "the changed file must be analyzed");
    Ok(result
        .findings
        .iter()
        .filter(|finding| finding.probe.location.line == 3)
        .count())
}

/// End to end through `analyze_diff`: a return-type-only change on a tested
/// function yields no probe, while a default-value change on the same
/// signature line is still probed. Removing the guard in `mod.rs` makes the
/// first assertion fail with one finding.
#[test]
fn analyze_diff_drops_return_type_only_change_and_keeps_default_value_change() -> Result<(), String>
{
    assert_eq!(
        findings_on_line_3(
            "export function run(o: Opts, scale = 3): number {",
            "return-type"
        )?,
        0,
        "a return-type-only change has no runtime behavior and must not be probed"
    );
    assert_eq!(
        findings_on_line_3(
            "export function run(o: Opts, scale = 2): unknown {",
            "default-value"
        )?,
        1,
        "a default-value change is runtime behavior and must keep its probe"
    );
    Ok(())
}
