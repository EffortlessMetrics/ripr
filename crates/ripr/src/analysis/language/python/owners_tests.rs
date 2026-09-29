use super::module_constants::{
    PythonModuleConstant, constants_visible_in_function, python_test_rebinding,
};
use super::parametrize::parametrize_cases;
#[cfg(test)]
use super::source_facts::extract_source_facts;
use super::source_utils::{
    SourceText, line_for_range_end, line_for_range_start, normalized_path, text_for_range,
};
use super::static_limits::{collect_static_cli_receiver_names, is_static_route_decorator};
use super::{
    PythonImport, PythonOwner, PythonParameter, PythonTest, collect_assertions_from_statements,
    expr_full_name, first_parenthesized_string_argument,
};
use crate::domain::OwnerKind;
use rustpython_parser::{
    ast::{self, Expr, Ranged, Stmt},
    text_size::TextRange,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[cfg(test)]
pub(super) fn extract_owners(file: &Path, source: &str) -> Vec<PythonOwner> {
    extract_source_facts(file, source).owners
}

pub(super) fn collect_owners_from_statements(
    file: &Path,
    source: &SourceText<'_>,
    statements: &[Stmt],
    class_context: Option<&str>,
    imports: &[PythonImport],
    module_constants: &[PythonModuleConstant],
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
                        module_constants,
                    },
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    &function.args,
                    &function.body,
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
                        module_constants,
                    },
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    &function.args,
                    &function.body,
                    true,
                ));
            }
            Stmt::ClassDef(class) => {
                let class_path = class_context.map_or_else(
                    || class.name.to_string(),
                    |outer| format!("{outer}.{}", class.name),
                );
                collect_owners_from_statements(
                    file,
                    source,
                    &class.body,
                    Some(class_path.as_str()),
                    imports,
                    module_constants,
                    out,
                );
                out.push(owner_from_class(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                        module_constants,
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

/// The innermost class of a dotted class path (`Inner` for `Outer.Inner`).
fn innermost_class(class_path: &str) -> &str {
    class_path.rsplit('.').next().unwrap_or(class_path)
}

#[derive(Clone, Copy)]
struct PythonOwnerContext<'a> {
    file: &'a Path,
    source: &'a SourceText<'a>,
    class_context: Option<&'a str>,
    imports: &'a [PythonImport],
    module_constants: &'a [PythonModuleConstant],
}

fn owner_from_function(
    context: PythonOwnerContext<'_>,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
    args: &ast::Arguments,
    body: &[Stmt],
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
        .map(|class| format!("{}.{name}", innermost_class(class)))
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
        parameters: function_parameters(context.source, args),
        reexport_modules: Vec::new(),
        ambiguous_src_modules: Vec::new(),
        module_constants: constants_visible_in_function(
            context.module_constants,
            args,
            body,
            &text_for_range(context.source, range),
        ),
        class_path: context.class_context.unwrap_or_default().to_string(),
    }
}

/// Declared parameters in binding order: positional-only, then regular, then
/// keyword-only. `*args` / `**kwargs` are not bindable names and are omitted.
fn function_parameters(source: &str, args: &ast::Arguments) -> Vec<PythonParameter> {
    let parameter =
        |arg: &ast::ArgWithDefault, keyword_only: bool, positional_only: bool| PythonParameter {
            name: arg.def.arg.to_string(),
            default: arg
                .default
                .as_ref()
                .map(|default| text_for_range(source, default.range()).trim().to_string()),
            keyword_only,
            positional_only,
        };
    args.posonlyargs
        .iter()
        .map(|arg| parameter(arg, false, true))
        .chain(args.args.iter().map(|arg| parameter(arg, false, false)))
        .chain(
            args.kwonlyargs
                .iter()
                .map(|arg| parameter(arg, true, false)),
        )
        .collect()
}

fn owner_from_class(
    context: PythonOwnerContext<'_>,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
) -> PythonOwner {
    let qualified_name = context
        .class_context
        .map(|class| format!("{}.{name}", innermost_class(class)))
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
        parameters: Vec::new(),
        reexport_modules: Vec::new(),
        ambiguous_src_modules: Vec::new(),
        module_constants: Vec::new(),
        class_path: String::new(),
    }
}

pub(super) fn module_owner(
    file: &Path,
    source: &SourceText<'_>,
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
        parameters: Vec::new(),
        reexport_modules: Vec::new(),
        ambiguous_src_modules: Vec::new(),
        module_constants: Vec::new(),
        class_path: String::new(),
    }
}

#[cfg(test)]
pub(super) fn extract_tests(file: &Path, source: &str) -> Vec<PythonTest> {
    extract_source_facts(file, source).tests
}

