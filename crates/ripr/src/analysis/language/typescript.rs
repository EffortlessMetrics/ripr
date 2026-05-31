//! TypeScript preview adapter.
//!
//! See `docs/specs/RIPR-SPEC-0027-typescript-preview-static-facts.md` and
//! `docs/adr/0008-typescript-parser-substrate.md`.
//!
//! This adapter extracts syntax-first owner, Jest/Vitest test/assertion,
//! related-test, probe-shape, and static-limit facts from TypeScript /
//! JavaScript files, then emits one preview-tagged `Finding` per changed
//! production line that falls within an owner.
//!
//! The adapter remains a preview evidence surface: it does not invoke `tsc`,
//! `tsserver`, package graph resolution, Jest/Vitest runtime execution,
//! providers, generated tests, or source edits. Heuristic links stay
//! uncertainty-only, static limits fail closed, and incomplete repair packets
//! remain advisory.

use super::super::{AnalysisOptions, diff::ChangedFile, probes};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding,
    LanguageId as DomainLanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind,
    OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
    RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind, StopReason, SymbolId,
};
use crate::domain::{FlowSinkFact, FlowSinkKind};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, BindingPattern, Class, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, Function, ImportDeclarationSpecifier,
    ImportOrExportKind, MethodDefinition, ModuleExportName, ObjectPropertyKind, PropertyKey,
    Statement, VariableDeclaration, VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::path::{Path, PathBuf};

/// TypeScript / JavaScript preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeScriptAdapter;

fn source_type_for(path: &Path) -> SourceType {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("tsx") => SourceType::tsx(),
        Some("ts") => SourceType::ts(),
        Some("jsx") => SourceType::jsx(),
        Some("js") => SourceType::mjs(),
        _ => SourceType::mjs(),
    }
}

/// Owner extracted from a TypeScript / JavaScript source file.
///
/// Covers the syntax-first owner kinds accepted for the preview surface.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptOwner {
    name: String,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    owner_kind: OwnerKind,
    class_name: Option<String>,
    decorated: bool,
    imports: Vec<TypeScriptImport>,
}

impl TypeScriptOwner {
    fn symbol_id(&self) -> SymbolId {
        SymbolId(format!(
            "{}:{}::{}",
            output_language_for(&self.file).as_str(),
            normalized_path(&self.file),
            self.name
        ))
    }
}

