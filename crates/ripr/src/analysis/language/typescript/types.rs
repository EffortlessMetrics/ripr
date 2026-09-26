//! Shared data types for the TypeScript preview adapter.

use super::*;

/// Owner extracted from a TypeScript / JavaScript source file.
///
/// Covers the syntax-first owner kinds accepted for the preview surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptOwner {
    pub(crate) name: String,
    pub(crate) file: PathBuf,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) owner_kind: OwnerKind,
    pub(crate) class_name: Option<String>,
    pub(crate) decorated: bool,
    /// `true` when the owner declaration is the module's default export
    /// (`export default function name(...)`, `export default const name = ...`).
    /// A default import (`import local from './owner'`) binds exactly this
    /// declaration, so the relation gate may credit a call through `local`
    /// only when this fact is recorded (#4103 under-credit: a named default
    /// export imported under a different local name was previously
    /// unreachable from every relation arm).
    pub(crate) exported_as_default: bool,
    pub(crate) imports: Vec<TypeScriptImport>,
    /// Method-shape refinement for `OwnerKind::Method` owners: a getter is
    /// invoked by property READS on a receiver, and the constructor runs on
    /// every `new ClassName(...)` — both change which relation needle is
    /// honest for the owner (#4104-B).
    pub(crate) method_kind: TypeScriptMethodKind,
    /// `true` when the owner's CONTAINING CLASS is the module's default
    /// export (`export default class Cart { ... }`). A class default export
    /// has no class-level owner entry — only its methods are indexed — so
    /// each method carries this marker instead, and `exported_as_default`
    /// stays false for them: a method is not itself the module's default
    /// export. Constructor matching uses it to credit
    /// `new <default-import local>(...)` (#4104-B, review #4138).
    pub(crate) class_default_export: bool,
    /// Parameter facts resolved from the owner signature (issue #4102).
    /// `Some(n)` only when every parameter is a plain binding identifier and
    /// there is no rest parameter; `None` when the list could not be resolved
    /// (destructuring, rest, or extraction unavailable). A boundary witness
    /// may only credit an argument position a parameter could actually read.
    pub(crate) arity: Option<usize>,
    /// Parameter names in signature order; empty unless `arity` is `Some`.
    pub(crate) parameters: Vec<String>,
    /// The owner's own source text from its declaration start to its end, when
    /// extraction had the containing source. Enables expected-semantics checks
    /// that need the owner body (predicate expected-side liveness, #4102).
    pub(crate) source_text: Option<String>,
}

/// Syntactic method-shape refinement recorded during owner extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum TypeScriptMethodKind {
    /// An ordinary callable method — invoked only by member CALLS.
    #[default]
    Ordinary,
    /// `get total()` — invoked by property reads (`cart.total`).
    Getter,
    /// `set total(v)` — invoked by property writes (`cart.total = v`).
    Setter,
    /// `constructor(...)` — invoked by `new ClassName(...)`.
    Constructor,
}

