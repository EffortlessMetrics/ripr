//! Language identity and adapter status vocabulary.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! These are pure-data enums shared between the analysis adapter layer and
//! the output renderers that emit additive optional language metadata fields.

/// Program prefix of a generated pytest verify command.
///
/// `python -m pytest` rather than bare `pytest`: `-m` puts the working
/// directory on `sys.path`, so a flat-layout package at the repository root
/// imports without a `pythonpath` setting, where bare `pytest` fails
/// collection with `ModuleNotFoundError`. The interpreter is spelled `python`,
/// like the unittest route's `python -m unittest`: inside a virtual
/// environment, where pytest is normally installed, `python` names the
/// environment's interpreter on Linux, macOS and Windows alike, while
/// `python3` is absent from Windows virtual environments.
pub(crate) const PYTEST_VERIFY_PROGRAM: &str = "python -m pytest";

/// Whether a verify command runs pytest.
///
/// Accepts the generated `python -m pytest ...` form and the bare
/// `pytest ...` form earlier artifacts and recorded evals carry, so a
/// consumer reading either keeps its pytest-specific behavior.
pub(crate) fn is_pytest_verify_command(command: &str) -> bool {
    command
        .strip_prefix(PYTEST_VERIFY_PROGRAM)
        .or_else(|| command.strip_prefix("pytest"))
        .is_some_and(|rest| rest.starts_with(' '))
}

/// The set of source languages an adapter can report.
///
/// `Rust` is the reference language. `TypeScript`, `JavaScript`, `Python`,
/// and `Perl` are preview surfaces added in later work items.
/// JavaScript is implemented by the TypeScript-family adapter and remains
/// separately labeled in output. Adding a new variant here is a deliberate
/// contract change and must update RIPR-SPEC-0026.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageId {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Perl,
}

impl LanguageId {
    /// Every language id, in declaration order.
    pub(crate) const ALL: [LanguageId; 5] = [
        LanguageId::Rust,
        LanguageId::TypeScript,
        LanguageId::JavaScript,
        LanguageId::Python,
        LanguageId::Perl,
    ];

    /// Human-facing language name for prose (`TypeScript`, `JavaScript`),
    /// distinct from the lowercase wire string returned by [`Self::as_str`].
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            LanguageId::Rust => "Rust",
            LanguageId::TypeScript => "TypeScript",
            LanguageId::JavaScript => "JavaScript",
            LanguageId::Python => "Python",
            LanguageId::Perl => "Perl",
        }
    }

    /// Display name for a wire string, or `None` for an unknown language.
    pub(crate) fn display_name_for_wire(wire: &str) -> Option<&'static str> {
        Self::ALL
            .into_iter()
            .find(|language| language.as_str() == wire)
            .map(Self::display_name)
    }

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

    /// Inverse of [`LanguageId::as_str`] for the stable wire string.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "rust" => Some(LanguageId::Rust),
            "typescript" => Some(LanguageId::TypeScript),
            "javascript" => Some(LanguageId::JavaScript),
            "python" => Some(LanguageId::Python),
            "perl" => Some(LanguageId::Perl),
            _ => None,
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

    /// What a user needs before this language can be analyzed when its
    /// adapter is not compiled into this ripr binary.
    ///
    /// Single text owner for every surface that reports an unavailable
    /// adapter (the check note, JSON/diff-report `why`, the typed outcome
    /// recovery, the pipeline run reason, the `languages.enabled` config
    /// error, doctor, and pilot's unavailable notice, which delegates here).
    /// Perl names both prerequisites because enabling
    /// `perl` in `ripr.toml` is not enough on its own: the adapter only
    /// consumes packets from an external fact exporter, and the canonical
    /// exporter is not yet published. Bounded well under the 512-character
    /// analysis-outcome detail limit.
    pub(crate) fn unavailable_adapter_recovery(self) -> String {
        match self {
            LanguageId::Perl => format!(
                "Perl analysis is not available from this ripr binary. It needs both a ripr build with Cargo feature `lang-perl` (`cargo install ripr --features lang-perl`) and a compatible Perl fact exporter (`{PERL_FACT_EXPORTER}`), which is not yet published; no released ripr setup analyzes Perl yet, and adding `perl` to ripr.toml [languages] alone does not enable it"
            ),
            other => format!(
                "rebuild ripr with Cargo feature `{}` to analyze {} files",
                other.required_feature(),
                other.as_str()
            ),
        }
    }

    /// Plain notice for a language whose adapter is not compiled into this
    /// binary, or `None` when it is.
    ///
    /// Routes a user onward (currently `ripr pilot`) by restating the
    /// [`LanguageId::unavailable_adapter_recovery`] wording, so every surface
    /// names the same prerequisites.
    pub(crate) fn unavailable_adapter_notice(self) -> Option<String> {
        if self.is_available() {
            return None;
        }
        let recovery = self.unavailable_adapter_recovery();
        if recovery.ends_with('.') {
            Some(recovery)
        } else {
            Some(format!("{recovery}."))
        }
    }

    /// Extra prerequisite that enabling this language in `ripr.toml` does not
    /// satisfy on its own, for builds where the adapter IS compiled in.
    ///
    /// Perl consumes externally produced fact packets, so enabling it still
    /// needs a packet (`--perl-facts`) or a compatible managed exporter.
    /// Other preview languages have no such prerequisite.
    pub(crate) fn enable_prerequisite(self) -> Option<String> {
        match self {
            LanguageId::Perl => Some(format!(
                "Perl also needs a fact packet: {}",
                perl_fact_packet_guidance()
            )),
            _ => None,
        }
    }
}

