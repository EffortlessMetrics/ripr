#[cfg(test)]
use super::source_facts::extract_source_facts;
use super::source_utils::{
    line_for_range_end, line_for_range_start, normalized_path, text_for_range,
};
use super::static_limits::{collect_static_cli_receiver_names, is_static_route_decorator};
use super::{
    PythonImport, PythonOwner, PythonTest, collect_assertions_from_statements, expr_full_name,
    first_parenthesized_string_argument,
};
use crate::domain::OwnerKind;
use rustpython_parser::{
    ast::{self, Expr, Ranged, Stmt},
    text_size::TextRange,
};
use std::path::Path;

#[cfg(test)]
pub(super) fn extract_owners(file: &Path, source: &str) -> Vec<PythonOwner> {
    extract_source_facts(file, source).owners
}

pub(super) fn collect_owners_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    class_context: Option<&str>,
    imports: &[PythonImport],
    out: &mut Vec<PythonOwner>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) => {
                out.push(owner_from_function(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                    },
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    false,
                ));
            }
            Stmt::AsyncFunctionDef(function) => {
                out.push(owner_from_function(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                    },
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    true,
                ));
            }
            Stmt::ClassDef(class) => {
                collect_owners_from_statements(
                    file,
                    source,
                    &class.body,
                    Some(class.name.as_str()),
                    imports,
                    out,
                );
                out.push(owner_from_class(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                    },
                    class.name.as_str(),
                    class.range,
                    &class.decorator_list,
                ));
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
struct PythonOwnerContext<'a> {
    file: &'a Path,
    source: &'a str,
    class_context: Option<&'a str>,
    imports: &'a [PythonImport],
}

fn owner_from_function(
    context: PythonOwnerContext<'_>,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
    is_async: bool,
) -> PythonOwner {
    let decorator_names = decorator_names(decorators);
    let owner_kind = if context.class_context.is_some()
        && decorator_names.iter().any(|decorator| {
            decorator.ends_with("classmethod") || decorator.ends_with("staticmethod")
        }) {
        OwnerKind::ClassMethod
    } else if context.class_context.is_some() {
        OwnerKind::Method
    } else {
        OwnerKind::Function
    };
    let qualified_name = context
        .class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string());
    let route_paths = collect_static_route_paths(context.source, decorators);
    let dynamic_route_decorators = collect_dynamic_route_decorators(context.source, decorators);
    let mut decorators = decorator_names;
    if is_async {
        decorators.push("async_def".to_string());
    }
    PythonOwner {
        name: name.to_string(),
        qualified_name,
        file: context.file.to_path_buf(),
        start_line: line_for_range_start(context.source, range),
        end_line: line_for_range_end(context.source, range),
        owner_kind: Some(owner_kind),
        decorators,
        imports: context.imports.to_vec(),
        cli_receiver_names: collect_static_cli_receiver_names(context.source, context.imports),
        route_paths,
        dynamic_route_decorators,
    }
}

fn owner_from_class(
    context: PythonOwnerContext<'_>,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
) -> PythonOwner {
    let qualified_name = context
        .class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string());
    PythonOwner {
        name: name.to_string(),
        qualified_name,
        file: context.file.to_path_buf(),
        start_line: line_for_range_start(context.source, range),
        end_line: line_for_range_end(context.source, range),
        owner_kind: None,
        decorators: decorator_names(decorators),
        imports: context.imports.to_vec(),
        cli_receiver_names: collect_static_cli_receiver_names(context.source, context.imports),
        route_paths: collect_static_route_paths(context.source, decorators),
        dynamic_route_decorators: collect_dynamic_route_decorators(context.source, decorators),
    }
}

pub(super) fn module_owner(
    file: &Path,
    source: &str,
    range: TextRange,
    imports: &[PythonImport],
) -> PythonOwner {
    PythonOwner {
        name: "<module>".to_string(),
        qualified_name: "<module>".to_string(),
        file: file.to_path_buf(),
        start_line: line_for_range_start(source, range),
        end_line: line_for_range_end(source, range),
        owner_kind: Some(OwnerKind::ModuleFunction),
        decorators: Vec::new(),
        imports: imports.to_vec(),
        cli_receiver_names: collect_static_cli_receiver_names(source, imports),
        route_paths: Vec::new(),
        dynamic_route_decorators: Vec::new(),
    }
}

#[cfg(test)]
pub(super) fn extract_tests(file: &Path, source: &str) -> Vec<PythonTest> {
    extract_source_facts(file, source).tests
}

