//! Language identity and adapter status vocabulary.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! These are pure-data enums shared between the analysis adapter layer and
//! the output renderers that emit additive optional language metadata fields.

/// The set of source languages an adapter can report.
///
/// `Rust` is the reference language. `TypeScript`, `JavaScript`, `Python`,
/// and `Perl` are preview surfaces added in later work items.
/// JavaScript is implemented by the TypeScript-family adapter and remains
/// separately labeled in output. Adding a new variant here is a deliberate
/// contract change and must update RIPR-SPEC-0026.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Perl,
}

impl LanguageId {
    /// Stable wire string used when this id is serialized into the additive
    /// optional `language` output field.
    pub fn as_str(&self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::TypeScript => "typescript",
            LanguageId::JavaScript => "javascript",
            LanguageId::Python => "python",
            LanguageId::Perl => "perl",
        }
    }

    pub(crate) fn is_available(self) -> bool {
        match self {
            LanguageId::Rust => cfg!(feature = "lang-rust"),
            LanguageId::TypeScript => cfg!(feature = "lang-typescript"),
            LanguageId::JavaScript => cfg!(feature = "lang-typescript"),
            LanguageId::Python => cfg!(feature = "lang-python"),
            LanguageId::Perl => cfg!(feature = "lang-perl"),
        }
    }

    pub(crate) fn required_feature(self) -> &'static str {
        match self {
            LanguageId::Rust => "lang-rust",
            LanguageId::TypeScript => "lang-typescript",
            LanguageId::JavaScript => "lang-typescript",
            LanguageId::Python => "lang-python",
            LanguageId::Perl => "lang-perl",
        }
    }
}

/// Whether an adapter is the reference (`Stable`) implementation for a
/// language or a `Preview` adapter.
///
/// Only Rust is permitted to claim `Stable` under the current capability
/// vocabulary. TypeScript and Python adapters land as `Preview` per
/// RIPR-SPEC-0026. The wire field is omitted entirely for Rust per the
/// spec; preview adapters set `Preview`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguageStatus {
    Stable,
    Preview,
}

impl LanguageStatus {
    /// Stable wire string used when this status is serialized into the
    /// additive optional `language_status` output field.
    pub fn as_str(&self) -> &'static str {
        match self {
            LanguageStatus::Stable => "stable",
            LanguageStatus::Preview => "preview",
        }
    }
}

/// Stable owner vocabulary for syntax-first language adapters.
///
/// These labels are additive optional finding metadata per RIPR-SPEC-0026.
/// They let preview adapters identify the syntactic owner that received a
/// changed line without forcing downstream consumers to parse evidence text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerKind {
    Function,
    Method,
    ClassMethod,
    ArrowFunction,
    Component,
    ModuleFunction,
}

impl OwnerKind {
    /// Stable wire string used when this kind is serialized into the
    /// additive optional `owner_kind` output field.
    pub fn as_str(&self) -> &'static str {
        match self {
            OwnerKind::Function => "function",
            OwnerKind::Method => "method",
            OwnerKind::ClassMethod => "class_method",
            OwnerKind::ArrowFunction => "arrow_function",
            OwnerKind::Component => "component",
            OwnerKind::ModuleFunction => "module_function",
        }
    }
}

/// Stable static limitation categories for syntax-first preview evidence.
///
/// These labels are additive optional finding metadata per RIPR-SPEC-0026.
/// They give downstream consumers a typed discriminator for display and
/// reporting without parsing human evidence text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticLimitKind {
    DynamicDispatch,
    Metaprogramming,
    MissingImportGraph,
    DecoratorIndirection,
    MockedModule,
    OpaqueCustomAssertionHelper,
    PropertyBasedTest,
    UnresolvedPytestFixture,
    UnsupportedSyntax,
    /// The changed Rust seam owner is FFI/binding-exposed; whether an
    /// external-language (e.g. TypeScript) test oracle discriminates this
    /// behavior is not statically known — verify the external oracle rather
    /// than adding a Rust test.
    CrossLanguageOracleVisibilityUnresolved,
    /// A test appears to call public API that may transitively reach the
    /// changed owner through a `pub -> pub(crate)` helper chain or similar
    /// internal call graph, but ripr's lexical call facts cannot fully resolve
    /// the path (macro invocations, generics, trait dispatch, or depth > 3
    /// stop the walk). The classification stays `no_static_path` -- this label
    /// is a named limitation, not a coverage claim. See RIPR-SPEC-0114.
    RustTransitiveReachUnresolved,
}