/// Test block extracted from a TypeScript / JavaScript test file.
///
/// Covers syntax-first Jest/Vitest `test('name', fn)`, `it('name', fn)`,
/// and array-form `.each(...)('name', fn)` expression statements, including
/// nested `describe(...)` blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptTest {
    /// Qualified display name. Nested `describe(...)` names are joined with the
    /// local `test(...)` / `it(...)` name so user surfaces can show context.
    name: String,
    /// The local `test(...)` / `it(...)` string before describe qualification.
    local_name: String,
    /// Nested describe names in outer-to-inner order.
    describe_names: Vec<String>,
    file: PathBuf,
    line: usize,
    body_text: String,
    assertions: Vec<TypeScriptAssertion>,
    /// Module paths referenced by syntactic `vi.mock("...")` /
    /// `jest.mock("...")` calls discovered at the top level of the
    /// containing test file. Populated once per file and cloned into
    /// every `TypeScriptTest` parsed from that file so the classifier
    /// can surface the `mocked_module` static-limit without re-parsing.
    /// Empty when no syntactic mock indirection is present.
    mocks_in_file: Vec<String>,
    /// Runtime imports discovered at the top level of the containing test
    /// file. Used only to map relative named or namespace imports back to a
    /// source owner before considering alias calls related.
    imports_in_file: Vec<TypeScriptImport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptImport {
    source: String,
    imported: Option<String>,
    local: String,
    namespace: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptParseLimit {
    file: PathBuf,
    reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeScriptRelationKind {
    DirectOwnerCall,
    ImportedOwnerCall,
    ModuleValueReference,
    ReceiverOwnerCall,
    SameFileProximity,
    DescribeName,
    TestName,
}

impl TypeScriptRelationKind {
    fn rank(self) -> u8 {
        match self {
            Self::DirectOwnerCall => 5,
            Self::ImportedOwnerCall => 4,
            Self::ModuleValueReference => 4,
            Self::ReceiverOwnerCall => 4,
            Self::SameFileProximity => 3,
            Self::DescribeName => 2,
            Self::TestName => 1,
        }
    }

    fn uses_oracle(self) -> bool {
        matches!(
            self,
            Self::DirectOwnerCall
                | Self::ImportedOwnerCall
                | Self::ModuleValueReference
                | Self::ReceiverOwnerCall
        )
    }

    fn is_uncertain(self) -> bool {
        !self.uses_oracle()
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::DirectOwnerCall => "direct_owner_call",
            Self::ImportedOwnerCall => "imported_owner_call",
            Self::ModuleValueReference => "module_value_reference",
            Self::ReceiverOwnerCall => "receiver_owner_call",
            Self::SameFileProximity => "same_file_proximity",
            Self::DescribeName => "describe_name",
            Self::TestName => "test_name",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TypeScriptRelatedCandidate<'a> {
    test: &'a TypeScriptTest,
    relation: TypeScriptRelationKind,
}

/// Assertion shape extracted from a single `expect(actual).matcher(...)`
/// chain inside a test body.
///
/// `matcher` is the canonical matcher name (`toBe`, `toEqual`, `toThrow`,
/// `toMatchSnapshot`, `toHaveBeenCalledWith`, ...). The full Jest/Vitest
/// matcher surface is large; this preview slice maps the most common
/// matchers to oracle vocabulary and tags the rest as `Unknown`.
/// Async-aware (`.resolves` / `.rejects`) chains are recognised by syntax;
/// custom matchers stay `Unknown`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptAssertion {
    matcher: String,
    argument_count: usize,
    line: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
    mock_payload: Option<TypeScriptMockPayload>,
    error_payload: Option<TypeScriptErrorPayload>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptMockPayload {
    target: String,
    expected: String,
    kind: TypeScriptMockPayloadKind,
}

impl TypeScriptMockPayload {
    fn oracle_text(&self) -> String {
        match self.kind {
            TypeScriptMockPayloadKind::CalledWith => {
                format!(
                    "expect({}).toHaveBeenCalledWith({})",
                    self.target, self.expected
                )
            }
            TypeScriptMockPayloadKind::CalledTimes => {
                format!(
                    "expect({}).toHaveBeenCalledTimes({})",
                    self.target, self.expected
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeScriptMockPayloadKind {
    CalledWith,
    CalledTimes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptErrorPayload {
    expected: String,
    kind: TypeScriptErrorPayloadKind,
}

impl TypeScriptErrorPayload {
    fn oracle_text(&self) -> String {
        match self.kind {
            TypeScriptErrorPayloadKind::ThrowsLiteral => {
                format!("expect(...).toThrow({})", self.expected)
            }
            TypeScriptErrorPayloadKind::RejectsThrowLiteral => {
                format!("await expect(...).rejects.toThrow({})", self.expected)
            }
            TypeScriptErrorPayloadKind::RejectsMatchObject => {
                format!("await expect(...).rejects.toMatchObject({})", self.expected)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeScriptErrorPayloadKind {
    ThrowsLiteral,
    RejectsThrowLiteral,
    RejectsMatchObject,
}

fn oracle_for_matcher(matcher: &str) -> (OracleKind, OracleStrength) {
    match matcher {
        "toBe" | "toEqual" | "toStrictEqual" => (OracleKind::ExactValue, OracleStrength::Strong),
        "toThrow" | "toThrowError" => (OracleKind::BroadError, OracleStrength::Weak),
        "toMatchSnapshot" | "toMatchInlineSnapshot" => {
            (OracleKind::Snapshot, OracleStrength::Medium)
        }
        "toHaveBeenCalled"
        | "toHaveBeenCalledWith"
        | "toHaveBeenCalledTimes"
        | "toHaveBeenLastCalledWith"
        | "toHaveBeenNthCalledWith" => (OracleKind::MockExpectation, OracleStrength::Medium),
        "toBeTruthy" | "toBeFalsy" | "toBeDefined" | "toBeUndefined" | "toBeNull" | "toBeNaN" => {
            (OracleKind::SmokeOnly, OracleStrength::Smoke)
        }
        "toContain"
        | "toMatch"
        | "toBeGreaterThan"
        | "toBeGreaterThanOrEqual"
        | "toBeLessThan"
        | "toBeLessThanOrEqual"
        | "toHaveLength"
        | "toHaveProperty" => (OracleKind::RelationalCheck, OracleStrength::Weak),
        _ => (OracleKind::Unknown, OracleStrength::Unknown),
    }
}

fn weak_oracle_missing_summary(
    owner_name: &str,
    oracle_kind: &OracleKind,
    probe_family: &ProbeFamily,
    mock_payload_oracle: Option<&str>,
) -> String {
    match oracle_kind {
        OracleKind::Snapshot => format!(
            "Related test reaches `{owner_name}` with snapshot evidence; keep the snapshot as weak preview evidence and add an exact-value assertion for the changed discriminator before routing a repair packet."
        ),
        OracleKind::SmokeOnly => format!(
            "Related test reaches `{owner_name}` with a smoke-only oracle; replace or augment the truthiness check with an exact-value assertion for the changed discriminator before routing a repair packet."
        ),
        OracleKind::MockExpectation if matches!(probe_family, ProbeFamily::SideEffect) => {
            mock_payload_oracle.map_or_else(
                || format!(
                    "Related test reaches `{owner_name}` with a mock interaction oracle, but TypeScript preview does not yet establish the changed call payload; keep the item advisory until mock-shape actionability can name the callee, expected arguments, verify command, receipt command, and edit boundaries."
                ),
                |oracle| format!(
                    "Related test reaches `{owner_name}` with bounded mock payload evidence `{oracle}`; keep the item advisory until mock-shape actionability can name verify command, receipt command, evidence refs, and edit boundaries."
                ),
            )
        }
        OracleKind::BroadError => format!(
            "Related test reaches `{owner_name}` with broad error evidence; keep it weak until TypeScript preview can establish the thrown or rejected payload and emit a bounded error-path repair packet."
        ),
        _ => format!(
            "Related test reaches `{owner_name}` but the strongest extracted oracle is `{}`; upgrade by adding an exact-value (`toBe` / `toEqual` / `toStrictEqual`) assertion. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.",
            oracle_kind.as_str()
        ),
    }
}

fn weak_oracle_recommendation(
    oracle_kind: &OracleKind,
    discriminator: &str,
    mock_payload_oracle: Option<&str>,
) -> String {
    match oracle_kind {
        OracleKind::Snapshot => format!(
            "TypeScript preview advisory: add an exact-value assertion alongside the snapshot for missing discriminator `{discriminator}`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
        OracleKind::SmokeOnly => format!(
            "TypeScript preview advisory: replace or augment the smoke-only assertion with an exact-value assertion for missing discriminator `{discriminator}`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
        OracleKind::MockExpectation => mock_payload_oracle.map_or_else(
                || format!(
                    "TypeScript preview advisory: related mock interaction evidence is present, but mock payloads are not yet a safe discriminator for `{discriminator}`; no actionable repair packet is emitted until mock-shape support can name verify, receipt, evidence refs, and edit boundaries."
                ),
                |oracle| format!(
                    "TypeScript preview advisory: related mock payload evidence `{oracle}` is syntax-bounded for `{discriminator}`, but no actionable repair packet is emitted until verify, receipt, evidence refs, and edit boundaries are available."
                ),
        ),
        OracleKind::BroadError => format!(
            "TypeScript preview advisory: broad error evidence does not establish missing discriminator `{discriminator}`; no actionable repair packet is emitted until error payload/variant support can name verify, receipt, and edit-boundary fields."
        ),
        _ => format!(
            "TypeScript preview advisory: add or strengthen a focused assertion for missing discriminator `{discriminator}`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
    }
}

/// Whether a path is a test file by convention (`*.test.ts`, `*.spec.ts`,
/// and `.tsx` / `.js` / `.jsx` variants).
fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem_extensions: &[&str] = &[
        ".test.ts",
        ".test.tsx",
        ".test.js",
        ".test.jsx",
        ".spec.ts",
        ".spec.tsx",
        ".spec.js",
        ".spec.jsx",
    ];
    stem_extensions
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
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

fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

fn extract_owners(file: &Path, source: &str) -> Vec<TypeScriptOwner> {
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

fn parse_error_reason(file: &Path, source: &str) -> Option<String> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if ret.errors.is_empty() {
        None
    } else {
        Some(format!("{} parser error(s)", ret.errors.len()))
    }
}

fn owners_from_statement(
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

fn owners_from_statement_declaration(
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

fn owners_from_declaration(
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

fn owners_from_default_export(
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

fn owners_from_variable_declaration(
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

fn owner_from_variable_declarator(
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

fn owner_from_function(
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

fn owner_from_arrow(
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

fn owners_from_class(
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

fn owner_from_method(
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

fn binding_identifier_name<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

fn function_owner_kind(file: &Path, source: &str, name: &str, start: u32, end: u32) -> OwnerKind {
    if looks_like_component_owner(file, source, name, start, end) {
        OwnerKind::Component
    } else {
        OwnerKind::Function
    }
}

fn arrow_owner_kind(file: &Path, source: &str, name: &str, start: u32, end: u32) -> OwnerKind {
    if looks_like_component_owner(file, source, name, start, end) {
        OwnerKind::Component
    } else {
        OwnerKind::ArrowFunction
    }
}

fn looks_like_component_owner(file: &Path, source: &str, name: &str, start: u32, end: u32) -> bool {
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

fn starts_with_uppercase(name: &str) -> bool {
    name.chars().next().is_some_and(|ch| ch.is_uppercase())
}

fn contains_jsx_like_return(slice: &str) -> bool {
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

fn extract_tests(file: &Path, source: &str) -> Vec<TypeScriptTest> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if !ret.errors.is_empty() {
        return Vec::new();
    }
    let imports = extract_imports_from_statements(&ret.program.body);
    let mocks = extract_mocks_from_statements(&ret.program.body);
    let mut tests = Vec::new();
    collect_tests_from_statements(
        &ret.program.body,
        file,
        source,
        &mocks,
        &imports,
        &mut Vec::new(),
        &mut tests,
    );
    tests
}

fn extract_imports_from_statements(
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

fn push_unique_import(out: &mut Vec<TypeScriptImport>, import: TypeScriptImport) {
    if !out.iter().any(|existing| existing == &import) {
        out.push(import);
    }
}

fn push_unique_string(out: &mut Vec<String>, value: String) {
    if !out.iter().any(|existing| existing == &value) {
        out.push(value);
    }
}

fn module_export_name_text(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(ident) => Some(ident.name.as_str().to_string()),
        ModuleExportName::IdentifierReference(ident) => Some(ident.name.as_str().to_string()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.to_string()),
    }
}

/// Walk a list of top-level statements and collect every syntactic
/// `vi.mock("path")` / `jest.mock("path")` argument we see. The list is
/// deduplicated and used by the classifier to surface the
/// `mocked_module` static-limit per RIPR-SPEC-0026.
///
/// This is purely syntactic — the adapter does not resolve the mocked
/// module identifier through the project's import graph, so the limit
/// surfaces exactly when the test file contains the mock call shape.
fn extract_mocks_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for stmt in statements {
        let Statement::ExpressionStatement(expr_stmt) = stmt else {
            continue;
        };
        let Expression::CallExpression(call) = &expr_stmt.expression else {
            continue;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            continue;
        };
        let Expression::Identifier(object_ident) = &member.object else {
            continue;
        };
        let object_name = object_ident.name.as_str();
        if object_name != "vi" && object_name != "jest" {
            continue;
        }
        if member.property.name.as_str() != "mock" {
            continue;
        }
        let Some(first_arg) = call.arguments.first() else {
            continue;
        };
        let oxc_ast::ast::Argument::StringLiteral(literal) = first_arg else {
            continue;
        };
        let path = literal.value.to_string();
        if !out.iter().any(|existing| existing == &path) {
            out.push(path);
        }
    }
    out
}

fn collect_tests_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    file: &Path,
    source: &str,
    mocks: &[String],
    imports: &[TypeScriptImport],
    describe_stack: &mut Vec<String>,
    tests: &mut Vec<TypeScriptTest>,
) {
    for stmt in statements {
        if let Some((describe_name, body)) = describe_body_from_statement(stmt) {
            describe_stack.push(describe_name);
            collect_tests_from_statements(
                body,
                file,
                source,
                mocks,
                imports,
                describe_stack,
                tests,
            );
            describe_stack.pop();
            continue;
        }
        if let Some(mut test) = test_from_statement(stmt, file, source, describe_stack) {
            test.mocks_in_file = mocks.to_vec();
            test.imports_in_file = imports.to_vec();
            tests.push(test);
        }
    }
}

fn describe_body_from_statement<'a>(
    stmt: &'a Statement<'a>,
) -> Option<(String, &'a oxc_allocator::Vec<'a, Statement<'a>>)> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    let Expression::Identifier(ident) = &call.callee else {
        return None;
    };
    if ident.name.as_str() != "describe" {
        return None;
    }
    let name = string_argument(call.arguments.first()?)?;
    let body = function_body_statements_from_argument(call.arguments.get(1)?)?;
    Some((name, body))
}

fn test_from_statement(
    stmt: &Statement<'_>,
    file: &Path,
    source: &str,
    describe_stack: &[String],
) -> Option<TypeScriptTest> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    let (name, assertions) = test_name_and_assertions_from_call(call, source)?;
    Some(TypeScriptTest {
        name: qualified_test_name(describe_stack, &name),
        local_name: name,
        describe_names: describe_stack.to_vec(),
        file: file.to_path_buf(),
        line: line_for_offset(source, call.span.start as usize),
        body_text: source[call.span.start as usize..call.span.end as usize].to_string(),
        assertions,
        // Populated by `extract_tests` (the only public extractor) once
        // per file before the test is returned to the caller.
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    })
}

fn test_name_and_assertions_from_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> Option<(String, Vec<TypeScriptAssertion>)> {
    if test_callee_is_identifier(call) {
        let name = string_argument(call.arguments.first()?)?;
        let assertions = function_body_statements_from_argument(call.arguments.get(1)?)
            .map(|statements| collect_expect_assertions_in_statements(statements, source))
            .unwrap_or_default();
        return Some((name, assertions));
    }

    if test_callee_is_each(call) {
        let name = string_argument(call.arguments.first()?)?;
        let assertions = function_body_statements_from_argument(call.arguments.get(1)?)
            .map(|statements| collect_expect_assertions_in_statements(statements, source))
            .unwrap_or_default();
        return Some((name, assertions));
    }

    None
}

fn test_callee_is_identifier(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let Expression::Identifier(ident) = &call.callee else {
        return false;
    };
    matches!(ident.name.as_str(), "test" | "it")
}

fn test_callee_is_each(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let Expression::CallExpression(each_call) = &call.callee else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &each_call.callee else {
        return false;
    };
    if member.property.name.as_str() != "each" {
        return false;
    }
    let Expression::Identifier(ident) = &member.object else {
        return false;
    };
    matches!(ident.name.as_str(), "test" | "it")
}

fn string_argument(arg: &oxc_ast::ast::Argument<'_>) -> Option<String> {
    match arg {
        oxc_ast::ast::Argument::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn function_body_statements_from_argument<'a>(
    arg: &'a oxc_ast::ast::Argument<'a>,
) -> Option<&'a oxc_allocator::Vec<'a, Statement<'a>>> {
    match arg {
        oxc_ast::ast::Argument::ArrowFunctionExpression(arrow) => Some(&arrow.body.statements),
        oxc_ast::ast::Argument::FunctionExpression(func) => {
            func.body.as_ref().map(|body| &body.statements)
        }
        _ => None,
    }
}

fn qualified_test_name(describe_stack: &[String], name: &str) -> String {
    if describe_stack.is_empty() {
        return name.to_string();
    }
    let mut parts = describe_stack.to_vec();
    parts.push(name.to_string());
    parts.join(" ")
}

/// Walk a list of statements (e.g., a function body) and collect every
/// `expect(actual).matcher(...)` expression statement we recognise. Test
/// discriminators are often guarded by setup branches or cleanup blocks, so
/// this recurses through common control-flow bodies while still staying
/// syntax-only and conservative.
fn collect_expect_assertions_in_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &str,
) -> Vec<TypeScriptAssertion> {
    let mut out = Vec::new();
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, &mut out);
    }
    out
}

fn collect_expect_assertions_in_statement(
    stmt: &Statement<'_>,
    source: &str,
    out: &mut Vec<TypeScriptAssertion>,
) {
    match stmt {
        Statement::BlockStatement(block) => {
            collect_expect_assertions_from_statement_vec(&block.body, source, out);
        }
        Statement::ExpressionStatement(expr_stmt) => {
            if let Some(assertion) = expect_assertion_from_expression(&expr_stmt.expression, source)
            {
                out.push(assertion);
            }
        }
        Statement::ReturnStatement(return_stmt) => {
            if let Some(argument) = &return_stmt.argument
                && let Some(assertion) = expect_assertion_from_expression(argument, source)
            {
                out.push(assertion);
            }
        }
        Statement::IfStatement(if_stmt) => {
            collect_expect_assertions_in_statement(&if_stmt.consequent, source, out);
            if let Some(alternate) = &if_stmt.alternate {
                collect_expect_assertions_in_statement(alternate, source, out);
            }
        }
        Statement::DoWhileStatement(do_while) => {
            collect_expect_assertions_in_statement(&do_while.body, source, out);
        }
        Statement::WhileStatement(while_stmt) => {
            collect_expect_assertions_in_statement(&while_stmt.body, source, out);
        }
        Statement::ForStatement(for_stmt) => {
            collect_expect_assertions_in_statement(&for_stmt.body, source, out);
        }
        Statement::ForInStatement(for_in) => {
            collect_expect_assertions_in_statement(&for_in.body, source, out);
        }
        Statement::ForOfStatement(for_of) => {
            collect_expect_assertions_in_statement(&for_of.body, source, out);
        }
        Statement::LabeledStatement(labeled) => {
            collect_expect_assertions_in_statement(&labeled.body, source, out);
        }
        Statement::SwitchStatement(switch_stmt) => {
            for case in &switch_stmt.cases {
                collect_expect_assertions_from_statement_vec(&case.consequent, source, out);
            }
        }
        Statement::TryStatement(try_stmt) => {
            collect_expect_assertions_from_statement_vec(&try_stmt.block.body, source, out);
            if let Some(handler) = &try_stmt.handler {
                collect_expect_assertions_from_statement_vec(&handler.body.body, source, out);
            }
            if let Some(finalizer) = &try_stmt.finalizer {
                collect_expect_assertions_from_statement_vec(&finalizer.body, source, out);
            }
        }
        Statement::WithStatement(with_stmt) => {
            collect_expect_assertions_in_statement(&with_stmt.body, source, out);
        }
        _ => {}
    }
}

fn collect_expect_assertions_from_statement_vec(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &str,
    out: &mut Vec<TypeScriptAssertion>,
) {
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, out);
    }
}

/// Match the simplest `expect(actual).matcher(...)` shape on a top-level
/// expression. Async-aware `.resolves.matcher` / `.rejects.matcher`
/// chains are recognised by checking for one extra member-access hop
/// before the inner `expect(...)` call; the matcher remains the final
/// property name.
fn expect_assertion_from_expression(
    expr: &Expression<'_>,
    source: &str,
) -> Option<TypeScriptAssertion> {
    let expr = match expr {
        Expression::AwaitExpression(await_expr) => &await_expr.argument,
        _ => expr,
    };
    let Expression::CallExpression(outer_call) = expr else {
        return None;
    };
    let Expression::StaticMemberExpression(outer_member) = &outer_call.callee else {
        return None;
    };
    let matcher = outer_member.property.name.as_str();

    // Inner shape is either `expect(...)` directly or an
    // `expect(...).resolves` / `.rejects` chain.
    let inner = &outer_member.object;
    let async_modifier = expect_assertion_chain_modifier(inner);
    let expect_call = expect_call_from_assertion_inner(inner)?;

    let mock_payload = mock_payload_from_assertion(matcher, expect_call, outer_call, source);
    let error_payload = error_payload_from_assertion(matcher, async_modifier, outer_call, source);
    let (oracle_kind, oracle_strength) = if error_payload.is_some() {
        (OracleKind::ExactErrorVariant, OracleStrength::Strong)
    } else {
        oracle_for_matcher(matcher)
    };
    Some(TypeScriptAssertion {
        matcher: matcher.to_string(),
        argument_count: outer_call.arguments.len(),
        line: line_for_offset(source, outer_call.span.start as usize),
        oracle_kind,
        oracle_strength,
        mock_payload,
        error_payload,
    })
}

fn expect_assertion_chain_modifier<'a>(inner: &'a Expression<'a>) -> Option<&'a str> {
    match inner {
        Expression::StaticMemberExpression(inner_member) => {
            Some(inner_member.property.name.as_str())
                .filter(|modifier| *modifier == "resolves" || *modifier == "rejects")
        }
        _ => None,
    }
}

fn expect_call_from_assertion_inner<'a>(
    inner: &'a Expression<'a>,
) -> Option<&'a oxc_ast::ast::CallExpression<'a>> {
    match inner {
        // Direct: expect(...).matcher(...)
        Expression::CallExpression(inner_call) if call_expression_is_expect(inner_call) => {
            Some(inner_call)
        }
        // Async chain: expect(...).resolves.matcher(...) etc.
        Expression::StaticMemberExpression(inner_member) => {
            let modifier = inner_member.property.name.as_str();
            if modifier != "resolves" && modifier != "rejects" {
                return None;
            }
            match &inner_member.object {
                Expression::CallExpression(inner_call) if call_expression_is_expect(inner_call) => {
                    Some(inner_call)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn call_expression_is_expect(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    matches!(
        &call.callee,
        Expression::Identifier(ident) if ident.name.as_str() == "expect"
    )
}

fn mock_payload_from_assertion(
    matcher: &str,
    expect_call: &oxc_ast::ast::CallExpression<'_>,
    matcher_call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> Option<TypeScriptMockPayload> {
    let target = safe_mock_target_text(expect_call.arguments.first()?, source)?;
    match matcher {
        "toHaveBeenCalledWith" if matcher_call.arguments.len() == 1 => {
            let expected =
                safe_mock_expected_argument_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptMockPayload {
                target,
                expected,
                kind: TypeScriptMockPayloadKind::CalledWith,
            })
        }
        "toHaveBeenCalledTimes" if matcher_call.arguments.len() == 1 => {
            let expected = safe_mock_call_count_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptMockPayload {
                target,
                expected,
                kind: TypeScriptMockPayloadKind::CalledTimes,
            })
        }
        _ => None,
    }
}

fn error_payload_from_assertion(
    matcher: &str,
    async_modifier: Option<&str>,
    matcher_call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> Option<TypeScriptErrorPayload> {
    match (async_modifier, matcher) {
        (None, "toThrow" | "toThrowError") if matcher_call.arguments.len() == 1 => {
            let expected =
                safe_error_literal_payload_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptErrorPayload {
                expected,
                kind: TypeScriptErrorPayloadKind::ThrowsLiteral,
            })
        }
        (Some("rejects"), "toThrow" | "toThrowError") if matcher_call.arguments.len() == 1 => {
            let expected =
                safe_error_literal_payload_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptErrorPayload {
                expected,
                kind: TypeScriptErrorPayloadKind::RejectsThrowLiteral,
            })
        }
        (Some("rejects"), "toMatchObject") if matcher_call.arguments.len() == 1 => {
            let expected = safe_error_object_payload_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptErrorPayload {
                expected,
                kind: TypeScriptErrorPayloadKind::RejectsMatchObject,
            })
        }
        _ => None,
    }
}

fn safe_error_literal_payload_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    matches!(arg, Argument::StringLiteral(_)).then(|| source_text_for_argument(arg, source))?
}

fn safe_error_object_payload_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    match arg {
        Argument::ObjectExpression(object) if safe_mock_expected_object(object) => {
            source_text_for_argument(arg, source)
        }
        _ => None,
    }
}

fn safe_mock_target_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    let text = source_text_for_argument(arg, source)?;
    is_safe_javascript_member_path(&text).then_some(text)
}

fn safe_mock_expected_argument_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    safe_mock_expected_argument(arg).then(|| source_text_for_argument(arg, source))?
}

fn safe_mock_call_count_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    matches!(arg, Argument::NumericLiteral(_)).then(|| source_text_for_argument(arg, source))?
}

fn source_text_for_argument(arg: &Argument<'_>, source: &str) -> Option<String> {
    let span = arg.span();
    Some(
        source
            .get(span.start as usize..span.end as usize)?
            .trim()
            .to_string(),
    )
}

fn safe_mock_expected_argument(arg: &Argument<'_>) -> bool {
    match arg {
        Argument::StringLiteral(_)
        | Argument::NumericLiteral(_)
        | Argument::BooleanLiteral(_)
        | Argument::NullLiteral(_) => true,
        Argument::ObjectExpression(object) => safe_mock_expected_object(object),
        _ => false,
    }
}

fn safe_mock_expected_object(object: &oxc_ast::ast::ObjectExpression<'_>) -> bool {
    object.properties.iter().all(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            !property.computed
                && !property.shorthand
                && safe_mock_expected_object_key(&property.key)
                && safe_mock_expected_object_value(&property.value)
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

fn safe_mock_expected_object_key(key: &PropertyKey<'_>) -> bool {
    matches!(
        key,
        PropertyKey::StaticIdentifier(_)
            | PropertyKey::StringLiteral(_)
            | PropertyKey::NumericLiteral(_)
    )
}

fn safe_mock_expected_object_value(value: &Expression<'_>) -> bool {
    matches!(
        value,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
    )
}

fn is_safe_javascript_member_path(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && !text.starts_with('.')
        && !text.ends_with('.')
        && text
            .split('.')
            .all(|segment| is_safe_javascript_identifier(segment.trim()))
}

fn is_safe_javascript_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(is_javascript_identifier_char)
}

fn related_test_candidates<'a>(
    owner: &TypeScriptOwner,
    all_tests: &'a [TypeScriptTest],
) -> Vec<TypeScriptRelatedCandidate<'a>> {
    let mut candidates: Vec<TypeScriptRelatedCandidate<'a>> = all_tests
        .iter()
        .filter_map(|test| {
            owner_call_relation(test, owner)
                .map(|relation| TypeScriptRelatedCandidate { test, relation })
        })
        .collect();
    if candidates.is_empty() {
        candidates = all_tests
            .iter()
            .filter_map(|test| {
                heuristic_relation(test, owner)
                    .map(|relation| TypeScriptRelatedCandidate { test, relation })
            })
            .collect();
    }
    sort_related_candidates(&mut candidates);
    candidates
}

fn sort_related_candidates(candidates: &mut [TypeScriptRelatedCandidate<'_>]) {
    candidates.sort_by(|left, right| {
        right
            .relation
            .rank()
            .cmp(&left.relation.rank())
            .then_with(|| {
                let left_rank = strongest_assertion(&left.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                let right_rank = strongest_assertion(&right.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                right_rank.cmp(&left_rank)
            })
            .then_with(|| left.test.file.cmp(&right.test.file))
            .then_with(|| left.test.line.cmp(&right.test.line))
            .then_with(|| left.test.name.cmp(&right.test.name))
    });
}

fn owner_call_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Option<TypeScriptRelationKind> {
    if owner.owner_kind == OwnerKind::ModuleFunction {
        return module_initializer_observer_relation(test, owner)
            .then_some(TypeScriptRelationKind::ModuleValueReference);
    }
    if matches!(owner.owner_kind, OwnerKind::Method | OwnerKind::ClassMethod) {
        return receiver_owner_call_relation(test, owner)
            .then_some(TypeScriptRelationKind::ReceiverOwnerCall);
    }
    if contains_call_name(&test.body_text, &owner.name)
        && !owner_name_shadowed_by_unrelated_import(test, owner)
    {
        return Some(TypeScriptRelationKind::DirectOwnerCall);
    }
    if test.imports_in_file.iter().any(|import| {
        import_source_matches_owner(import, &test.file, owner)
            && import_references_owner_call(import, &test.body_text, owner)
    }) {
        return Some(TypeScriptRelationKind::ImportedOwnerCall);
    }
    None
}

fn module_initializer_observer_relation(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    if owner.owner_kind != OwnerKind::ModuleFunction || test_mocks_owner_module(test, owner) {
        return false;
    }
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file)
        && !local_identifier_declared_in_test_body(&test.body_text, &owner.name)
        && expect_actual_references_identifier(&test.body_text, &owner.name)
    {
        return true;
    }
    test.imports_in_file.iter().any(|import| {
        if !import_source_matches_owner(import, &test.file, owner) {
            return false;
        }
        if import.namespace {
            return expect_actual_references_member(&test.body_text, &import.local, &owner.name);
        }
        import.imported.as_deref() == Some(owner.name.as_str())
            && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
            && expect_actual_references_identifier(&test.body_text, &import.local)
    })
}

fn receiver_owner_call_relation(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    if owner.owner_kind != OwnerKind::Method || test_mocks_owner_module(test, owner) {
        return false;
    }
    let constructor_names = constructor_names_for_method_owner(test, owner);
    if constructor_names.is_empty() {
        return false;
    }
    receiver_names_for_constructor_calls(&test.body_text, &constructor_names)
        .iter()
        .any(|receiver| contains_member_call_name(&test.body_text, receiver, &owner.name))
}

fn constructor_names_for_method_owner(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Vec<String> {
    let Some(class_name) = owner.class_name.as_deref() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file) {
        push_unique_string(&mut names, class_name.to_string());
    }
    for import in &test.imports_in_file {
        if import.namespace || !import_source_matches_owner(import, &test.file, owner) {
            continue;
        }
        if import.imported.as_deref() == Some(class_name) {
            push_unique_string(&mut names, import.local.clone());
        }
    }
    names
}

fn receiver_names_for_constructor_calls(
    body_text: &str,
    constructor_names: &[String],
) -> Vec<String> {
    let mut receiver_names = Vec::new();
    for line in body_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some(after_keyword) = ["const ", "let ", "var "]
            .into_iter()
            .find_map(|keyword| trimmed.strip_prefix(keyword))
        else {
            continue;
        };
        let Some((declaration, initializer)) = after_keyword.split_once('=') else {
            continue;
        };
        let Some(receiver_name) = receiver_name_from_declaration(declaration) else {
            continue;
        };
        if constructor_names
            .iter()
            .any(|constructor| contains_new_constructor_call(initializer, constructor))
        {
            push_unique_string(&mut receiver_names, receiver_name);
        }
    }
    receiver_names
}

fn receiver_name_from_declaration(declaration: &str) -> Option<String> {
    if declaration.contains(',') {
        return None;
    }
    let name = declaration.split(':').next()?.trim();
    is_safe_javascript_identifier(name).then(|| name.to_string())
}

fn contains_new_constructor_call(text: &str, constructor_name: &str) -> bool {
    let needle = format!("new {constructor_name}(");
    text.match_indices(&needle).any(|(idx, _)| {
        text[..idx]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
            && !line_prefix_looks_like_comment_or_string(text, idx)
            && !inside_block_comment(text, idx)
    })
}

fn test_mocks_owner_module(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.mocks_in_file.iter().any(|source| {
        normalized_relative_import_module(&test.file, source)
            .is_some_and(|module| module == normalized_module_path(&owner.file))
    })
}

fn heuristic_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> Option<TypeScriptRelationKind> {
    if !heuristic_owner_supported(owner) {
        return None;
    }
    if !heuristic_relation_allowed(test, owner) {
        return None;
    }
    if same_file_proximity_related(test, owner) {
        return Some(TypeScriptRelationKind::SameFileProximity);
    }
    if describe_name_similar_to_owner(test, owner) {
        return Some(TypeScriptRelationKind::DescribeName);
    }
    if test_name_similar_to_owner(test, owner) {
        return Some(TypeScriptRelationKind::TestName);
    }
    None
}

fn heuristic_owner_supported(owner: &TypeScriptOwner) -> bool {
    matches!(
        owner.owner_kind,
        OwnerKind::Function | OwnerKind::ArrowFunction | OwnerKind::Component
    )
}

fn find_related_tests(owner: &TypeScriptOwner, all_tests: &[TypeScriptTest]) -> Vec<RelatedTest> {
    related_test_candidates(owner, all_tests)
        .into_iter()
        .map(|candidate| {
            let strongest = candidate
                .relation
                .uses_oracle()
                .then(|| strongest_assertion(&candidate.test.assertions))
                .flatten();
            let (oracle_kind, oracle_strength, oracle_text) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion_oracle_text(assertion)),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            RelatedTest {
                name: candidate.test.name.clone(),
                file: candidate.test.file.clone(),
                line: candidate.test.line,
                oracle: oracle_text,
                oracle_kind,
                oracle_strength,
            }
        })
        .collect()
}

fn heuristic_relation_allowed(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    !owner_name_shadowed_by_unrelated_import(test, owner)
        && !owner_export_imported_from_unrelated_source(test, owner)
}

fn contains_call_name(body_text: &str, call_name: &str) -> bool {
    let needle = format!("{call_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn has_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

fn owner_name_shadowed_by_unrelated_import(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.imports_in_file
        .iter()
        .filter(|import| import.local == owner.name)
        .any(|import| {
            import.namespace
                || !import_source_matches_owner(import, &test.file, owner)
                || import.imported.as_deref().is_some_and(|imported| {
                    imported != owner.name.as_str() && imported != "default"
                })
        })
}

fn owner_export_imported_from_unrelated_source(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
) -> bool {
    test.imports_in_file.iter().any(|import| {
        import.imported.as_deref() == Some(owner.name.as_str())
            && !import_source_matches_owner(import, &test.file, owner)
    })
}

fn import_references_owner_call(
    import: &TypeScriptImport,
    body_text: &str,
    owner: &TypeScriptOwner,
) -> bool {
    if import.namespace {
        return contains_member_call_name(body_text, &import.local, &owner.name);
    }
    import.imported.as_deref() == Some(owner.name.as_str())
        && contains_call_name(body_text, &import.local)
}

fn import_source_matches_owner(
    import: &TypeScriptImport,
    test_file: &Path,
    owner: &TypeScriptOwner,
) -> bool {
    normalized_relative_import_module(test_file, &import.source)
        .is_some_and(|module| module == normalized_module_path(&owner.file))
}

fn normalized_relative_import_module(test_file: &Path, source: &str) -> Option<String> {
    if !source.starts_with("./") && !source.starts_with("../") {
        return None;
    }
    let mut parts = normalized_path(test_file.parent().unwrap_or_else(|| Path::new("")))
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let normalized_source = source.replace('\\', "/");
    for part in normalized_source.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part.to_string()),
        }
    }
    Some(strip_typescript_module_extension(&parts.join("/")))
}

fn normalized_module_path(path: &Path) -> String {
    strip_typescript_module_extension(&normalized_path(path))
}

fn strip_typescript_module_extension(path: &str) -> String {
    for suffix in [".tsx", ".ts", ".jsx", ".js"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

fn contains_member_call_name(body_text: &str, object_name: &str, method_name: &str) -> bool {
    let needle = format!("{object_name}.{method_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn expect_actual_references_identifier(body_text: &str, identifier: &str) -> bool {
    is_safe_javascript_identifier(identifier)
        && expect_actual_slices(body_text).iter().any(|actual| {
            actual.trim_start().starts_with(identifier)
                && actual
                    .trim_start()
                    .get(identifier.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(|ch| !is_javascript_identifier_char(ch))
        })
}

fn expect_actual_references_member(
    body_text: &str,
    object_name: &str,
    property_name: &str,
) -> bool {
    if !is_safe_javascript_identifier(object_name) || !is_safe_javascript_identifier(property_name)
    {
        return false;
    }
    let reference = format!("{object_name}.{property_name}");
    expect_actual_slices(body_text).iter().any(|actual| {
        actual.trim_start().starts_with(&reference)
            && actual
                .trim_start()
                .get(reference.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|ch| !is_javascript_identifier_char(ch))
    })
}

fn expect_actual_slices(body_text: &str) -> Vec<&str> {
    body_text
        .match_indices("expect(")
        .filter_map(|(idx, _)| {
            if line_prefix_looks_like_comment_or_string(body_text, idx)
                || inside_block_comment(body_text, idx)
            {
                return None;
            }
            body_text.get(idx + "expect(".len()..)
        })
        .collect()
}

fn local_identifier_declared_in_test_body(body_text: &str, identifier: &str) -> bool {
    body_text.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("//") && declaration_line_declares_identifier(trimmed, identifier)
    })
}

fn declaration_line_declares_identifier(line: &str, identifier: &str) -> bool {
    ["const ", "let ", "var ", "function "]
        .into_iter()
        .filter_map(|keyword| line.strip_prefix(keyword))
        .filter_map(|after| {
            after
                .split(|ch: char| {
                    ch == ':'
                        || ch == '='
                        || ch == '('
                        || ch == ','
                        || ch == ';'
                        || ch.is_whitespace()
                })
                .find(|part| !part.is_empty())
        })
        .any(|declared| declared == identifier)
}

fn has_member_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

fn line_prefix_looks_like_comment_or_string(body_text: &str, idx: usize) -> bool {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &body_text[line_start..idx];
    prefix.trim_start().starts_with("//") || has_unclosed_quote_or_template(prefix)
}

fn inside_block_comment(body_text: &str, idx: usize) -> bool {
    let prefix = &body_text[..idx];
    let comment_start = prefix.rfind("/*");
    let comment_end = prefix.rfind("*/");
    comment_start.is_some_and(|start| comment_end.is_none_or(|end| start > end))
}

fn has_unclosed_quote_or_template(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double && !in_template {
            in_single = !in_single;
        } else if ch == '"' && !in_single && !in_template {
            in_double = !in_double;
        } else if ch == '`' && !in_single && !in_double {
            in_template = !in_template;
        }
    }
    in_single || in_double || in_template
}

fn is_javascript_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn same_file_proximity_related(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    let Some(owner_stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(test_stem) = test.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let owner_key = normalize_typescript_test_stem(owner_stem);
    let test_key = normalize_typescript_test_stem(test_stem);
    !owner_key.is_empty() && owner_key == test_key
}

fn normalize_typescript_test_stem(stem: &str) -> String {
    let mut value = stem.to_string();
    for suffix in [".test", ".spec", "_test", "-test"] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            value = stripped.to_string();
            break;
        }
    }
    for prefix in ["test.", "test_", "test-"] {
        if let Some(stripped) = value.strip_prefix(prefix) {
            value = stripped.to_string();
            break;
        }
    }
    normalize_similarity_key(&value)
}

fn describe_name_similar_to_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.describe_names.iter().any(|name| {
        let describe_key = normalize_similarity_key(name);
        owner_similarity_keys(owner)
            .into_iter()
            .any(|key| similarity_key_contains(&describe_key, &key))
    })
}

fn test_name_similar_to_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    let test_key = normalize_similarity_key(&test.local_name);
    owner_similarity_keys(owner)
        .into_iter()
        .any(|key| similarity_key_contains(&test_key, &key))
}

fn owner_similarity_keys(owner: &TypeScriptOwner) -> Vec<String> {
    let mut keys = Vec::new();
    push_unique_similarity_key(&mut keys, normalize_similarity_key(&owner.name));
    keys
}

fn push_unique_similarity_key(keys: &mut Vec<String>, key: String) {
    if !key.is_empty() && !keys.iter().any(|existing| existing == &key) {
        keys.push(key);
    }
}

fn normalize_similarity_key(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = true;
    let mut previous_was_lower_or_digit = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && !out.is_empty()
                && !last_was_separator
                && previous_was_lower_or_digit
            {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !out.is_empty() && !last_was_separator {
            out.push('_');
            last_was_separator = true;
            previous_was_lower_or_digit = false;
        } else {
            previous_was_lower_or_digit = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack == needle
        || haystack
            .strip_prefix(needle)
            .is_some_and(|suffix| suffix.starts_with('_'))
        || haystack
            .strip_suffix(needle)
            .is_some_and(|prefix| prefix.ends_with('_'))
        || haystack.contains(&format!("_{needle}_"))
}

fn assertion_oracle_text(assertion: &TypeScriptAssertion) -> String {
    if let Some(mock_payload) = &assertion.mock_payload {
        return mock_payload.oracle_text();
    }
    if let Some(error_payload) = &assertion.error_payload {
        return error_payload.oracle_text();
    }
    if matches!(assertion.matcher.as_str(), "toThrow" | "toThrowError")
        && assertion.argument_count == 0
    {
        format!("expect(...).{}()", assertion.matcher)
    } else {
        format!("expect(...).{}(...)", assertion.matcher)
    }
}

/// Pick the highest-rank assertion from a test body. Used to summarise a
/// related test's strongest oracle for the classifier.
fn strongest_assertion(assertions: &[TypeScriptAssertion]) -> Option<&TypeScriptAssertion> {
    assertions
        .iter()
        .max_by_key(|assertion| assertion.oracle_strength.rank())
}

fn related_mock_payload_oracle(related: &[RelatedTest]) -> Option<String> {
    related.iter().find_map(|test| {
        (test.oracle_kind == OracleKind::MockExpectation)
            .then_some(test.oracle.as_deref())
            .flatten()
            .filter(|oracle| !oracle.contains("..."))
            .map(str::to_string)
    })
}

/// Collect the deduplicated set of module paths that any related test
/// file mocks via syntactic `vi.mock("path")` / `jest.mock("path")`.
///
/// Related tests are identified through the same fallback ordering as
/// `find_related_tests`: trusted call/import relations first, then
/// uncertainty-only name/proximity links only when no trusted relation exists.
/// Each selected test's `mocks_in_file` list is contributed once. The
/// classifier uses the resulting list to surface the `mocked_module`
/// static-limit per RIPR-SPEC-0026.
fn collect_related_mock_paths(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for candidate in related_test_candidates(owner, all_tests) {
        for path in &candidate.test.mocks_in_file {
            if !paths.iter().any(|existing| existing == path) {
                paths.push(path.clone());
            }
        }
    }
    paths
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptStaticLimit {
    kind: StaticLimitKind,
    evidence: Vec<String>,
    missing: String,
    repair_route: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptActionability {
    gap_state: &'static str,
    category: &'static str,
    why_not_actionable: String,
    repair_route: String,
    missing_fields: Vec<&'static str>,
    evidence_needed: &'static str,
}

impl TypeScriptActionability {
    fn evidence(&self, raw_evidence_ref: String) -> Vec<String> {
        let mut evidence = vec![
            format!("gap_state: {}", self.gap_state),
            format!("actionability_category: {}", self.category),
            format!("why_not_actionable: {}", self.why_not_actionable),
            format!("repair_route: {}", self.repair_route),
        ];
        if !self.missing_fields.is_empty() {
            evidence.push(format!(
                "missing_actionability_fields: {}",
                self.missing_fields.join(", ")
            ));
        }
        evidence.push(format!(
            "evidence_needed_to_promote: {}",
            self.evidence_needed
        ));
        evidence.push(raw_evidence_ref);
        evidence
    }

    fn missing_summary(&self) -> String {
        format!(
            "TypeScript preview actionability `{}` / `{}`: {}. Repair route: {}",
            self.gap_state, self.category, self.why_not_actionable, self.repair_route
        )
    }
}

fn typescript_actionability_for(
    class: &ExposureClass,
    static_limit: Option<&TypeScriptStaticLimit>,
    has_oracle_eligible_relation: bool,
    missing_discriminators: &[MissingDiscriminatorFact],
) -> TypeScriptActionability {
    if let Some(limit) = static_limit {
        return TypeScriptActionability {
            gap_state: "static_limitation",
            category: limit.kind.as_str(),
            why_not_actionable: format!(
                "static limit `{}` prevents bounded TypeScript repair guidance",
                limit.kind.as_str()
            ),
            repair_route: normalize_repair_route(&limit.repair_route),
            missing_fields: Vec::new(),
            evidence_needed: "resolve the named static limit and re-run TypeScript preview evidence extraction",
        };
    }

    if matches!(class, ExposureClass::Exposed) {
        return TypeScriptActionability {
            gap_state: "already_observed",
            category: "strong_oracle_observed",
            why_not_actionable:
                "related Jest/Vitest evidence already has a strong exact oracle; no repair packet should be emitted"
                    .to_string(),
            repair_route:
                "keep the finding advisory preview and verify the existing assertion still targets the changed behavior"
                    .to_string(),
            missing_fields: Vec::new(),
            evidence_needed:
                "none for a repair packet; retain strong related-test evidence as non-actionable context",
        };
    }

    if matches!(class, ExposureClass::NoStaticPath) {
        return TypeScriptActionability {
            gap_state: "advisory",
            category: "missing_context",
            why_not_actionable:
                "no trusted related Jest/Vitest test or observer is available for a bounded TypeScript repair route"
                    .to_string(),
            repair_route:
                "add trusted related-test matching for this owner shape before emitting a repair packet"
                    .to_string(),
            missing_fields: vec![
                "related_test_or_observer",
                "target_test_shape",
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "trusted related test or observer, target shape, verify command, receipt command, and edit boundaries",
        };
    }

    if !has_oracle_eligible_relation {
        return TypeScriptActionability {
            gap_state: "advisory",
            category: "ambiguous_related_test",
            why_not_actionable:
                "related-test link is heuristic-only and cannot safely borrow extracted assertions as proof"
                    .to_string(),
            repair_route:
                "add a direct owner-call, import-aware, or receiver-aware relation before repair packet projection"
                    .to_string(),
            missing_fields: vec![
                "related_test_or_observer",
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "trusted token-aware relation plus complete verify, receipt, and edit-boundary fields",
        };
    }

    if missing_discriminators.is_empty() {
        return TypeScriptActionability {
            gap_state: "advisory",
            category: "missing_target_shape",
            why_not_actionable:
                "TypeScript preview found related test evidence but cannot name a safe target discriminator or observer shape"
                    .to_string(),
            repair_route:
                "add probe-specific discriminator extraction for this expression before repair packet projection"
                    .to_string(),
            missing_fields: vec![
                "repair_kind",
                "target_test_shape",
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "safe probe discriminator, repair kind, target shape, verify command, receipt command, and edit boundaries",
        };
    }

    TypeScriptActionability {
        gap_state: "advisory",
        category: "incomplete_repair_packet",
        why_not_actionable:
            "TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract"
                .to_string(),
        repair_route:
            "project canonical TypeScript repair packet fields only after verify, receipt, evidence refs, and edit boundaries are available"
                .to_string(),
        missing_fields: vec![
            "canonical_gap_id",
            "repair_kind",
            "target_test_shape",
            "related_test_or_observer",
            "verify_command",
            "receipt_command",
            "must_not_change",
            "allowed_edit_surface",
            "raw_evidence_refs",
        ],
        evidence_needed:
            "canonical gap identity, repair kind, target test shape, related observer, verify command, receipt command, raw evidence refs, and edit constraints",
    }
}

fn normalize_repair_route(route: &str) -> String {
    route
        .strip_prefix("Repair route: ")
        .unwrap_or(route)
        .trim_end_matches('.')
        .to_string()
}

fn typescript_raw_evidence_ref(
    file: &Path,
    line: usize,
    owner: Option<&TypeScriptOwner>,
    source_id: &str,
) -> String {
    let mut parts = vec![
        format!("file={}", normalized_path(file)),
        format!("line={line}"),
        "kind=typescript_preview_probe".to_string(),
        format!("source_id={source_id}"),
    ];
    if let Some(owner) = owner {
        parts.push(format!("owner={}", owner.name));
    }
    format!("raw_evidence_ref: {}", parts.join(";"))
}

fn static_limit_for_change(
    line_text: &str,
    owner: &TypeScriptOwner,
    mock_paths: &[String],
) -> Option<TypeScriptStaticLimit> {
    let trimmed = line_text.trim();
    if is_computed_member_call(trimmed) {
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::DynamicDispatch,
            evidence: vec![
                "static_limit dynamic_dispatch: changed line uses computed member invocation"
                    .to_string(),
            ],
            missing: "Static limit `dynamic_dispatch`: the TypeScript preview adapter saw a computed member call such as `obj[name](...)`; syntax alone cannot resolve the called behavior. Repair route: inspect the concrete dispatch key or add analyzer support for explicit dispatch-map resolution before issuing a repair packet.".to_string(),
            repair_route: "Repair route: inspect the concrete dispatch key or add analyzer support for explicit dispatch-map resolution before issuing a repair packet.".to_string(),
        });
    }
    if contains_metaprogramming(trimmed) {
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::Metaprogramming,
            evidence: vec![
                "static_limit metaprogramming: changed line uses metaprogramming syntax"
                    .to_string(),
            ],
            missing: "Static limit `metaprogramming`: the TypeScript preview adapter saw Proxy, Reflect, or property-definition metaprogramming syntax and does not infer runtime-created behavior. Repair route: add metaprogramming-aware modeling or keep the finding as human-review-only before issuing a repair packet.".to_string(),
            repair_route: "Repair route: add metaprogramming-aware modeling or keep the finding as human-review-only before issuing a repair packet.".to_string(),
        });
    }
    if owner.decorated || trimmed.starts_with('@') {
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: vec![format!(
                "static_limit decorator_indirection: owner `{}` uses TypeScript decorators",
                owner.name
            )],
            missing: format!(
                "Static limit `decorator_indirection`: owner `{}` uses TypeScript decorators; syntax-first preview evidence does not resolve decorator-modified call behavior. Repair route: add decorator-aware owner modeling or verify decorator-modified behavior manually before issuing a repair packet.",
                owner.name
            ),
            repair_route: "Repair route: add decorator-aware owner modeling or verify decorator-modified behavior manually before issuing a repair packet.".to_string(),
        });
    }
    if !mock_paths.is_empty() {
        let preview: String = mock_paths
            .iter()
            .map(|path| format!("`{path}`"))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::MockedModule,
            evidence: mock_paths
                .iter()
                .map(|path| format!("static_limit mocked_module: `{path}`"))
                .collect(),
            missing: format!(
                "Static limit `mocked_module`: related test file mocks {preview} via `vi.mock(...)` / `jest.mock(...)`. The TypeScript preview adapter does not resolve mocked module semantics, so the substitution under test is opaque to static evidence. Repair route: add mock-shape support or validate the real substitution under test before issuing a repair packet."
            ),
            repair_route: "Repair route: add mock-shape support or validate the real substitution under test before issuing a repair packet.".to_string(),
        });
    }
    if let Some(import) = imported_symbol_call(trimmed, &owner.imports) {
        let symbol = if import.namespace {
            format!("{}.*", import.local)
        } else {
            import.local.clone()
        };
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::MissingImportGraph,
            evidence: vec![format!(
                "static_limit missing_import_graph: changed line calls imported symbol `{symbol}`"
            )],
            missing: format!(
                "Static limit `missing_import_graph`: the changed line calls imported symbol `{symbol}` from `{}`; the TypeScript preview adapter does not build a package or import graph for production implementation semantics. Repair route: add import graph support or inspect the imported implementation before issuing a repair packet.",
                import.source
            ),
            repair_route: "Repair route: add import graph support or inspect the imported implementation before issuing a repair packet.".to_string(),
        });
    }
    None
}

fn contains_metaprogramming(text: &str) -> bool {
    [
        "new Proxy(",
        "Proxy(",
        "Reflect.",
        "Object.defineProperty(",
        "Object.defineProperties(",
    ]
    .iter()
    .any(|shape| contains_unquoted_shape(text, shape))
}

fn imported_symbol_call<'a>(
    line_text: &str,
    imports: &'a [TypeScriptImport],
) -> Option<&'a TypeScriptImport> {
    imports.iter().find(|import| {
        if import.namespace {
            contains_namespace_import_call(line_text, &import.local)
        } else {
            contains_call_name(line_text, &import.local)
        }
    })
}

fn contains_namespace_import_call(line_text: &str, namespace: &str) -> bool {
    let needle = format!("{namespace}.");
    line_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(line_text, idx)
            && !line_prefix_looks_like_comment_or_string(line_text, idx)
            && !inside_block_comment(line_text, idx)
            && line_text
                .get(idx + needle.len()..)
                .is_some_and(namespace_tail_has_call)
    })
}

fn namespace_tail_has_call(tail: &str) -> bool {
    let mut saw_name = false;
    for ch in tail.chars() {
        if ch == '(' {
            return saw_name;
        }
        if ch.is_whitespace() || ch == ';' || ch == ',' || ch == ')' || ch == ']' || ch == '}' {
            return false;
        }
        if ch == '?' || ch == '.' {
            continue;
        }
        if is_javascript_identifier_char(ch) {
            saw_name = true;
            continue;
        }
        return false;
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptProbeShape {
    family: ProbeFamily,
    delta: DeltaKind,
    specific: bool,
}

impl TypeScriptProbeShape {
    fn new(family: ProbeFamily, delta: DeltaKind) -> Self {
        Self {
            family,
            delta,
            specific: true,
        }
    }

    fn ambiguous_fallback() -> Self {
        Self {
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            specific: false,
        }
    }
}

/// Syntax-first probe-family classifier for a changed line of TypeScript
/// or JavaScript source.
///
/// Inspects the leading non-whitespace tokens of `line_text` and falls
/// back to substring shape checks for ternary / arrow-bodied expressions.
/// Matches the families documented in RIPR-SPEC-0027 and pinned by the
/// TypeScript probe-fixture family.
///
/// The adapter operates without a type checker, so ambiguous shapes keep
/// the historical `Predicate` / `Control` fallback but are marked
/// non-specific so later repair guidance does not invent discriminators.
fn classify_probe_shape_detail(line_text: &str) -> TypeScriptProbeShape {
    let trimmed = line_text.trim_start();
    // Strip a leading `} ` (e.g., `} else if (...)`, `} else {`) so the
    // dedicated-keyword check still fires on close-brace-continuation
    // shapes that are common in JavaScript-style if/else ladders.
    let leading = trimmed.strip_prefix("} ").unwrap_or(trimmed).trim_start();

    if leading.starts_with("throw ")
        || leading.starts_with("throw(")
        || leading.starts_with("return Promise.reject(")
        || leading.starts_with("return Promise.reject ")
        || leading.starts_with("return await Promise.reject(")
        || leading.starts_with("return await Promise.reject ")
        || leading.starts_with("await Promise.reject(")
        || leading.starts_with("await Promise.reject ")
        || leading.starts_with("} catch ")
        || leading.starts_with("catch ")
    {
        return TypeScriptProbeShape::new(ProbeFamily::ErrorPath, DeltaKind::Control);
    }
    if is_object_literal_return_line(leading) {
        return TypeScriptProbeShape::new(ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    if leading.starts_with("return ") || leading == "return;" || leading.starts_with("return;") {
        return TypeScriptProbeShape::new(ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if leading.starts_with("if (")
        || leading.starts_with("if(")
        || leading.starts_with("else if (")
        || leading.starts_with("else if(")
        || leading.starts_with("while (")
        || leading.starts_with("while(")
        || leading.starts_with("for (")
        || leading.starts_with("for(")
        || leading.starts_with("switch (")
        || leading.starts_with("switch(")
        || leading.starts_with("case ")
        || leading.starts_with("default:")
    {
        return TypeScriptProbeShape::new(ProbeFamily::Predicate, DeltaKind::Control);
    }
    // Top-level ternary or short-circuit expression that is *not* embedded
    // in a `return` or assignment — treat as a predicate boundary.
    if (leading.contains("? ") && leading.contains(" : "))
        && !leading.starts_with("const ")
        && !leading.starts_with("let ")
        && !leading.starts_with("var ")
    {
        return TypeScriptProbeShape::new(ProbeFamily::Predicate, DeltaKind::Control);
    }
    if is_object_literal_field_line(leading) {
        return TypeScriptProbeShape::new(ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    // Field / property assignments: `this.x = ...`, `obj.x = ...`, or
    // top-level binding declarations inside a constructor / setter body.
    // Detected only when the line has the form `<ident chain> = <expr>`
    // without a leading function-call shape; this keeps statement-level
    // call expressions in the SideEffect bucket below.
    if let Some(eq_idx) = leading.find(" = ")
        && !leading.starts_with("if ")
        && !leading.starts_with("else ")
        && !leading.starts_with("return")
        && !leading.starts_with("throw")
    {
        let lhs = &leading[..eq_idx];
        let looks_like_assignment = lhs
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']');
        let looks_like_declaration =
            lhs.starts_with("const ") || lhs.starts_with("let ") || lhs.starts_with("var ");
        if looks_like_assignment && !looks_like_declaration {
            return TypeScriptProbeShape::new(ProbeFamily::FieldConstruction, DeltaKind::Value);
        }
    }
    // Bare call-expression statement (e.g., `tracker.record(event);`,
    // `await logger.flush();`). Detected by trailing `);` after stripping
    // optional `await ` / `void ` / trailing comments.
    let call_candidate = leading
        .strip_prefix("await ")
        .unwrap_or(leading)
        .strip_prefix("void ")
        .unwrap_or_else(|| leading.strip_prefix("await ").unwrap_or(leading))
        .trim_end();
    let call_candidate = call_candidate
        .strip_suffix(';')
        .unwrap_or(call_candidate)
        .trim_end();
    if call_candidate.ends_with(')')
        && call_candidate.contains('(')
        && !call_candidate.starts_with("if")
        && !call_candidate.starts_with("while")
        && !call_candidate.starts_with("for")
        && !call_candidate.starts_with("switch")
        && !call_candidate.starts_with("return")
        && !call_candidate.starts_with("throw")
        && !call_candidate.starts_with("const ")
        && !call_candidate.starts_with("let ")
        && !call_candidate.starts_with("var ")
    {
        return TypeScriptProbeShape::new(ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    // Fall through conservatively. The adapter does not recognise this shape,
    // so flagging it as a generic predicate-control change avoids committing
    // to a more specific family the preview surface cannot confirm.
    TypeScriptProbeShape::ambiguous_fallback()
}

#[cfg(test)]
fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let detail = classify_probe_shape_detail(line_text);
    (detail.family, detail.delta)
}

fn is_object_literal_return_line(line_text: &str) -> bool {
    let trimmed = line_text.trim_start();
    trimmed.starts_with("return {") || trimmed.starts_with("return ({")
}

fn is_object_literal_field_line(line_text: &str) -> bool {
    let trimmed = line_text.trim();
    if trimmed.starts_with("case ")
        || trimmed.starts_with("default:")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("export ")
        || trimmed.ends_with(';')
        || trimmed.contains("=>")
    {
        return false;
    }
    let Some((key, rest)) = trimmed.split_once(':') else {
        return false;
    };
    let key = key.trim().trim_matches('"').trim_matches('\'');
    !key.is_empty()
        && !rest
            .trim_end_matches(',')
            .trim_end_matches('}')
            .trim()
            .is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn typescript_flow_sink_for(
    probe_shape: &TypeScriptProbeShape,
    owner: &TypeScriptOwner,
    line: usize,
    line_text: &str,
) -> Option<FlowSinkFact> {
    if !probe_shape.specific {
        return None;
    }
    let kind = match probe_shape.family {
        ProbeFamily::ReturnValue => FlowSinkKind::ReturnValue,
        ProbeFamily::ErrorPath => FlowSinkKind::ErrorVariant,
        ProbeFamily::FieldConstruction => {
            if is_computed_field_construction(line_text) {
                return None;
            }
            FlowSinkKind::StructField
        }
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            if is_computed_member_call(line_text) {
                return None;
            }
            FlowSinkKind::CallEffect
        }
        ProbeFamily::Predicate | ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => {
            return None;
        }
    };

    Some(FlowSinkFact {
        kind,
        text: line_text.trim().to_string(),
        line,
        owner: Some(owner.symbol_id()),
    })
}

fn typescript_missing_discriminators(
    probe_shape: &TypeScriptProbeShape,
    line: usize,
    line_text: &str,
    flow_sink: Option<&FlowSinkFact>,
) -> Vec<MissingDiscriminatorFact> {
    if !probe_shape.specific {
        return Vec::new();
    }
    let Some(value) = typescript_missing_discriminator_value(&probe_shape.family, line_text) else {
        return Vec::new();
    };

    vec![MissingDiscriminatorFact {
        value,
        reason: typescript_missing_discriminator_reason(&probe_shape.family, line),
        flow_sink: flow_sink.cloned(),
    }]
}

fn typescript_missing_discriminator_reason(probe_family: &ProbeFamily, line: usize) -> String {
    let shape = match probe_family {
        ProbeFamily::Predicate => "equality-boundary",
        ProbeFamily::ReturnValue => "returned-value",
        ProbeFamily::ErrorPath => "thrown or rejected error",
        ProbeFamily::FieldConstruction => "field/object value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "call side effect",
        ProbeFamily::MatchArm => "match-arm",
        ProbeFamily::StaticUnknown => "static",
    };
    format!("changed TypeScript {shape} at line {line} lacks a concrete preview discriminator")
}

fn typescript_missing_discriminator_value(
    probe_family: &ProbeFamily,
    line_text: &str,
) -> Option<String> {
    match probe_family {
        ProbeFamily::Predicate => typescript_boundary_discriminator(line_text),
        ProbeFamily::ReturnValue => typescript_return_value_discriminator(line_text),
        ProbeFamily::ErrorPath => typescript_error_path_discriminator(line_text),
        ProbeFamily::FieldConstruction => typescript_field_value_discriminator(line_text),
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            typescript_call_effect_discriminator(line_text)
        }
        ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => None,
    }
}

fn typescript_boundary_discriminator(line_text: &str) -> Option<String> {
    let expression = strip_typescript_control_prefix(line_text);
    for operator in ["===", "!==", ">=", "<=", "==", "!=", ">", "<"] {
        if let Some(idx) = expression.find(operator) {
            let left_raw = expression.get(..idx)?.trim();
            let right_raw = expression.get(idx + operator.len()..)?.trim();
            if operand_looks_like_call(left_raw) || operand_looks_like_call(right_raw) {
                return None;
            }
            let left = comparison_operand_before(&expression, idx)?;
            let right = comparison_operand_after(&expression, idx + operator.len())?;
            if is_simple_typescript_discriminator_operand(&left)
                && is_simple_typescript_discriminator_operand(&right)
            {
                return Some(format!("{left} == {right}"));
            }
        }
    }
    None
}

fn typescript_return_value_discriminator(line_text: &str) -> Option<String> {
    let expression = line_text
        .trim()
        .strip_prefix("return")?
        .trim()
        .trim_end_matches(';')
        .trim();
    if expression.is_empty() || expression == "{" || expression == "({" {
        None
    } else {
        Some(format!("return value == {expression}"))
    }
}

fn typescript_error_path_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim().trim_end_matches(';').trim();
    if text.starts_with("throw ") || text.starts_with("throw(") {
        let raised = text
            .strip_prefix("throw ")
            .or_else(|| text.strip_prefix("throw("))
            .unwrap_or(text)
            .trim()
            .trim_end_matches(')');
        return typescript_error_value("throws", raised);
    }
    if let Some(argument) = promise_reject_argument(text) {
        return typescript_error_value("rejects", argument);
    }
    if text.starts_with("catch ") || text.starts_with("} catch ") {
        return Some("catch branch executes".to_string());
    }
    None
}

fn promise_reject_argument(text: &str) -> Option<&str> {
    let marker = "Promise.reject(";
    let start = text.find(marker)? + marker.len();
    let tail = text.get(start..)?;
    let end = tail.rfind(')')?;
    Some(tail.get(..end)?.trim())
}

fn typescript_error_value(prefix: &str, expression: &str) -> Option<String> {
    let expression = expression.trim();
    let error_type = if let Some(constructed) = expression.strip_prefix("new ") {
        constructed
            .split_once('(')
            .map(|(ty, _)| ty.trim())
            .unwrap_or(constructed.trim())
    } else if let Some((callee, _)) = expression.split_once('(') {
        let callee = callee.trim();
        if !starts_with_uppercase(callee) && !callee.ends_with("Error") {
            return None;
        }
        callee
    } else if let Some(message) = first_typescript_string_literal(expression) {
        return Some(format!("{prefix} error matching {message}"));
    } else {
        return None;
    };
    if error_type.is_empty() {
        return None;
    }
    if let Some(message) = first_typescript_string_literal(expression) {
        Some(format!("{prefix} {error_type} matching {message}"))
    } else {
        Some(format!("{prefix} {error_type}"))
    }
}

fn typescript_field_value_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim().trim_end_matches(';').trim();
    if let Some(discriminator) = typescript_return_object_field_discriminator(text) {
        return Some(discriminator);
    }
    if let Some(discriminator) = typescript_object_field_discriminator(text) {
        return Some(discriminator);
    }
    let (lhs, rhs) = split_typescript_assignment(text)?;
    if lhs.is_empty() || rhs.is_empty() || lhs.contains('[') || lhs.contains(']') {
        None
    } else {
        Some(format!("{lhs} == {rhs}"))
    }
}

fn typescript_return_object_field_discriminator(line_text: &str) -> Option<String> {
    let expression = line_text
        .strip_prefix("return ")?
        .trim()
        .strip_prefix('(')
        .unwrap_or_else(|| {
            line_text
                .strip_prefix("return ")
                .unwrap_or(line_text)
                .trim()
        })
        .trim();
    let body = expression.strip_prefix('{')?;
    typescript_object_field_discriminator(body)
}

fn typescript_object_field_discriminator(line_text: &str) -> Option<String> {
    let body = line_text.trim().trim_end_matches(')').trim_end_matches('}');
    let (raw_key, rest) = body.split_once(':')?;
    let key = raw_key.trim().trim_matches('"').trim_matches('\'');
    let value = rest
        .split(',')
        .next()
        .unwrap_or(rest)
        .trim()
        .trim_end_matches('}')
        .trim();
    if !is_simple_typescript_object_key(key) || value.is_empty() {
        None
    } else {
        Some(format!("{key} == {value}"))
    }
}

fn typescript_call_effect_discriminator(line_text: &str) -> Option<String> {
    if is_computed_member_call(line_text) {
        return None;
    }
    let (callee, args) = typescript_call_parts(line_text)?;
    let first_arg = args
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(')')
        .trim();
    if callee.to_ascii_lowercase().contains("mock") || callee.to_ascii_lowercase().contains("spy") {
        if first_arg.is_empty() {
            Some(format!("mock interaction {callee} is called"))
        } else {
            Some(format!("mock interaction {callee} called with {first_arg}"))
        }
    } else if let Some(literal) = first_typescript_string_literal(line_text) {
        if callee.contains("log") || callee.starts_with("console.") {
            Some(format!("log contains {literal}"))
        } else {
            Some(format!("call {callee} includes {literal}"))
        }
    } else if first_arg.is_empty() {
        Some(format!("call {callee} occurs"))
    } else {
        Some(format!("call {callee} includes {first_arg}"))
    }
}

fn split_typescript_assignment(text: &str) -> Option<(&str, &str)> {
    if text.contains("==") || text.contains("!=") || text.contains(">=") || text.contains("<=") {
        return None;
    }
    let (lhs, rhs) = text.split_once(" = ")?;
    Some((lhs.trim(), rhs.trim().trim_end_matches(';').trim()))
}

fn is_computed_field_construction(line_text: &str) -> bool {
    let text = line_text.trim();
    if let Some((lhs, _)) = split_typescript_assignment(text) {
        return lhs.contains('[') || lhs.contains(']');
    }
    contains_unquoted_shape(text, "{[") || contains_unquoted_shape(text, "{ [")
}

fn strip_typescript_control_prefix(line_text: &str) -> String {
    let mut text = line_text
        .trim()
        .trim_start_matches('}')
        .trim()
        .trim_end_matches('{')
        .trim()
        .to_string();
    for prefix in ["if", "else if", "while", "for", "case"] {
        if let Some(stripped) = text.strip_prefix(prefix) {
            text = stripped.trim().to_string();
            break;
        }
    }
    text.trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string()
}

fn comparison_operand_before(expression: &str, operator_start: usize) -> Option<String> {
    let left = expression.get(..operator_start)?.trim_end();
    let operand = left
        .rsplit(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                )
        })
        .find(|part| !part.is_empty())?;
    Some(operand.trim().to_string())
}

fn comparison_operand_after(expression: &str, operator_end: usize) -> Option<String> {
    let right = expression.get(operator_end..)?.trim_start();
    let operand = right
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                )
        })
        .find(|part| !part.is_empty())?;
    Some(operand.trim().to_string())
}

fn is_simple_typescript_discriminator_operand(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '"' || ch == '\''
        })
}