pub(super) fn collect_tests_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    class_context: Option<&str>,
    in_unittest_class: bool,
    imports: &[PythonImport],
    out: &mut Vec<PythonTest>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) if function.name.as_str().starts_with("test_") => {
                let framework = if in_unittest_class {
                    "unittest"
                } else {
                    "pytest"
                };
                let name = function.name.to_string();
                out.push(PythonTest {
                    qualified_name: qualified_test_name(class_context, &name),
                    name,
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    imports: imports.to_vec(),
                    decorators: decorator_names(&function.decorator_list),
                    fixtures: fixture_parameter_names(&function.args, framework),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework,
                    assertions: collect_assertions_from_statements(&function.body, source),
                });
            }
            Stmt::AsyncFunctionDef(function) if function.name.as_str().starts_with("test_") => {
                let framework = if in_unittest_class {
                    "unittest"
                } else {
                    "pytest"
                };
                let name = function.name.to_string();
                out.push(PythonTest {
                    qualified_name: qualified_test_name(class_context, &name),
                    name,
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    imports: imports.to_vec(),
                    decorators: decorator_names(&function.decorator_list),
                    fixtures: fixture_parameter_names(&function.args, framework),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework,
                    assertions: collect_assertions_from_statements(&function.body, source),
                });
            }
            Stmt::ClassDef(class) => {
                let class_is_unittest = is_unittest_class(class) || in_unittest_class;
                if class_is_unittest || is_pytest_class(class) {
                    let class_name = class.name.to_string();
                    let nested_class_context = qualified_test_name(class_context, &class_name);
                    collect_tests_from_statements(
                        file,
                        source,
                        &class.body,
                        Some(&nested_class_context),
                        class_is_unittest,
                        imports,
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}

fn qualified_test_name(class_context: Option<&str>, name: &str) -> String {
    class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn fixture_parameter_names(args: &ast::Arguments, framework: &str) -> Vec<String> {
    let mut names: Vec<String> = args
        .posonlyargs
        .iter()
        .chain(args.args.iter())
        .chain(args.kwonlyargs.iter())
        .map(|arg| arg.def.arg.to_string())
        .collect();
    if let Some(arg) = &args.vararg {
        names.push(arg.arg.to_string());
    }
    if let Some(arg) = &args.kwarg {
        names.push(arg.arg.to_string());
    }
    names.retain(|name| {
        name != "self"
            && name != "cls"
            && (framework == "pytest" || !matches!(name.as_str(), "subTest"))
    });
    names.sort();
    names.dedup();
    names
}

pub(super) fn collect_imports_from_statements(
    file: &Path,
    statements: &[Stmt],
) -> Vec<PythonImport> {
    let mut imports = Vec::new();
    for stmt in statements {
        match stmt {
            Stmt::Import(import) => {
                for alias in &import.names {
                    let imported = alias.name.to_string();
                    imports.push(PythonImport {
                        alias: alias
                            .asname
                            .as_ref()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| imported.clone()),
                        imported,
                        // A plain `import X` has no `from` module source.
                        source_module: String::new(),
                    });
                }
            }
            Stmt::ImportFrom(import) => {
                // `from src.handler import validate [as v]` — the source module
                // (`src.handler`) is the free-function identity evidence. For
                // package-local tests, resolve explicit relative imports against
                // the importing file so common Python layouts (`from .pricing
                // import discount`) can still carry owner-module identity.
                let source_module = import_source_module(file, import);
                for alias in &import.names {
                    let imported = alias.name.to_string();
                    imports.push(PythonImport {
                        alias: alias
                            .asname
                            .as_ref()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| imported.clone()),
                        imported,
                        source_module: source_module.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    imports
}

fn import_source_module(file: &Path, import: &ast::StmtImportFrom) -> String {
    let module = import
        .module
        .as_ref()
        .map(|module| module.to_string())
        .unwrap_or_default();
    let level = import
        .level
        .as_ref()
        .map(|level| level.to_usize())
        .unwrap_or(0);
    if level == 0 {
        return module;
    }
    let normalized = normalized_path(file);
    let mut parts = normalized.split('/').collect::<Vec<_>>();
    parts.pop();
    let package_depth = level.saturating_sub(1);
    for _ in 0..package_depth {
        if parts.pop().is_none() {
            return String::new();
        }
    }
    if !module.is_empty() {
        parts.extend(module.split('.').filter(|part| !part.is_empty()));
    }
    parts.join(".")
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

fn is_pytest_class(class: &ast::StmtClassDef) -> bool {
    class.name.as_str().starts_with("Test")
}

pub(super) fn decorator_names(decorators: &[Expr]) -> Vec<String> {
    decorators.iter().filter_map(expr_full_name).collect()
}

fn collect_static_route_paths(source: &str, decorators: &[Expr]) -> Vec<String> {
    decorators
        .iter()
        .filter_map(|decorator| {
            let name = expr_full_name(decorator)?;
            if !is_static_route_decorator(&name) {
                return None;
            }
            route_decorator_literal_argument(source, decorator, &name)
        })
        .collect()
}

fn collect_dynamic_route_decorators(source: &str, decorators: &[Expr]) -> Vec<String> {
    decorators
        .iter()
        .filter_map(|decorator| {
            let name = expr_full_name(decorator)?;
            if !is_static_route_decorator(&name) {
                return None;
            }
            route_decorator_literal_argument(source, decorator, &name)
                .is_none()
                .then_some(name)
        })
        .collect()
}

fn route_decorator_literal_argument(source: &str, decorator: &Expr, name: &str) -> Option<String> {
    let text = text_for_range(source, decorator.range());
    let after_name = text
        .strip_prefix(name)
        .or_else(|| text.find(name).and_then(|idx| text.get(idx + name.len()..)))?;
    first_parenthesized_string_argument(after_name.trim_start())
}
