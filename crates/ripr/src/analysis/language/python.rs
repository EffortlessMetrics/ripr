//! Python preview adapter.
//!
//! See `docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md` and
//! `docs/adr/0009-python-parser-substrate.md`.
//!
//! This slice extracts the first useful syntax-first Python facts:
//!
//! - owners for module functions, async functions, class methods, and
//!   `@staticmethod` / `@classmethod` methods;
//! - pytest `test_*` functions, parametrized pytest tests, and
//!   `unittest.TestCase.test_*` methods;
//! - related-test references by simple syntactic call/name matching.
//!
//! Richer probe families, import-graph matching, static limits, editor
//! routing, generated tests, runtime execution, and provider calls remain
//! out of scope. Related tests with strong extracted assertion oracles
//! produce `exposed`; broad or unknown assertion evidence remains
//! `weakly_exposed`; missing related tests produce `no_static_path`.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, LanguageId as DomainLanguageId, LanguageStatus,
    OracleKind, OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest,
    RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
};
use rustpython_parser::{
    Mode,
    ast::{self, CmpOp, Expr, Mod, Stmt},
    parse,
    text_size::TextRange,
};
use std::path::{Path, PathBuf};

/// Python preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PythonAdapter;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonOwner {
    name: String,
    qualified_name: String,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    owner_kind: OwnerKind,
    decorators: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonTest {
    name: String,
    file: PathBuf,
    line: usize,
    body_text: String,
    assertions: Vec<PythonAssertion>,
    parametrized: bool,
    framework: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonAssertion {
    text: String,
    line: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
}

fn parse_module(path: &Path, source: &str) -> Option<Mod> {
    let source_path = path.to_string_lossy();
    let module = parse(source, Mode::Module, source_path.as_ref()).ok()?;
    match module {
        Mod::Module(_) => Some(module),
        _ => None,
    }
}

/// 1-indexed line for a 0-indexed byte offset.
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

fn line_for_range_start(source: &str, range: TextRange) -> usize {
    line_for_offset(source, usize::from(range.start()))
}

fn line_for_range_end(source: &str, range: TextRange) -> usize {
    line_for_offset(source, usize::from(range.end()))
}

fn text_for_range(source: &str, range: TextRange) -> String {
    let start = usize::from(range.start()).min(source.len());
    let end = usize::from(range.end()).min(source.len());
    source.get(start..end).unwrap_or_default().to_string()
}

fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if file_name.starts_with("test_") || file_name.ends_with("_test.py") {
        return true;
    }
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        text == "tests" || text == "test"
    })
}

fn extract_owners(file: &Path, source: &str) -> Vec<PythonOwner> {
    let Some(Mod::Module(module)) = parse_module(file, source) else {
        return Vec::new();
    };
    let mut owners = Vec::new();
    collect_owners_from_statements(file, source, &module.body, None, &mut owners);
    owners
}

fn collect_owners_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    class_context: Option<&str>,
    out: &mut Vec<PythonOwner>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) => {
                out.push(owner_from_function(
                    file,
                    source,
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    class_context,
                    false,
                ));
            }
            Stmt::AsyncFunctionDef(function) => {
                out.push(owner_from_function(
                    file,
                    source,
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    class_context,
                    true,
                ));
            }
            Stmt::ClassDef(class) => {
                collect_owners_from_statements(
                    file,
                    source,
                    &class.body,
                    Some(class.name.as_str()),
                    out,
                );
            }
            _ => {}
        }
    }
}

fn owner_from_function(
    file: &Path,
    source: &str,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
    class_context: Option<&str>,
    is_async: bool,
) -> PythonOwner {
    let decorator_names = decorator_names(decorators);
    let owner_kind = if class_context.is_some()
        && decorator_names.iter().any(|decorator| {
            decorator.ends_with("classmethod") || decorator.ends_with("staticmethod")
        }) {
        OwnerKind::ClassMethod
    } else if class_context.is_some() {
        OwnerKind::Method
    } else {
        OwnerKind::Function
    };
    let qualified_name = class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string());
    let mut decorators = decorator_names;
    if is_async {
        decorators.push("async_def".to_string());
    }
    PythonOwner {
        name: name.to_string(),
        qualified_name,
        file: file.to_path_buf(),
        start_line: line_for_range_start(source, range),
        end_line: line_for_range_end(source, range),
        owner_kind,
        decorators,
    }
}