fn is_simple_typescript_object_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn operand_looks_like_call(value: &str) -> bool {
    value.contains('(') || value.contains(')')
}

fn typescript_call_parts(line_text: &str) -> Option<(String, String)> {
    let mut text = line_text
        .trim()
        .strip_prefix("await ")
        .unwrap_or(line_text.trim())
        .trim();
    text = text.strip_prefix("void ").unwrap_or(text).trim();
    let text = text.trim_end_matches(';').trim();
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    let callee = text.get(..open)?.trim();
    if callee.is_empty()
        || !callee
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.')
    {
        return None;
    }
    let args = text.get(open + 1..close)?.trim();
    Some((callee.to_string(), args.to_string()))
}

fn is_computed_member_call(line_text: &str) -> bool {
    let text = line_text.trim();
    ["](", "]?.", "?.["]
        .iter()
        .any(|shape| contains_unquoted_shape(text, shape))
}

fn contains_unquoted_shape(text: &str, shape: &str) -> bool {
    text.match_indices(shape).any(|(idx, _)| {
        !line_prefix_looks_like_comment_or_string(text, idx) && !inside_block_comment(text, idx)
    })
}

fn first_typescript_string_literal(text: &str) -> Option<String> {
    let mut start = None;
    let mut escaped = false;
    for (idx, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            start = Some((idx, ch));
            break;
        }
    }
    let (start_idx, quote) = start?;
    escaped = false;
    for (relative_idx, ch) in text[start_idx + quote.len_utf8()..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            let end_idx = start_idx + quote.len_utf8() + relative_idx + quote.len_utf8();
            return text.get(start_idx..end_idx).map(str::to_string);
        }
    }
    None
}

fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[TypeScriptOwner],
    all_tests: &[TypeScriptTest],
) -> Option<Finding> {
    let changed_file = normalized_path(file);
    let owner = owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .find(|owner| line >= owner.start_line && line <= owner.end_line)?;
    let related_candidates = related_test_candidates(owner, all_tests);
    let related = find_related_tests(owner, all_tests);
    let mock_paths = collect_related_mock_paths(owner, all_tests);
    let static_limit = static_limit_for_change(line_text, owner, &mock_paths);
    let has_oracle_eligible_relation = related_candidates
        .iter()
        .any(|candidate| candidate.relation.uses_oracle());
    let probe_shape = classify_probe_shape_detail(line_text);

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
    let mock_payload_oracle = related_mock_payload_oracle(&related);

    let (class, reach_state, observe_state, discriminate_state, mut missing) = if related.is_empty()
    {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![no_static_path_missing(owner)],
        )
    } else if !has_oracle_eligible_relation {
        (
            ExposureClass::WeaklyExposed,
            StageState::Weak,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Only heuristic TypeScript test links were found for `{}`; verify the suggested test location or add a direct Jest/Vitest owner call with an exact-value assertion.",
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
                "Related test reaches `{}` with a `{}` oracle. Static evidence suggests the changed behavior is observed under an exact-value or exact-error-variant discriminator.",
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
            vec![weak_oracle_missing_summary(
                &owner.name,
                &strongest_kind,
                &probe_shape.family,
                mock_payload_oracle.as_deref(),
            )],
        )
    };
    if let Some(limit) = &static_limit {
        missing.push(limit.missing.clone());
    }

    let flow_sink = typescript_flow_sink_for(&probe_shape, owner, line, line_text);
    let missing_discriminators = if matches!(class, ExposureClass::WeaklyExposed)
        && has_oracle_eligible_relation
        && static_limit.is_none()
    {
        typescript_missing_discriminators(&probe_shape, line, line_text, flow_sink.as_ref())
    } else {
        Vec::new()
    };

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let family = probe_shape.family.clone();
    let delta = probe_shape.delta.clone();
    let expected_sinks = if probe_shape.specific {
        probes::expected_sinks(line_text, &family)
    } else {
        Vec::new()
    };
    let required_oracles = if probe_shape.specific {
        probes::required_oracles(line_text, &family)
    } else {
        Vec::new()
    };
    let probe = Probe {
        id: ProbeId(format!("probe:{id_path}:{line}:typescript_preview")),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(owner.symbol_id()),
        family: family.clone(),
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks,
        required_oracles,
    };
    let actionability = typescript_actionability_for(
        &class,
        static_limit.as_ref(),
        has_oracle_eligible_relation,
        &missing_discriminators,
    );
    missing.push(actionability.missing_summary());

    let related_count = related.len();
    let reach_summary = format!(
        "{} related test(s) found for owner `{}`",
        related_count, owner.name
    );
    let reach = StageEvidence::new(reach_state.clone(), Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model infection.",
    );
    let propagate = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model propagation.",
    );
    let observe_summary = format!(
        "Strongest extracted oracle kind: `{}` (rank {})",
        strongest_kind.as_str(),
        strongest_strength
    );
    let observe = StageEvidence::new(observe_state, Confidence::Low, &observe_summary);
    let discriminate_summary = if strongest_strength >= OracleStrength::Strong.rank() {
        format!(
            "Related test uses a `{}` oracle; static evidence suggests the changed behavior is discriminated.",
            strongest_kind.as_str()
        )
    } else if let Some(discriminator) = missing_discriminators.first() {
        format!(
            "TypeScript preview adapter found no strong discriminator; missing proof: `{}`.",
            discriminator.value
        )
    } else {
        "TypeScript preview adapter found no strong discriminator; use `toBe` / `toEqual` / `toStrictEqual` to escalate. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, &discriminate_summary);

    let recommended = if let Some(limit) = &static_limit {
        format!(
            "TypeScript preview advisory: static limit `{}`; {}; no actionable repair packet is emitted.",
            limit.kind.as_str(),
            limit.repair_route
        )
    } else {
        match &class {
        ExposureClass::Exposed => {
            "TypeScript preview advisory: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            no_static_path_recommendation(owner)
        }
        _ if !has_oracle_eligible_relation => {
            "TypeScript preview advisory: related-test proximity is heuristic only; add a direct owner call before treating this as an actionable repair target.".to_string()
        }
        _ if let Some(discriminator) = missing_discriminators.first() => {
            weak_oracle_recommendation(
                &strongest_kind,
                &discriminator.value,
                mock_payload_oracle.as_deref(),
            )
        }
        _ if owner.owner_kind == OwnerKind::ModuleFunction => {
            format!(
                "TypeScript preview advisory: related module-initializer observer reaches `{}` but no safe target shape is available; add an exact value assertion for the exported value and keep the finding advisory until repair-card fields are complete.",
                owner.name
            )
        }
        _ => {
            "TypeScript preview advisory: add a test that exercises the changed behavior with an exact-value assertion (`toBe` / `toEqual` / `toStrictEqual`); no actionable repair packet is emitted until the target shape is explicit.".to_string()
        }
        }
    };
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else {
        0.4
    };

    let mut evidence = vec![format!("owner: {}", owner.name)];
    if !probe_shape.specific {
        evidence.push("probe_fact: ambiguous_fallback".to_string());
    }
    for discriminator in &missing_discriminators {
        evidence.push(format!("missing_discriminator: {}", discriminator.value));
    }
    if let Some(oracle) = &mock_payload_oracle {
        evidence.push(format!("mock_payload_evidence: {oracle}"));
    }
    if let Some(limit) = &static_limit {
        evidence.extend(limit.evidence.iter().cloned());
    }
    evidence.extend(actionability.evidence(typescript_raw_evidence_ref(
        file,
        line,
        Some(owner),
        &probe.id.0,
    )));
    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.is_uncertain())
    {
        evidence.push(format!(
            "related_test_relation: {} ({})",
            candidate.relation.as_str(),
            candidate.test.name
        ));
        evidence.push(format!(
            "related_test_uncertain: {} ({})",
            candidate.relation.as_str(),
            candidate.test.name
        ));
    }
    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
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
        confidence: confidence_value,
        evidence,
        missing,
        flow_sinks: flow_sink.into_iter().collect(),
        activation: ActivationEvidence {
            observed_values: Vec::new(),
            missing_discriminators,
        },
        stop_reasons: Vec::new(),
        related_tests: related,
        recommended_next_step: Some(recommended),
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: Some(owner.owner_kind),
        static_limit_kind: static_limit.map(|limit| limit.kind),
    })
}

