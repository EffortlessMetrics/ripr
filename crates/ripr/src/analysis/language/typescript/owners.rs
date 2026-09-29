//! Owner extraction for the TypeScript preview adapter.

use super::*;
use oxc_ast::ast::{AssignmentOperator, AssignmentTarget, PropertyKind, StaticMemberExpression};

pub(crate) fn extract_owners(file: &Path, source: &str) -> Vec<TypeScriptOwner> {
    // Parse (and walk) on the dedicated large-stack worker behind the
    // nesting budget (#4101): a refused source or a failed worker yields no
    // owners, exactly like the previous parse-error path, while the oxc
    // recursion no longer runs on the caller's small stack.
    let Ok(owners) = parse_on_worker(file, source, |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file)).parse();
        if !ret.errors.is_empty() {
            return Vec::new();
        }
        let imports = extract_imports_from_statements(&ret.program.body);
        // One line index per source; every owner line below is a binary
        // search against it rather than a rescan from byte 0.
        let source = &SourceText::new(source);
        // `(owner, is a CommonJS assignment export)`, in source order.
        let mut owners = Vec::new();
        for stmt in &ret.program.body {
            let commonjs = matches!(stmt, Statement::ExpressionStatement(_));
            owners.extend(
                owners_from_statement(stmt, file, source, &imports)
                    .into_iter()
                    .map(|owner| (owner, commonjs)),
            );
        }
        let mut owners = without_reassigned_commonjs_exports(owners);
        let entries = module_entries_by_owner(&ret.program.body, source);
        for owner in &mut owners {
            if owner.class_name.is_none()
                && owner.owner_kind != OwnerKind::ModuleFunction
                && let Some(owner_entries) = entries.get(&owner.name)
            {
                owner.module_entries = owner_entries.clone();
            }
        }
        owners
    }) else {
        return Vec::new();
    };
    owners
}

pub(crate) fn owners_from_statement(
    stmt: &Statement<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    if let Statement::FunctionDeclaration(func) = stmt
        && let Some(id) = &func.id
    {
        return vec![owner_from_function(
            file,
            source,
            id.name.as_str(),
            func,
            function_owner_kind(
                file,
                source,
                id.name.as_str(),
                func.span.start,
                func.span.end,
            ),
            false,
            imports,
        )];
    }
    if let Statement::ExportNamedDeclaration(export) = stmt
        && let Some(decl) = export.declaration.as_ref()
    {
        return owners_from_declaration(decl, file, source, imports);
    }
    if let Statement::ExportDefaultDeclaration(export) = stmt {
        return owners_from_default_export(&export.declaration, file, source, imports);
    }
    if let Statement::ExpressionStatement(expression) = stmt {
        return owners_from_commonjs_export(&expression.expression, file, source, imports);
    }
    owners_from_statement_declaration(stmt, file, source, imports)
}