fn extract_tests(file: &Path, source: &str) -> Vec<PythonTest> {
    let Some(Mod::Module(module)) = parse_module(file, source) else {
        return Vec::new();
    };
    let mut tests = Vec::new();
    collect_tests_from_statements(file, source, &module.body, false, &mut tests);
    tests
}

fn collect_tests_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    in_unittest_class: bool,
    out: &mut Vec<PythonTest>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) if function.name.as_str().starts_with("test_") => {
                out.push(PythonTest {
                    name: function.name.to_string(),
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    assertions: collect_assertions_from_statements(source, &function.body),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework: if in_unittest_class {
                        "unittest"
                    } else {
                        "pytest"
                    },
                });
            }
            Stmt::AsyncFunctionDef(function) if function.name.as_str().starts_with("test_") => {
                out.push(PythonTest {
                    name: function.name.to_string(),
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    assertions: collect_assertions_from_statements(source, &function.body),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework: if in_unittest_class {
                        "unittest"
                    } else {
                        "pytest"
                    },
                });
            }
            Stmt::ClassDef(class) => {
                collect_tests_from_statements(
                    file,
                    source,
                    &class.body,
                    is_unittest_class(class) || in_unittest_class,
                    out,
                );
            }
            _ => {}
        }
    }
}

fn collect_assertions_from_statements(source: &str, statements: &[Stmt]) -> Vec<PythonAssertion> {
    let mut out = Vec::new();
    for stmt in statements {
        match stmt {
            Stmt::Assert(assertion) => {
                out.push(assertion_from_assert_stmt(source, assertion));
            }
            Stmt::Expr(expr_stmt) => {
                if let Some(assertion) = assertion_from_call_expr(source, &expr_stmt.value) {
                    out.push(assertion);
                }
            }
            Stmt::With(with_stmt) => {
                for item in &with_stmt.items {
                    if let Some(assertion) = assertion_from_context_expr(source, &item.context_expr)
                    {
                        out.push(assertion);
                    }
                }
                out.extend(collect_assertions_from_statements(source, &with_stmt.body));
            }
            Stmt::AsyncWith(with_stmt) => {
                for item in &with_stmt.items {
                    if let Some(assertion) = assertion_from_context_expr(source, &item.context_expr)
                    {
                        out.push(assertion);
                    }
                }
                out.extend(collect_assertions_from_statements(source, &with_stmt.body));
            }
            Stmt::If(if_stmt) => {
                out.extend(collect_assertions_from_statements(source, &if_stmt.body));
                out.extend(collect_assertions_from_statements(source, &if_stmt.orelse));
            }
            _ => {}
        }
    }
    out
}

fn assertion_from_assert_stmt(source: &str, assertion: &ast::StmtAssert) -> PythonAssertion {
    let line = line_for_range_start(source, assertion.range);
    let text = text_for_range(source, assertion.range).trim().to_string();
    let (oracle_kind, oracle_strength) = match assertion.test.as_ref() {
        Expr::Compare(compare) => oracle_for_compare(&compare.ops),
        Expr::Call(call) if call_name_is(&call.func, "isinstance") => {
            (OracleKind::RelationalCheck, OracleStrength::Weak)
        }
        _ => (OracleKind::SmokeOnly, OracleStrength::Smoke),
    };
    PythonAssertion {
        text,
        line,
        oracle_kind,
        oracle_strength,
    }
}

