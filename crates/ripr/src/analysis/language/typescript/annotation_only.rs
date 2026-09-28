//! Annotation-only signature change detection for the TypeScript preview
//! adapter (#4282).
//!
//! TypeScript erases type syntax, so a changed line whose old and new forms
//! differ only in type annotations (return type, parameter annotations,
//! optional `?` markers, generic parameter lists, a `this` parameter, a
//! variable annotation) has no runtime behavior and yields no probe. This is
//! the TypeScript counterpart of the Python adapter's
//! `is_annotation_only_def_change` (#1289).
//!
//! Each side is parsed with oxc as a one-line snippet and reduced to a
//! runtime skeleton: everything the emitted JavaScript keeps, and nothing
//! TypeScript erases. Equal skeletons mean an annotation-only change. The
//! check fails closed: a line that does not parse as a supported shape on
//! either side, or whose runtime text differs anywhere (a default value, a
//! parameter name, a body, an `as` cast inside an expression), keeps its
//! probe.

use super::*;
use oxc_ast::ast::MethodDefinitionKind;

/// Whether a paired removed/added line change touches ONLY TypeScript type
/// syntax. Fails closed (returns `false`) on identical lines, on lines that do
/// not parse as a function/method signature or a variable declaration, on any
/// runtime difference, and on a method line when `file_has_decorators` (its
/// decorator may sit on a line above).
pub(crate) fn is_annotation_only_signature_change(
    file: &Path,
    old_line: &str,
    new_line: &str,
    file_has_decorators: bool,
) -> bool {
    // Decorators can turn erased types into runtime values: with
    // `emitDecoratorMetadata`, a decorated class's parameter and return types
    // become `design:paramtypes` / `design:returntype` metadata that DI
    // frameworks read. A line carrying `@` fails closed.
    if old_line.trim() == new_line.trim() || old_line.contains('@') || new_line.contains('@') {
        return false;
    }
    match (
        runtime_skeleton(file, old_line),
        runtime_skeleton(file, new_line),
    ) {
        (Some(RuntimeSkeleton::Method { .. }), _) if file_has_decorators => false,
        (Some(old), Some(new)) => old == new,
        _ => false,
    }
}

