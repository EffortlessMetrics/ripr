//! Producer-owned source-role model for Rust files (#3213 / #3283, S2).
//!
//! One typed role replaces the split path predicates that diff probe
//! seeding, repo seam inventory, and evidence selection each re-derived
//! independently. The role is derived from authoritative context in
//! priority order:
//!
//! 1. explicit opt-in (`[analysis] production_like_targets` in
//!    `ripr.toml`) — a repository that intentionally treats a test-support
//!    target as production-like behavior restores ordinary production
//!    analysis for that target only;
//! 2. registered test-harness targets (`[analysis.test_harnesses]`,
//!    #3532) — exact configured custom-harness and registered-test-producer
//!    files are evidence role;
//! 3. declared Cargo targets (`[[test]]` / `[[bench]]` with an explicit
//!    `path = ...`) — confirms evidence role even outside the default
//!    `tests/` / `benches/` layouts, which is what lets a confirmed
//!    `*_test.rs` / `test_*.rs` convention carry evidence role while an
//!    unconfirmed filename stays a production subject;
//! 4. package layout (`tests/`, `benches/`, `examples/`) and the
//!    non-source directories (`fixtures/`, `target/`, `.git/`, `.ripr/`,
//!    `node_modules/`, `editors/`);
//! 5. everything else under a source layout is a production subject.
//!
//! A filename convention alone never classifies a file: without target
//! metadata or layout corroboration, `src/foo_test.rs` remains a
//! production subject.
//!
//! Evidence roles stay fully indexed — functions in test, bench, and
//! fixture files remain available for owner relation, activation input,
//! sink/oracle evidence, selectors, and receipts. They never seed
//! production findings. `TestFact` semantics are untouched: source role
//! never registers a helper as an executable test selector (#3273 kept
//! that separation for inline `#[cfg(test)]` modules; this module keeps
//! it for whole files).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The producer-owned role of one source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceRole {
    /// Ordinary production subject: seeds diff probes and repo seams.
    ProductionSubject,
    /// Cargo integration test (`tests/**` or a declared `[[test]]`
    /// target). Indexed evidence; never a production subject.
    TestEvidence,
    /// Cargo bench (`benches/**` or a declared `[[bench]]` target).
    /// Indexed evidence; harness plumbing seeds no obligations (#3283).
    BenchEvidence,
    /// Cargo example (`examples/**`). Indexed evidence, consistent with
    /// the repo production-set exclusion that already applies today.
    ExampleEvidence,
    /// Registered non-source directories (`fixtures/`, `target/`, `.git/`,
    /// `.ripr/`, `node_modules/`, `editors/`). Skipped by discovery;
    /// classified for a complete, typed model.
    FixtureOrReceiptEvidence,
    /// An explicitly opted-in test-infrastructure target that this
    /// repository treats as production-like behavior. Ordinary production
    /// analysis applies to this target only.
    ProductionLikeTestInfrastructure,
    /// Reserved for ambiguous inputs (generated files, custom harnesses)
    /// that no current producer classifies. Never silently treated as
    /// production: a future producer must name its evidence before any
    /// consumer relies on it. Constructed only by tests until a real
    /// producer exists.
    #[cfg(test)]
    UnknownRole,
}

impl SourceRole {
    /// Whether files with this role seed production findings (diff probes
    /// and repo seam subjects).
    pub(crate) fn seeds_production_findings(self) -> bool {
        matches!(
            self,
            Self::ProductionSubject | Self::ProductionLikeTestInfrastructure
        )
    }

    /// Whether files with this role are indexed evidence inputs.
    #[cfg(test)]
    pub(crate) fn is_evidence(self) -> bool {
        matches!(
            self,
            Self::TestEvidence
                | Self::BenchEvidence
                | Self::ExampleEvidence
                | Self::FixtureOrReceiptEvidence
        )
    }
}