fn oracle_for_compare(ops: &[CmpOp]) -> (OracleKind, OracleStrength) {
    match ops.first() {
        Some(CmpOp::Eq) if ops.len() == 1 => (OracleKind::ExactValue, OracleStrength::Strong),
        Some(CmpOp::NotEq) if ops.len() == 1 => (OracleKind::RelationalCheck, OracleStrength::Weak),
        Some(_) => (OracleKind::RelationalCheck, OracleStrength::Weak),
        None => (OracleKind::SmokeOnly, OracleStrength::Smoke),
    }
}

fn assertion_from_context_expr(source: &str, expr: &Expr) -> Option<PythonAssertion> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let name = expr_full_name(&call.func)?;
    if name == "raises" || name.ends_with(".raises") {
        return Some(error_assertion_from_call(source, call, "pytest.raises"));
    }
    None
}

fn assertion_from_call_expr(source: &str, expr: &Expr) -> Option<PythonAssertion> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let name = expr_full_name(&call.func)?;
    let line = line_for_range_start(source, call.range);
    let text = text_for_range(source, call.range).trim().to_string();
    let tail = name.rsplit('.').next().unwrap_or(name.as_str());
    let (oracle_kind, oracle_strength) = match tail {
        "assertEqual" => (OracleKind::ExactValue, OracleStrength::Strong),
        "assertNotEqual" => (OracleKind::RelationalCheck, OracleStrength::Weak),
        "assertTrue" | "assertFalse" => (OracleKind::SmokeOnly, OracleStrength::Smoke),
        "assertRaises" => {
            return Some(error_assertion_from_call(source, call, "assertRaises"));
        }
        "assert_called_once_with" | "assert_called_with" | "assert_called_once" => {
            (OracleKind::MockExpectation, OracleStrength::Medium)
        }
        "assert_called" | "assert_not_called" => {
            (OracleKind::MockExpectation, OracleStrength::Weak)
        }
        _ => return None,
    };
    Some(PythonAssertion {
        text,
        line,
        oracle_kind,
        oracle_strength,
    })
}

fn error_assertion_from_call(source: &str, call: &ast::ExprCall, label: &str) -> PythonAssertion {
    let line = line_for_range_start(source, call.range);
    let exception = call.args.first().and_then(expr_full_name);
    let is_broad = exception
        .as_deref()
        .is_none_or(|name| matches!(name, "Exception" | "BaseException"));
    let (oracle_kind, oracle_strength) = if is_broad {
        (OracleKind::BroadError, OracleStrength::Weak)
    } else {
        (OracleKind::ExactErrorVariant, OracleStrength::Strong)
    };
    let text = if let Some(exception) = exception {
        format!("{label}({exception})")
    } else {
        format!("{label}(...)")
    };
    PythonAssertion {
        text,
        line,
        oracle_kind,
        oracle_strength,
    }
}

fn call_name_is(func: &Expr, expected: &str) -> bool {
    expr_full_name(func)
        .is_some_and(|name| name == expected || name.ends_with(&format!(".{expected}")))
}

fn is_parametrized(decorators: &[Expr]) -> bool {
    decorator_names(decorators).iter().any(|decorator| {
        decorator == "parametrize"
            || decorator.ends_with(".parametrize")
            || decorator.ends_with("mark.parametrize")
    })
}

fn is_unittest_class(class: &ast::StmtClassDef) -> bool {
    class.bases.iter().any(|base| {
        expr_full_name(base).is_some_and(|name| name == "TestCase" || name.ends_with(".TestCase"))
    })
}

fn decorator_names(decorators: &[Expr]) -> Vec<String> {
    decorators.iter().filter_map(expr_full_name).collect()
}

fn expr_full_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => expr_full_name(attribute.value.as_ref())
            .map(|prefix| format!("{prefix}.{}", attribute.attr)),
        Expr::Call(call) => expr_full_name(call.func.as_ref()),
        _ => None,
    }
}

fn find_related_tests(owner: &PythonOwner, all_tests: &[PythonTest]) -> Vec<RelatedTest> {
    all_tests
        .iter()
        .filter(|test| test_references_owner(test, owner))
        .map(|test| {
            let strongest = strongest_assertion(&test.assertions);
            let (oracle_kind, oracle_strength, oracle) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion_oracle_text(assertion)),
                ),
                None => (
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                    test.parametrized
                        .then_some("pytest.mark.parametrize".to_string()),
                ),
            };
            RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.line,
                oracle,
                oracle_kind,
                oracle_strength,
            }
        })
        .collect()
}

