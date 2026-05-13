//! Python preview adapter - owner + test sub-slice.
//!
//! See `docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md` and
//! `docs/adr/0009-python-parser-substrate.md`.
//!
//! This sub-slice extracts first useful Python preview facts without
//! crossing into editor routing or runtime behavior:
//!
//! - it implements [`LanguageAdapter::accepts_path`] via the shared
//!   router so `.py` files route to it when `[languages] enabled`
//!   lists `python`;
//! - it carries a single `rustpython_parser` parse path through
//!   `parse_module_body` so the dependency has a real production
//!   consumer and the API surface is exercised by tests;
//! - it recognises Python function/method owners and pytest/unittest
//!   test functions;
//! - it emits preview-tagged findings for changed lines inside
//!   production owners, classifying related-test presence as
//!   `weakly_exposed` and no related static path as `no_static_path`.
//!
//! Assertion, richer probe-family, related-test precision, and static-limit
//! extraction land in later Campaign 27 sub-slices against
//! RIPR-SPEC-0028's fixture corpus.
//!
//! ADR 0009 originally selected `ruff_python_parser`; that pick was
//! superseded in-place after discovering the crate is `publish = false`
//! in the astral-sh/ruff workspace and unavailable on crates.io. The
//! corrected substrate is `rustpython-parser`, the documented natural
//! fallback already named under the ADR's Revisit Criteria.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, LanguageId as DomainLanguageId, LanguageStatus,
    OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
    RiprEvidence, SourceLocation, StageEvidence, StageState,
};
use rustpython_parser::{
    Mode,
    ast::{self, Ranged},
    parse,
};
use std::path::{Path, PathBuf};

/// Python preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PythonAdapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PythonOwnerKind {
    Function,
    Method,
    ClassMethod,
    ModuleFunction,
}