/// Authoritative context beyond package layout: declared Cargo targets,
/// the repository's explicit production-like opt-in, and registered
/// test-harness targets.
#[derive(Clone, Debug, Default)]
pub(crate) struct SourceRoleContext {
    /// Relative paths of files named by explicit `[[test]]` `path = ...`
    /// entries.
    pub(crate) declared_test_targets: BTreeSet<PathBuf>,
    /// Relative paths of files named by explicit `[[bench]]` `path = ...`
    /// entries.
    pub(crate) declared_bench_targets: BTreeSet<PathBuf>,
    /// Relative paths opted in via `[analysis] production_like_targets`.
    pub(crate) production_like_targets: BTreeSet<PathBuf>,
    /// Relative paths registered via `[analysis.test_harnesses]` (#3532):
    /// exact configured custom-harness targets and registered test
    /// producers. Evidence role; the explicit production-like opt-in
    /// still wins over a harness registration.
    pub(crate) harness_targets: BTreeSet<PathBuf>,
}

impl SourceRoleContext {
    /// Context-free classification is the `Default` context: layout rules
    /// only, no target metadata, no opt-in.
    pub(crate) fn empty() -> Self {
        Self::default()
    }
}

/// Classify a workspace-relative path with full authoritative context
/// (priority: opt-in, declared targets, layout, production default).
pub(crate) fn classify_with(path: &Path, context: &SourceRoleContext) -> SourceRole {
    let normalized = normalize(path);
    if context.production_like_targets.contains(&normalized) {
        return SourceRole::ProductionLikeTestInfrastructure;
    }
    if context.harness_targets.contains(&normalized) {
        return SourceRole::TestEvidence;
    }
    if context.declared_test_targets.contains(&normalized) {
        return SourceRole::TestEvidence;
    }
    if context.declared_bench_targets.contains(&normalized) {
        return SourceRole::BenchEvidence;
    }
    classify(&normalized)
}

/// Bounded positive test-surface recognition over a portable repo-relative
/// path, for consumers that must authorize test-only edit surfaces (the
/// repair driver's edit-cage gate). It reuses the analyzer's existing
/// test-file notions rather than forking them: the `tests`/`test` layout
/// component shared by `rust_index::is_test_file` and the Python adapter's
/// `is_test_file`, plus the per-language file-name conventions — `test_*.py`
/// prefixes and `*_test.py`/`*_tests.py`/`*_test.rs`/`*_tests.rs` suffixes.
///
/// The recognition is deliberately bounded and case-sensitive so lookalike
/// names fail closed: `src/testing.py`, `lib/testutil.py`, and unusual
/// casing are refused. This is role-derived evidence for an authorization
/// decision; consumers keep the policy (what to refuse and what to name in
/// the diagnostic) on their side.
pub(crate) fn is_test_surface_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized
        .split('/')
        .any(|component| component == "tests" || component == "test")
    {
        return true;
    }
    let file_name = normalized.rsplit('/').next().unwrap_or_default();
    if file_name.ends_with("_test.rs")
        || file_name.ends_with("_tests.rs")
        || file_name.ends_with("_test.py")
        || file_name.ends_with("_tests.py")
    {
        return true;
    }
    file_name.starts_with("test_") && file_name.ends_with(".py")
}

/// Layout-only classification (context-free base shared by every
/// consumer, including surfaces without Cargo metadata at hand).
///
/// Expects a workspace-relative path; absolute inputs keep their
/// components and still classify, but context-set membership is keyed on
/// the workspace-relative normalized identity.
///
/// The production-subject rules carry over the retired pre-#3283
/// path-predicate contract exactly: a production subject
/// requires a `src` component, is not named `tests.rs`, and is not under
/// `xtask/` or a non-source directory. Anything the old repo predicate
/// excluded stays non-production here, so routing the repo production
/// set through this model cannot widen it.
pub(crate) fn classify(path: &Path) -> SourceRole {
    let normalized = normalize(path);
    let components = normalized.components().collect::<Vec<_>>();
    let has_component = |name: &str| {
        components
            .iter()
            .any(|component| component_name(component) == name)
    };
    if has_component("tests") {
        return SourceRole::TestEvidence;
    }
    // Cargo autodiscovery shapes govern `benches/` and `examples/`:
    // Cargo only finds `<dir>/<name>.rs` and `<dir>/<name>/main.rs`. A
    // path like `examples/sample/src/lib.rs` is NOT discoverable as an
    // example target and stays a production subject — matching the
    // pre-#3283 diff behavior for nested fixtures. `tests/` keeps the
    // broader any-segment rule: in-src module dirs named `tests` are
    // pinned as evidence by existing contracts (#3273 era).
    if cargo_discoverable_under(&components, "benches") {
        return SourceRole::BenchEvidence;
    }
    if cargo_discoverable_under(&components, "examples") {
        return SourceRole::ExampleEvidence;
    }
    if has_component("fixtures")
        || has_component("target")
        || has_component(".git")
        || has_component(".ripr")
        || has_component("node_modules")
        || has_component("editors")
        || has_component("xtask")
    {
        return SourceRole::FixtureOrReceiptEvidence;
    }
    // A file whose stem is exactly `tests` (e.g. `src/tests.rs`) is the
    // module aggregate for inline tests — the pre-#3283 production
    // predicate excluded it and the pin in `workspace::classify` still
    // does.
    if normalized.file_stem().is_some_and(|stem| stem == "tests") {
        return SourceRole::TestEvidence;
    }
    // Production requires a `src` layout: loose root files (build.rs,
    // metrics subjects) were never repo production subjects and stay
    // non-production here.
    if !has_component("src") {
        return SourceRole::FixtureOrReceiptEvidence;
    }
    SourceRole::ProductionSubject
}