/// Owners carved from a top-level CommonJS assignment export (#4545):
///
/// - `exports.NAME = function ... / arrow` and
///   `module.exports.NAME = function ... / arrow` yield an owner named by the
///   static property `NAME`;
/// - `module.exports = function NAME(...) ... / arrow` yields the module's
///   default-export owner (named by the function id, else `default`), the
///   CommonJS mirror of `export default function`;
/// - `module.exports = { NAME() {}, NAME: function () {}, NAME: () => ... }`
///   yields one owner per static-identifier key whose value is a function.
///
/// Anything else — a non-function value, a computed or string-literal
/// property, a compound operator, a chained assignment, a getter/setter or
/// spread entry — yields no owner (fail-closed: no invented owner).
fn owners_from_commonjs_export(
    expression: &Expression<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    let Expression::AssignmentExpression(assign) = expression else {
        return Vec::new();
    };
    if assign.operator != AssignmentOperator::Assign {
        return Vec::new();
    }
    let AssignmentTarget::StaticMemberExpression(target) = &assign.left else {
        return Vec::new();
    };
    let property = target.property.name.as_str();
    let named_export_object = match &target.object {
        // `exports.NAME = ...`
        Expression::Identifier(object) => object.name == "exports",
        // `module.exports.NAME = ...`
        Expression::StaticMemberExpression(object) => is_module_exports(object),
        _ => false,
    };
    if named_export_object {
        return commonjs_function_owner(
            &assign.right,
            property,
            assign.span.start,
            file,
            source,
            imports,
        )
        .into_iter()
        .collect();
    }
    if !is_module_exports(target) {
        return Vec::new();
    }
    // `module.exports = ...`
    match &assign.right {
        Expression::FunctionExpression(func) => {
            let name = func
                .id
                .as_ref()
                .map(|id| id.name.as_str())
                .unwrap_or("default");
            commonjs_function_owner(
                &assign.right,
                name,
                assign.span.start,
                file,
                source,
                imports,
            )
            .map(|mut owner| {
                owner.exported_as_default = true;
                vec![owner]
            })
            .unwrap_or_default()
        }
        Expression::ArrowFunctionExpression(_) => commonjs_function_owner(
            &assign.right,
            "default",
            assign.span.start,
            file,
            source,
            imports,
        )
        .map(|mut owner| {
            owner.exported_as_default = true;
            vec![owner]
        })
        .unwrap_or_default(),
        Expression::ObjectExpression(object) => object
            .properties
            .iter()
            .filter_map(|property| {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                if property.computed || property.kind != PropertyKind::Init {
                    return None;
                }
                let name = property_key_name(&property.key)?;
                commonjs_function_owner(
                    &property.value,
                    &name,
                    property.span.start,
                    file,
                    source,
                    imports,
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Drop every CommonJS export owner whose name another CommonJS export in
/// the same file also defines (#4638 review). `module.exports = { f: fn }`
/// followed by `module.exports.f = ...` leaves only the last value live, but
/// which one runs depends on statement order and control flow the syntax
/// walk does not model; crediting the dead one would report an exposure for
/// code no test reaches. Fail-closed: no owner for a reassigned export.
fn without_reassigned_commonjs_exports(
    owners: Vec<(TypeScriptOwner, bool)>,
) -> Vec<TypeScriptOwner> {
    let commonjs_names: Vec<String> = owners
        .iter()
        .filter(|(_, commonjs)| *commonjs)
        .map(|(owner, _)| owner.name.clone())
        .collect();
    owners
        .into_iter()
        .filter(|(owner, commonjs)| {
            !*commonjs
                || commonjs_names
                    .iter()
                    .filter(|name| **name == owner.name)
                    .count()
                    == 1
        })
        .map(|(owner, _)| owner)
        .collect()
}

/// `module.exports` as a static member expression on the bare `module`
/// identifier.
fn is_module_exports(member: &StaticMemberExpression<'_>) -> bool {
    member.property.name == "exports"
        && matches!(&member.object, Expression::Identifier(object) if object.name == "module")
}

/// A function owner for a CommonJS export value: `function ...` or an arrow;
/// any other value yields `None`.
fn commonjs_function_owner(
    value: &Expression<'_>,
    name: &str,
    owner_start: u32,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Option<TypeScriptOwner> {
    match value {
        Expression::FunctionExpression(func) => Some(owner_from_function(
            file,
            source,
            name,
            func,
            function_owner_kind(file, source, name, func.span.start, func.span.end),
            false,
            imports,
        )),
        Expression::ArrowFunctionExpression(arrow) => Some(owner_from_arrow(
            file,
            source,
            name,
            arrow,
            owner_start,
            false,
            imports,
        )),
        _ => None,
    }
}

pub(crate) fn owners_from_statement_declaration(
    stmt: &Statement<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    match stmt {
        Statement::VariableDeclaration(decl) => {
            owners_from_variable_declaration(decl, file, source, imports)
        }
        Statement::ClassDeclaration(class) => owners_from_class(class, file, source, imports),
        _ => Vec::new(),
    }
}

pub(crate) fn owners_from_declaration(
    decl: &Declaration<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    match decl {
        Declaration::FunctionDeclaration(func) => func
            .id
            .as_ref()
            .map(|id| {
                vec![owner_from_function(
                    file,
                    source,
                    id.name.as_str(),
                    func,
                    function_owner_kind(
                        file,
                        source,
                        id.name.as_str(),
                        func.span.start,
                        func.span.end,
                    ),
                    false,
                    imports,
                )]
            })
            .unwrap_or_default(),
        Declaration::VariableDeclaration(decl) => {
            owners_from_variable_declaration(decl, file, source, imports)
        }
        Declaration::ClassDeclaration(class) => owners_from_class(class, file, source, imports),
        _ => Vec::new(),
    }
}

pub(crate) fn owners_from_default_export(
    decl: &ExportDefaultDeclarationKind<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    let mut owners = match decl {
        ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            let name = func
                .id
                .as_ref()
                .map(|id| id.name.as_str())
                .unwrap_or("default");
            vec![owner_from_function(
                file,
                source,
                name,
                func,
                function_owner_kind(file, source, name, func.span.start, func.span.end),
                false,
                imports,
            )]
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            owners_from_class(class, file, source, imports)
        }
        ExportDefaultDeclarationKind::ArrowFunctionExpression(arrow) => vec![owner_from_arrow(
            file,
            source,
            "default",
            arrow,
            arrow.span.start,
            false,
            imports,
        )],
        _ => Vec::new(),
    };
    // The relation gate may only credit a default import
    // (`import local from './owner'`) against an owner that IS the module's
    // default export (#4103 under-credit). Function and arrow owners carved
    // from `export default <declaration>` are that export. A default
    // EXPORTED CLASS has no class-level owner entry — `owners_from_class`
    // yields only its methods — so those methods carry the separate
    // `class_default_export` marker (constructor matching, #4104-B) while
    // `exported_as_default` stays false: a method is not itself the
    // module's default export (review #4138).
    match decl {
        ExportDefaultDeclarationKind::ClassDeclaration(_) => {
            for owner in &mut owners {
                owner.class_default_export = true;
            }
        }
        _ => {
            for owner in &mut owners {
                owner.exported_as_default = true;
            }
        }
    }
    owners
}

pub(crate) fn owners_from_variable_declaration(
    decl: &VariableDeclaration<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    decl.declarations
        .iter()
        .filter_map(|declarator| owner_from_variable_declarator(declarator, file, source, imports))
        .collect()
}

pub(crate) fn owner_from_variable_declarator(
    declarator: &VariableDeclarator<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Option<TypeScriptOwner> {
    let name = binding_identifier_name(&declarator.id)?;
    let init = declarator.init.as_ref()?;
    match init {
        Expression::ArrowFunctionExpression(arrow) => Some(owner_from_arrow(
            file,
            source,
            name,
            arrow,
            declarator.span.start,
            false,
            imports,
        )),
        Expression::FunctionExpression(func) => Some(owner_from_function(
            file,
            source,
            name,
            func,
            function_owner_kind(file, source, name, func.span.start, func.span.end),
            false,
            imports,
        )),
        _ => Some(TypeScriptOwner {
            name: name.to_string(),
            file: file.to_path_buf(),
            start_line: source.line_for_offset(declarator.span.start as usize),
            end_line: source.line_for_offset(declarator.span.end as usize),
            owner_kind: OwnerKind::ModuleFunction,
            class_name: None,
            decorated: false,
            exported_as_default: false,
            imports: imports.to_vec(),
            method_kind: TypeScriptMethodKind::Ordinary,
            class_default_export: false,
            module_entries: Vec::new(),
            // Non-function initializers carry no resolvable signature facts:
            // no fixed positional signature the boundary witness could
            // arity-check against.
            params: Vec::new(),
            arity: None,
            source_text: None,
        }),
    }
}

/// Parameter facts for the owner signature (issue #4102): `(Some(n), names)`
/// when every parameter is a plain binding identifier and there is no rest
/// parameter; `(None, [])` otherwise (destructuring, rest, or empty list is
/// still resolvable — a zero-parameter function is `Some(0)`).
fn parameter_facts(params: &FormalParameters<'_>) -> (Option<usize>, Vec<String>) {
    if params.rest.is_some() {
        return (None, Vec::new());
    }
    let mut names = Vec::with_capacity(params.items.len());
    for item in &params.items {
        match binding_identifier_name(&item.pattern) {
            Some(name) => names.push(name.to_string()),
            // Destructured / pattern parameters cannot be mapped to an
            // argument position — fail closed to unknown arity.
            None => return (None, Vec::new()),
        }
    }
    (Some(names.len()), names)
}

pub(crate) fn owner_from_function(
    file: &Path,
    source: &SourceText<'_>,
    name: &str,
    func: &Function<'_>,
    owner_kind: OwnerKind,
    decorated: bool,
    imports: &[TypeScriptImport],
) -> TypeScriptOwner {
    let (arity, params) = parameter_facts(&func.params);
    TypeScriptOwner {
        name: name.to_string(),
        file: file.to_path_buf(),
        start_line: source.line_for_offset(func.span.start as usize),
        end_line: source.line_for_offset(func.span.end as usize),
        owner_kind,
        class_name: None,
        decorated,
        exported_as_default: false,
        imports: imports.to_vec(),
        method_kind: TypeScriptMethodKind::Ordinary,
        class_default_export: false,
        module_entries: Vec::new(),
        params,
        arity,
        source_text: Some(source[func.span.start as usize..func.span.end as usize].to_string()),
    }
}

pub(crate) fn owner_from_arrow(
    file: &Path,
    source: &SourceText<'_>,
    name: &str,
    arrow: &ArrowFunctionExpression<'_>,
    owner_start: u32,
    decorated: bool,
    imports: &[TypeScriptImport],
) -> TypeScriptOwner {
    let (arity, params) = parameter_facts(&arrow.params);
    TypeScriptOwner {
        name: name.to_string(),
        file: file.to_path_buf(),
        start_line: source.line_for_offset(owner_start as usize),
        end_line: source.line_for_offset(arrow.span.end as usize),
        owner_kind: arrow_owner_kind(file, source, name, arrow.span.start, arrow.span.end),
        class_name: None,
        decorated,
        exported_as_default: false,
        imports: imports.to_vec(),
        method_kind: TypeScriptMethodKind::Ordinary,
        class_default_export: false,
        module_entries: Vec::new(),
        params,
        arity,
        source_text: Some(source[arrow.span.start as usize..arrow.span.end as usize].to_string()),
    }
}

pub(crate) fn owners_from_class(
    class: &Class<'_>,
    file: &Path,
    source: &SourceText<'_>,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    let mut owners = Vec::new();
    let class_decorated = !class.decorators.is_empty();
    let class_name = class
        .id
        .as_ref()
        .map(|identifier| identifier.name.as_str().to_string());
    for element in &class.body.body {
        if let ClassElement::MethodDefinition(method) = element
            && let Some(owner) = owner_from_method(
                method,
                file,
                source,
                class_decorated,
                class_name.as_deref(),
                imports,
            )
        {
            owners.push(owner);
        }
    }
    owners
}

pub(crate) fn owner_from_method(
    method: &MethodDefinition<'_>,
    file: &Path,
    source: &SourceText<'_>,
    class_decorated: bool,
    class_name: Option<&str>,
    imports: &[TypeScriptImport],
) -> Option<TypeScriptOwner> {
    if method.computed {
        return None;
    }
    let name = property_key_name(&method.key)?;
    let (arity, params) = parameter_facts(&method.value.params);
    Some(TypeScriptOwner {
        name,
        file: file.to_path_buf(),
        start_line: source.line_for_offset(method.span.start as usize),
        end_line: source.line_for_offset(method.span.end as usize),
        owner_kind: if method.r#static {
            OwnerKind::ClassMethod
        } else {
            OwnerKind::Method
        },
        class_name: class_name.map(str::to_string),
        decorated: class_decorated || !method.decorators.is_empty(),
        exported_as_default: false,
        imports: imports.to_vec(),
        method_kind: match method.kind {
            oxc_ast::ast::MethodDefinitionKind::Constructor => TypeScriptMethodKind::Constructor,
            oxc_ast::ast::MethodDefinitionKind::Get => TypeScriptMethodKind::Getter,
            oxc_ast::ast::MethodDefinitionKind::Set => TypeScriptMethodKind::Setter,
            oxc_ast::ast::MethodDefinitionKind::Method => TypeScriptMethodKind::Ordinary,
        },
        class_default_export: false,
        module_entries: Vec::new(),
        params,
        arity,
        source_text: Some(source[method.span.start as usize..method.span.end as usize].to_string()),
    })
}

pub(crate) fn binding_identifier_name<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

pub(crate) fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

pub(crate) fn function_owner_kind(
    file: &Path,
    source: &str,
    name: &str,
    start: u32,
    end: u32,
) -> OwnerKind {
    if looks_like_component_owner(file, source, name, start, end) {
        OwnerKind::Component
    } else {
        OwnerKind::Function
    }
}

pub(crate) fn arrow_owner_kind(
    file: &Path,
    source: &str,
    name: &str,
    start: u32,
    end: u32,
) -> OwnerKind {
    if looks_like_component_owner(file, source, name, start, end) {
        OwnerKind::Component
    } else {
        OwnerKind::ArrowFunction
    }
}

pub(crate) fn looks_like_component_owner(
    file: &Path,
    source: &str,
    name: &str,
    start: u32,
    end: u32,
) -> bool {
    if !matches!(
        file.extension().and_then(|extension| extension.to_str()),
        Some("tsx" | "jsx")
    ) || !starts_with_uppercase(name)
    {
        return false;
    }
    let start = start as usize;
    let end = end as usize;
    let Some(slice) = source.get(start..end) else {
        return false;
    };
    contains_jsx_like_return(slice)
}

pub(crate) fn starts_with_uppercase(name: &str) -> bool {
    name.chars().next().is_some_and(|ch| ch.is_uppercase())
}

pub(crate) fn contains_jsx_like_return(slice: &str) -> bool {
    slice.contains("return <")
        || slice.contains("=> <")
        || slice
            .split("return (")
            .skip(1)
            .any(|tail| tail.trim_start().starts_with('<'))
        || slice
            .split("=> (")
            .skip(1)
            .any(|tail| tail.trim_start().starts_with('<'))
}

pub(crate) fn extract_imports_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
) -> Vec<TypeScriptImport> {
    let mut out: Vec<TypeScriptImport> = Vec::new();
    for stmt in statements {
        // ES module import declarations: `import { x } from './y'`
        if let Statement::ImportDeclaration(import) = stmt {
            if import.import_kind == ImportOrExportKind::Type {
                continue;
            }
            let source = import.source.value.to_string();
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        if specifier.import_kind == ImportOrExportKind::Type {
                            continue;
                        }
                        let Some(imported) = module_export_name_text(&specifier.imported) else {
                            continue;
                        };
                        push_unique_import(
                            &mut out,
                            TypeScriptImport {
                                source: source.clone(),
                                imported: Some(imported),
                                local: specifier.local.name.as_str().to_string(),
                                namespace: false,
                            },
                        );
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        push_unique_import(
                            &mut out,
                            TypeScriptImport {
                                source: source.clone(),
                                imported: Some("default".to_string()),
                                local: specifier.local.name.as_str().to_string(),
                                namespace: false,
                            },
                        );
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        push_unique_import(
                            &mut out,
                            TypeScriptImport {
                                source: source.clone(),
                                imported: None,
                                local: specifier.local.name.as_str().to_string(),
                                namespace: true,
                            },
                        );
                    }
                }
            }
            continue;
        }

        // Simple re-exports: `export { x } from './y'`
        // These appear as ExportNamedDeclaration with a source string and no
        // inline declaration.  We record each re-exported specifier as an
        // import so that `import_source_matches_owner` can resolve the
        // re-export chain one hop back.
        if let Statement::ExportNamedDeclaration(export) = stmt {
            if export.declaration.is_none()
                && let Some(re_source) = &export.source
            {
                let src = re_source.value.to_string();
                for specifier in &export.specifiers {
                    if specifier.export_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    let Some(local_name) = module_export_name_text(&specifier.local) else {
                        continue;
                    };
                    let exported_name = module_export_name_text(&specifier.exported)
                        .unwrap_or_else(|| local_name.clone());
                    push_unique_import(
                        &mut out,
                        TypeScriptImport {
                            source: src.clone(),
                            imported: Some(local_name),
                            local: exported_name,
                            namespace: false,
                        },
                    );
                }
            }
            continue;
        }

        // CommonJS require(): `const { x } = require('./y')` or
        // `const x = require('./y')`.
        // Only handles the simple synchronous require() form with a string
        // literal path.  Dynamic require / factory-returning requires are
        // fail-closed (not extracted → no import match → caller emits
        // `typescript_target_unresolved` when ownership cannot resolve).
        if let Statement::VariableDeclaration(var_decl) = stmt {
            for declarator in &var_decl.declarations {
                let Some(init) = &declarator.init else {
                    continue;
                };
                let source_str = require_string_literal_source(init);
                let Some(source_str) = source_str else {
                    continue;
                };
                // Simple binding: `const x = require('./y')`
                if let Some(name) = binding_identifier_name(&declarator.id) {
                    push_unique_import(
                        &mut out,
                        TypeScriptImport {
                            source: source_str.clone(),
                            imported: Some("default".to_string()),
                            local: name.to_string(),
                            namespace: true, // namespace-like: callers access via x.method
                        },
                    );
                    continue;
                }
                // Destructured binding: `const { x, y: z } = require('./y')`
                if let BindingPattern::ObjectPattern(obj) = &declarator.id {
                    for prop in &obj.properties {
                        let Some(key_name) = object_binding_key_name(prop) else {
                            continue;
                        };
                        let local_name = binding_identifier_name(&prop.value)
                            .unwrap_or(key_name)
                            .to_string();
                        push_unique_import(
                            &mut out,
                            TypeScriptImport {
                                source: source_str.clone(),
                                imported: Some(key_name.to_string()),
                                local: local_name,
                                namespace: false,
                            },
                        );
                    }
                }
            }
        }
    }
    out
}

/// Extract the string literal source from a `require('...')` call expression.
///
/// Returns `Some(source)` only for the simple form `require("./literal")`.
/// Dynamic arguments (variables, template literals, concatenations) return
/// `None` so the caller can fail closed.
pub(crate) fn require_string_literal_source(expr: &Expression<'_>) -> Option<String> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let Expression::Identifier(callee) = &call.callee else {
        return None;
    };
    if callee.name.as_str() != "require" {
        return None;
    }
    let first_arg = call.arguments.first()?;
    let oxc_ast::ast::Argument::StringLiteral(literal) = first_arg else {
        return None;
    };
    Some(literal.value.to_string())
}