/// Canonical name of the external Perl fact exporter that managed producer
/// mode invokes (`<exporter> ripr-facts --schema ...`). It is not yet
/// published, so no surface may present Perl analysis as installable.
pub(crate) const PERL_FACT_EXPORTER: &str = "perl-ripr-facts";

/// How to supply a Perl fact packet, shared by the enable prerequisite and
/// the adapter's `unavailable` reason so the two cannot diverge.
pub(crate) fn perl_fact_packet_guidance() -> String {
    format!(
        "pass --perl-facts <packet.json>, or configure [perl].producer with a compatible Perl fact exporter (`{PERL_FACT_EXPORTER}`, not yet published)"
    )
}

/// Whether an adapter is the reference (`Stable`) implementation for a
/// language or a `Preview` adapter.
///
/// Only Rust is permitted to claim `Stable` under the current capability
/// vocabulary. TypeScript and Python adapters land as `Preview` per
/// RIPR-SPEC-0026. The wire field is omitted entirely for Rust per the
/// spec; preview adapters set `Preview`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// the path (macro invocations, generics, trait dispatch, or depth > 5
    /// stop the walk). The classification stays `no_static_path` -- this label
    /// is a named limitation, not a coverage claim. See RIPR-SPEC-0114.
    RustTransitiveReachUnresolved,
    /// An integration test appears to call a crate public API, or a test helper
    /// that calls that public API, and a bounded same-repo lexical path may lead
    /// toward the changed owner. The classification stays `no_static_path`;
    /// this label names the unresolved integration/public-API edge, not a
    /// coverage claim. See RIPR-SPEC-0118.
    RustIntegrationPublicApiPathUnresolved,
    /// A Rust test reaches an entry point whose path toward the changed owner
    /// stops at a same-repo macro invocation that ripr does not expand. The
    /// macro definition lexically mentions the changed owner, but the
    /// classification stays `no_static_path`; this label names the unresolved
    /// macro edge, not a coverage claim. See RIPR-SPEC-0117.
    RustMacroReachUnresolved,
    /// A Rust test directly invokes a same-repo macro whose definition
    /// lexically mentions the changed owner. ripr does not expand the macro,
    /// so the classification stays `no_static_path`; this label names the
    /// unresolved test-macro edge, not a coverage claim. See RIPR-SPEC-0119.
    RustMacroWrappedTestCallUnresolved,
    /// A Rust test reaches the changed owner, but the only assertion-like
    /// observer ripr can see is a custom macro that it does not classify as an
    /// oracle. The classification stays reachable-but-undiscriminated; this
    /// label names the unresolved assertion macro, not a coverage claim.
    RustMacroWrappedAssertionUnresolved,
    /// A changed Rust let binding uses a bounded value-producing operation,
    /// but the syntax-first analyzer cannot carry that value into the
    /// same-owner equality predicate. Classification remains `static_unknown`;
    /// this is a named limitation, not a coverage or repair claim.
    RustValuePropagationUnresolved,
    /// An integration test invokes a Cargo-built binary, but ripr does not
    /// yet map that binary target back to the changed owner. The
    /// classification stays `no_static_path`; this is a named limitation,
    /// not a subprocess reach or receipt claim.
    RustSubprocessBinaryReachUnresolved,
    /// A wrapper error conversion (`callee(..).map_err(..)`) whose
    /// error-variant identity lives in the converted callee, not in the
    /// changed line. Whether the wrapper faithfully carries the callee's
    /// error variant through the boxed conversion (`Into`/`From` through
    /// `Box<dyn Error>`) is not statically established, so the seam stays
    /// below `exposed` even when witnesses pin exact variants via downcast.
    /// This label names the unresolved conversion binding, not a coverage
    /// claim. See #3700.
    WrapperErrorBindingUnresolved,
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
            StaticLimitKind::RustIntegrationPublicApiPathUnresolved => {
                "rust_integration_public_api_path_unresolved"
            }
            StaticLimitKind::RustMacroReachUnresolved => "rust_macro_reach_unresolved",
            StaticLimitKind::RustMacroWrappedTestCallUnresolved => {
                "rust_macro_wrapped_test_call_unresolved"
            }
            StaticLimitKind::RustMacroWrappedAssertionUnresolved => {
                "rust_macro_wrapped_assertion_unresolved"
            }
            StaticLimitKind::RustValuePropagationUnresolved => "rust_value_propagation_unresolved",
            StaticLimitKind::RustSubprocessBinaryReachUnresolved => {
                "rust_subprocess_binary_reach_unresolved"
            }
            StaticLimitKind::WrapperErrorBindingUnresolved => "wrapper_error_binding_unresolved",
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
                 fully trace (macros, generics, trait dispatch, or depth greater than 5). This is a \
                 named limitation, not a coverage claim."
            }
            StaticLimitKind::RustIntegrationPublicApiPathUnresolved => {
                "An integration test may reach this change through a crate public API or test-helper \
                 path ripr cannot fully trace. This is a named limitation, not a coverage claim."
            }
            StaticLimitKind::RustMacroReachUnresolved => {
                "A test may reach this change through a Rust macro path ripr cannot expand. The \
                 classification stays no_static_path because the macro edge is unresolved; this is \
                 a named limitation, not a coverage claim."
            }
            StaticLimitKind::RustMacroWrappedTestCallUnresolved => {
                "A test directly invokes a Rust macro whose definition mentions this change, but \
                 ripr cannot expand the macro to confirm the path. This is a named limitation, not \
                 a coverage claim."
            }
            StaticLimitKind::RustMacroWrappedAssertionUnresolved => {
                "A reachable Rust test uses an assertion-like macro that ripr does not classify, \
                 so the assertion semantics are unresolved. This is a named limitation, not a \
                 coverage claim."
            }
            StaticLimitKind::RustValuePropagationUnresolved => {
                "A changed Rust value binding uses a bounded string or character operation, but \
                 ripr cannot carry that value into the same-owner equality predicate. This is a \
                 named analyzer limitation, not a coverage or repair claim."
            }
            StaticLimitKind::RustSubprocessBinaryReachUnresolved => {
                "An integration test invokes a Cargo-built binary, but ripr cannot yet map that \
                 executable back to the changed owner. This is a named subprocess boundary \
                 limitation, not a reach, receipt, or coverage claim."
            }
            StaticLimitKind::WrapperErrorBindingUnresolved => {
                "The changed line converts a callee's error through a boxed wrapper \
                 (`map_err(Into::into)`), so whether the wrapper faithfully carries the \
                 callee's error variant is not statically established; ripr cannot credit \
                 a downcast witness to this conversion."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #4323: a lost string-literal continuation left ~18-space runs inside
    /// this gloss, which rendered as mid-sentence gaps in human and JSON output.
    #[test]
    fn wrapper_error_binding_gloss_has_no_whitespace_runs() {
        let gloss = StaticLimitKind::WrapperErrorBindingUnresolved.describe();
        assert!(!gloss.contains("  "), "{gloss}");
        assert!(
            gloss.contains("boxed wrapper (`map_err(Into::into)`), so"),
            "{gloss}"
        );
    }

    #[test]
    fn pytest_verify_command_accepts_module_and_legacy_bare_forms() {
        assert!(is_pytest_verify_command(
            "python -m pytest tests/test_pricing.py::test_boundary"
        ));
        assert!(is_pytest_verify_command("pytest tests/test_pricing.py"));
        assert!(!is_pytest_verify_command("python -m pytestx tests"));
        assert!(!is_pytest_verify_command("pytestx tests"));
        assert!(!is_pytest_verify_command("python -m unittest tests.test_x"));
        assert!(!is_pytest_verify_command("python -m pytest"));
    }

    #[test]
    fn language_id_wire_strings_are_stable() {
        assert_eq!(LanguageId::Rust.as_str(), "rust");
        assert_eq!(LanguageId::TypeScript.as_str(), "typescript");
        assert_eq!(LanguageId::JavaScript.as_str(), "javascript");
        assert_eq!(LanguageId::Python.as_str(), "python");
        assert_eq!(LanguageId::Perl.as_str(), "perl");
    }

    #[test]
    fn unavailable_adapter_recovery_names_the_feature_for_non_perl_languages() {
        for language in [
            LanguageId::TypeScript,
            LanguageId::JavaScript,
            LanguageId::Python,
        ] {
            let recovery = language.unavailable_adapter_recovery();
            assert_eq!(
                recovery,
                format!(
                    "rebuild ripr with Cargo feature `{}` to analyze {} files",
                    language.required_feature(),
                    language.as_str()
                )
            );
            assert!(
                !recovery.contains("perl-ripr-facts"),
                "only Perl names the external exporter: {recovery}"
            );
        }
        let perl = LanguageId::Perl.unavailable_adapter_recovery();
        assert!(perl.contains("lang-perl") && perl.contains(PERL_FACT_EXPORTER));
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
    fn unavailable_adapter_notice_restates_recovery_owner_only_when_missing() {
        for language in [
            LanguageId::Rust,
            LanguageId::TypeScript,
            LanguageId::JavaScript,
            LanguageId::Python,
            LanguageId::Perl,
        ] {
            assert_eq!(
                language.unavailable_adapter_notice().is_some(),
                !language.is_available(),
                "{language:?}"
            );
            if let Some(notice) = language.unavailable_adapter_notice() {
                let recovery = language.unavailable_adapter_recovery();
                assert_eq!(
                    notice,
                    format!("{recovery}."),
                    "the pilot notice must restate the recovery owner's wording"
                );
            }
        }
        if !cfg!(feature = "lang-perl") {
            let perl = LanguageId::Perl.unavailable_adapter_notice();
            assert!(
                perl.as_deref().is_some_and(|text| {
                    text.contains("lang-perl")
                        && text.contains(PERL_FACT_EXPORTER)
                        && text.contains("not yet published")
                }),
                "the pilot notice must name both Perl prerequisites: {perl:?}"
            );
        }
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
        assert_eq!(
            StaticLimitKind::RustIntegrationPublicApiPathUnresolved.as_str(),
            "rust_integration_public_api_path_unresolved"
        );
        assert_eq!(
            StaticLimitKind::RustValuePropagationUnresolved.as_str(),
            "rust_value_propagation_unresolved"
        );
        assert_eq!(
            StaticLimitKind::RustSubprocessBinaryReachUnresolved.as_str(),
            "rust_subprocess_binary_reach_unresolved"
        );
        assert_eq!(
            StaticLimitKind::RustMacroReachUnresolved.as_str(),
            "rust_macro_reach_unresolved"
        );
        assert_eq!(
            StaticLimitKind::RustMacroWrappedTestCallUnresolved.as_str(),
            "rust_macro_wrapped_test_call_unresolved"
        );
        assert_eq!(
            StaticLimitKind::RustMacroWrappedAssertionUnresolved.as_str(),
            "rust_macro_wrapped_assertion_unresolved"
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
            StaticLimitKind::RustIntegrationPublicApiPathUnresolved,
            StaticLimitKind::RustMacroReachUnresolved,
            StaticLimitKind::RustMacroWrappedTestCallUnresolved,
            StaticLimitKind::RustMacroWrappedAssertionUnresolved,
            StaticLimitKind::RustValuePropagationUnresolved,
            StaticLimitKind::RustSubprocessBinaryReachUnresolved,
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

    #[test]
    fn rust_transitive_reach_description_matches_depth_contract() {
        assert!(
            StaticLimitKind::RustTransitiveReachUnresolved
                .describe()
                .contains("depth greater than 5"),
            "transitive-reach limitation text must match RIPR-SPEC-0114's depth-5 bound"
        );
    }

    #[test]
    fn display_names_use_product_casing_and_round_trip_wire_strings() {
        let pairs: Vec<(&str, &str)> = LanguageId::ALL
            .into_iter()
            .map(|language| (language.as_str(), language.display_name()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("rust", "Rust"),
                ("typescript", "TypeScript"),
                ("javascript", "JavaScript"),
                ("python", "Python"),
                ("perl", "Perl"),
            ]
        );
        for language in LanguageId::ALL {
            assert_eq!(
                LanguageId::display_name_for_wire(language.as_str()),
                Some(language.display_name())
            );
        }
        assert_eq!(LanguageId::display_name_for_wire("cobol"), None);
    }
}