/// Follow the default pytest `python_functions` and unittest
/// `TestLoader.testMethodPrefix`: both use `test`, not `test_`.
/// Custom collection prefixes and hooks are not resolved here.
pub(super) fn collect_tests_from_statements(
    file: &Path,
    source: &SourceText<'_>,
    statements: &[Stmt],
    class_context: Option<&str>,
    in_unittest_class: bool,
    imports: &[PythonImport],
    out: &mut Vec<PythonTest>,
) {
    let local_classes = LocalTestClasses::of(statements);
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) if function.name.as_str().starts_with("test") => {
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
                    imports: test_imports(file, imports, &function.body),
                    decorators: decorator_names(&function.decorator_list),
                    fixtures: fixture_parameter_names(&function.args, framework),
                    parametrized: is_parametrized(&function.decorator_list),
                    // pytest does not parametrize unittest methods.
                    parametrize: (framework == "pytest")
                        .then(|| parametrize_cases(source, &function.decorator_list))
                        .flatten()
                        .and_then(|cases| cases.excluding_body_bindings(&function.body)),
                    framework,
                    assertions: collect_assertions_from_statements(&function.body, source),
                    constant_rebinding: python_test_rebinding(
                        &function.args,
                        &function.body,
                        &text_for_range(source, function.range),
                    ),
                });
            }
            Stmt::AsyncFunctionDef(function) if function.name.as_str().starts_with("test") => {
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
                    imports: test_imports(file, imports, &function.body),
                    decorators: decorator_names(&function.decorator_list),
                    fixtures: fixture_parameter_names(&function.args, framework),
                    parametrized: is_parametrized(&function.decorator_list),
                    // pytest does not parametrize unittest methods.
                    parametrize: (framework == "pytest")
                        .then(|| parametrize_cases(source, &function.decorator_list))
                        .flatten()
                        .and_then(|cases| cases.excluding_body_bindings(&function.body)),
                    framework,
                    assertions: collect_assertions_from_statements(&function.body, source),
                    constant_rebinding: python_test_rebinding(
                        &function.args,
                        &function.body,
                        &text_for_range(source, function.range),
                    ),
                });
            }
            Stmt::ClassDef(class) if local_classes.is_last_definition(class) => {
                let class_name = class.name.as_str();
                let class_is_unittest =
                    in_unittest_class || local_classes.unittest.contains(class_name);
                if class_is_unittest || local_classes.pytest.contains(class_name) {
                    let nested_class_context = qualified_test_name(class_context, class_name);
                    collect_tests_from_statements(
                        file,
                        source,
                        &class.body,
                        Some(&nested_class_context),
                        class_is_unittest,
                        imports,
                        out,
                    );
                    // Mixin members this class resolves to run as this
                    // class's tests, under its selector
                    // (`test_x.py::TestConsumer::test_shared`).
                    for (mixin, members) in local_classes
                        .inherited
                        .get(class_name)
                        .into_iter()
                        .flatten()
                    {
                        let mut from_mixin = Vec::new();
                        collect_tests_from_statements(
                            file,
                            source,
                            &mixin.body,
                            Some(&nested_class_context),
                            class_is_unittest,
                            imports,
                            &mut from_mixin,
                        );
                        out.extend(from_mixin.into_iter().filter(|test| {
                            members.contains(test.name.as_str())
                                && test.qualified_name
                                    == qualified_test_name(Some(&nested_class_context), &test.name)
                        }));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Module imports plus the imports at the top level of the test body
/// (`def test_x(): from pkg.utils import sign`), which bind the same way for
/// the rest of the test (#4567). A later import of the same name replaces the
/// earlier one, as Python binds it: `from owner import sign` followed by
/// `from other import sign` leaves only `other.sign`.
fn test_imports(file: &Path, module_imports: &[PythonImport], body: &[Stmt]) -> Vec<PythonImport> {
    let mut imports = module_imports.to_vec();
    for import in collect_imports_from_statements(file, body) {
        imports.retain(|earlier| earlier.alias != import.alias);
        imports.push(import);
    }
    imports
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

/// Which same-scope classes pytest or unittest collects (#4562).
///
/// A class is collected on its own when it reaches `unittest.TestCase`
/// through same-scope bases, or when pytest collects it: a `Test*` name, and
/// neither it nor a same-scope ancestor defines `__init__`/`__new__` or is a
/// dataclass. A same-scope ancestor of a collected class that is not itself
/// collected is a mixin; each of its `test*` members runs as a test of every
/// collected subclass that resolves that member to the mixin along its C3
/// method resolution order, and is recorded under that subclass
/// (`TestConsumer.test_shared`), the selector the runner accepts. An imported base, a base defined after the subclass, or
/// a redefined name is external: reaching it first makes the resolution
/// unknown, so the member is not collected. Only the last definition of a
/// class name is collected, as the loaders see the module attribute.
#[derive(Default)]
struct LocalTestClasses<'a> {
    /// Classes unittest collects.
    unittest: BTreeSet<&'a str>,
    /// `Test*` classes pytest collects.
    pytest: BTreeSet<&'a str>,
    /// Per collected class, in its method resolution order, each mixin and
    /// the `test*` members the class resolves to that mixin.
    inherited: BTreeMap<&'a str, Vec<(&'a ast::StmtClassDef, BTreeSet<&'a str>)>>,
    last_definitions: BTreeSet<*const ast::StmtClassDef>,
}

impl<'a> LocalTestClasses<'a> {
    fn of(statements: &'a [Stmt]) -> Self {
        let scope = ScopeClasses::of(statements);
        let mut found = Self {
            last_definitions: scope
                .defs
                .values()
                .map(|(_, class)| *class as *const _)
                .collect(),
            ..Self::default()
        };
        let mut own_unittest: BTreeSet<&'a str> = BTreeSet::new();
        let mut changed = true;
        while changed {
            changed = false;
            for (name, (_, class)) in &scope.defs {
                if !own_unittest.contains(name)
                    && (is_unittest_class(class)
                        || scope
                            .local_bases(name)
                            .iter()
                            .any(|base| own_unittest.contains(base)))
                {
                    own_unittest.insert(name);
                    changed = true;
                }
            }
        }
        for name in scope.defs.keys() {
            if name.starts_with("Test")
                && !own_unittest.contains(name)
                && !scope.ancestors_or_self(name).iter().any(|ancestor| {
                    scope
                        .defs
                        .get(ancestor)
                        .is_some_and(|(_, class)| blocks_pytest_collection(class))
                })
            {
                found.pytest.insert(name);
            }
        }
        let collected: BTreeSet<&'a str> = own_unittest.union(&found.pytest).copied().collect();
        for name in &collected {
            let Some(mro) = scope.mro(name, &mut Vec::new()) else {
                continue;
            };
            let mut inherited = Vec::new();
            for entry in &mro {
                let MroEntry::Local(mixin) = entry else {
                    continue;
                };
                if collected.contains(mixin) {
                    continue;
                }
                let Some((_, class)) = scope.defs.get(mixin) else {
                    continue;
                };
                let members: BTreeSet<&'a str> = test_member_names(class)
                    .into_iter()
                    .filter(|member| scope.resolve_member(&mro, member) == Some(*mixin))
                    .collect();
                if !members.is_empty() {
                    inherited.push((*class, members));
                }
            }
            if !inherited.is_empty() {
                found.inherited.insert(name, inherited);
            }
        }
        found.unittest = own_unittest;
        found
    }

    /// Whether `class` is the last definition of its name in this scope, the
    /// one pytest and unittest see as the module attribute.
    fn is_last_definition(&self, class: &ast::StmtClassDef) -> bool {
        self.last_definitions.contains(&(class as *const _))
    }
}

/// The classes of one scope, by the last definition of each name.
struct ScopeClasses<'a> {
    /// Name -> (statement index, last definition).
    defs: BTreeMap<&'a str, (usize, &'a ast::StmtClassDef)>,
    /// Names defined more than once.
    redefined: BTreeSet<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MroEntry<'a> {
    Local(&'a str),
    /// A base this scope does not define before the class (an import, a
    /// forward reference, a redefined name): its members are unknown.
    External(String),
}

impl<'a> ScopeClasses<'a> {
    fn of(statements: &'a [Stmt]) -> Self {
        let mut defs = BTreeMap::new();
        let mut redefined = BTreeSet::new();
        for (index, stmt) in statements.iter().enumerate() {
            if let Stmt::ClassDef(class) = stmt
                && defs.insert(class.name.as_str(), (index, class)).is_some()
            {
                redefined.insert(class.name.as_str());
            }
        }
        Self { defs, redefined }
    }

    /// The bases of `name` in order, local when the scope defines the base
    /// exactly once and before `name`. `object` and `TestCase` define no
    /// `test*` member and are left out.
    fn bases(&self, name: &str) -> Vec<MroEntry<'a>> {
        let Some((index, class)) = self.defs.get(name) else {
            return Vec::new();
        };
        class
            .bases
            .iter()
            .filter_map(expr_full_name)
            .filter(|base| !matches!(base.as_str(), "object" | "TestCase" | "unittest.TestCase"))
            .map(|base| match self.defs.get_key_value(base.as_str()) {
                Some((&key, (base_index, _)))
                    if base_index < index && !self.redefined.contains(key) =>
                {
                    MroEntry::Local(key)
                }
                _ => MroEntry::External(base),
            })
            .collect()
    }

    fn local_bases(&self, name: &str) -> Vec<&'a str> {
        self.bases(name)
            .into_iter()
            .filter_map(|base| match base {
                MroEntry::Local(key) => Some(key),
                MroEntry::External(_) => None,
            })
            .collect()
    }

    /// `name` and its local ancestors, each once.
    fn ancestors_or_self(&self, name: &'a str) -> Vec<&'a str> {
        let mut seen: Vec<&'a str> = Vec::new();
        let mut stack = vec![name];
        while let Some(current) = stack.pop() {
            if !seen.contains(&current) {
                seen.push(current);
                stack.extend(self.local_bases(current));
            }
        }
        seen
    }

    /// Python's C3 linearization of `name`, or None when it does not exist.
    fn mro(&self, name: &'a str, visiting: &mut Vec<&'a str>) -> Option<Vec<MroEntry<'a>>> {
        if visiting.contains(&name) {
            return None;
        }
        visiting.push(name);
        let bases = self.bases(name);
        let mut sequences: Vec<Vec<MroEntry<'a>>> = Vec::new();
        for base in &bases {
            sequences.push(match base {
                MroEntry::Local(key) => self.mro(key, visiting)?,
                MroEntry::External(_) => vec![base.clone()],
            });
        }
        sequences.push(bases);
        visiting.pop();
        let mut linear = vec![MroEntry::Local(name)];
        loop {
            sequences.retain(|sequence| !sequence.is_empty());
            if sequences.is_empty() {
                return Some(linear);
            }
            let head = sequences
                .iter()
                .map(|sequence| &sequence[0])
                .find(|candidate| {
                    !sequences
                        .iter()
                        .any(|sequence| sequence[1..].contains(candidate))
                })?
                .clone();
            for sequence in &mut sequences {
                if sequence[0] == head {
                    sequence.remove(0);
                }
            }
            linear.push(head);
        }
    }

    /// The class `member` resolves to along `mro`, or None when it is
    /// missing or an external base comes first.
    fn resolve_member(&self, mro: &[MroEntry<'a>], member: &str) -> Option<&'a str> {
        for entry in mro {
            match entry {
                MroEntry::Local(key) => {
                    if self
                        .defs
                        .get(key)
                        .is_some_and(|(_, class)| test_member_names(class).contains(member))
                    {
                        return Some(key);
                    }
                }
                MroEntry::External(_) => return None,
            }
        }
        None
    }
}

/// A class pytest will not collect as a test class: it defines `__init__` or
/// `__new__`, or a dataclass decorator generates `__init__`.
fn blocks_pytest_collection(class: &ast::StmtClassDef) -> bool {
    class.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::FunctionDef(function) if matches!(function.name.as_str(), "__init__" | "__new__")
        ) || matches!(
            stmt,
            Stmt::AsyncFunctionDef(function) if matches!(function.name.as_str(), "__init__" | "__new__")
        )
    }) || class.decorator_list.iter().any(|decorator| {
        let callee = match decorator {
            Expr::Call(call) => call.func.as_ref(),
            other => other,
        };
        expr_full_name(callee).is_some_and(|name| {
            name == "dataclass"
                || name.ends_with(".dataclass")
                || matches!(name.as_str(), "attr.s" | "attr.attrs" | "attrs.define" | "attrs.frozen" | "define" | "frozen")
        })
    })
}

/// The `test*` names a class body binds: methods, and class-level
/// assignments that override an inherited test (`test_x = None`).
fn test_member_names(class: &ast::StmtClassDef) -> BTreeSet<&str> {
    let mut names = BTreeSet::new();
    for stmt in &class.body {
        match stmt {
            Stmt::FunctionDef(function) => {
                names.insert(function.name.as_str());
            }
            Stmt::AsyncFunctionDef(function) => {
                names.insert(function.name.as_str());
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Expr::Name(name) = target {
                        names.insert(name.id.as_str());
                    }
                }
            }
            Stmt::AnnAssign(assign) => {
                if let Expr::Name(name) = assign.target.as_ref() {
                    names.insert(name.id.as_str());
                }
            }
            _ => {}
        }
    }
    names.retain(|name| name.starts_with("test"));
    names
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

#[cfg(test)]
mod tests;