impl PythonOwnerKind {
    fn as_str(&self) -> &'static str {
        match self {
            PythonOwnerKind::Function => "function",
            PythonOwnerKind::Method => "method",
            PythonOwnerKind::ClassMethod => "class_method",
            PythonOwnerKind::ModuleFunction => "module_function",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonOwner {
    name: String,
    class_name: Option<String>,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    kind: PythonOwnerKind,
    decorators: Vec<String>,
}

impl PythonOwner {
    fn display_name(&self) -> String {
        if let Some(class_name) = &self.class_name {
            format!("{class_name}.{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonTest {
    name: String,
    file: PathBuf,
    line: usize,
    body_text: String,
    markers: Vec<String>,
}

fn parse_module_body(path: &Path, source: &str) -> Option<Vec<ast::Stmt>> {
    let source_path = path.to_string_lossy();
    let parsed = parse(source, Mode::Module, source_path.as_ref()).ok()?;
    match parsed {
        ast::Mod::Module(module) => Some(module.body),
        _ => None,
    }
}

fn line_for_offset(source: &str, offset: usize) -> usize {
    let mut line: usize = 1;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
}

fn range_lines(source: &str, range: rustpython_parser::text_size::TextRange) -> (usize, usize) {
    (
        line_for_offset(source, range.start().to_usize()),
        line_for_offset(source, range.end().to_usize()),
    )
}

fn range_text(source: &str, range: rustpython_parser::text_size::TextRange) -> String {
    let start = range.start().to_usize();
    let end = range.end().to_usize();
    source
        .get(start..end)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

fn expr_name(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Name(name) => Some(name.id.to_string()),
        ast::Expr::Attribute(attr) => {
            let base = expr_name(&attr.value)?;
            Some(format!("{base}.{}", attr.attr))
        }
        ast::Expr::Call(call) => expr_name(&call.func),
        _ => None,
    }
}

fn decorator_texts(source: &str, decorators: &[ast::Expr]) -> Vec<String> {
    decorators
        .iter()
        .map(|decorator| range_text(source, decorator.range()))
        .filter(|text| !text.is_empty())
        .collect()
}

fn has_decorator(decorators: &[String], name: &str) -> bool {
    decorators
        .iter()
        .any(|decorator| decorator == name || decorator.ends_with(&format!(".{name}")))
}

fn extract_owners(file: &Path, source: &str) -> Vec<PythonOwner> {
    let Some(body) = parse_module_body(file, source) else {
        return Vec::new();
    };
    let mut owners = Vec::new();
    for stmt in &body {
        match stmt {
            ast::Stmt::FunctionDef(func) => owners.push(owner_from_function(
                file,
                source,
                &func.name,
                func.range,
                &func.decorator_list,
                None,
                PythonOwnerKind::Function,
            )),
            ast::Stmt::AsyncFunctionDef(func) => owners.push(owner_from_function(
                file,
                source,
                &func.name,
                func.range,
                &func.decorator_list,
                None,
                PythonOwnerKind::Function,
            )),
            ast::Stmt::ClassDef(class_def) => {
                owners.extend(owners_from_class(file, source, class_def));
            }
            ast::Stmt::Assign(_)
            | ast::Stmt::AnnAssign(_)
            | ast::Stmt::AugAssign(_)
            | ast::Stmt::Expr(_) => {
                owners.push(module_expression_owner(file, source, stmt));
            }
            _ => {}
        }
    }
    owners
}

fn owner_from_function(
    file: &Path,
    source: &str,
    name: &ast::Identifier,
    range: rustpython_parser::text_size::TextRange,
    decorators: &[ast::Expr],
    class_name: Option<&str>,
    kind: PythonOwnerKind,
) -> PythonOwner {
    let (start_line, end_line) = range_lines(source, range);
    PythonOwner {
        name: name.to_string(),
        class_name: class_name.map(str::to_string),
        file: file.to_path_buf(),
        start_line,
        end_line,
        kind,
        decorators: decorator_texts(source, decorators),
    }
}

fn owners_from_class(file: &Path, source: &str, class_def: &ast::StmtClassDef) -> Vec<PythonOwner> {
    let mut owners = Vec::new();
    let class_name = class_def.name.to_string();
    for stmt in &class_def.body {
        match stmt {
            ast::Stmt::FunctionDef(func) => {
                let decorators = decorator_texts(source, &func.decorator_list);
                let kind = if has_decorator(&decorators, "classmethod") {
                    PythonOwnerKind::ClassMethod
                } else {
                    PythonOwnerKind::Method
                };
                let (start_line, end_line) = range_lines(source, func.range);
                owners.push(PythonOwner {
                    name: func.name.to_string(),
                    class_name: Some(class_name.clone()),
                    file: file.to_path_buf(),
                    start_line,
                    end_line,
                    kind,
                    decorators,
                });
            }
            ast::Stmt::AsyncFunctionDef(func) => {
                let decorators = decorator_texts(source, &func.decorator_list);
                let kind = if has_decorator(&decorators, "classmethod") {
                    PythonOwnerKind::ClassMethod
                } else {
                    PythonOwnerKind::Method
                };
                let (start_line, end_line) = range_lines(source, func.range);
                owners.push(PythonOwner {
                    name: func.name.to_string(),
                    class_name: Some(class_name.clone()),
                    file: file.to_path_buf(),
                    start_line,
                    end_line,
                    kind,
                    decorators,
                });
            }
            _ => {}
        }
    }
    owners
}

fn module_expression_owner(file: &Path, source: &str, stmt: &ast::Stmt) -> PythonOwner {
    let range = stmt.range();
    let (start_line, end_line) = range_lines(source, range);
    PythonOwner {
        name: format!("module_expression_line_{start_line}"),
        class_name: None,
        file: file.to_path_buf(),
        start_line,
        end_line,
        kind: PythonOwnerKind::ModuleFunction,
        decorators: Vec::new(),
    }
}

fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    file_name.ends_with(".py")
        && (file_name.starts_with("test_") || file_name.ends_with("_test.py"))
}

fn extract_tests(file: &Path, source: &str) -> Vec<PythonTest> {
    let Some(body) = parse_module_body(file, source) else {
        return Vec::new();
    };
    let mut tests = Vec::new();
    for stmt in &body {
        match stmt {
            ast::Stmt::FunctionDef(func) if func.name.as_str().starts_with("test_") => {
                tests.push(test_from_function(
                    file,
                    source,
                    &func.name,
                    func.range,
                    &func.decorator_list,
                    None,
                ));
            }
            ast::Stmt::AsyncFunctionDef(func) if func.name.as_str().starts_with("test_") => {
                tests.push(test_from_function(
                    file,
                    source,
                    &func.name,
                    func.range,
                    &func.decorator_list,
                    None,
                ));
            }
            ast::Stmt::ClassDef(class_def) if class_extends_unittest_case(class_def) => {
                tests.extend(tests_from_unittest_class(file, source, class_def));
            }
            _ => {}
        }
    }
    tests
}

fn test_from_function(
    file: &Path,
    source: &str,
    name: &ast::Identifier,
    range: rustpython_parser::text_size::TextRange,
    decorators: &[ast::Expr],
    class_name: Option<&str>,
) -> PythonTest {
    let (line, _) = range_lines(source, range);
    let body_text = range_text(source, range);
    let markers = decorator_texts(source, decorators);
    let name = if let Some(class_name) = class_name {
        format!("{class_name}.{}", name)
    } else {
        name.to_string()
    };
    PythonTest {
        name,
        file: file.to_path_buf(),
        line,
        body_text,
        markers,
    }
}

fn class_extends_unittest_case(class_def: &ast::StmtClassDef) -> bool {
    class_def.bases.iter().any(|base| {
        let Some(name) = expr_name(base) else {
            return false;
        };
        name == "TestCase" || name.ends_with(".TestCase")
    })
}

fn tests_from_unittest_class(
    file: &Path,
    source: &str,
    class_def: &ast::StmtClassDef,
) -> Vec<PythonTest> {
    let mut tests = Vec::new();
    let class_name = class_def.name.to_string();
    for stmt in &class_def.body {
        match stmt {
            ast::Stmt::FunctionDef(func) if func.name.as_str().starts_with("test_") => {
                tests.push(test_from_function(
                    file,
                    source,
                    &func.name,
                    func.range,
                    &func.decorator_list,
                    Some(&class_name),
                ));
            }
            ast::Stmt::AsyncFunctionDef(func) if func.name.as_str().starts_with("test_") => {
                tests.push(test_from_function(
                    file,
                    source,
                    &func.name,
                    func.range,
                    &func.decorator_list,
                    Some(&class_name),
                ));
            }
            _ => {}
        }
    }
    tests
}

fn find_related_tests(owner: &PythonOwner, all_tests: &[PythonTest]) -> Vec<RelatedTest> {
    let direct_call = format!("{}(", owner.name);
    let method_call = format!(".{}(", owner.name);
    all_tests
        .iter()
        .filter(|test| {
            test.body_text.contains(&direct_call) || test.body_text.contains(&method_call)
        })
        .map(|test| RelatedTest {
            name: test.name.clone(),
            file: test.file.clone(),
            line: test.line,
            oracle: None,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::Unknown,
        })
        .collect()
}

fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let trimmed = line_text.trim_start();
    if trimmed.starts_with("raise ") || trimmed.starts_with("raise(") {
        return (ProbeFamily::ErrorPath, DeltaKind::Control);
    }
    if trimmed.starts_with("return ") || trimmed == "return" {
        return (ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if trimmed.starts_with("if ")
        || trimmed.starts_with("elif ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("match ")
        || trimmed.starts_with("case ")
    {
        return (ProbeFamily::Predicate, DeltaKind::Control);
    }
    if trimmed.contains('.') && trimmed.contains('=') {
        return (ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    if trimmed.ends_with(')') && trimmed.contains('(') {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    (ProbeFamily::Predicate, DeltaKind::Control)
}

fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[PythonOwner],
    all_tests: &[PythonTest],
) -> Option<Finding> {
    let changed_file = normalized_path(file);
    let owner = owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .find(|owner| line >= owner.start_line && line <= owner.end_line)?;
    let related = find_related_tests(owner, all_tests);
    let owner_display = owner.display_name();

    let (class, reach_state, observe_state, discriminate_state, missing) = if related.is_empty() {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![format!(
                "No Python test references `{}(` or `.{}(`; add a test that calls the changed owner.",
                owner.name, owner.name
            )],
        )
    } else {
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Related Python test reaches `{owner_display}` but assertion extraction is not implemented yet; add or verify an exact assertion for the changed behavior."
            )],
        )
    };

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let (family, delta) = classify_probe_shape(line_text);
    let probe = Probe {
        id: ProbeId(format!("probe:{id_path}:{line}:python_preview")),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: None,
        family,
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };

    let related_count = related.len();
    let reach_summary =
        format!("{related_count} related test(s) found for owner `{owner_display}`");
    let reach = StageEvidence::new(reach_state.clone(), Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "Python preview adapter does not yet model infection.",
    );
    let propagate = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "Python preview adapter does not yet model propagation.",
    );
    let observe_summary = if related.is_empty() {
        "No related Python test reached the changed owner.".to_string()
    } else {
        "Python owner/test preview found related tests; assertion extraction lands in a follow-up."
            .to_string()
    };
    let observe = StageEvidence::new(observe_state, Confidence::Low, observe_summary);
    let discriminate_summary = if related.is_empty() {
        "No Python discriminator is visible because no related test was found.".to_string()
    } else {
        "Python preview adapter found reach evidence but no extracted assertion discriminator yet."
            .to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, discriminate_summary);

    let recommended = if matches!(class, ExposureClass::NoStaticPath) {
        format!(
            "Python preview: add a test that calls `{}` and asserts the changed behavior.",
            owner.name
        )
    } else {
        format!(
            "Python preview: verify `{owner_display}` with a focused exact assertion for the changed behavior."
        )
    };

    let mut evidence = vec![
        format!("owner: {owner_display}"),
        format!("owner_kind: {}", owner.kind.as_str()),
    ];
    if !owner.decorators.is_empty() {
        evidence.push(format!("decorators: {}", owner.decorators.join(", ")));
    }
    for test in all_tests.iter().filter(|test| !test.markers.is_empty()) {
        if find_related_tests(owner, std::slice::from_ref(test)).is_empty() {
            continue;
        }
        evidence.push(format!(
            "test_marker: {} on {}",
            test.markers.join(", "),
            test.name
        ));
    }

    let confidence = if matches!(class, ExposureClass::NoStaticPath) {
        0.35
    } else {
        0.4
    };
    Some(Finding {
        id: probe.id.0.clone(),
        probe,
        class,
        ripr: RiprEvidence {
            reach,
            infect,
            propagate,
            reveal: RevealEvidence {
                observe,
                discriminate,
            },
        },
        confidence,
        evidence,
        missing,
        flow_sinks: Vec::new(),
        activation: Default::default(),
        stop_reasons: Vec::new(),
        related_tests: related,
        recommended_next_step: Some(recommended),
        language: Some(DomainLanguageId::Python),
        language_status: Some(LanguageStatus::Preview),
        static_limit_kind: None,
    })
}

fn collect_workspace_python_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    visit_workspace(root, root, &mut out);
    out.sort();
    out
}

fn visit_workspace(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name == ".git"
            || name == "target"
            || name == "__pycache__"
            || name == ".venv"
            || name == "venv"
            || name == ".ripr"
            || name == ".direnv"
        {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            visit_workspace(root, &path, out);
        } else if file_type.is_file() {
            let adapter = PythonAdapter;
            if adapter.accepts_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
}

impl LanguageAdapter for PythonAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Python))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        let workspace_files = collect_workspace_python_files(&options.root);
        let mut all_owners: Vec<PythonOwner> = Vec::new();
        let mut all_tests: Vec<PythonTest> = Vec::new();
        for relative in &workspace_files {
            let absolute = options.root.join(relative);
            let Ok(source) = std::fs::read_to_string(&absolute) else {
                continue;
            };
            if is_test_file(relative) {
                all_tests.extend(extract_tests(relative, &source));
            } else {
                all_owners.extend(extract_owners(relative, &source));
            }
        }

        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        for changed in changed_files {
            if !self.accepts_path(&changed.path) {
                continue;
            }
            changed_count += 1;
            if is_test_file(&changed.path) {
                continue;
            }
            for added in &changed.added_lines {
                if let Some(finding) = classify_change(
                    &changed.path,
                    added.line,
                    &added.text,
                    &all_owners,
                    &all_tests,
                ) {
                    findings.push(finding);
                }
            }
        }
        Ok(LanguageDiffResult {
            findings,
            changed_files: changed_count,
        })
    }

    fn analyze_repo(
        &self,
        _options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        // Scaffold: no repo-mode preview output yet.
        Ok(LanguageRepoResult {
            findings: Vec::new(),
            production_files: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn changed(path: &str) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: Vec::new(),
            removed_lines: Vec::new(),
        }
    }

    fn changed_with_line(path: &str, line: usize, text: &str) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: vec![super::super::super::diff::ChangedLine {
                line,
                text: text.to_string(),
            }],
            removed_lines: Vec::new(),
        }
    }

    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-python-{name}-{nanos}"));
        fs::create_dir_all(&root)
            .map_err(|err| format!("failed to create temp root {}: {err}", root.display()))?;
        Ok(root)
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!("failed to create temp parent {}: {err}", parent.display())
            })?;
        }
        fs::write(path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
    }

    #[test]
    fn accepts_py_paths() {
        let adapter = PythonAdapter;
        assert!(adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(adapter.accepts_path(Path::new("src/lib/util.py")));
        assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(!adapter.accepts_path(Path::new("src/index.ts")));
        assert!(!adapter.accepts_path(Path::new("src/index.tsx")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
        assert!(!adapter.accepts_path(Path::new("no-extension")));
    }

    #[test]
    fn parse_source_accepts_simple_python() {
        let ok = parse_module_body(
            Path::new("src/discount.py"),
            "def discount(amount: int) -> int:\n    return amount\n",
        )
        .is_some();
        assert!(ok, "valid Python should parse without errors");
    }

    #[test]
    fn parse_source_accepts_class_and_decorator() {
        let ok = parse_module_body(
            Path::new("src/repo.py"),
            "class Repo:\n    @staticmethod\n    def make() -> 'Repo':\n        return Repo()\n",
        )
        .is_some();
        assert!(ok, "decorated class methods should parse");
    }

    #[test]
    fn parse_source_accepts_async_def_and_fstring() {
        let ok = parse_module_body(
            Path::new("src/http.py"),
            "async def load(url: str) -> str:\n    return f\"{url}!\"\n",
        )
        .is_some();
        assert!(ok, "async def + f-string should parse");
    }

    #[test]
    fn parse_source_rejects_garbage() {
        let ok = parse_module_body(
            Path::new("src/oops.py"),
            "this is not :: valid +++ python at all",
        )
        .is_some();
        assert!(!ok, "garbage source should produce parse errors");
    }

    #[test]
    fn extract_owners_recognizes_function_async_methods_and_decorators() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            r#"def apply_discount(amount, threshold):
    return amount

async def load_price(client):
    return await client.price()

class Pricing:
    @staticmethod
    def clamp(amount):
        return amount

    @classmethod
    def from_amount(cls, amount):
        return cls()

    @retry(times=3)
    def save(self):
        return True
"#,
        );

        let names: Vec<_> = owners
            .iter()
            .map(|owner| (owner.display_name(), owner.kind.as_str().to_string()))
            .collect();
        assert!(names.contains(&("apply_discount".to_string(), "function".to_string())));
        assert!(names.contains(&("load_price".to_string(), "function".to_string())));
        assert!(names.contains(&("Pricing.clamp".to_string(), "method".to_string())));
        assert!(names.contains(&(
            "Pricing.from_amount".to_string(),
            "class_method".to_string()
        )));
        let save = owners
            .iter()
            .find(|owner| owner.display_name() == "Pricing.save")
            .ok_or_else(|| "expected decorated Pricing.save owner".to_string())?;
        assert_eq!(save.kind, PythonOwnerKind::Method);
        assert_eq!(save.decorators, vec!["retry(times=3)".to_string()]);
        Ok(())
    }

    #[test]
    fn extract_owners_recognizes_module_scope_expression_owner() {
        let owners = extract_owners(
            Path::new("src/bootstrap.py"),
            r#"CONFIGURED = bootstrap()
"#,
        );
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].kind, PythonOwnerKind::ModuleFunction);
        assert_eq!(owners[0].name, "module_expression_line_1");
    }

    #[test]
    fn extract_tests_recognizes_pytest_unittest_and_parametrize() -> Result<(), String> {
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            r#"import unittest
import pytest

@pytest.mark.parametrize("amount", [1, 2])
def test_discount_boundary(amount):
    assert apply_discount(amount, 1) >= 0

class PricingTest(unittest.TestCase):
    def test_class_method(self):
        self.assertTrue(apply_discount(10, 1))
"#,
        );
        assert_eq!(tests.len(), 2);
        let pytest = tests
            .iter()
            .find(|test| test.name == "test_discount_boundary")
            .ok_or_else(|| "expected pytest function".to_string())?;
        assert_eq!(
            pytest.markers,
            vec!["pytest.mark.parametrize(\"amount\", [1, 2])".to_string()]
        );
        assert!(
            tests
                .iter()
                .any(|test| test.name == "PricingTest.test_class_method")
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_weakly_exposed_with_preview_metadata() -> Result<(), String> {
        let owners = vec![PythonOwner {
            name: "apply_discount".to_string(),
            class_name: None,
            file: PathBuf::from("src/pricing.py"),
            start_line: 1,
            end_line: 4,
            kind: PythonOwnerKind::Function,
            decorators: Vec::new(),
        }];
        let tests = vec![PythonTest {
            name: "test_discount".to_string(),
            file: PathBuf::from("tests/test_pricing.py"),
            line: 1,
            body_text: "assert apply_discount(100, 50) == 90".to_string(),
            markers: Vec::new(),
        }];
        let finding = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= threshold:",
            &owners,
            &tests,
        )
        .ok_or_else(|| "expected a Python preview finding".to_string())?;
        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.language, Some(DomainLanguageId::Python));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.related_tests.len(), 1);
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line == "owner_kind: function")
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_none_when_no_projectable_owner() {
        let finding = classify_change(
            Path::new("src/pricing.py"),
            1,
            "# changed comment",
            &[],
            &[],
        );
        assert!(finding.is_none());
    }

    #[test]
    fn analyze_diff_emits_python_preview_finding_when_workspace_has_related_test()
    -> Result<(), String> {
        let root = temp_root("related-test")?;
        write(
            &root.join("src/pricing.py"),
            r#"def apply_discount(amount, threshold):
    if amount >= threshold:
        return amount - 10
    return amount
"#,
        )?;
        write(
            &root.join("tests/test_pricing.py"),
            r#"def test_discount_above_threshold():
    assert apply_discount(100, 50) == 90
"#,
        )?;
        let adapter = PythonAdapter;
        let options = AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![changed_with_line(
            "src/pricing.py",
            2,
            "    if amount >= threshold:",
        )];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        assert_eq!(result.changed_files, 1);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].language, Some(DomainLanguageId::Python));
        assert_eq!(
            result.findings[0].language_status,
            Some(LanguageStatus::Preview)
        );
        assert_eq!(result.findings[0].related_tests.len(), 1);
        Ok(())
    }

    #[test]
    fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
        let adapter = PythonAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![
            changed("scripts/run.py"),
            changed("src/lib.rs"),
            changed("docs/README.md"),
            changed("src/util.py"),
            changed("src/index.ts"),
        ];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.changed_files, 2);
        Ok(())
    }

    #[test]
    fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
        let adapter = PythonAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Deep,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let result = adapter.analyze_repo(&options, &policy)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.production_files, 0);
        Ok(())
    }
}