fn no_static_path_missing(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method => format!(
            "No trusted TypeScript method receiver relation for `{}`. Direct `new ClassName(...)` receiver calls are supported, but factories, dependency injection, mocked modules, prototype aliases, and dynamic property access stay ambiguous in preview.",
            owner.name
        ),
        OwnerKind::ClassMethod => format!(
            "No trusted TypeScript class-method relation for `{}`. Static class calls stay missing context until class-method related-test matching lands.",
            owner.name
        ),
        OwnerKind::ModuleFunction => format!(
            "No trusted TypeScript module-initializer observer for `{}`. Direct `expect(IMPORTED_CONST)...` and `expect(namespace.EXPORT)...` observers are supported, but helper-derived values, shadowed aliases, dynamic initialization, and non-expect references stay advisory in preview.",
            owner.name
        ),
        _ => format!(
            "No test references `{}(` — add a test that calls the changed owner.",
            owner.name
        ),
    }
}

fn no_static_path_recommendation(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method => {
            "TypeScript preview advisory: method receiver relation is missing context; use a direct `new ClassName(...)` receiver observer when safe, and keep factories, dependency injection, mocked modules, prototype aliases, and dynamic property access advisory.".to_string()
        }
        OwnerKind::ClassMethod => {
            "TypeScript preview advisory: class-method relation is missing context; keep the finding advisory until class-method related-test matching lands.".to_string()
        }
        OwnerKind::ModuleFunction => {
            "TypeScript preview advisory: module initializer observer is missing context; add a direct `expect(IMPORTED_CONST).toBe(...)` or `expect(namespace.EXPORT).toEqual(...)` observer when safe, and keep helper-derived or dynamic initialization evidence advisory.".to_string()
        }
        _ => {
            "TypeScript preview advisory: no test references the changed owner; add a test that calls the owner and asserts the changed behavior with `toBe` / `toEqual` before any repair packet is emitted.".to_string()
        }
    }
}

fn output_language_for(path: &Path) -> DomainLanguageId {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("js" | "jsx") => DomainLanguageId::JavaScript,
        _ => DomainLanguageId::TypeScript,
    }
}

fn parse_limit_for_file<'a>(
    file: &Path,
    limits: &'a [TypeScriptParseLimit],
) -> Option<&'a TypeScriptParseLimit> {
    let changed_file = normalized_path(file);
    limits
        .iter()
        .find(|limit| normalized_path(&limit.file) == changed_file)
}

fn unsupported_syntax_finding(
    file: &Path,
    line: usize,
    line_text: &str,
    limit: &TypeScriptParseLimit,
) -> Finding {
    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let probe = Probe {
        id: ProbeId(format!(
            "probe:{id_path}:{line}:typescript_preview_unsupported_syntax"
        )),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: None,
        family: ProbeFamily::StaticUnknown,
        delta: DeltaKind::Unknown,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };
    let summary = format!(
        "TypeScript preview parser could not build syntax facts for `{}`: {}",
        normalized_path(file),
        limit.reason
    );
    let stage = StageEvidence::new(StageState::Unknown, Confidence::Low, &summary);
    let missing = format!(
        "Static limit `unsupported_syntax`: malformed TypeScript/JavaScript prevented syntax-first owner, test, and probe extraction for `{}`. Repair route: fix or isolate the unsupported syntax before relying on repair guidance.",
        normalized_path(file)
    );
    let why_not_actionable = format!(
        "static limit `unsupported_syntax` prevents bounded TypeScript repair guidance: {}",
        limit.reason
    );
    let repair_route =
        "fix or isolate the unsupported syntax before relying on repair guidance".to_string();
    let recommended = "TypeScript preview advisory: static limit `unsupported_syntax`; Repair route: fix or isolate the unsupported syntax before relying on repair guidance; no actionable repair packet is emitted.".to_string();

    Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
        probe,
        class: ExposureClass::StaticUnknown,
        ripr: RiprEvidence {
            reach: stage.clone(),
            infect: stage.clone(),
            propagate: stage.clone(),
            reveal: RevealEvidence {
                observe: stage.clone(),
                discriminate: stage,
            },
        },
        confidence: 0.2,
        evidence: vec![
            format!("static_limit unsupported_syntax: {}", limit.reason),
            "gap_state: static_limitation".to_string(),
            "actionability_category: unsupported_syntax".to_string(),
            format!("why_not_actionable: {why_not_actionable}"),
            format!("repair_route: {repair_route}"),
            "evidence_needed_to_promote: resolve the named static limit and re-run TypeScript preview evidence extraction".to_string(),
            typescript_raw_evidence_ref(
                file,
                line,
                None,
                &format!("probe:{id_path}:{line}:typescript_preview_unsupported_syntax"),
            ),
        ],
        missing: vec![
            missing,
            format!(
                "TypeScript preview actionability `static_limitation` / `unsupported_syntax`: {why_not_actionable}. Repair route: {repair_route}"
            ),
        ],
        flow_sinks: Vec::new(),
        activation: Default::default(),
        stop_reasons: vec![StopReason::StaticProbeUnknown],
        related_tests: Vec::new(),
        recommended_next_step: Some(recommended),
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: None,
        static_limit_kind: Some(StaticLimitKind::UnsupportedSyntax),
    }
}

fn collect_workspace_typescript_files(root: &Path) -> Vec<PathBuf> {
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
            let adapter = TypeScriptAdapter;
            if adapter.accepts_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
}