/// Extract the key name from an object-pattern binding property.
///
/// Handles `{ x }` (shorthand) and `{ x: y }` (renamed) but not computed
/// keys (`{ [expr]: y }`).
pub(crate) fn object_binding_key_name<'a>(
    prop: &'a oxc_ast::ast::BindingProperty<'a>,
) -> Option<&'a str> {
    if prop.computed {
        return None;
    }
    match &prop.key {
        PropertyKey::StaticIdentifier(ident) => Some(ident.name.as_str()),
        PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
        _ => None,
    }
}

pub(crate) fn push_unique_import(out: &mut Vec<TypeScriptImport>, import: TypeScriptImport) {
    if !out.iter().any(|existing| existing == &import) {
        out.push(import);
    }
}

pub(crate) fn push_unique_string(out: &mut Vec<String>, value: String) {
    if !out.iter().any(|existing| existing == &value) {
        out.push(value);
    }
}

pub(crate) fn module_export_name_text(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(ident) => Some(ident.name.as_str().to_string()),
        ModuleExportName::IdentifierReference(ident) => Some(ident.name.as_str().to_string()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.to_string()),
    }
}

// ── Owner-extraction gap detection (#4104-A) ─────────────────────────────────

/// Detect the first changed owner-shaped construct the syntax-first owner
/// extractor does not index (#4104-A).
///
/// Real producer for the `typescript_owner_extraction_partial` named
/// limitation (classification-neutral additive disclosure, mirroring
/// `typescript_test_extraction_partial`): the owner universe still contains
/// exactly what `extract_owners` extracts; this function only reports that
/// it is PARTIAL for the changed lines, so a silent zero-finding result
/// (`analysis_complete: true`, no findings, no other limitations) is known
/// to mean "the changed line sits in an unsupported owner shape", not "the
/// changed behavior has no owning seam".
///
/// Detected shapes (bounded preview slice — extracting these shapes is a
/// separate backlog item; this lane only discloses them):
///
/// - `class C { f = (a) => {...} }` — arrow-function class fields.
/// - `class C { #adjust(a) {...} }` (also string/numeric/computed keys) —
///   class methods whose key the extractor cannot name.
/// - `class C { static {...} }` — class static blocks.
/// - `class C { accessor level = 1 }` — TC39 auto-accessors.
/// - `enum PlanRate { Basic = 9 }` — TS enum member value changes.
/// - `namespace Pricing { export function tax ... }` — TS module/namespace
///   declarations.
///
/// Returns `None` when the file fails to parse (parse-error disclosure owns
/// that case), when no changed line is supplied, or when every changed line
/// falls inside supported owner shapes (the negative-control contract).
pub(crate) fn detect_owner_extraction_gap(
    file: &Path,
    source: &str,
    changed_lines: &[usize],
) -> Option<TypeScriptOwnerExtractionGap> {
    if changed_lines.is_empty() {
        return None;
    }
    let changed: std::collections::HashSet<usize> = changed_lines.iter().copied().collect();
    let Ok(gap) = parse_on_worker(file, source, move |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file)).parse();
        if !ret.errors.is_empty() {
            // Parse-error disclosure owns this case; do not double-report.
            return None;
        }
        find_owner_extraction_gap(&ret.program.body, &SourceText::new(source), &changed)
    }) else {
        return None;
    };
    gap.map(|(sample_line, shape, span)| TypeScriptOwnerExtractionGap {
        file: file.to_path_buf(),
        sample_line,
        shape,
        snippet: snippet_for_span(source, span.0, span.1),
    })
}

