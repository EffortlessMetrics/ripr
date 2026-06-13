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
    pub(crate) imports: Vec<TypeScriptImport>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeScriptRelationKind {
    DirectOwnerCall,
    ImportedOwnerCall,
    ModuleValueReference,
    ReceiverOwnerCall,
    ClassMethodCall,
    SameFileProximity,
    DescribeName,
    TestName,
}

impl TypeScriptRelationKind {
    pub(crate) fn rank(self) -> u8 {
        match self {
            Self::DirectOwnerCall => 5,
            Self::ImportedOwnerCall => 4,
            Self::ModuleValueReference => 4,
            Self::ReceiverOwnerCall => 4,
            Self::ClassMethodCall => 4,
            Self::SameFileProximity => 3,
            Self::DescribeName => 2,
            Self::TestName => 1,
        }
    }

    pub(crate) fn uses_oracle(self) -> bool {
        matches!(
            self,
            Self::DirectOwnerCall
                | Self::ImportedOwnerCall
                | Self::ModuleValueReference
                | Self::ReceiverOwnerCall
                | Self::ClassMethodCall
        )
    }

    pub(crate) fn is_uncertain(self) -> bool {
        !self.uses_oracle()
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DirectOwnerCall => "direct_owner_call",
            Self::ImportedOwnerCall => "imported_owner_call",
            Self::ModuleValueReference => "module_value_reference",
            Self::ReceiverOwnerCall => "receiver_owner_call",
            Self::ClassMethodCall => "class_method_call",
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
pub(crate) enum TypeScriptErrorPayloadKind {
    ThrowsLiteral,
    RejectsThrowLiteral,
    RejectsMatchObject,
}
