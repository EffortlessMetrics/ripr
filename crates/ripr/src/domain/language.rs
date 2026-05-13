//! Language identity and adapter status vocabulary.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! These are pure-data enums shared between the analysis adapter layer and
//! the output renderers that emit additive optional language metadata fields.

/// The set of source languages an adapter can identify itself as.
///
/// `Rust` is the reference language. `TypeScript` and `Python` are preview
/// adapters added in later work items in Campaign 27. Adding a new variant
/// here is a deliberate contract change and must update RIPR-SPEC-0026.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    TypeScript,
    Python,
}

impl LanguageId {
    /// Stable wire string used when this id is serialized into the additive
    /// optional `language` output field.
    pub fn as_str(&self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::TypeScript => "typescript",
            LanguageId::Python => "python",
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
        assert_eq!(LanguageId::Python.as_str(), "python");
    }

    #[test]
    fn language_status_wire_strings_are_stable() {
        assert_eq!(LanguageStatus::Stable.as_str(), "stable");
        assert_eq!(LanguageStatus::Preview.as_str(), "preview");
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
    }
}