impl TypeScriptOwner {
    pub(crate) fn symbol_id(&self) -> SymbolId {
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
pub(crate) struct TypeScriptTest {
    /// Qualified display name. Nested `describe(...)` names are joined with the
    /// local `test(...)` / `it(...)` name so user surfaces can show context.
    pub(crate) name: String,
    /// The local `test(...)` / `it(...)` string before describe qualification.
    pub(crate) local_name: String,
    /// Nested describe names in outer-to-inner order.
    pub(crate) describe_names: Vec<String>,
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
    pub(crate) body_text: String,
    pub(crate) assertions: Vec<TypeScriptAssertion>,
    /// Module paths referenced by syntactic `vi.mock("...")` /
    /// `jest.mock("...")` calls discovered at the top level of the
    /// containing test file. Populated once per file and cloned into
    /// every `TypeScriptTest` parsed from that file so the classifier
    /// can surface the `mocked_module` static-limit without re-parsing.
    /// Empty when no syntactic mock indirection is present.
    pub(crate) mocks_in_file: Vec<String>,
    /// Runtime imports discovered at the top level of the containing test
    /// file. Used only to map relative named or namespace imports back to a
    /// source owner before considering alias calls related.
    pub(crate) imports_in_file: Vec<TypeScriptImport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptImport {
    pub(crate) source: String,
    pub(crate) imported: Option<String>,
    pub(crate) local: String,
    pub(crate) namespace: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptParseLimit {
    pub(crate) file: PathBuf,
    pub(crate) reason: String,
}

/// A workspace file that could not be read at all (permissions, encoding,
/// transient I/O). Unlike a parse limit the adapter has no syntax facts at
/// all for this path; the file silently vanished from both the owner index
/// and the test index before this record existed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptReadFailure {
    pub(crate) file: PathBuf,
    pub(crate) error: String,
}

/// One concrete, observed gap between the tests a recognized test file
/// registers and the tests the syntax-first extractor actually indexed.
/// Produced by `detect_partial_test_extraction`; consumed by the
/// `typescript_test_extraction_partial` named-limitation producer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptTestExtractionGap {
    pub(crate) file: PathBuf,
    pub(crate) sample_line: usize,
    /// Detected shape, one of `template-literal title`, `tagged-template .each`,
    /// or `test/it call in loop/callback/nested body`.
    pub(crate) shape: &'static str,
    /// Source snippet (single-line, bounded) of the unextracted registration.
    pub(crate) snippet: String,
}

/// Owner-shaped construct the syntax-first owner extractor does not index
/// (#4104-A). One per affected changed file (the first detected shape).
pub(crate) struct TypeScriptOwnerExtractionGap {
    pub(crate) file: PathBuf,
    pub(crate) sample_line: usize,
    /// Detected shape, one of `arrow-function class field`,
    /// `class method with unsupported key`, `class static block`,
    /// `accessor auto-accessor`, `enum declaration`, or
    /// `module/namespace declaration`.
    pub(crate) shape: &'static str,
    /// Source snippet (single-line, bounded) of the unextracted owner shape.
    pub(crate) snippet: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeScriptRelationKind {
    DirectOwnerCall,
    /// Test imports the owner under an explicit `{ OriginalName as local }`
    /// renaming alias from the owner file and calls `local(...)` in the body.
    /// Syntactically equivalent certainty to `DirectOwnerCall`: the rename is
    /// explicit and the body verifiably calls the local alias.
    /// Shadow guard applied: if `local` is re-declared inside the test body,
    /// falls through (not credited at High).
    ImportAliasOwnerCall,
    ImportedOwnerCall,
    ModuleValueReference,
    ReceiverOwnerCall,
    ClassMethodCall,
    /// Test imports a name from an intermediate file that re-exports it from
    /// the owner file via a single `export { N } from './owner'` hop.
    /// The import chain is explicit in-source; only ONE hop is followed
    /// (fail-closed on deeper transitive chains).
    ReExportChainFollowed,
    SameFileProximity,
    DescribeName,
    TestName,
}

impl TypeScriptRelationKind {
    pub(crate) fn rank(self) -> u8 {
        match self {
            Self::DirectOwnerCall | Self::ImportAliasOwnerCall => 5,
            Self::ImportedOwnerCall => 4,
            Self::ModuleValueReference => 4,
            Self::ReceiverOwnerCall => 4,
            Self::ClassMethodCall => 4,
            // Re-export chain: same rank as other imported calls — the test
            // genuinely exercises the owner, just via an intermediate file.
            Self::ReExportChainFollowed => 4,
            Self::SameFileProximity => 3,
            Self::DescribeName => 2,
            Self::TestName => 1,
        }
    }

    pub(crate) fn uses_oracle(self) -> bool {
        matches!(
            self,
            Self::DirectOwnerCall
                | Self::ImportAliasOwnerCall
                | Self::ImportedOwnerCall
                | Self::ModuleValueReference
                | Self::ReceiverOwnerCall
                | Self::ClassMethodCall
                | Self::ReExportChainFollowed
        )
    }

    pub(crate) fn is_uncertain(self) -> bool {
        !self.uses_oracle()
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DirectOwnerCall => "direct_owner_call",
            Self::ImportAliasOwnerCall => "import_alias_owner_call",
            Self::ImportedOwnerCall => "imported_owner_call",
            Self::ModuleValueReference => "module_value_reference",
            Self::ReceiverOwnerCall => "receiver_owner_call",
            Self::ClassMethodCall => "class_method_call",
            Self::ReExportChainFollowed => "re_export_chain_followed",
            Self::SameFileProximity => "same_file_proximity",
            Self::DescribeName => "describe_name",
            Self::TestName => "test_name",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TypeScriptRelatedCandidate<'a> {
    pub(crate) test: &'a TypeScriptTest,
    pub(crate) relation: TypeScriptRelationKind,
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
///
/// Oracle metadata fields (RIPR-SPEC-0085 §PR5):
/// - `observed_expression` — the `expect(<expr>)` argument as source text.
/// - `expected_value_or_variant` — the matcher argument as source text, only
///   when the argument is a concrete resolvable literal (not dynamic).
///   `None` when the matcher argument is a variable, function call, or other
///   non-literal expression; those cases emit `typescript_dynamic_assertion_unresolved`.
/// - `oracle_confidence` — derived from `oracle_strength` plus whether the
///   expected value is a concrete literal.
/// - `raw_evidence_ref` — `file:line` back to the `expect(...)` call site,
///   i.e. `<file>:<assertion_line>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptAssertion {
    pub(crate) matcher: String,
    pub(crate) argument_count: usize,
    pub(crate) line: usize,
    pub(crate) oracle_kind: OracleKind,
    pub(crate) oracle_strength: OracleStrength,
    pub(crate) mock_payload: Option<TypeScriptMockPayload>,
    pub(crate) error_payload: Option<TypeScriptErrorPayload>,
    /// The `expect(<expr>)` argument as source text (RIPR-SPEC-0085 §PR5).
    pub(crate) observed_expression: Option<String>,
    /// The matcher argument as source text when it is a concrete literal
    /// (RIPR-SPEC-0085 §PR5). `None` for dynamic/non-literal arguments.
    pub(crate) expected_value_or_variant: Option<String>,
    /// Whether the matcher argument is a non-literal dynamic expression
    /// (RIPR-SPEC-0085 §PR5). When `true` and `expected_value_or_variant`
    /// is `None`, the `typescript_dynamic_assertion_unresolved` limitation is emitted.
    pub(crate) has_dynamic_matcher_arg: bool,
    /// Confidence derived from oracle_strength + literal concreteness
    /// (RIPR-SPEC-0085 §PR5).
    pub(crate) oracle_confidence: OracleConfidence,
}

/// Oracle confidence level derived from `oracle_strength` plus whether the
/// expected value is a concrete literal (RIPR-SPEC-0085 §PR5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OracleConfidence {
    /// Strong oracle strength with a concrete literal expected value.
    High,
    /// Strong oracle strength but no resolvable literal expected value.
    Medium,
    /// Weak or smoke oracle strength.
    Low,
    /// Unknown oracle kind or no expected value evidence.
    Unknown,
}

impl OracleConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptMockPayload {
    pub(crate) target: String,
    pub(crate) expected: String,
    pub(crate) kind: TypeScriptMockPayloadKind,
}

impl TypeScriptMockPayload {
    pub(crate) fn oracle_text(&self) -> String {
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
pub(crate) enum TypeScriptMockPayloadKind {
    CalledWith,
    CalledTimes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptErrorPayload {
    pub(crate) expected: String,
    pub(crate) kind: TypeScriptErrorPayloadKind,
}

impl TypeScriptErrorPayload {
    pub(crate) fn oracle_text(&self) -> String {
        match self.kind {
            TypeScriptErrorPayloadKind::ThrowsLiteral
            | TypeScriptErrorPayloadKind::ThrowsObject
            | TypeScriptErrorPayloadKind::ThrowsClass => {
                format!("expect(...).toThrow({})", self.expected)
            }
            TypeScriptErrorPayloadKind::RejectsThrowLiteral
            | TypeScriptErrorPayloadKind::RejectsThrowObject
            | TypeScriptErrorPayloadKind::RejectsThrowClass => {
                format!("await expect(...).rejects.toThrow({})", self.expected)
            }
            TypeScriptErrorPayloadKind::RejectsMatchObject => {
                format!("await expect(...).rejects.toMatchObject({})", self.expected)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeScriptErrorPayloadKind {
    /// `.toThrow("exact message")` — string literal payload.
    ThrowsLiteral,
    /// `.toThrow({ code: "X" })` — object literal payload with only
    /// literal-valued keys (fail-closed: shorthand / spread / dynamic
    /// computed properties are rejected).
    ThrowsObject,
    /// `.toThrow(SomeError)` or `.toThrow(SomeError.SubClass)` — class /
    /// constructor reference as a safe member path.
    ThrowsClass,
    /// `await expect(...).rejects.toThrow("exact message")`.
    RejectsThrowLiteral,
    /// `await expect(...).rejects.toThrow({ code: "X" })`.
    RejectsThrowObject,
    /// `await expect(...).rejects.toThrow(SomeError)`.
    RejectsThrowClass,
    /// `await expect(...).rejects.toMatchObject({ code: "X" })`.
    RejectsMatchObject,
}
