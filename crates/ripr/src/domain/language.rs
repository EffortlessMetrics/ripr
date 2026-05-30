//! Language identity and adapter status vocabulary.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! These are pure-data enums shared between the analysis adapter layer and
//! the output renderers that emit additive optional language metadata fields.

/// The set of source languages an adapter can report.
///
/// `Rust` is the reference language. `TypeScript`, `JavaScript`, and
/// `Python` are preview surfaces added in later work items in Campaign 27.
/// JavaScript is implemented by the TypeScript-family adapter and remains
/// separately labeled in output. Adding a new variant here is a deliberate
/// contract change and must update RIPR-SPEC-0026.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    TypeScript,
    JavaScript,
    Python,
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
        }
    }

    pub(crate) fn is_available(self) -> bool {
        match self {
            LanguageId::Rust => cfg!(feature = "lang-rust"),
            LanguageId::TypeScript => cfg!(feature = "lang-typescript"),
            LanguageId::JavaScript => cfg!(feature = "lang-typescript"),
            LanguageId::Python => cfg!(feature = "lang-python"),
        }
    }

    pub(crate) fn required_feature(self) -> &'static str {
        match self {
            LanguageId::Rust => "lang-rust",
            LanguageId::TypeScript => "lang-typescript",
            LanguageId::JavaScript => "lang-typescript",
            LanguageId::Python => "lang-python",
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
    UnsupportedSyntax,
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
            StaticLimitKind::UnsupportedSyntax => "unsupported_syntax",
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
        assert_eq!(LanguageId::JavaScript.required_feature(), "lang-typescript");
        assert_eq!(LanguageId::Python.required_feature(), "lang-python");
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
    }
}