fn strongest_assertion(assertions: &[PythonAssertion]) -> Option<&PythonAssertion> {
    assertions
        .iter()
        .max_by_key(|assertion| assertion.oracle_strength.rank())
}

fn assertion_oracle_text(assertion: &PythonAssertion) -> String {
    format!(
        "{} at line {} ({}/{})",
        assertion.text,
        assertion.line,
        assertion.oracle_kind.as_str(),
        assertion.oracle_strength.as_str()
    )
}

fn test_references_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    let direct_call = format!("{}(", owner.name);
    let method_call = format!(".{}(", owner.name);
    let qualified_call = format!("{}(", owner.qualified_name);
    test.body_text.contains(&direct_call)
        || test.body_text.contains(&method_call)
        || test.body_text.contains(&qualified_call)
}

fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let trimmed = line_text.trim_start();
    if trimmed.starts_with("if ") || trimmed.starts_with("elif ") {
        return (ProbeFamily::Predicate, DeltaKind::Control);
    }
    if trimmed.starts_with("return ") || trimmed == "return" {
        return (ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if trimmed.starts_with("raise ") || trimmed == "raise" {
        return (ProbeFamily::ErrorPath, DeltaKind::Control);
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
    let strongest_strength = related
        .iter()
        .map(|test| test.oracle_strength.rank())
        .max()
        .unwrap_or(0);
    let strongest_kind = related
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .map(|test| test.oracle_kind.clone())
        .unwrap_or(OracleKind::Unknown);

    let (class, reach_state, observe_state, discriminate_state, missing) = if related.is_empty() {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![format!(
                "No Python test references `{}(`; add a pytest or unittest test that calls the changed owner.",
                owner.name
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        (
            ExposureClass::Exposed,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            vec![format!(
                "Related Python test reaches `{}` with a `{}` oracle. Preview static evidence suggests the changed behavior is observed by a strong assertion.",
                owner.name,
                strongest_kind.as_str()
            )],
        )
    } else {
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Related Python test reaches `{}` but the strongest extracted oracle is `{}`; upgrade by adding an exact-value or exact-error assertion for the changed behavior.",
                owner.name,
                strongest_kind.as_str()
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
    let reach_summary = format!(
        "{} related Python test(s) found for owner `{}`",
        related_count, owner.name
    );
    let reach = StageEvidence::new(reach_state, Confidence::Low, &reach_summary);
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
    let observe = StageEvidence::new(
        observe_state,
        Confidence::Low,
        format!(
            "Strongest extracted Python oracle kind: `{}` (rank {})",
            strongest_kind.as_str(),
            strongest_strength
        ),
    );
    let discriminate_summary = if strongest_strength >= OracleStrength::Strong.rank() {
        format!(
            "Related Python test uses a `{}` oracle; preview static evidence treats it as a discriminator.",
            strongest_kind.as_str()
        )
    } else {
        "Python preview found no strong discriminator; use exact-value or exact-error assertions to escalate.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, &discriminate_summary);

    let recommended = match class {
        ExposureClass::Exposed => {
            "Python preview: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            "Python preview: no related test calls the changed owner; add a pytest or unittest test that exercises this behavior.".to_string()
        }
        _ => {
            "Python preview: related test found; add or verify a focused exact-value or exact-error assertion for the changed behavior.".to_string()
        }
    };
    let confidence = if matches!(class, ExposureClass::Exposed) {
        0.55
    } else {
        0.4
    };

    let mut evidence = vec![
        format!("owner: {}", owner.qualified_name),
        format!("owner_kind: {}", owner.owner_kind.as_str()),
    ];
    if !owner.decorators.is_empty() {
        evidence.push(format!("owner_decorators: {}", owner.decorators.join(", ")));
    }
    for test in all_tests
        .iter()
        .filter(|test| test_references_owner(test, owner))
    {
        evidence.push(format!(
            "test_framework: {} ({})",
            test.framework, test.name
        ));
    }

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
        owner_kind: Some(owner.owner_kind),
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
            || name == "node_modules"
            || name == ".ripr"
            || name == ".direnv"
            || name == "__pycache__"
            || name == ".venv"
            || name == "venv"
            || name == "env"
            || name == ".mypy_cache"
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
        // Repo-mode preview output lands in a follow-up. The current
        // sub-slice scopes to diff-mode for the smallest useful fixture.
        Ok(LanguageRepoResult {
            findings: Vec::new(),
            production_files: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn changed(path: &str) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: Vec::new(),
            removed_lines: Vec::new(),
        }
    }

    fn test_named<'a>(tests: &'a [PythonTest], name: &str) -> Result<&'a PythonTest, String> {
        tests
            .iter()
            .find(|test| test.name == name)
            .ok_or_else(|| format!("expected Python test named {name}"))
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
        let ok = parse_module(
            Path::new("src/discount.py"),
            "def discount(amount: int) -> int:\n    return amount\n",
        )
        .is_some();
        assert!(ok, "valid Python should parse without errors");
    }

    #[test]
    fn parse_source_accepts_class_and_decorator() {
        let ok = parse_module(
            Path::new("src/repo.py"),
            "class Repo:\n    @staticmethod\n    def make() -> 'Repo':\n        return Repo()\n",
        )
        .is_some();
        assert!(ok, "decorated class methods should parse");
    }

    #[test]
    fn parse_source_accepts_async_def_and_fstring() {
        let ok = parse_module(
            Path::new("src/http.py"),
            "async def load(url: str) -> str:\n    return f\"{url}!\"\n",
        )
        .is_some();
        assert!(ok, "async def + f-string should parse");
    }

    #[test]
    fn parse_source_rejects_garbage() {
        let ok = parse_module(
            Path::new("src/oops.py"),
            "this is not :: valid +++ python at all",
        )
        .is_some();
        assert!(!ok, "garbage source should produce parse errors");
    }

    #[test]
    fn extract_owners_recognizes_functions_and_methods() {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            r#"
def apply_discount(amount):
    return amount

async def load_total(client):
    return await client.total()

class Policy:
    def apply(self, amount):
        return amount

    @staticmethod
    def normalize(amount):
        return amount

    @classmethod
    def from_config(cls, config):
        return cls()
"#,
        );

        assert_eq!(
            owners
                .iter()
                .map(|owner| owner.qualified_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "apply_discount",
                "load_total",
                "Policy.apply",
                "Policy.normalize",
                "Policy.from_config"
            ]
        );
        assert_eq!(owners[0].owner_kind, OwnerKind::Function);
        assert_eq!(owners[1].decorators, vec!["async_def"]);
        assert_eq!(owners[2].owner_kind, OwnerKind::Method);
        assert_eq!(owners[3].owner_kind, OwnerKind::ClassMethod);
        assert_eq!(owners[4].owner_kind, OwnerKind::ClassMethod);
    }

    #[test]
    fn extract_tests_recognizes_pytest_parametrize_and_unittest() {
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            r#"
import unittest
import pytest

@pytest.mark.parametrize("amount", [1, 2])
def test_apply_discount(amount):
    apply_discount(amount)

class PriceTests(unittest.TestCase):
    def test_apply_method(self):
        Policy().apply(10)
"#,
        );

        assert_eq!(
            tests
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["test_apply_discount", "test_apply_method"]
        );
        assert!(tests[0].parametrized);
        assert_eq!(tests[0].framework, "pytest");
        assert_eq!(tests[1].framework, "unittest");
    }

    #[test]
    fn extract_tests_recognizes_python_assertion_oracles() -> Result<(), String> {
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            r#"
import unittest
import pytest

def test_exact_value():
    assert apply_discount(100, 50) == 90

def test_truthiness():
    assert apply_discount(100, 50)

def test_type_shape():
    assert isinstance(apply_discount(100, 50), int)

def test_specific_error():
    with pytest.raises(ValueError):
        apply_discount(-1, 50)

def test_broad_error():
    with pytest.raises(Exception):
        apply_discount(-1, 50)

def test_mock_interaction(mock):
    notify_customer(mock)
    mock.assert_called_once_with("paid")

def test_mock_weak(mock):
    notify_customer(mock)
    mock.assert_not_called()

class PriceTests(unittest.TestCase):
    def test_unittest_equal(self):
        self.assertEqual(apply_discount(100, 50), 90)

    def test_unittest_raises(self):
        self.assertRaises(ValueError, apply_discount, -1, 50)
"#,
        );

        let exact = test_named(&tests, "test_exact_value")?;
        assert_eq!(exact.assertions[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(exact.assertions[0].oracle_strength, OracleStrength::Strong);

        let truthiness = test_named(&tests, "test_truthiness")?;
        assert_eq!(truthiness.assertions[0].oracle_kind, OracleKind::SmokeOnly);
        assert_eq!(
            truthiness.assertions[0].oracle_strength,
            OracleStrength::Smoke
        );

        let broad_type = test_named(&tests, "test_type_shape")?;
        assert_eq!(
            broad_type.assertions[0].oracle_kind,
            OracleKind::RelationalCheck
        );
        assert_eq!(
            broad_type.assertions[0].oracle_strength,
            OracleStrength::Weak
        );

        let specific_error = test_named(&tests, "test_specific_error")?;
        assert_eq!(
            specific_error.assertions[0].oracle_kind,
            OracleKind::ExactErrorVariant
        );
        assert_eq!(
            specific_error.assertions[0].oracle_strength,
            OracleStrength::Strong
        );

        let broad_error = test_named(&tests, "test_broad_error")?;
        assert_eq!(
            broad_error.assertions[0].oracle_kind,
            OracleKind::BroadError
        );
        assert_eq!(
            broad_error.assertions[0].oracle_strength,
            OracleStrength::Weak
        );

        let mock = test_named(&tests, "test_mock_interaction")?;
        assert_eq!(mock.assertions[0].oracle_kind, OracleKind::MockExpectation);
        assert_eq!(mock.assertions[0].oracle_strength, OracleStrength::Medium);

        let mock_weak = test_named(&tests, "test_mock_weak")?;
        assert_eq!(
            mock_weak.assertions[0].oracle_kind,
            OracleKind::MockExpectation
        );
        assert_eq!(
            mock_weak.assertions[0].oracle_strength,
            OracleStrength::Weak
        );

        let unittest_equal = test_named(&tests, "test_unittest_equal")?;
        assert_eq!(
            unittest_equal.assertions[0].oracle_kind,
            OracleKind::ExactValue
        );
        assert_eq!(
            unittest_equal.assertions[0].oracle_strength,
            OracleStrength::Strong
        );

        let unittest_raises = test_named(&tests, "test_unittest_raises")?;
        assert_eq!(
            unittest_raises.assertions[0].oracle_kind,
            OracleKind::ExactErrorVariant
        );
        assert_eq!(
            unittest_raises.assertions[0].oracle_strength,
            OracleStrength::Strong
        );

        Ok(())
    }

    #[test]
    fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    if amount >= 100:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_apply_discount():\n    result = apply_discount(100)\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= 100:",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.language, Some(DomainLanguageId::Python));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
        assert_eq!(finding.related_tests.len(), 1);
        Ok(())
    }

    #[test]
    fn classify_change_returns_exposed_with_strong_python_oracle() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    if amount >= 100:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_apply_discount():\n    assert apply_discount(100) == 90\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= 100:",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::Exposed);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        Ok(())
    }

    #[test]
    fn classify_change_keeps_broad_python_oracle_weak() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_apply_discount():\n    assert apply_discount(100)\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::SmokeOnly);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Smoke
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_no_static_path_without_related_test() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_other.py"),
            "def test_other():\n    other_behavior()\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
        assert!(finding.related_tests.is_empty());
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