/// The runtime-significant shape of one changed line. Two lines with equal
/// skeletons emit the same JavaScript.
#[derive(Debug, PartialEq, Eq)]
enum RuntimeSkeleton {
    Function {
        /// `export` / `export default` / none: part of the runtime module shape.
        export: &'static str,
        function: FunctionSkeleton,
    },
    Method {
        key: String,
        kind: String,
        computed: bool,
        is_static: bool,
        function: FunctionSkeleton,
    },
    Variables {
        export: &'static str,
        kind: String,
        declarators: Vec<(String, Option<InitSkeleton>)>,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum InitSkeleton {
    Function(FunctionSkeleton),
    Arrow {
        is_async: bool,
        params: Vec<ParamSkeleton>,
        rest: Option<String>,
        body: String,
    },
    /// Any other initializer is compared by its exact source text, so type
    /// syntax inside an expression (`as`, `satisfies`, generic arguments)
    /// fails closed.
    Expression(String),
}

#[derive(Debug, PartialEq, Eq)]
struct FunctionSkeleton {
    name: Option<String>,
    is_async: bool,
    generator: bool,
    params: Vec<ParamSkeleton>,
    rest: Option<String>,
    /// `None` for an overload or `declare`d signature, which emits nothing.
    body: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ParamSkeleton {
    pattern: String,
    initializer: Option<String>,
    /// Parameter-property modifiers (`private x`, `readonly x`) emit a field
    /// assignment in the constructor, so they are runtime-significant.
    accessibility: Option<String>,
    readonly: bool,
    is_override: bool,
}

/// Name of the synthetic class that wraps a method line.
const PROBE_CLASS: &str = "RiprAnnotationProbe";

/// Snippet wrappers tried in order. A signature line that opens a body
/// (`function f(a: T): R {`) needs a closing brace to parse; a method
/// signature needs an enclosing class. The first wrapper that parses to one
/// supported statement wins; the skeleton records the shape it parsed as, so
/// a function line never matches a method line.
fn snippets(line: &str) -> [String; 4] {
    [
        line.to_string(),
        format!("{line}\n}}"),
        format!("class {PROBE_CLASS} {{\n{line}\n}}"),
        format!("class {PROBE_CLASS} {{\n{line}\n}}\n}}"),
    ]
}

fn runtime_skeleton(file: &Path, line: &str) -> Option<RuntimeSkeleton> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidates = snippets(trimmed);
    parse_on_worker(file, trimmed, move |file, _line, allocator| {
        candidates.iter().enumerate().find_map(|(index, snippet)| {
            let ret = Parser::new(allocator, snippet, source_type_for(file)).parse();
            if !ret.errors.is_empty() || ret.program.body.len() != 1 {
                return None;
            }
            // Wrappers 2 and 3 are the synthetic class; only they may yield a
            // method, and only they may yield a class at all (a one-line real
            // class declaration is not a signature).
            let wrapped = index >= 2;
            match &ret.program.body[0] {
                Statement::ClassDeclaration(class) if wrapped => method_skeleton(class, snippet),
                statement if !wrapped => statement_skeleton(statement, snippet),
                _ => None,
            }
        })
    })
    .ok()
    .flatten()
}

fn statement_skeleton(statement: &Statement<'_>, src: &str) -> Option<RuntimeSkeleton> {
    match statement {
        Statement::FunctionDeclaration(function) => Some(RuntimeSkeleton::Function {
            export: "",
            function: function_skeleton(function, src),
        }),
        Statement::VariableDeclaration(declaration) => variables_skeleton("", declaration, src),
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref()? {
            Declaration::FunctionDeclaration(function) => Some(RuntimeSkeleton::Function {
                export: "export",
                function: function_skeleton(function, src),
            }),
            Declaration::VariableDeclaration(declaration) => {
                variables_skeleton("export", declaration, src)
            }
            _ => None,
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                Some(RuntimeSkeleton::Function {
                    export: "export default",
                    function: function_skeleton(function, src),
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// The single method of the synthetic wrapper class. A constructor fails
/// closed: its parameter types feed decorator metadata on a decorated class,
/// which this one-line view cannot see.
fn method_skeleton(class: &Class<'_>, src: &str) -> Option<RuntimeSkeleton> {
    if class.id.as_ref().is_none_or(|id| id.name != PROBE_CLASS) {
        return None;
    }
    let [ClassElement::MethodDefinition(method)] = class.body.body.as_slice() else {
        return None;
    };
    if method.kind == MethodDefinitionKind::Constructor {
        return None;
    }
    Some(RuntimeSkeleton::Method {
        key: text(src, method.key.span()),
        kind: format!("{:?}", method.kind),
        computed: method.computed,
        is_static: method.r#static,
        function: function_skeleton(&method.value, src),
    })
}

fn variables_skeleton(
    export: &'static str,
    declaration: &VariableDeclaration<'_>,
    src: &str,
) -> Option<RuntimeSkeleton> {
    if declaration.declare {
        return None;
    }
    let declarators = declaration
        .declarations
        .iter()
        .map(|declarator| {
            (
                text(src, declarator.id.span()),
                declarator
                    .init
                    .as_ref()
                    .map(|init| init_skeleton(init, src)),
            )
        })
        .collect();
    Some(RuntimeSkeleton::Variables {
        export,
        kind: format!("{:?}", declaration.kind),
        declarators,
    })
}

fn init_skeleton(init: &Expression<'_>, src: &str) -> InitSkeleton {
    match init {
        Expression::FunctionExpression(function) => {
            InitSkeleton::Function(function_skeleton(function, src))
        }
        Expression::ArrowFunctionExpression(arrow) => {
            let (params, rest) = params_skeleton(&arrow.params, src);
            InitSkeleton::Arrow {
                is_async: arrow.r#async,
                params,
                rest,
                body: text(src, arrow.body.span),
            }
        }
        other => InitSkeleton::Expression(text(src, other.span())),
    }
}

fn function_skeleton(function: &Function<'_>, src: &str) -> FunctionSkeleton {
    let (params, rest) = params_skeleton(&function.params, src);
    FunctionSkeleton {
        name: function.id.as_ref().map(|id| id.name.to_string()),
        is_async: function.r#async,
        generator: function.generator,
        params,
        rest,
        body: function.body.as_ref().map(|body| text(src, body.span)),
    }
}

fn params_skeleton(
    params: &FormalParameters<'_>,
    src: &str,
) -> (Vec<ParamSkeleton>, Option<String>) {
    let items = params
        .items
        .iter()
        .map(|param| ParamSkeleton {
            // The binding pattern's span excludes the parameter's own type
            // annotation and `?` marker, which live on `FormalParameter`.
            pattern: text(src, param.pattern.span()),
            initializer: param
                .initializer
                .as_ref()
                .map(|init| text(src, init.span())),
            accessibility: param
                .accessibility
                .map(|accessibility| format!("{accessibility:?}")),
            readonly: param.readonly,
            is_override: param.r#override,
        })
        .collect();
    let rest = params
        .rest
        .as_ref()
        .map(|rest| text(src, rest.rest.argument.span()));
    (items, rest)
}

fn text(src: &str, span: oxc_span::Span) -> String {
    src.get(span.start as usize..span.end as usize)
        .unwrap_or_default()
        .to_string()
}

/// Whether `line` only OPENS a function, method, or arrow declaration: it
/// parses as a signature whose body is an empty block once the synthetic
/// closing brace is supplied, and it carries no runtime-significant signature
/// text (no default value, no destructuring default, no parameter property,
/// no computed key, no decorator). Such a line has no behavior of its own;
/// the behavior lives on the body lines. Fails closed on every other shape,
/// including a one-line arrow or function with an inline body.
pub(crate) fn is_signature_opening_line(file: &Path, line: &str) -> bool {
    if line.contains('@') {
        return false;
    }
    match runtime_skeleton(file, line) {
        Some(RuntimeSkeleton::Function { function, .. }) => function_only_opens(&function),
        Some(RuntimeSkeleton::Method {
            computed, function, ..
        }) => !computed && function_only_opens(&function),
        Some(RuntimeSkeleton::Variables { declarators, .. }) => match declarators.as_slice() {
            [(_, Some(InitSkeleton::Function(function)))] => function_only_opens(function),
            [
                (
                    _,
                    Some(InitSkeleton::Arrow {
                        params, rest, body, ..
                    }),
                ),
            ] => rest_is_plain(rest.as_deref()) && params_are_plain(params) && is_empty_block(body),
            _ => false,
        },
        None => false,
    }
}

fn function_only_opens(function: &FunctionSkeleton) -> bool {
    rest_is_plain(function.rest.as_deref())
        && params_are_plain(&function.params)
        && function.body.as_deref().is_some_and(is_empty_block)
}

fn params_are_plain(params: &[ParamSkeleton]) -> bool {
    params.iter().all(|param| {
        param.initializer.is_none()
            && param.accessibility.is_none()
            && !param.readonly
            && !param.is_override
            && !param.pattern.contains('=')
    })
}

fn rest_is_plain(rest: Option<&str>) -> bool {
    rest.is_none_or(|rest| !rest.contains('='))
}

fn is_empty_block(body: &str) -> bool {
    let compact: String = body.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact == "{}"
}

/// Whether an unpaired added `line` opens a NEW function, method, or arrow
/// owner whose own span carries at least one other probe-candidate added line
/// (`has_added_probe_line`). Owner kinds without a callable body (module
/// initializers) and constructors (parameter properties and decorator
/// metadata) fail closed, as does any line [`is_signature_opening_line`]
/// rejects.
/// Whether `text` holds `name` as a whole JS identifier.
fn mentions_identifier(text: &str, name: &str) -> bool {
    let is_ident = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
    !name.is_empty()
        && text.match_indices(name).any(|(start, _)| {
            let end = start + name.len();
            (start == 0 || !is_ident(text.as_bytes()[start - 1]))
                && !text.as_bytes().get(end).is_some_and(|byte| is_ident(*byte))
        })
}

pub(crate) fn is_new_owner_opening_line(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[TypeScriptOwner],
    removed_texts: &[&str],
    has_added_probe_line: impl Fn(usize) -> bool,
) -> bool {
    let changed_file = normalized_path(file);
    let opens_owner_with_added_body = owners.iter().any(|owner| {
        owner.start_line == line
            && owner.end_line > line
            && normalized_path(&owner.file) == changed_file
            && !matches!(owner.owner_kind, OwnerKind::ModuleFunction)
            && owner.method_kind != TypeScriptMethodKind::Constructor
            && (line + 1..=owner.end_line).any(&has_added_probe_line)
            // Git can pair a changed signature's removed line with an
            // unrelated inserted line; an old line naming the owner means it
            // existed before, so its signature change keeps its probe.
            && !removed_texts
                .iter()
                .any(|removed| mentions_identifier(removed, &owner.name))
    });
    opens_owner_with_added_body && is_signature_opening_line(file, line_text)
}