/// Whether the path lives under the repository's own automation
/// directory. Automation files are evidence-role for repo-mode indexing,
/// but a *changed* automation file is reviewed behavior: the pre-#3283
/// diff loop seeded probes for every non-test changed file, and the
/// source-owned `source_promotion_verification` fixture pins probe
/// seeding for `xtask/` changes. The Rust diff-probe loop consults this
/// alongside [`SourceRole::seeds_production_findings`] so the evidence
/// taxonomy keeps repo-mode scope without silencing automation diffs.
pub(crate) fn is_repo_automation_path(path: &Path) -> bool {
    normalize(path)
        .components()
        .any(|component| component_name(&component) == "xtask")
}

fn component_name(component: &std::path::Component) -> String {
    component.as_os_str().to_string_lossy().to_string()
}

/// Whether the path below a `dir` component matches a Cargo
/// autodiscovery target shape: `<dir>/<name>.rs` or
/// `<dir>/<name>/main.rs`.
pub(crate) fn cargo_discoverable_under(components: &[std::path::Component], dir: &str) -> bool {
    components.iter().enumerate().any(|(index, component)| {
        if component.as_os_str().to_string_lossy() != dir {
            return false;
        }
        let rest = &components[index + 1..];
        match rest.len() {
            1 => rest[0].as_os_str().to_string_lossy().ends_with(".rs"),
            2 => component_name(&rest[0]) != "src" && component_name(&rest[1]) == "main.rs",
            _ => false,
        }
    })
}