impl LanguageAdapter for TypeScriptAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::TypeScript))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        // Phase 1: discover and index every accepted file in the workspace
        // so we can find related tests for any owner regardless of whether
        // the test file itself changed in this diff.
        let workspace_files = collect_workspace_typescript_files(&options.root);
        let mut all_owners: Vec<TypeScriptOwner> = Vec::new();
        let mut all_tests: Vec<TypeScriptTest> = Vec::new();
        let mut parse_limits: Vec<TypeScriptParseLimit> = Vec::new();
        for relative in &workspace_files {
            let absolute = options.root.join(relative);
            let Ok(source) = std::fs::read_to_string(&absolute) else {
                continue;
            };
            if let Some(reason) = parse_error_reason(relative, &source) {
                if !is_test_file(relative) {
                    parse_limits.push(TypeScriptParseLimit {
                        file: relative.clone(),
                        reason,
                    });
                }
                continue;
            }
            if is_test_file(relative) {
                all_tests.extend(extract_tests(relative, &source));
            } else {
                all_owners.extend(extract_owners(relative, &source));
            }
        }

        // Phase 2: for each accepted changed file, classify each changed
        // line that falls inside an owner.
        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        for changed in changed_files {
            if !self.accepts_path(&changed.path) {
                continue;
            }
            changed_count += 1;
            // Skip test-file changes for finding generation; classifier
            // operates on production owners. Test file edits are still
            // counted in the file tally.
            if is_test_file(&changed.path) {
                continue;
            }
            if let Some(limit) = parse_limit_for_file(&changed.path, &parse_limits) {
                if let Some(added) = changed.added_lines.first() {
                    findings.push(unsupported_syntax_finding(
                        &changed.path,
                        added.line,
                        &added.text,
                        limit,
                    ));
                }
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

    fn test_owner(name: &str, file: &str) -> TypeScriptOwner {
        TypeScriptOwner {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 20,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        }
    }

    fn smoke_assertion() -> TypeScriptAssertion {
        TypeScriptAssertion {
            matcher: "toBeTruthy".to_string(),
            argument_count: 0,
            line: 2,
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Smoke,
            mock_payload: None,
            error_payload: None,
        }
    }

    fn weak_direct_test_for(owner_name: &str) -> TypeScriptTest {
        TypeScriptTest {
            name: format!("{owner_name} smoke"),
            local_name: format!("{owner_name} smoke"),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: format!(
                "const result = {owner_name}(50, 100);\nexpect(result).toBeTruthy();"
            ),
            assertions: vec![smoke_assertion()],
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }
    }

    fn mock_interaction_test_for(owner_name: &str) -> TypeScriptTest {
        TypeScriptTest {
            name: format!("{owner_name} records status"),
            local_name: format!("{owner_name} records status"),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: format!(
                "const sink = {{ record: vi.fn() }};\n{owner_name}(status, sink);\nexpect(sink.record).toHaveBeenCalledWith(status);"
            ),
            assertions: vec![TypeScriptAssertion {
                matcher: "toHaveBeenCalledWith".to_string(),
                argument_count: 1,
                line: 3,
                oracle_kind: OracleKind::MockExpectation,
                oracle_strength: OracleStrength::Medium,
                mock_payload: None,
                error_payload: None,
            }],
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }
    }

    fn direct_test_with_assertion(
        test_name: &str,
        body_text: impl Into<String>,
        matcher: &str,
        argument_count: usize,
        oracle_kind: OracleKind,
        oracle_strength: OracleStrength,
    ) -> TypeScriptTest {
        TypeScriptTest {
            name: test_name.to_string(),
            local_name: test_name.to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: body_text.into(),
            assertions: vec![TypeScriptAssertion {
                matcher: matcher.to_string(),
                argument_count,
                line: 2,
                oracle_kind,
                oracle_strength,
                mock_payload: None,
                error_payload: None,
            }],
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }
    }

    fn heuristic_name_test_for(owner_name: &str) -> TypeScriptTest {
        TypeScriptTest {
            name: format!("{owner_name} boundary"),
            local_name: format!("{owner_name} boundary"),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "expect(90).toBe(90);".to_string(),
            assertions: vec![TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 1,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
            }],
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }
    }

    fn classify_weak_direct_line(line_text: &str) -> Result<Finding, String> {
        let owner = test_owner("applyDiscount", "src/lib.ts");
        let test = weak_direct_test_for("applyDiscount");
        classify_change(Path::new("src/lib.ts"), 2, line_text, &[owner], &[test])
            .ok_or_else(|| "expected TypeScript preview finding".to_string())
    }

    fn missing_discriminator_values(finding: &Finding) -> Vec<String> {
        finding
            .activation
            .missing_discriminators
            .iter()
            .map(|fact| fact.value.clone())
            .collect()
    }

    fn assert_static_limit(finding: &Finding, kind: StaticLimitKind, expected_text: &str) {
        assert_eq!(finding.static_limit_kind, Some(kind));
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.contains(expected_text)),
            "expected evidence containing {expected_text:?}, got {:?}",
            finding.evidence
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains(expected_text)),
            "expected missing text containing {expected_text:?}, got {:?}",
            finding.missing
        );
        let recommended = finding.recommended_next_step.as_deref().unwrap_or_default();
        assert!(
            recommended.contains(expected_text) && recommended.contains("Repair route:"),
            "expected limitation-oriented next step for {expected_text:?}, got {recommended:?}"
        );
        assert!(finding.activation.missing_discriminators.is_empty());
        assert_evidence_contains(finding, "gap_state: static_limitation");
        assert_evidence_contains(
            finding,
            &format!("actionability_category: {}", kind.as_str()),
        );
        assert_evidence_contains(finding, "why_not_actionable: static limit");
    }

    fn assert_evidence_contains(finding: &Finding, expected_text: &str) {
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.contains(expected_text)),
            "expected evidence containing {expected_text:?}, got {:?}",
            finding.evidence
        );
    }

    #[test]
    fn accepts_ts_jsx_paths() {
        let adapter = TypeScriptAdapter;
        assert!(adapter.accepts_path(Path::new("src/index.ts")));
        assert!(adapter.accepts_path(Path::new("src/component.tsx")));
        assert!(adapter.accepts_path(Path::new("src/index.js")));
        assert!(adapter.accepts_path(Path::new("src/component.jsx")));
        assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(!adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
    }

    #[test]
    fn extract_owners_returns_empty_when_source_does_not_parse() {
        let owners = extract_owners(
            Path::new("src/index.ts"),
            "this is not :: valid +++ typescript",
        );
        assert!(owners.is_empty());
    }

    #[test]
    fn parse_error_reason_reports_parser_errors() {
        let reason = parse_error_reason(
            Path::new("src/index.ts"),
            "this is not :: valid +++ typescript",
        );
        assert!(reason.is_some());
        let reason = reason.unwrap_or_default();
        assert!(reason.contains("parser error"));
    }

    #[test]
    fn unsupported_syntax_finding_is_preview_static_unknown() {
        let limit = TypeScriptParseLimit {
            file: PathBuf::from("src/index.ts"),
            reason: "1 parser error(s)".to_string(),
        };
        let finding =
            unsupported_syntax_finding(Path::new("src/index.ts"), 3, "  const value = ;", &limit);

        assert!(matches!(finding.class, ExposureClass::StaticUnknown));
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::UnsupportedSyntax)
        );
        assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
        assert_evidence_contains(
            &finding,
            "evidence_needed_to_promote: resolve the named static limit and re-run TypeScript preview evidence extraction",
        );
    }

    #[test]
    fn is_test_file_matches_test_and_spec_suffixes() {
        assert!(is_test_file(Path::new("tests/lib.test.ts")));
        assert!(is_test_file(Path::new("src/Header.spec.tsx")));
        assert!(is_test_file(Path::new("legacy.test.js")));
        assert!(!is_test_file(Path::new("src/lib.ts")));
        assert!(!is_test_file(Path::new("README.md")));
    }

    #[test]
    fn line_for_offset_counts_newlines() {
        let source = "line1\nline2\nline3\n";
        assert_eq!(line_for_offset(source, 0), 1);
        assert_eq!(line_for_offset(source, 5), 1);
        assert_eq!(line_for_offset(source, 6), 2);
        assert_eq!(line_for_offset(source, 12), 3);
    }

    #[test]
    fn normalized_path_strips_dot_prefix_and_normalizes_separators() {
        assert_eq!(normalized_path(Path::new(r".\src\b.ts")), "src/b.ts");
    }

    #[test]
    fn extract_owners_recognizes_function_declaration() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            "function applyDiscount(amount: number): number {\n    return amount;\n}\n",
        );
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].name, "applyDiscount");
        assert_eq!(owners[0].start_line, 1);
        assert_eq!(owners[0].owner_kind, OwnerKind::Function);
    }

    #[test]
    fn extract_owners_recognizes_exported_function() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            "export function publicHelper(): void {}\n",
        );
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].name, "publicHelper");
        assert_eq!(owners[0].owner_kind, OwnerKind::Function);
    }

    #[test]
    fn extract_owners_recognizes_arrow_const_and_module_initializer() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            r#"const formatPrice = (amount: number) => {
    return amount.toFixed(2);
};
const defaultRate = 0.08;
"#,
        );
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0].name, "formatPrice");
        assert_eq!(owners[0].owner_kind, OwnerKind::ArrowFunction);
        assert_eq!(owners[0].start_line, 1);
        assert_eq!(owners[0].end_line, 3);
        assert_eq!(owners[1].name, "defaultRate");
        assert_eq!(owners[1].owner_kind, OwnerKind::ModuleFunction);
        assert_eq!(owners[1].start_line, 4);
    }

    #[test]
    fn extract_owners_recognizes_class_methods() {
        let owners = extract_owners(
            Path::new("src/cart.ts"),
            r#"class Cart {
    total() {
        return 1;
    }
    static build() {
        return new Cart();
    }
}
"#,
        );
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0].name, "total");
        assert_eq!(owners[0].owner_kind, OwnerKind::Method);
        assert_eq!(owners[0].start_line, 2);
        assert_eq!(owners[1].name, "build");
        assert_eq!(owners[1].owner_kind, OwnerKind::ClassMethod);
        assert_eq!(owners[1].start_line, 5);
    }

    #[test]
    fn extract_owners_recognizes_default_function_and_class_methods() {
        let function_owners = extract_owners(
            Path::new("src/defaults.ts"),
            r#"export default function calculate(value: number) {
    return value + 1;
}
"#,
        );
        let class_owners = extract_owners(
            Path::new("src/default-class.ts"),
            r#"
export default class Formatter {
    render() {
        return "ok";
    }
}
"#,
        );
        assert_eq!(function_owners.len(), 1);
        assert_eq!(function_owners[0].name, "calculate");
        assert_eq!(function_owners[0].owner_kind, OwnerKind::Function);
        assert_eq!(class_owners.len(), 1);
        assert_eq!(class_owners[0].name, "render");
        assert_eq!(class_owners[0].owner_kind, OwnerKind::Method);
    }

    #[test]
    fn extract_owners_recognizes_reactish_function_and_arrow_components() {
        let owners = extract_owners(
            Path::new("src/card.tsx"),
            r#"export function PriceTag() {
    return <span>price</span>;
}
const InlinePrice = () => (
    <span>price</span>
);
"#,
        );
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0].name, "PriceTag");
        assert_eq!(owners[0].owner_kind, OwnerKind::Component);
        assert_eq!(owners[1].name, "InlinePrice");
        assert_eq!(owners[1].owner_kind, OwnerKind::Component);
    }

    #[test]
    fn extract_owners_does_not_create_owner_from_comments_or_strings() {
        let owners = extract_owners(
            Path::new("src/docs.ts"),
            r#"// function fakeOwner() {}
"function stringOwner() {}";
"#,
        );
        assert!(owners.is_empty());
    }

    #[test]
    fn extract_tests_recognizes_test_and_it_blocks() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("alpha", () => { expect(applyDiscount(50, 100)).toBe(50); });