/// 1-based inclusive line ranges of top-level ambient declarations
/// (`declare function`, `export declare const`, `declare module 'x' { ... }`,
/// `declare global { ... }`), read from the syntax tree. They are type-only
/// and erased at compile time, so a changed line inside one has no runtime
/// behavior to probe. A file that does not parse yields no ranges, so its
/// lines stay probed.
pub(crate) fn ambient_declaration_lines(file: &Path, source: &str) -> Vec<(usize, usize)> {
    parse_on_worker(file, source, |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file)).parse();
        if !ret.errors.is_empty() {
            return Vec::new();
        }
        let source = SourceText::new(source);
        ret.program
            .body
            .iter()
            .filter(|stmt| match stmt {
                Statement::ExportNamedDeclaration(export) => export
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.declare()),
                _ => stmt
                    .as_declaration()
                    .is_some_and(|declaration| declaration.declare()),
            })
            .map(|stmt| {
                let span = stmt.span();
                (
                    source.line_for_offset(span.start as usize),
                    source.line_for_offset(span.end as usize),
                )
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Walk top-level statements for unsupported owner shapes intersecting a
/// changed line. Returns `(sample_line, shape, (span_start, span_end))`.
fn find_owner_extraction_gap(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &SourceText<'_>,
    changed: &std::collections::HashSet<usize>,
) -> Option<(usize, &'static str, (usize, usize))> {
    for stmt in statements {
        match stmt {
            Statement::TSEnumDeclaration(decl) => {
                if let Some(hit) = span_hits_changed_line(decl.span, source, changed) {
                    return Some((
                        hit,
                        "enum declaration",
                        (decl.span.start as usize, decl.span.end as usize),
                    ));
                }
            }
            Statement::TSModuleDeclaration(decl) => {
                if let Some(hit) = span_hits_changed_line(decl.span, source, changed) {
                    return Some((
                        hit,
                        "module/namespace declaration",
                        (decl.span.start as usize, decl.span.end as usize),
                    ));
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(gap) = unsupported_class_element_gap(class, source, changed) {
                    return Some(gap);
                }
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::TSEnumDeclaration(decl)) => {
                    if let Some(hit) = span_hits_changed_line(decl.span, source, changed) {
                        return Some((
                            hit,
                            "enum declaration",
                            (decl.span.start as usize, decl.span.end as usize),
                        ));
                    }
                }
                Some(Declaration::TSModuleDeclaration(decl)) => {
                    if let Some(hit) = span_hits_changed_line(decl.span, source, changed) {
                        return Some((
                            hit,
                            "module/namespace declaration",
                            (decl.span.start as usize, decl.span.end as usize),
                        ));
                    }
                }
                Some(Declaration::ClassDeclaration(class)) => {
                    if let Some(gap) = unsupported_class_element_gap(class, source, changed) {
                        return Some(gap);
                    }
                }
                _ => {}
            },
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration
                    && let Some(gap) = unsupported_class_element_gap(class, source, changed)
                {
                    return Some(gap);
                }
            }
            _ => {}
        }
    }
    None
}

/// Scan class body elements for the unsupported member shapes the extractor
/// skips, returning the first one that intersects a changed line.
fn unsupported_class_element_gap(
    class: &Class<'_>,
    source: &SourceText<'_>,
    changed: &std::collections::HashSet<usize>,
) -> Option<(usize, &'static str, (usize, usize))> {
    for element in &class.body.body {
        let (shape, span) = match element {
            ClassElement::PropertyDefinition(prop) => {
                // Only function-valued fields are owner-shaped; a plain data
                // field change is not an extracted-owner regression.
                let is_function_init = matches!(
                    prop.value.as_ref(),
                    Some(Expression::ArrowFunctionExpression(_))
                        | Some(Expression::FunctionExpression(_))
                );
                if !is_function_init {
                    continue;
                }
                ("arrow-function class field", prop.span)
            }
            ClassElement::StaticBlock(block) => ("class static block", block.span),
            ClassElement::AccessorProperty(accessor) => ("accessor auto-accessor", accessor.span),
            ClassElement::MethodDefinition(method) => {
                // `#private`, string-literal, numeric, and computed keys are
                // all skipped by `property_key_name`; changed lines inside
                // them are silent today (#4104-A F2b).
                if property_key_name(&method.key).is_some() {
                    continue;
                }
                ("class method with unsupported key", method.span)
            }
            _ => continue,
        };
        if let Some(hit) = span_hits_changed_line(span, source, changed) {
            return Some((hit, shape, (span.start as usize, span.end as usize)));
        }
    }
    None
}

/// First changed line inside `span`'s line range, if any.
fn span_hits_changed_line(
    span: impl GetSpan,
    source: &SourceText<'_>,
    changed: &std::collections::HashSet<usize>,
) -> Option<usize> {
    let span = span.span();
    let start_line = source.line_for_offset(span.start as usize);
    let end_line = source.line_for_offset(span.end as usize);
    (start_line..=end_line).find(|line| changed.contains(line))
}