/// Workspace-relative, forward-slashed identity used by every context
/// set, so Windows and POSIX paths compare equal.
fn normalize(path: &Path) -> PathBuf {
    let text = path.to_string_lossy().replace('\\', "/");
    PathBuf::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(path: &str) -> SourceRole {
        classify(Path::new(path))
    }

    #[test]
    fn test_surface_recognition_is_bounded_and_fail_closed() {
        // The bounded edit-surface recognition reuses the analyzer's
        // test-file notions: `tests`/`test` layout components plus the
        // per-language file-name conventions. Lookalike names fail closed.
        for accepted in [
            "tests/pricing.rs",
            "tests/helpers/mod.rs",
            "test/smoke.py",
            "src/test_login.py",
            "src/login_test.py",
            "src/login_test.rs",
            "src/login_tests.rs",
            "src/login_tests.py",
        ] {
            assert!(
                is_test_surface_path(accepted),
                "test surface `{accepted}` was refused"
            );
        }
        for rejected in [
            "src/production.rs",
            "src/testing.py",
            "lib/testutil.py",
            "atest/helper.py",
            "src/mod.rs",
            "tests.rs",
        ] {
            assert!(
                !is_test_surface_path(rejected),
                "non-test path `{rejected}` was accepted"
            );
        }
    }

    #[test]
    fn cargo_autodiscovery_shapes_govern_benches_and_examples() {
        // Cargo only discovers <dir>/<name>.rs and <dir>/<name>/main.rs;
        // nested src layouts below examples/benches are ordinary
        // production subjects (pre-#3283 behavior for
        // crates/ripr/examples/sample/src/lib.rs).
        assert_eq!(role("benches/exposure.rs"), SourceRole::BenchEvidence);
        assert_eq!(role("benches/perf/main.rs"), SourceRole::BenchEvidence);
        assert_eq!(role("examples/demo.rs"), SourceRole::ExampleEvidence);
        assert_eq!(role("examples/demo/main.rs"), SourceRole::ExampleEvidence);
        assert_eq!(
            role("examples/sample/src/lib.rs"),
            SourceRole::ProductionSubject
        );
        assert_eq!(
            role("benches/suite/src/lib.rs"),
            SourceRole::ProductionSubject
        );
    }

    #[test]
    fn repo_production_contract_carries_over_exactly() {
        // The retired path-predicate exclusions must all
        // carry into the role model: routing the repo production set
        // through the role cannot widen it (#3283 review finding).
        assert_eq!(
            role("xtask/src/main.rs"),
            SourceRole::FixtureOrReceiptEvidence
        );
        assert_eq!(role("src/tests.rs"), SourceRole::TestEvidence);
        assert_eq!(role("build.rs"), SourceRole::FixtureOrReceiptEvidence);
        assert_eq!(
            role("metrics/subjects/source.after.rs"),
            SourceRole::FixtureOrReceiptEvidence
        );
        // Production layouts stay production subjects and keep seeding —
        // pinned directly rather than against `classify` (which `role`
        // already calls), so a both-sides flip to evidence role fails
        // here — plus exactly one declared divergence: a nested src
        // layout under examples/ (e.g. ripr's own
        // `crates/ripr/examples/sample/src/lib.rs`) is not a
        // Cargo-discoverable example target, seeded production probes in
        // diff mode since the beginning, and stays a production subject.
        for path in ["src/lib.rs", "crates/x/src/lib.rs"] {
            assert_eq!(
                role(path),
                SourceRole::ProductionSubject,
                "production layout must retain its role for {path}"
            );
            assert!(
                role(path).seeds_production_findings(),
                "production role must seed findings for {path}"
            );
        }
        assert_eq!(
            role("examples/sample/src/lib.rs"),
            SourceRole::ProductionSubject,
            "declared divergence: nested-src demo crates stay production"
        );
    }

    #[test]
    fn layout_classifies_evidence_and_production() {
        assert_eq!(role("tests/pricing.rs"), SourceRole::TestEvidence);
        assert_eq!(role("crates/x/tests/it.rs"), SourceRole::TestEvidence);
        assert_eq!(role("benches/exposure.rs"), SourceRole::BenchEvidence);
        assert_eq!(role("examples/demo.rs"), SourceRole::ExampleEvidence);
        assert_eq!(
            role("fixtures/sample/input.rs"),
            SourceRole::FixtureOrReceiptEvidence
        );
        assert_eq!(role("src/lib.rs"), SourceRole::ProductionSubject);
        // Filename conventions alone never classify (#3283 acceptance):
        // an unconfirmed *_test.rs stays a production subject.
        assert_eq!(role("src/pricing_test.rs"), SourceRole::ProductionSubject);
        assert_eq!(role("src/test_pricing.rs"), SourceRole::ProductionSubject);
    }

    #[test]
    fn declared_targets_and_opt_in_override_layout() {
        let mut context = SourceRoleContext::empty();
        context
            .declared_test_targets
            .insert(PathBuf::from("src/contract_test.rs"));
        context
            .declared_bench_targets
            .insert(PathBuf::from("src/perf.rs"));
        context
            .production_like_targets
            .insert(PathBuf::from("tests/api_contract.rs"));

        assert_eq!(
            classify_with(Path::new("src/contract_test.rs"), &context),
            SourceRole::TestEvidence,
            "a declared [[test]] target confirms the convention"
        );
        assert_eq!(
            classify_with(Path::new("src/perf.rs"), &context),
            SourceRole::BenchEvidence
        );
        assert_eq!(
            classify_with(Path::new("tests/api_contract.rs"), &context),
            SourceRole::ProductionLikeTestInfrastructure,
            "the opt-in restores production analysis for the selected target only"
        );
        assert_eq!(
            classify_with(Path::new("tests/other.rs"), &context),
            SourceRole::TestEvidence,
            "the opt-in does not leak to sibling targets"
        );
        assert_eq!(
            classify_with(Path::new("src/unrelated_test.rs"), &context),
            SourceRole::ProductionSubject,
            "confirmation does not leak to undeclared filenames"
        );
    }

    #[test]
    fn harness_registrations_confirm_evidence_role_but_opt_in_wins() {
        // #3532: an exact registered harness target is evidence role even
        // outside the default layouts; the explicit production-like opt-in
        // stays a separate, stronger control; nothing leaks to siblings.
        let mut context = SourceRoleContext::empty();
        context
            .harness_targets
            .insert(PathBuf::from("src/custom_mimic.rs"));
        context
            .production_like_targets
            .insert(PathBuf::from("tests/opted_in.rs"));
        context
            .declared_test_targets
            .insert(PathBuf::from("src/contract_test.rs"));

        assert_eq!(
            classify_with(Path::new("src/custom_mimic.rs"), &context),
            SourceRole::TestEvidence,
            "a registered harness target is evidence role in any layout"
        );
        assert!(
            !classify_with(Path::new("src/custom_mimic.rs"), &context).seeds_production_findings(),
            "a registered harness target never seeds production seams"
        );
        assert_eq!(
            classify_with(Path::new("tests/opted_in.rs"), &context),
            SourceRole::ProductionLikeTestInfrastructure,
            "the production-like opt-in wins over harness role"
        );
        assert_eq!(
            classify_with(Path::new("tests/other.rs"), &context),
            SourceRole::TestEvidence
        );
        assert_eq!(
            classify_with(Path::new("src/undeclared.rs"), &context),
            SourceRole::ProductionSubject,
            "registration must not leak to unregistered files"
        );
        // Declared-target confirmation and harness registration agree on
        // evidence role; priority between them is not observable.
        assert_eq!(
            classify_with(Path::new("src/contract_test.rs"), &context),
            SourceRole::TestEvidence
        );
    }

    #[test]
    fn windows_paths_match_context_sets() {
        let mut context = SourceRoleContext::empty();
        context
            .declared_test_targets
            .insert(PathBuf::from("src/contract_test.rs"));
        assert_eq!(
            classify_with(Path::new("src\\contract_test.rs"), &context),
            SourceRole::TestEvidence,
            "normalized identity compares across separators"
        );
    }

    #[test]
    fn repo_automation_paths_are_flagged_for_diff_seeding() {
        // `xtask/` stays evidence-role for repo-mode indexing, but the
        // path flag lets the diff-probe loop seed changed automation
        // files (pre-#3283 diff behavior, pinned by the
        // `source_promotion_verification` fixture).
        assert!(super::is_repo_automation_path(Path::new(
            "xtask/src/reports/source_promotion_verify.rs"
        )));
        assert!(super::is_repo_automation_path(Path::new(
            "xtask/src/main.rs"
        )));
        assert!(!super::is_repo_automation_path(Path::new(
            "crates/ripr/src/lib.rs"
        )));
        assert!(!super::is_repo_automation_path(Path::new(
            "fixtures/entropy/input/src/lib.rs"
        )));
    }

    #[test]
    fn seeding_and_evidence_partitions_are_disjoint() {
        for value in [
            SourceRole::ProductionSubject,
            SourceRole::TestEvidence,
            SourceRole::BenchEvidence,
            SourceRole::ExampleEvidence,
            SourceRole::FixtureOrReceiptEvidence,
            SourceRole::ProductionLikeTestInfrastructure,
            SourceRole::UnknownRole,
        ] {
            // Production-like and plain production seed findings; every
            // evidence role and the reserved unknown do not.
            assert_eq!(
                value.seeds_production_findings(),
                matches!(
                    value,
                    SourceRole::ProductionSubject | SourceRole::ProductionLikeTestInfrastructure
                ),
                "{value:?}"
            );
            assert_eq!(
                value.is_evidence(),
                matches!(
                    value,
                    SourceRole::TestEvidence
                        | SourceRole::BenchEvidence
                        | SourceRole::ExampleEvidence
                        | SourceRole::FixtureOrReceiptEvidence
                ),
                "{value:?}"
            );
        }
    }
}
