//! Owner extraction for the TypeScript preview adapter.

use super::*;

pub(crate) fn extract_owners(file: &Path, source: &str) -> Vec<TypeScriptOwner> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if !ret.errors.is_empty() {
        return Vec::new();
    }
    let imports = extract_imports_from_statements(&ret.program.body);
    let mut owners = Vec::new();
    for stmt in &ret.program.body {
        owners.extend(owners_from_statement(stmt, file, source, &imports));
    }
    owners
}

pub(crate) fn owners_from_statement(
    stmt: &Statement<'_>,
    file: &Path,
    source: &str,
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
    owners_from_statement_declaration(stmt, file, source, imports)
}

pub(crate) fn owners_from_statement_declaration(
    stmt: &Statement<'_>,
    file: &Path,
    source: &str,
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
    source: &str,
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
    source: &str,
    imports: &[TypeScriptImport],
) -> Vec<TypeScriptOwner> {
    match decl {
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
    }
}

pub(crate) fn owners_from_variable_declaration(
    decl: &VariableDeclaration<'_>,
    file: &Path,
    source: &str,
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
    source: &str,
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
            start_line: line_for_offset(source, declarator.span.start as usize),
            end_line: line_for_offset(source, declarator.span.end as usize),
            owner_kind: OwnerKind::ModuleFunction,
            class_name: None,
            decorated: false,
            imports: imports.to_vec(),
        }),
    }
}

pub(crate) fn owner_from_function(
    file: &Path,
    source: &str,
    name: &str,
    func: &Function<'_>,
    owner_kind: OwnerKind,
    decorated: bool,
    imports: &[TypeScriptImport],
) -> TypeScriptOwner {
    TypeScriptOwner {
        name: name.to_string(),
        file: file.to_path_buf(),
        start_line: line_for_offset(source, func.span.start as usize),
        end_line: line_for_offset(source, func.span.end as usize),
        owner_kind,
        class_name: None,
        decorated,
        imports: imports.to_vec(),
    }
}

pub(crate) fn owner_from_arrow(
    file: &Path,
    source: &str,
    name: &str,
    arrow: &ArrowFunctionExpression<'_>,
    owner_start: u32,
    decorated: bool,
    imports: &[TypeScriptImport],
) -> TypeScriptOwner {
    TypeScriptOwner {
        name: name.to_string(),
        file: file.to_path_buf(),
        start_line: line_for_offset(source, owner_start as usize),
        end_line: line_for_offset(source, arrow.span.end as usize),
        owner_kind: arrow_owner_kind(file, source, name, arrow.span.start, arrow.span.end),
        class_name: None,
        decorated,
        imports: imports.to_vec(),
    }
}

pub(crate) fn owners_from_class(
    class: &Class<'_>,
    file: &Path,
    source: &str,
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
    source: &str,
    class_decorated: bool,
    class_name: Option<&str>,
    imports: &[TypeScriptImport],
) -> Option<TypeScriptOwner> {
    if method.computed {
        return None;
    }
    let name = property_key_name(&method.key)?;
    Some(TypeScriptOwner {
        name,
        file: file.to_path_buf(),
        start_line: line_for_offset(source, method.span.start as usize),
        end_line: line_for_offset(source, method.span.end as usize),
        owner_kind: if method.r#static {
            OwnerKind::ClassMethod
        } else {
            OwnerKind::Method
        },
        class_name: class_name.map(str::to_string),
        decorated: class_decorated || !method.decorators.is_empty(),
        imports: imports.to_vec(),
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
        let Statement::ImportDeclaration(import) = stmt else {
            continue;
        };
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
    }
    out
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