impl StaticLimitKind {
    /// Stable wire string used when this kind is serialized into the
    /// additive optional `static_limit_kind` output field.
    pub fn as_str(&self) -> &'static str {
        match self {
            StaticLimitKind::DynamicDispatch => "dynamic_dispatch",
            StaticLimitKind::Metaprogramming => "metaprogramming",
            StaticLimitKind::MissingImportGraph => "missing_import_graph",
            StaticLimitKind::DecoratorIndirection => "decorator_indirection",
            StaticLimitKind::MockedModule => "mocked_module",
            StaticLimitKind::OpaqueCustomAssertionHelper => "opaque_custom_assertion_helper",
            StaticLimitKind::PropertyBasedTest => "property_based_test",
            StaticLimitKind::UnresolvedPytestFixture => "unresolved_pytest_fixture",
            StaticLimitKind::UnsupportedSyntax => "unsupported_syntax",
            StaticLimitKind::CrossLanguageOracleVisibilityUnresolved => {
                "cross_language_oracle_visibility_unresolved"
            }
            StaticLimitKind::RustTransitiveReachUnresolved => "rust_transitive_reach_unresolved",
        }
    }

    /// One-sentence plain-English explanation of what this limitation means and
    /// why ripr cannot resolve the path — surfaced next to the stable
    /// [`as_str`](Self::as_str) token so a reader sees *why* a finding is
    /// limited, not just an opaque snake_case label (#1162 explain enhancement).
    /// Conservative static language only: these describe what ripr could NOT
    /// statically resolve; none assert coverage or adequacy.
    pub fn describe(&self) -> &'static str {
        match self {
            StaticLimitKind::DynamicDispatch => {
                "Dynamic dispatch (trait objects or virtual calls) hides which implementation runs, \
                 so ripr cannot statically resolve whether a test reaches this change."
            }
            StaticLimitKind::Metaprogramming => {
                "Metaprogramming (macros or code generation) produces calls ripr cannot see in the \
                 source, so the reaching path is not statically resolvable."
            }
            StaticLimitKind::MissingImportGraph => {
                "The import graph could not be resolved (for example a relative or dynamic import), \
                 so ripr cannot connect a test to this change."
            }
            StaticLimitKind::DecoratorIndirection => {
                "A decorator wraps the changed owner, so ripr cannot statically confirm a test \
                 exercises the underlying behavior."
            }
            StaticLimitKind::MockedModule => {
                "A mocked module replaces the real implementation, so a passing test may not \
                 observe the actual changed behavior."
            }
            StaticLimitKind::OpaqueCustomAssertionHelper => {
                "A custom assertion helper hides what is checked, so ripr cannot confirm the \
                 assertion would discriminate this change."
            }
            StaticLimitKind::PropertyBasedTest => {
                "A property-based test generates its inputs at runtime, so ripr cannot statically \
                 confirm it exercises this specific change."
            }
            StaticLimitKind::UnresolvedPytestFixture => {
                "A pytest fixture could not be resolved, so ripr cannot statically connect the test \
                 setup to this change."
            }
            StaticLimitKind::UnsupportedSyntax => {
                "The surrounding syntax is not yet supported by ripr's static model, so the \
                 reaching path is not resolvable."
            }
            StaticLimitKind::CrossLanguageOracleVisibilityUnresolved => {
                "This owner is exposed across a language boundary (FFI or binding); whether an \
                 external-language test observes the change is not statically known \u{2014} verify \
                 the external oracle rather than adding a same-language test."
            }
            StaticLimitKind::RustTransitiveReachUnresolved => {
                "A test may reach this change through an internal helper-call chain ripr cannot \
                 fully trace (macros, generics, trait dispatch, or depth greater than 3). This is a \
                 named limitation, not a coverage claim."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_id_wire_strings_are_stable() {
        assert_eq!(LanguageId::Rust.as_str(), "rust");
        assert_eq!(LanguageId::TypeScript.as_str(), "typescript");
        assert_eq!(LanguageId::JavaScript.as_str(), "javascript");
        assert_eq!(LanguageId::Python.as_str(), "python");
        assert_eq!(LanguageId::Perl.as_str(), "perl");
    }

    #[test]
    fn language_feature_availability_matches_build() {
        assert!(LanguageId::Rust.is_available());
        assert_eq!(
            LanguageId::TypeScript.is_available(),
            cfg!(feature = "lang-typescript")
        );
        assert_eq!(
            LanguageId::JavaScript.is_available(),
            cfg!(feature = "lang-typescript")
        );
        assert_eq!(
            LanguageId::Python.is_available(),
            cfg!(feature = "lang-python")
        );
        assert_eq!(LanguageId::Perl.is_available(), cfg!(feature = "lang-perl"));
        assert_eq!(LanguageId::JavaScript.required_feature(), "lang-typescript");
        assert_eq!(LanguageId::Python.required_feature(), "lang-python");
        assert_eq!(LanguageId::Perl.required_feature(), "lang-perl");
    }

    #[test]
    fn language_status_wire_strings_are_stable() {
        assert_eq!(LanguageStatus::Stable.as_str(), "stable");
        assert_eq!(LanguageStatus::Preview.as_str(), "preview");
    }

    #[test]
    fn owner_kind_wire_strings_are_stable() {
        assert_eq!(OwnerKind::Function.as_str(), "function");
        assert_eq!(OwnerKind::Method.as_str(), "method");
        assert_eq!(OwnerKind::ClassMethod.as_str(), "class_method");
        assert_eq!(OwnerKind::ArrowFunction.as_str(), "arrow_function");
        assert_eq!(OwnerKind::Component.as_str(), "component");
        assert_eq!(OwnerKind::ModuleFunction.as_str(), "module_function");
    }

    #[test]
    fn static_limit_kind_wire_strings_are_stable() {
        assert_eq!(
            StaticLimitKind::DynamicDispatch.as_str(),
            "dynamic_dispatch"
        );
        assert_eq!(StaticLimitKind::Metaprogramming.as_str(), "metaprogramming");
        assert_eq!(
            StaticLimitKind::MissingImportGraph.as_str(),
            "missing_import_graph"
        );
        assert_eq!(
            StaticLimitKind::DecoratorIndirection.as_str(),
            "decorator_indirection"
        );
        assert_eq!(StaticLimitKind::MockedModule.as_str(), "mocked_module");
        assert_eq!(
            StaticLimitKind::UnsupportedSyntax.as_str(),
            "unsupported_syntax"
        );
        assert_eq!(
            StaticLimitKind::OpaqueCustomAssertionHelper.as_str(),
            "opaque_custom_assertion_helper"
        );
        assert_eq!(
            StaticLimitKind::PropertyBasedTest.as_str(),
            "property_based_test"
        );
        assert_eq!(
            StaticLimitKind::UnresolvedPytestFixture.as_str(),
            "unresolved_pytest_fixture"
        );
        assert_eq!(
            StaticLimitKind::CrossLanguageOracleVisibilityUnresolved.as_str(),
            "cross_language_oracle_visibility_unresolved"
        );
        assert_eq!(
            StaticLimitKind::RustTransitiveReachUnresolved.as_str(),
            "rust_transitive_reach_unresolved"
        );
    }

    #[test]
    fn static_limit_kind_describe_is_present_and_distinct() {
        let kinds = [
            StaticLimitKind::DynamicDispatch,
            StaticLimitKind::Metaprogramming,
            StaticLimitKind::MissingImportGraph,
            StaticLimitKind::DecoratorIndirection,
            StaticLimitKind::MockedModule,
            StaticLimitKind::OpaqueCustomAssertionHelper,
            StaticLimitKind::PropertyBasedTest,
            StaticLimitKind::UnresolvedPytestFixture,
            StaticLimitKind::UnsupportedSyntax,
            StaticLimitKind::CrossLanguageOracleVisibilityUnresolved,
            StaticLimitKind::RustTransitiveReachUnresolved,
        ];
        // Every variant has a non-empty, distinct explanation. Conservative
        // static-language vocabulary is enforced repo-wide by
        // `cargo xtask check-static-language` (which scans this prose too), so it
        // is not re-checked here with literal forbidden terms.
        let mut seen = std::collections::HashSet::new();
        for kind in kinds {
            let described = kind.describe();
            assert!(described.len() > 20, "describe too short for {kind:?}");
            assert!(
                seen.insert(described),
                "duplicate describe text for {kind:?}"
            );
        }
    }
}