it("beta", () => { expect(otherHelper()).toBe(true); });
"#,
        );
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].name, "alpha");
        assert_eq!(tests[1].name, "beta");
        assert!(tests[0].body_text.contains("applyDiscount(50, 100)"));
    }

    #[test]
    fn find_related_tests_matches_by_call_name() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![
            TypeScriptTest {
                name: "alpha".to_string(),
                local_name: "alpha".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 1,
                body_text: r#"test("alpha", () => { expect(applyDiscount(50, 100)).toBe(50); });"#
                    .to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "unrelated".to_string(),
                local_name: "unrelated".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/other.test.ts"),
                line: 1,
                body_text: r#"test("unrelated", () => { expect(otherHelper()).toBe(true); });"#
                    .to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
        ];
        let related = find_related_tests(&owner, &tests);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "alpha");
    }

    #[test]
    fn find_related_tests_ignores_object_method_calls_for_function_owners() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![TypeScriptTest {
            name: "method call on another object".to_string(),
            local_name: "method call on another object".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/cart.test.ts"),
            line: 1,
            body_text: "expect(order.applyDiscount(50)).toBe(40);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }];

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_matches_bounded_method_receiver_calls() {
        let owner = TypeScriptOwner {
            name: "total".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 5,
            end_line: 8,
            owner_kind: OwnerKind::Method,
            class_name: Some("Cart".to_string()),
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import { Cart as Subject } from "../src/owners";

test("cart total observes receiver", () => {
    const cart = new Subject();
    expect(cart.total()).toBe(1);
});
"#,
        );

        let candidates = related_test_candidates(&owner, &tests);
        let related = find_related_tests(&owner, &tests);

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].relation,
            TypeScriptRelationKind::ReceiverOwnerCall
        );
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "cart total observes receiver");
        assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
    }

    #[test]
    fn find_related_tests_keeps_factory_receiver_calls_unrelated_for_method_owners() {
        let owner = TypeScriptOwner {
            name: "total".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 5,
            end_line: 8,
            owner_kind: OwnerKind::Method,
            class_name: Some("Cart".to_string()),
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import { Cart } from "../src/owners";

test("cart total through factory stays ambiguous", () => {
    const cart = makeCart();
    expect(cart.total()).toBe(1);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_keeps_dynamic_method_receiver_calls_unrelated() {
        let owner = TypeScriptOwner {
            name: "total".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 5,
            end_line: 8,
            owner_kind: OwnerKind::Method,
            class_name: Some("Cart".to_string()),
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import { Cart } from "../src/owners";

test("cart total through dynamic method stays ambiguous", () => {
    const cart = new Cart();
    const method = "total";
    expect(cart[method]()).toBe(1);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_keeps_mocked_method_receiver_calls_unrelated() {
        let owner = TypeScriptOwner {
            name: "total".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 5,
            end_line: 8,
            owner_kind: OwnerKind::Method,
            class_name: Some("Cart".to_string()),
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import { Cart } from "../src/owners";

vi.mock("../src/owners");

test("mocked cart total stays ambiguous", () => {
    const cart = new Cart();
    expect(cart.total()).toBe(1);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_matches_module_initializer_named_import_observer() {
        let owner = TypeScriptOwner {
            name: "DEFAULT_RATE".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 15,
            end_line: 15,
            owner_kind: OwnerKind::ModuleFunction,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import { DEFAULT_RATE as rate } from "../src/owners";

test("rate value observes initializer", () => {
    expect(rate).toBe(0.09);
});
"#,
        );

        let candidates = related_test_candidates(&owner, &tests);
        let related = find_related_tests(&owner, &tests);

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].relation,
            TypeScriptRelationKind::ModuleValueReference
        );
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "rate value observes initializer");
        assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
    }

    #[test]
    fn find_related_tests_matches_module_initializer_namespace_observer() {
        let owner = TypeScriptOwner {
            name: "DEFAULT_RATE".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 15,
            end_line: 15,
            owner_kind: OwnerKind::ModuleFunction,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import * as owners from "../src/owners";

test("rate value observes namespace initializer", () => {
    expect(owners.DEFAULT_RATE).toBe(0.09);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "rate value observes namespace initializer");
        assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn find_related_tests_keeps_module_initializer_shadow_and_non_expect_references_unrelated() {
        let owner = TypeScriptOwner {
            name: "DEFAULT_RATE".to_string(),
            file: PathBuf::from("src/owners.ts"),
            start_line: 15,
            end_line: 15,
            owner_kind: OwnerKind::ModuleFunction,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/owners.test.ts"),
            r#"import { DEFAULT_RATE } from "../src/owners";

test("shadowed rate stays ambiguous", () => {
    const DEFAULT_RATE = 0.1;
    expect(DEFAULT_RATE).toBe(0.1);
});

test("derived rate stays ambiguous", () => {
    const actual = DEFAULT_RATE;
    expect(actual).toBe(0.09);
});

test("string mention stays ambiguous", () => {
    expect("DEFAULT_RATE").toBe("DEFAULT_RATE");
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_matches_named_import_alias_calls() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"import { applyDiscount as subject } from "../src/pricing";

test("alias import observes threshold", () => {
    expect(subject(100, 100)).toBe(90);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "alias import observes threshold");
    }

    #[test]
    fn find_related_tests_matches_namespace_import_member_calls() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"import * as pricing from "../src/pricing";

test("namespace import observes threshold", () => {
    expect(pricing.applyDiscount(100, 100)).toBe(90);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "namespace import observes threshold");
    }

    #[test]
    fn find_related_tests_ignores_unrelated_and_type_only_import_aliases() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"import { applyDiscount as otherSubject } from "../src/other-pricing";
import type { applyDiscount as typeOnlySubject } from "../src/pricing";
import { applyDiscount } from "../src/other-pricing";

test("wrong import source", () => {
    expect(otherSubject(100, 100)).toBe(90);
});

test("wrong direct import source", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});

test("type only import", () => {
    expect(typeOnlySubject(100, 100)).toBe(90);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_ignores_call_shaped_string_mentions() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![TypeScriptTest {
            name: "string mention".to_string(),
            local_name: "string mention".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/docs.test.ts"),
            line: 1,
            body_text: r#"expect("applyDiscount(").toContain("applyDiscount(");"#.to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }];

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_ignores_call_shaped_comment_mentions() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![
            TypeScriptTest {
                name: "line comment mention".to_string(),
                local_name: "line comment mention".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/docs.test.ts"),
                line: 1,
                body_text: "// applyDiscount(\nexpect(total).toBe(40);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "block comment mention".to_string(),
                local_name: "block comment mention".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/docs.test.ts"),
                line: 4,
                body_text: "/* applyDiscount(\n */\nexpect(total).toBe(40);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
        ];

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn related_test_candidates_use_name_and_proximity_links_as_uncertain_relations() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let mut tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"test("threshold documented elsewhere", () => {
    expect(90).toBe(90);
});
"#,
        );
        tests.extend(extract_tests(
            Path::new("tests/checkout.test.ts"),
            r#"describe("applyDiscount", () => {
    test("threshold documented elsewhere", () => {
        expect(90).toBe(90);
    });
});
"#,
        ));
        tests.extend(extract_tests(
            Path::new("tests/cart.test.ts"),
            r#"test("applyDiscount boundary", () => {
    expect(90).toBe(90);
});
"#,
        ));

        let candidates = related_test_candidates(&owner, &tests);
        let relations: Vec<_> = candidates
            .iter()
            .map(|candidate| candidate.relation)
            .collect();

        assert_eq!(
            relations,
            vec![
                TypeScriptRelationKind::SameFileProximity,
                TypeScriptRelationKind::DescribeName,
                TypeScriptRelationKind::TestName,
            ]
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.relation.is_uncertain())
        );

        let related = find_related_tests(&owner, &tests);
        assert_eq!(related.len(), 3);
        assert!(
            related
                .iter()
                .all(|test| test.oracle_kind == OracleKind::Unknown)
        );
    }

    #[test]
    fn related_test_name_proximity_ignores_partial_tokens() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/checkout.test.ts"),
            r#"describe("application discounting", () => {
    test("discount boundary", () => {
        expect(90).toBe(90);
    });
});
"#,
        );

        let candidates = related_test_candidates(&owner, &tests);

        assert!(candidates.is_empty());
    }

    #[test]
    fn classify_change_uses_heuristic_links_as_weak_uncertain_proximity() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"test("threshold documented elsewhere", () => {
    expect(90).toBe(90);
});
"#,
        );

        let finding = classify_change(
            Path::new("src/pricing.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &tests,
        )
        .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.ripr.reach.state, StageState::Weak);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Unknown);
        assert!(finding.evidence.iter().any(|item| item
            == "related_test_relation: same_file_proximity (threshold documented elsewhere)"));
        assert!(finding.evidence.iter().any(|item| item
            == "related_test_uncertain: same_file_proximity (threshold documented elsewhere)"));
        assert!(
            finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("heuristic only"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            local_name: "alpha".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;
        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.related_tests.len(), 1);
        Ok(())
    }

    #[test]
    fn classify_change_marks_weak_direct_typescript_candidate_advisory() -> Result<(), String> {
        let finding = classify_weak_direct_line("    if (amount >= threshold) {")?;

        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert_evidence_contains(&finding, "actionability_category: incomplete_repair_packet");
        assert_evidence_contains(
            &finding,
            "why_not_actionable: TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract",
        );
        assert_evidence_contains(&finding, "missing_actionability_fields: canonical_gap_id");
        assert_evidence_contains(&finding, "verify_command");
        assert_evidence_contains(&finding, "receipt_command");
        assert_evidence_contains(
            &finding,
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe",
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains("incomplete_repair_packet")),
            "expected actionability summary in missing text, got {:?}",
            finding.missing
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains("smoke-only oracle")),
            "expected weak smoke oracle guidance, got {:?}",
            finding.missing
        );
        assert!(
            finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("smoke-only assertion")
                    && step.contains("no actionable repair packet is emitted"))
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_weak_oracle_guidance_names_snapshot_exact_value_shape()
    -> Result<(), String> {
        let owner = test_owner("renderSummary", "src/lib.ts");
        let test = direct_test_with_assertion(
            "renders summary snapshot",
            "const value = renderSummary(status);\nexpect(value).toMatchSnapshot();",
            "toMatchSnapshot",
            0,
            OracleKind::Snapshot,
            OracleStrength::Medium,
        );
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    return `summary:${status.trim()}`;",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Snapshot);
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert!(
            finding.missing.iter().any(|line| {
                line.contains("snapshot evidence") && line.contains("add an exact-value assertion")
            }),
            "expected snapshot exact-value guidance, got {:?}",
            finding.missing
        );
        let recommended = finding
            .recommended_next_step
            .as_deref()
            .ok_or_else(|| "expected recommended next step".to_string())?;
        assert!(
            recommended.contains("add an exact-value assertion alongside the snapshot")
                && recommended.contains("no actionable repair packet is emitted"),
            "expected snapshot advisory recommendation, got {recommended:?}"
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_weak_oracle_guidance_names_smoke_exact_value_shape() -> Result<(), String>
    {
        let finding = classify_weak_direct_line("    return count >= 1;")?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::SmokeOnly);
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert!(
            finding.missing.iter().any(|line| {
                line.contains("smoke-only oracle") && line.contains("exact-value assertion")
            }),
            "expected smoke-only exact-value guidance, got {:?}",
            finding.missing
        );
        let recommended = finding
            .recommended_next_step
            .as_deref()
            .ok_or_else(|| "expected recommended next step".to_string())?;
        assert!(
            recommended.contains("replace or augment the smoke-only assertion")
                && recommended.contains("no actionable repair packet is emitted"),
            "expected smoke-only advisory recommendation, got {recommended:?}"
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_weak_oracle_guidance_keeps_broad_error_advisory() -> Result<(), String> {
        let owner = test_owner("parseUser", "src/lib.ts");
        let test = direct_test_with_assertion(
            "rejects empty user broadly",
            "expect(() => parseUser('')).toThrow();",
            "toThrow",
            0,
            OracleKind::BroadError,
            OracleStrength::Weak,
        );
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    throw new Error(\"empty user\");",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::BroadError);
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains("broad error evidence") && line.contains("keep it weak")),
            "expected broad-error advisory guidance, got {:?}",
            finding.missing
        );
        let recommended = finding
            .recommended_next_step
            .as_deref()
            .ok_or_else(|| "expected recommended next step".to_string())?;
        assert!(
            recommended.contains("broad error evidence does not establish missing discriminator")
                && recommended.contains("no actionable repair packet is emitted"),
            "expected broad-error advisory recommendation, got {recommended:?}"
        );
        assert!(
            !recommended.contains("exact-value assertion"),
            "broad error preview guidance should not ask for an exact-value assertion: {recommended:?}"
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_weak_oracle_guidance_distinguishes_mock_payload_limits()
    -> Result<(), String> {
        let owner = test_owner("notifyStatus", "src/lib.ts");
        let test = mock_interaction_test_for("notifyStatus");
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    sink.record(status);",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(
            finding.related_tests[0].oracle_kind,
            OracleKind::MockExpectation
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Medium
        );
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert_evidence_contains(&finding, "actionability_category: incomplete_repair_packet");
        assert!(
            finding.missing.iter().any(|line| line.contains(
                "mock interaction oracle, but TypeScript preview does not yet establish the changed call payload"
            )),
            "expected mock-payload limitation in missing text, got {:?}",
            finding.missing
        );
        let recommended = finding
            .recommended_next_step
            .as_deref()
            .ok_or_else(|| "expected recommended next step".to_string())?;
        assert!(
            recommended.contains("mock payloads are not yet a safe discriminator"),
            "expected mock-payload recommendation, got {recommended:?}"
        );
        assert!(
            !recommended.contains("exact-value assertion"),
            "mock interaction preview guidance should not ask for an exact-value assertion: {recommended:?}"
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_mock_payload_guidance_names_literal_payload_without_repair_packet()
    -> Result<(), String> {
        let owner = test_owner("notifyReady", "src/lib.ts");
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("records ready status", () => {
    const sink = { record: vi.fn() };
    notifyReady(sink);
    expect(sink.record).toHaveBeenCalledWith("ready");
});
"#,
        );
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    sink.record(\"ready\");",
            &[owner],
            &tests,
        )
        .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(
            finding.related_tests[0].oracle_kind,
            OracleKind::MockExpectation
        );
        assert_eq!(
            finding.related_tests[0].oracle.as_deref(),
            Some("expect(sink.record).toHaveBeenCalledWith(\"ready\")")
        );
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert_evidence_contains(
            &finding,
            "mock_payload_evidence: expect(sink.record).toHaveBeenCalledWith(\"ready\")",
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains("bounded mock payload evidence")
                    && line.contains("expect(sink.record).toHaveBeenCalledWith(\"ready\")")),
            "expected bounded mock-payload guidance, got {:?}",
            finding.missing
        );
        let recommended = finding
            .recommended_next_step
            .as_deref()
            .ok_or_else(|| "expected recommended next step".to_string())?;
        assert!(
            recommended.contains("related mock payload evidence")
                && recommended.contains("syntax-bounded")
                && recommended.contains("no actionable repair packet is emitted"),
            "expected advisory mock-payload recommendation, got {recommended:?}"
        );
        assert!(
            !recommended.contains("exact-value assertion"),
            "mock payload preview guidance should not ask for an exact-value assertion: {recommended:?}"
        );
        Ok(())
    }

    #[test]
    fn classify_change_labels_javascript_sources_separately() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.js"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            local_name: "alpha".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.js"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        };

        let finding = classify_change(
            Path::new("src/lib.js"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected a JavaScript preview finding".to_string())?;

        assert_eq!(finding.language, Some(DomainLanguageId::JavaScript));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        Ok(())
    }

    #[test]
    fn classify_change_matches_owner_file_before_line_range() -> Result<(), String> {
        let owners = vec![
            TypeScriptOwner {
                name: "alphaScore".to_string(),
                file: PathBuf::from("src/a.ts"),
                start_line: 1,
                end_line: 5,
                owner_kind: OwnerKind::Function,
                class_name: None,
                decorated: false,
                imports: Vec::new(),
            },
            TypeScriptOwner {
                name: "betaScore".to_string(),
                file: PathBuf::from("src/b.ts"),
                start_line: 1,
                end_line: 5,
                owner_kind: OwnerKind::Function,
                class_name: None,
                decorated: false,
                imports: Vec::new(),
            },
        ];
        let tests = vec![
            TypeScriptTest {
                name: "alpha keeps its threshold".to_string(),
                local_name: "alpha keeps its threshold".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/a.test.ts"),
                line: 1,
                body_text: "expect(alphaScore(12)).toBe(13);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "beta keeps its threshold".to_string(),
                local_name: "beta keeps its threshold".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/b.test.ts"),
                line: 1,
                body_text: "expect(betaScore(12)).toBe(13);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
        ];

        let finding = classify_change(
            Path::new("src/b.ts"),
            2,
            "    if (value >= 10) {",
            &owners,
            &tests,
        )
        .ok_or_else(|| "expected the changed file's owner to be selected".to_string())?;

        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry == "owner: betaScore")
        );
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].name, "beta keeps its threshold");
        assert_eq!(
            finding.related_tests[0].file,
            PathBuf::from("tests/b.test.ts")
        );
        assert!(finding.missing.iter().all(|line| !line.contains("alpha")));
        Ok(())
    }

    #[test]
    fn extract_tests_collects_expect_to_be_as_strong_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
    expect(applyDiscount(10000, 100)).toEqual(9990);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 2);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(tests[0].assertions[1].matcher, "toEqual");
    }

    #[test]
    fn extract_tests_recurses_nested_describe_blocks() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"describe("pricing", () => {
    describe("discounts", () => {
        it("pins threshold", () => {
            expect(applyDiscount(100, 100)).toStrictEqual(90);
        });
    });
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "pricing discounts pins threshold");
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toStrictEqual");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn extract_tests_recognizes_test_each_table_calls() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test.each([
    [100, 100, 90],
    [150, 100, 140],
])("discounts %#", (amount, threshold, expected) => {
    expect(applyDiscount(amount, threshold)).toBe(expected);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "discounts %#");
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert!(tests[0].body_text.contains("applyDiscount("));
    }

    #[test]
    fn extract_tests_recognizes_it_each_table_calls() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"it.each([
    ["ready"],
])("notifies %s", (status) => {
    const sink = { record: vi.fn() };
    notifyStatus(status, sink);
    expect(sink.record).toHaveBeenCalledWith(status);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "notifies %s");
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toHaveBeenCalledWith");
        assert_eq!(
            tests[0].assertions[0].oracle_kind,
            OracleKind::MockExpectation
        );
    }

    #[test]
    fn extract_tests_records_safe_mock_payload_shapes() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("mock payloads", () => {
    const sink = { record: vi.fn() };
    expect(sink.record).toHaveBeenCalledWith("ready");
    expect(sink.record).toHaveBeenCalledWith({ status: "ok" });
    expect(sink.record).toHaveBeenCalledTimes(1);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        let payloads: Vec<Option<String>> = tests[0]
            .assertions
            .iter()
            .map(|assertion| {
                assertion
                    .mock_payload
                    .as_ref()
                    .map(TypeScriptMockPayload::oracle_text)
            })
            .collect();
        assert_eq!(
            payloads,
            vec![
                Some("expect(sink.record).toHaveBeenCalledWith(\"ready\")".to_string()),
                Some("expect(sink.record).toHaveBeenCalledWith({ status: \"ok\" })".to_string()),
                Some("expect(sink.record).toHaveBeenCalledTimes(1)".to_string()),
            ]
        );
    }

    #[test]
    fn extract_tests_keeps_ambiguous_mock_payload_shapes_unbounded() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("mock payloads", () => {
    expect(sink.record).toHaveBeenCalledWith(status);
    expect(sink.record).toHaveBeenCalledWith({ status });
    expect(sink.record).toHaveBeenCalledWith(...args);
    expect(sink.record).toHaveBeenCalledWith("ready", "extra");
    expect(sink[method]).toHaveBeenCalledWith("ready");
    expect(getSink()).toHaveBeenCalledTimes(1);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 6);
        assert!(
            tests[0]
                .assertions
                .iter()
                .all(|assertion| assertion.mock_payload.is_none()),
            "ambiguous mock payloads must stay unbounded: {:?}",
            tests[0].assertions
        );
    }

    #[test]
    fn extract_tests_recognizes_resolves_async_chain() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("async", async () => {
    await expect(loader()).resolves.toBe(42);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn extract_tests_recognizes_return_await_resolves_async_chain() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("async return", async () => {
    return await expect(loader()).resolves.toBe(42);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn extract_tests_collects_assertions_nested_in_control_flow() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("nested", () => {
    if (enabled) {
        expect(applyDiscount(50, 100)).toBe(50);
    } else {
        expect(applyDiscount(1, 100)).toEqual(1);
    }
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 2);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[1].matcher, "toEqual");
    }

    #[test]
    fn extract_tests_collects_assertions_nested_in_loop_switch_and_label_bodies() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("nested statements", () => {
    while (enabled) {
        expect(loopValue).toBe(1);
    }
    do {
        expect(done).toBeTruthy();
    } while (retry);
    for (let index = 0; index < items.length; index++) {
        expect(items[index]).toBeDefined();
    }
    for (const key in record) {
        expect(record[key]).toEqual("value");
    }
    for (const item of items) {
        expect(item).toBeDefined();
    }
    retry: {
        expect(labelled).toBe(false);
    }
    switch (kind) {
        case "a":
            expect(kind).toBe("a");
            break;
        default:
            expect(kind).toEqual("fallback");
    }
});
"#,
        );
        assert_eq!(tests.len(), 1);
        let matchers: Vec<&str> = tests[0]
            .assertions
            .iter()
            .map(|assertion| assertion.matcher.as_str())
            .collect();
        assert_eq!(
            matchers,
            vec![
                "toBe",
                "toBeTruthy",
                "toBeDefined",
                "toEqual",
                "toBeDefined",
                "toBe",
                "toBe",
                "toEqual"
            ]
        );
    }

    #[test]
    fn extract_tests_collects_assertions_nested_in_try_catch_finally() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("try-catch", () => {
    try {
        expect(parseUser("Ada")).toEqual({ name: "Ada" });
    } catch (err) {
        expect(err).toBeDefined();
    } finally {
        expect(cleanup).toHaveBeenCalled();
    }
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 3);
        assert_eq!(tests[0].assertions[0].matcher, "toEqual");
        assert_eq!(tests[0].assertions[1].matcher, "toBeDefined");
        assert_eq!(tests[0].assertions[2].matcher, "toHaveBeenCalled");
    }

    #[test]
    fn extract_tests_unknown_matcher_maps_to_unknown_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("alpha", () => {
    expect(applyDiscount(50, 100)).customDomainMatcher(50);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::Unknown);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Unknown
        );
    }

    #[test]
    fn extract_tests_maps_bare_tothrow_to_broad_error_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("throws", () => {
    expect(() => parseUser("")).toThrow();
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toThrow");
        assert_eq!(tests[0].assertions[0].argument_count, 0);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
        assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    }

    #[test]
    fn extract_tests_maps_literal_tothrow_to_exact_error_variant_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("throws", () => {
    expect(() => parseUser("")).toThrow("empty user");
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toThrow");
        assert_eq!(tests[0].assertions[0].argument_count, 1);
        assert_eq!(
            tests[0].assertions[0].oracle_kind,
            OracleKind::ExactErrorVariant
        );
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(
            tests[0].assertions[0]
                .error_payload
                .as_ref()
                .map(TypeScriptErrorPayload::oracle_text)
                .as_deref(),
            Some("expect(...).toThrow(\"empty user\")")
        );
    }

    #[test]
    fn extract_tests_keeps_dynamic_tothrow_payload_broad() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("throws", () => {
    expect(() => parseUser("")).toThrow(message);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toThrow");
        assert_eq!(tests[0].assertions[0].argument_count, 1);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
        assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
        assert!(tests[0].assertions[0].error_payload.is_none());
    }

    #[test]
    fn extract_tests_maps_rejects_tothrow_literal_to_exact_error_variant_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("rejects", async () => {
    await expect(loadProfile("")).rejects.toThrow("missing id");
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toThrow");
        assert_eq!(
            tests[0].assertions[0].oracle_kind,
            OracleKind::ExactErrorVariant
        );
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(
            tests[0].assertions[0]
                .error_payload
                .as_ref()
                .map(TypeScriptErrorPayload::oracle_text)
                .as_deref(),
            Some("await expect(...).rejects.toThrow(\"missing id\")")
        );
    }

    #[test]
    fn extract_tests_maps_rejects_match_object_literal_to_exact_error_variant_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("rejects", async () => {
    await expect(loadProfile("")).rejects.toMatchObject({ code: "E_AUTH" });
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toMatchObject");
        assert_eq!(
            tests[0].assertions[0].oracle_kind,
            OracleKind::ExactErrorVariant
        );
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(
            tests[0].assertions[0]
                .error_payload
                .as_ref()
                .map(TypeScriptErrorPayload::oracle_text)
                .as_deref(),
            Some("await expect(...).rejects.toMatchObject({ code: \"E_AUTH\" })")
        );
    }

    #[test]
    fn extract_tests_keeps_dynamic_rejects_match_object_unbounded() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("rejects", async () => {
    await expect(loadProfile("")).rejects.toMatchObject({ code });
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toMatchObject");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::Unknown);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Unknown
        );
        assert!(tests[0].assertions[0].error_payload.is_none());
    }

    #[test]
    fn oracle_for_matcher_covers_canonical_jest_vitest_set() {
        assert_eq!(
            oracle_for_matcher("toBe"),
            (OracleKind::ExactValue, OracleStrength::Strong)
        );
        assert_eq!(
            oracle_for_matcher("toEqual"),
            (OracleKind::ExactValue, OracleStrength::Strong)
        );
        assert_eq!(
            oracle_for_matcher("toThrow"),
            (OracleKind::BroadError, OracleStrength::Weak)
        );
        assert_eq!(
            oracle_for_matcher("toMatchSnapshot"),
            (OracleKind::Snapshot, OracleStrength::Medium)
        );
        assert_eq!(
            oracle_for_matcher("toHaveBeenCalledWith"),
            (OracleKind::MockExpectation, OracleStrength::Medium)
        );
        assert_eq!(
            oracle_for_matcher("toBeTruthy"),
            (OracleKind::SmokeOnly, OracleStrength::Smoke)
        );
        assert_eq!(
            oracle_for_matcher("toContain"),
            (OracleKind::RelationalCheck, OracleStrength::Weak)
        );
        assert_eq!(
            oracle_for_matcher("someUnknownMatcher"),
            (OracleKind::Unknown, OracleStrength::Unknown)
        );
    }

    #[test]
    fn classify_change_returns_exposed_when_related_test_has_strong_oracle() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            local_name: "alpha".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: vec![TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 2,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
            }],
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected a finding for the changed line".to_string())?;
        assert!(matches!(finding.class, ExposureClass::Exposed));
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: already_observed");
        assert_evidence_contains(&finding, "actionability_category: strong_oracle_observed");
        assert_evidence_contains(
            &finding,
            "why_not_actionable: related Jest/Vitest evidence already has a strong exact oracle",
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_no_static_path_when_no_related_test() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[],
        )
        .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;
        assert!(matches!(finding.class, ExposureClass::NoStaticPath));
        assert!(finding.related_tests.is_empty());
        assert!(finding.canonical_gap.is_none());
        assert_evidence_contains(&finding, "gap_state: advisory");
        assert_evidence_contains(&finding, "actionability_category: missing_context");
        assert_evidence_contains(&finding, "related_test_or_observer");
        Ok(())
    }

    #[test]
    fn classify_change_returns_none_when_line_is_outside_any_owner() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 10,
            end_line: 20,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            5,
            "// top-level comment",
            &[owner],
            &[],
        );
        assert!(finding.is_none());
    }

    #[test]
    fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
        let adapter = TypeScriptAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("/nonexistent_workspace"),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![
            changed("src/index.ts"),
            changed("src/lib.rs"),
            changed("docs/README.md"),
            changed("src/Header.tsx"),
        ];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        // No workspace files on disk -> no findings; counted-file tally
        // still reflects accepted changed paths.
        assert!(result.findings.is_empty());
        assert_eq!(result.changed_files, 2);
        Ok(())
    }

    #[test]
    fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
        let adapter = TypeScriptAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("/nonexistent_workspace"),
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

    #[test]
    fn classify_probe_shape_recognises_if_predicate() {
        let (family, delta) = classify_probe_shape("    if (amount >= threshold) {");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_else_if_predicate() {
        let (family, delta) = classify_probe_shape("    } else if (amount === 0) {");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_return_value() {
        let (family, delta) = classify_probe_shape("    return amount - 10;");
        assert_eq!(family, ProbeFamily::ReturnValue);
        assert_eq!(delta, DeltaKind::Value);
    }

    #[test]
    fn classify_probe_shape_recognises_bare_return() {
        let (family, delta) = classify_probe_shape("    return;");
        assert_eq!(family, ProbeFamily::ReturnValue);
        assert_eq!(delta, DeltaKind::Value);
    }

    #[test]
    fn classify_probe_shape_recognises_throw_error_path() {
        let (family, delta) = classify_probe_shape("    throw new Error('out of range');");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_promise_reject_error_path() {
        let (family, delta) = classify_probe_shape("    return Promise.reject(new Error('boom'));");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_return_await_promise_reject_error_path() {
        let (family, delta) =
            classify_probe_shape("    return await Promise.reject(new Error('boom'));");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_bare_await_promise_reject_error_path() {
        let (family, delta) = classify_probe_shape("    await Promise.reject(new Error('boom'));");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_field_construction() {
        let (family, delta) = classify_probe_shape("    this.count = next;");
        assert_eq!(family, ProbeFamily::FieldConstruction);
        assert_eq!(delta, DeltaKind::Value);
    }

    #[test]
    fn classify_probe_shape_recognises_side_effect_call() {
        let (family, delta) = classify_probe_shape("    logger.record(event);");
        assert_eq!(family, ProbeFamily::SideEffect);
        assert_eq!(delta, DeltaKind::Effect);
    }

    #[test]
    fn classify_probe_shape_recognises_await_side_effect_call() {
        let (family, delta) = classify_probe_shape("    await logger.flush();");
        assert_eq!(family, ProbeFamily::SideEffect);
        assert_eq!(delta, DeltaKind::Effect);
    }

    #[test]
    fn classify_probe_shape_recognises_ternary_as_predicate() {
        let (family, delta) =
            classify_probe_shape("    amount >= threshold ? amount - 10 : amount;");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_falls_through_to_predicate_default_for_const_decl() {
        // `const` declarations do not match a specific family in the
        // preview adapter; conservative fall-through keeps the historical
        // owner+test sub-slice default (#777) rather than guessing.
        let (family, delta) =
            classify_probe_shape("    const total = applyDiscount(amount, threshold);");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_change_emits_predicate_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    if (amount >= threshold) {")?;

        assert_eq!(finding.probe.family, ProbeFamily::Predicate);
        assert!(
            finding
                .probe
                .expected_sinks
                .contains(&"branch result".to_string())
        );
        assert!(
            finding
                .probe
                .required_oracles
                .contains(&"boundary input".to_string())
        );
        assert!(finding.flow_sinks.is_empty());
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["amount == threshold"]
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry == "missing_discriminator: amount == threshold")
        );
        Ok(())
    }

    #[test]
    fn classify_change_emits_return_value_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    return amount - discount;")?;

        assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["return value == amount - discount"]
        );
        assert_eq!(
            finding.activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ReturnValue)
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_return_value_discriminator_for_bare_return() -> Result<(), String> {
        let finding = classify_weak_direct_line("    return;")?;

        assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .evidence
                .iter()
                .all(|entry| !entry.starts_with("missing_discriminator:"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_emits_error_path_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    throw new RangeError(\"too low\");")?;

        assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["throws RangeError matching \"too low\""]
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_error_discriminator_for_generic_throw_identifier() -> Result<(), String>
    {
        let finding = classify_weak_direct_line("    throw err;")?;

        assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert!(finding.activation.missing_discriminators.is_empty());
        Ok(())
    }

    #[test]
    fn classify_change_omits_error_discriminator_for_generic_rejected_identifier()
    -> Result<(), String> {
        let finding = classify_weak_direct_line("    return Promise.reject(err);")?;

        assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert!(finding.activation.missing_discriminators.is_empty());
        Ok(())
    }

    #[test]
    fn classify_change_emits_field_construction_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    profile.status = nextStatus;")?;

        assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["profile.status == nextStatus"]
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_field_discriminator_for_computed_field_assignment()
    -> Result<(), String> {
        let finding = classify_weak_direct_line("    profile[key] = nextStatus;")?;

        assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
        assert!(finding.flow_sinks.is_empty());
        assert!(finding.activation.missing_discriminators.is_empty());
        Ok(())
    }

    #[test]
    fn classify_change_emits_object_literal_field_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    return { status: nextStatus, total };")?;

        assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["status == nextStatus"]
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_object_field_discriminator_for_computed_object_key()
    -> Result<(), String> {
        let finding = classify_weak_direct_line("    return { [key]: nextStatus, total };")?;

        assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
        assert!(finding.flow_sinks.is_empty());
        assert!(finding.activation.missing_discriminators.is_empty());
        Ok(())
    }

    #[test]
    fn classify_change_emits_call_side_effect_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    audit.record(status);")?;

        assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::CallEffect);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["call audit.record includes status"]
        );
        assert!(
            missing_discriminator_values(&finding)
                .iter()
                .all(|value| !value.contains("mock interaction"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_emits_mock_interaction_probe_fact_discriminator() -> Result<(), String> {
        let finding = classify_weak_direct_line("    mockSend(payload);")?;

        assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::CallEffect);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["mock interaction mockSend called with payload"]
        );
        Ok(())
    }

    #[test]
    fn classify_change_uses_call_effect_wording_for_console_log_without_literal()
    -> Result<(), String> {
        let finding = classify_weak_direct_line("    console.log(status);")?;

        assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["call console.log includes status"]
        );
        assert!(
            missing_discriminator_values(&finding)
                .iter()
                .all(|value| !value.contains("log contains"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_probe_facts_for_ambiguous_const_expression() -> Result<(), String> {
        let finding =
            classify_weak_direct_line("    const total = applyDiscount(amount, threshold);")?;

        assert_eq!(finding.probe.family, ProbeFamily::Predicate);
        assert!(finding.probe.expected_sinks.is_empty());
        assert!(finding.probe.required_oracles.is_empty());
        assert!(finding.flow_sinks.is_empty());
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry == "probe_fact: ambiguous_fallback")
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_probe_facts_for_ambiguous_computed_member_call() -> Result<(), String>
    {
        let finding = classify_weak_direct_line("    handlers[name](payload);")?;

        assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
        assert!(finding.flow_sinks.is_empty());
        assert!(finding.activation.missing_discriminators.is_empty());
        assert_static_limit(
            &finding,
            StaticLimitKind::DynamicDispatch,
            "dynamic_dispatch",
        );
        Ok(())
    }

    #[test]
    fn classify_change_surfaces_metaprogramming_static_limit() -> Result<(), String> {
        let finding = classify_weak_direct_line("    return new Proxy(target, handler);")?;

        assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
        assert_static_limit(
            &finding,
            StaticLimitKind::Metaprogramming,
            "metaprogramming",
        );
        Ok(())
    }

    #[test]
    fn classify_change_does_not_surface_static_limits_from_string_literals() -> Result<(), String> {
        let proxy_string = classify_weak_direct_line("    return \"Proxy(\";")?;
        let computed_string = classify_weak_direct_line("    return \"actions[key](\";")?;

        assert_eq!(proxy_string.static_limit_kind, None);
        assert_eq!(computed_string.static_limit_kind, None);
        Ok(())
    }

    #[test]
    fn classify_change_surfaces_decorator_indirection_static_limit() -> Result<(), String> {
        let mut owner = test_owner("save", "src/service.ts");
        owner.decorated = true;
        let test = weak_direct_test_for("save");
        let finding = classify_change(
            Path::new("src/service.ts"),
            2,
            "    return value;",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected decorated owner finding".to_string())?;

        assert_static_limit(
            &finding,
            StaticLimitKind::DecoratorIndirection,
            "decorator_indirection",
        );
        Ok(())
    }

    #[test]
    fn extract_owners_marks_class_method_as_decorated_when_class_is_decorated() {
        let owners = extract_owners(
            Path::new("src/service.ts"),
            r#"@sealed
class Service {
    save(value: string) {
        return value;
    }
}
"#,
        );

        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].name, "save");
        assert!(owners[0].decorated);
    }

    #[test]
    fn classify_change_surfaces_missing_import_graph_static_limit() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.ts"),
            r#"import { normalizeTotal } from "./math";

export function discountedTotal(amount: number): number {
    return normalizeTotal(amount);
}
"#,
        );
        let test = weak_direct_test_for("discountedTotal");
        let finding = classify_change(
            Path::new("src/pricing.ts"),
            4,
            "    return normalizeTotal(amount);",
            &owners,
            &[test],
        )
        .ok_or_else(|| "expected imported-symbol finding".to_string())?;

        assert_static_limit(
            &finding,
            StaticLimitKind::MissingImportGraph,
            "missing_import_graph",
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.contains("normalizeTotal"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_omits_discriminator_for_call_shaped_predicate_operand() -> Result<(), String>
    {
        let finding = classify_weak_direct_line("    if (input.trim() === \"\") {")?;

        assert_eq!(finding.probe.family, ProbeFamily::Predicate);
        assert!(finding.flow_sinks.is_empty());
        assert!(finding.activation.missing_discriminators.is_empty());
        Ok(())
    }

    #[test]
    fn classify_change_omits_probe_facts_for_heuristic_only_related_test() -> Result<(), String> {
        let owner = test_owner("applyDiscount", "src/lib.ts");
        let test = heuristic_name_test_for("applyDiscount");
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected heuristic TypeScript preview finding".to_string())?;

        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("heuristic only"))
        );
        Ok(())
    }

    #[test]
    fn extract_tests_collects_vi_mock_paths_in_file() {
        let source = r#"
import { vi } from "vitest";
vi.mock("./api");
vi.mock("./logger");
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
        let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
        assert_eq!(tests.len(), 1);
        assert_eq!(
            tests[0].mocks_in_file,
            vec!["./api".to_string(), "./logger".to_string()]
        );
    }

    #[test]
    fn extract_tests_collects_jest_mock_paths_in_file() {
        let source = r#"
jest.mock("./repository");
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
        let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].mocks_in_file, vec!["./repository".to_string()]);
    }

    #[test]
    fn extract_tests_returns_empty_mock_list_when_no_mock_call() {
        let source = r#"
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
        let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
        assert_eq!(tests.len(), 1);
        assert!(tests[0].mocks_in_file.is_empty());
    }

    #[test]
    fn collect_related_mock_paths_dedups_across_tests_in_same_file() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![
            TypeScriptTest {
                name: "alpha".to_string(),
                local_name: "alpha".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 1,
                body_text: "applyDiscount(1, 2)".to_string(),
                assertions: Vec::new(),
                mocks_in_file: vec!["./api".to_string()],
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "beta".to_string(),
                local_name: "beta".to_string(),
                describe_names: Vec::new(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 2,
                body_text: "applyDiscount(3, 4)".to_string(),
                assertions: Vec::new(),
                mocks_in_file: vec!["./api".to_string()],
                imports_in_file: Vec::new(),
            },
        ];
        let paths = collect_related_mock_paths(&owner, &tests);
        assert_eq!(paths, vec!["./api".to_string()]);
    }

    #[test]
    fn collect_related_mock_paths_ignores_unrelated_tests() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![TypeScriptTest {
            name: "unrelated".to_string(),
            local_name: "unrelated".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/other.test.ts"),
            line: 1,
            body_text: "otherHelper()".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        }];
        let paths = collect_related_mock_paths(&owner, &tests);
        assert!(paths.is_empty());
    }

    #[test]
    fn collect_related_mock_paths_ignores_object_method_mentions() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![TypeScriptTest {
            name: "unrelated method".to_string(),
            local_name: "unrelated method".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/cart.test.ts"),
            line: 1,
            body_text: "expect(order.applyDiscount(50)).toBe(40);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        }];
        let paths = collect_related_mock_paths(&owner, &tests);
        assert!(paths.is_empty());
    }

    #[test]
    fn classify_change_surfaces_mocked_module_static_limit_in_missing_and_evidence()
    -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
        };
        let tests = vec![TypeScriptTest {
            name: "alpha".to_string(),
            local_name: "alpha".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        }];
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &tests,
        )
        .ok_or_else(|| "expected a finding for the changed line".to_string())?;
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains("Static limit `mocked_module`")
                    && line.contains("./api"))
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.starts_with("static_limit mocked_module:"))
        );
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::MockedModule)
        );
        Ok(())
    }
}
