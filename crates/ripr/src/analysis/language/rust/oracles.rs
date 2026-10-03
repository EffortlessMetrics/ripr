//! Rust-adapter-local oracle and assertion limitation extraction.
//!
//! Shared assertion classification lives in `analysis::extract::oracles`.
//! This module owns only the adapter-local post-classification limitations:
//! unresolved assertion-like macros, wrapper-error binding, and FFI/cross-
//! language oracle visibility. It consumes the lexical mask from `probes`
//! rather than duplicating it.

use super::probes::mask_rust_comments_and_strings;
use crate::analysis::facts::{FunctionSummary, RustIndex};
use crate::domain::{ExposureClass, Finding, Probe, StaticLimitKind};

/// Returns `true` when the owner function carries an FFI or language-binding
/// attribute that indicates its surface may be exercised by an external-language
/// test oracle rather than a Rust test.
///
/// The markers checked are the attribute path segments used by the major
/// Rust FFI and binding crates. `extern "C"` is intentionally excluded: it is
/// an ABI qualifier on the `fn` keyword and is not captured in
/// `FunctionFact.attrs`.
pub(super) fn owner_has_ffi_attr(owner_fn: &FunctionSummary) -> bool {
    owner_fn
        .attrs
        .iter()
        .chain(&owner_fn.impl_attrs)
        .any(|attr| attr_is_ffi_binding(attr))
}

/// PyO3's own attributes (`#[pyfunction]`, `#[pymethods]`, `#[pyclass]`,
/// `#[pymodule]`) do not contain the crate name `pyo3` unless written
/// path-qualified, so each is listed. A method is exposed through the
/// attribute on its `impl` block (`#[pymethods] impl Ledger`,
/// `#[wasm_bindgen] impl Counter`, `#[napi] impl Store`).
const FFI_ATTR_MARKERS: &[&str] = &[
    "no_mangle",
    "export_name",
    "wasm_bindgen",
    "napi",
    "pyo3",
    "pyfunction",
    "pymethods",
    "pyclass",
    "pymodule",
    "uniffi",
    "cxx",
];

/// Match the attribute's parsed path, not its text: `#[doc = "pyfunction
/// helper"]` or `#[my_pyfunction_like]` must not credit a binding. Only the
/// wrappers that carry another attribute as an argument are opened:
/// `#[unsafe(no_mangle)]` (Rust 2024) and `#[cfg_attr(pred, attr, ...)]`,
/// whose predicate is skipped.
pub(super) fn attr_is_ffi_binding(attr: &str) -> bool {
    let body = attr.trim();
    let body = body
        .strip_prefix("#![")
        .or_else(|| body.strip_prefix("#["))
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(body);
    attr_body_is_ffi_binding(body)
}

fn attr_body_is_ffi_binding(body: &str) -> bool {
    let body = body.trim();
    let path_end = body.find(['(', '=', '[', '{']).unwrap_or(body.len());
    let path = body[..path_end].trim();
    let segments: Vec<&str> = path.split("::").map(str::trim).collect();
    if segments
        .iter()
        .any(|segment| FFI_ATTR_MARKERS.contains(&segment.to_lowercase().as_str()))
    {
        return true;
    }
    let args = body[path_end..]
        .trim()
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'));
    let Some(args) = args else {
        return false;
    };
    match path {
        "unsafe" => attr_body_is_ffi_binding(args),
        "cfg_attr" => split_top_level_args(args)
            .into_iter()
            .skip(1)
            .any(attr_body_is_ffi_binding),
        _ => false,
    }
}

/// Split attribute arguments on commas outside nested delimiters and string
/// literals.
fn split_top_level_args(args: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, ch) in args.char_indices() {
        if in_string {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&args[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&args[start..]);
    parts
}

/// Resolve the probe's owner function from the index and check for FFI attrs.
/// Returns `Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)` when
/// the probe owner is FFI/binding-exposed and the finding class is an
/// unrevealed gap; `None` otherwise. Pure-Rust owners (no FFI attrs) return
/// `None` unconditionally.
pub(super) fn cross_language_limit_kind(
    probe: &crate::domain::Probe,
    index: &RustIndex,
    class: &ExposureClass,
) -> Option<StaticLimitKind> {
    // `NoStaticPath` is included: an FFI owner tested only from the other
    // language has no Rust test path by construction, and RIPR-SPEC-0062
    // forbids flattening that to a bare `no_static_path`.
    let is_gap_class = matches!(
        class,
        ExposureClass::WeaklyExposed
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::InfectionUnknown
            | ExposureClass::NoStaticPath
    );
    if !is_gap_class {
        return None;
    }
    let owner_id = probe.owner.as_ref()?;
    let owner_fn = index
        .functions
        .iter()
        .find(|function| &function.id == owner_id)?;
    if owner_has_ffi_attr(owner_fn) {
        Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
    } else {
        None
    }
}

/// Attach the cross-language limitation to an FFI-exposed owner's gap finding.
/// A `no_static_path` finding's generic next step tells the reader to add a
/// co-located Rust test, which is the wrong repair when the tests live in
/// the other language, so it takes the limitation's own guidance instead.
/// A `no_static_path` finding that already names a limitation keeps it: that
/// limitation (a transitive or macro reach witness, for example) points at a
/// Rust test the finding's evidence lines already describe.
pub(super) fn apply_cross_language_limit(finding: &mut Finding, probe: &Probe, index: &RustIndex) {
    let Some(limit) = cross_language_limit_kind(probe, index, &finding.class) else {
        return;
    };
    if finding.class == ExposureClass::NoStaticPath {
        if finding.static_limit_kind.is_some() {
            return;
        }
        finding.recommended_next_step = Some(limit.describe().to_string());
    }
    finding.static_limit_kind = Some(limit);
}

/// Borrowed, pass-local lookup. Each source identifier is indexed once, never
/// by copying a function/body. Empty inputs make every later lookup constant
/// time; no-path findings do not rescan all files or macro token trees.
pub(super) struct PropertyMacroMentionIndex<'a> {
    captured_root: Option<&'a std::path::Path>,
    selected_root: &'a std::path::Path,
    by_identifier: std::collections::BTreeMap<&'a str, Vec<PropertyMacroLocation<'a>>>,
    package_authority: Option<
        &'a std::collections::BTreeMap<
            std::path::PathBuf,
            crate::analysis::facts::WorkspaceFileAuthority,
        >,
    >,
}

struct PropertyMacroLocation<'a> {
    file: &'a std::path::Path,
    witness: &'a crate::analysis::facts::UnresolvedPropertyMacroFact,
}

impl<'a> PropertyMacroMentionIndex<'a> {
    pub(super) fn new(index: &'a RustIndex, selected_root: &'a std::path::Path) -> Self {
        let mut by_identifier = std::collections::BTreeMap::new();
        for (file, facts) in &index.files {
            for witness in &facts.unresolved_property_macros {
                for identifier in &witness.mentioned_identifiers {
                    by_identifier
                        .entry(identifier.as_str())
                        .or_insert_with(Vec::new)
                        .push(PropertyMacroLocation {
                            file: file.as_path(),
                            witness,
                        });
                }
            }
        }
        Self {
            by_identifier,
            selected_root,
            captured_root: index
                .workspace_authority
                .as_ref()
                .map(|authority| authority.root.as_path()),
            package_authority: index
                .workspace_authority
                .as_ref()
                .map(|authority| &authority.files),
        }
    }

    fn known_package(&self, indexed_file: &std::path::Path) -> Option<&str> {
        self.package_authority?
            .get(indexed_file)
            .filter(|authority| authority.valid)
            .map(|authority| authority.package_identity.as_str())
    }

    fn owner_package(&self, probe_file: &std::path::Path) -> Option<&str> {
        // Probe paths are constructed from the selected analysis root, unlike
        // the root-relative witness keys. Strip that exact prefix first so a
        // decoy indexed path beginning with the same root text cannot win.
        let relative = probe_file
            .strip_prefix(self.selected_root)
            .ok()
            .or_else(|| probe_file.strip_prefix(self.captured_root?).ok())?;
        self.known_package(relative)
    }
}

/// A suffix/name match is a possible lexical mention, never resolved reach.
/// A property block may generate tests, or may discard all its tokens.
pub(super) fn apply_unresolved_property_macro_limit(
    finding: &mut Finding,
    owner_name: &str,
    owner_file: &std::path::Path,
    mentions: &PropertyMacroMentionIndex<'_>,
) -> bool {
    let Some(locations) = mentions.by_identifier.get(owner_name) else {
        return false;
    };
    // This is retained manifest/source authority, not a path-layout guess.
    // No per-finding filesystem reads or manifest resolution are performed.
    let owner_package = mentions.owner_package(owner_file);
    let Some(PropertyMacroLocation { file, witness }) = locations.iter().find(|location| {
        match (owner_package, mentions.known_package(location.file)) {
            (Some(owner), Some(mentioned)) => owner == mentioned,
            // An unresolved package identity cannot prove unrelatedness.
            _ => true,
        }
    }) else {
        return false;
    };
    // A lexical mention supplies no runtime edge. Keep all affected stages
    // explicitly unresolved instead of inheriting discarded argument values.
    for stage in [
        &mut finding.ripr.reach,
        &mut finding.ripr.infect,
        &mut finding.ripr.propagate,
    ] {
        let old_summary = stage.summary.clone();
        stage.state = crate::domain::StageState::Unknown;
        stage.confidence = crate::domain::Confidence::Low;
        stage.summary = "Property macro expansion and owner execution are unresolved".to_string();
        finding.evidence.retain(|evidence| evidence != &old_summary);
    }
    finding.static_limit_kind = Some(StaticLimitKind::RustMacroReachUnresolved);
    finding
        .stop_reasons
        .push(crate::domain::StopReason::MacroReachUnresolved);
    finding.evidence.push(format!(
        "Opaque property macro `{}!` at {}:{} lexically mentions `{owner_name}`; macro provenance, test collection and execution are unresolved.",
        witness.name, file.display().to_string().replace('\\', "/"), witness.line,
    ));
    finding.evidence.extend([
        format!("limitation_last_established_edge: source invocation `{}!` at {}:{}", witness.name, file.display().to_string().replace('\\', "/"), witness.line),
        "limitation_first_unresolved_edge: property macro expansion, executable-test collection and owner reach".to_string(),
        "limitation_analyzer_route: analysis/rust-property-macro-provenance".to_string(),
        "limitation_non_claim: lexical mention only; no test existence, absence, execution, reach or assertion discrimination is established".to_string(),
    ]);
    finding.recommended_next_step = Some(
        "Inspect the property macro's definition and collected tests, then run its existing test suite; this limitation does not establish a missing test.".to_string(),
    );
    true
}

pub(super) fn apply_rust_macro_wrapped_assertion_limit(finding: &mut Finding, index: &RustIndex) {
    if !(finding.class == ExposureClass::ReachableUnrevealed
        && !finding.related_tests.is_empty()
        && finding.static_limit_kind.is_none()
        && finding.ripr.reveal.observe.state == crate::domain::StageState::No
        && finding
            .related_tests
            .iter()
            .all(|related| related.oracle.is_none()))
    {
        return;
    }

    let Some(witness) = find_unresolved_assertion_macro_witness(finding, index) else {
        return;
    };

    finding.static_limit_kind = Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved);
    if matches!(
        witness.macro_name.rsplit("::").next(),
        Some("prop_assert" | "prop_assert_eq" | "prop_assert_ne")
    ) {
        finding.recommended_next_step = Some(
            "Inspect the property assertion macro's definition and run the existing test; its spelling does not establish assertion semantics.".to_string(),
        );
    }
    finding.evidence.push(
        "A related Rust test uses an assertion-like macro that ripr does not classify as an oracle."
            .to_string(),
    );
    finding
        .evidence
        .push(rust_macro_assertion_witness_pointer(&witness));
    finding
        .evidence
        .extend(rust_macro_assertion_limitation_detail_lines(&witness));
}

/// #3700 (final consolidation): a wrapper error seam — a `map_err`
/// conversion whose changed expression carries no parseable error variant —
/// carries the typed `wrapper_error_binding_unresolved` limitation. Whether
/// the boxed conversion faithfully carries the converted callee's error
/// variant (`Into`/`From` through `Box<dyn Error>`) is not statically
/// establishable, so the seam stays below `exposed` and the limitation names
/// what ripr could not resolve instead of prescribing an assertion the suite
/// may already contain. Classification (already `weakly_exposed` via the
/// unconfirmed-observation rule in reveal) is unchanged.
pub(super) fn apply_wrapper_error_binding_limit(finding: &mut Finding, probe: &Probe) {
    if finding.class != ExposureClass::WeaklyExposed || finding.static_limit_kind.is_some() {
        return;
    }
    if !matches!(
        probe.family,
        crate::domain::ProbeFamily::ErrorPath | crate::domain::ProbeFamily::ReturnValue
    ) {
        return;
    }
    if !crate::analysis::classify::wrapper_error_seam_expression(&[probe.expression.as_str()]) {
        return;
    }

    finding.static_limit_kind = Some(StaticLimitKind::WrapperErrorBindingUnresolved);
    finding.evidence.push(
        "limitation_last_established_edge: changed wrapper conversion maps the converted callee's error into the boxed error channel".to_string(),
    );
    finding.evidence.push(
        "limitation_first_unresolved_edge: whether `Into`/`From` through `Box<dyn Error>` carries the callee's error variant to the wrapper's callers".to_string(),
    );
    finding
        .evidence
        .push("limitation_analyzer_route: analysis/wrapper-error-binding".to_string());
    finding.evidence.push(
        "limitation_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
            .to_string(),
    );
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RustMacroAssertionWitness {
    test_name: String,
    test_file: std::path::PathBuf,
    test_line: usize,
    macro_name: String,
    macro_line: usize,
}

fn find_unresolved_assertion_macro_witness(
    finding: &Finding,
    index: &RustIndex,
) -> Option<RustMacroAssertionWitness> {
    let mut candidates = Vec::new();
    for test in index
        .tests
        .iter()
        .chain(index.files.values().flat_map(|file| file.tests.iter()))
    {
        if !finding
            .related_tests
            .iter()
            .any(|related| related.name == test.name && related.file == test.file)
        {
            continue;
        }
        for (macro_name, macro_line) in
            unresolved_assertion_macro_invocations(&test.body, test.start_line)
        {
            candidates.push(RustMacroAssertionWitness {
                test_name: test.name.clone(),
                test_file: test.file.clone(),
                test_line: test.start_line,
                macro_name,
                macro_line,
            });
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates.into_iter().next()
}

fn unresolved_assertion_macro_invocations(body: &str, start_line: usize) -> Vec<(String, usize)> {
    let mut invocations = Vec::new();
    let masked_body = mask_rust_comments_and_strings(body);
    for (offset, line) in masked_body.lines().enumerate() {
        let mut search_start = 0usize;
        while let Some(relative_bang) = line[search_start..].find('!') {
            let bang = search_start + relative_bang;
            search_start = bang.saturating_add(1);
            if line[bang + 1..].starts_with('=') {
                continue;
            }
            if !line[bang + 1..]
                .trim_start()
                .chars()
                .next()
                .is_some_and(|ch| matches!(ch, '(' | '[' | '{'))
            {
                continue;
            }
            let Some(macro_name) = macro_name_before_bang(line, bang) else {
                continue;
            };
            if !is_unresolved_assertion_like_macro(&macro_name) {
                continue;
            }
            invocations.push((macro_name, start_line + offset));
        }
    }
    invocations.sort();
    invocations.dedup();
    invocations
}

fn macro_name_before_bang(line: &str, bang: usize) -> Option<String> {
    let prefix = line[..bang].trim_end();
    let end = prefix.len();
    if end == 0 {
        return None;
    }
    let mut start = end;
    for (idx, ch) in prefix.char_indices().rev() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' {
            start = idx;
        } else {
            break;
        }
    }
    let name = prefix[start..end].trim_matches(':');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn is_unresolved_assertion_like_macro(macro_name: &str) -> bool {
    if is_known_rust_assertion_macro(macro_name) {
        return false;
    }
    let base = macro_name.rsplit("::").next().unwrap_or(macro_name);
    base == "assert"
        || base.starts_with("assert_")
        || matches!(base, "prop_assert" | "prop_assert_eq" | "prop_assert_ne")
}

fn is_known_rust_assertion_macro(macro_name: &str) -> bool {
    let compact = macro_name.replace(' ', "");
    let base = compact.rsplit("::").next().unwrap_or(compact.as_str());
    matches!(
        base,
        "assert" | "assert_eq" | "assert_ne" | "assert_matches" | "matches"
    ) || compact.starts_with("insta::assert")
        || compact.contains("snapshot")
}

fn rust_source_pointer(path: &std::path::Path, line: usize) -> String {
    format!("{}:{}", path.display().to_string().replace('\\', "/"), line)
}

fn rust_macro_assertion_witness_pointer(witness: &RustMacroAssertionWitness) -> String {
    let test_location = rust_source_pointer(&witness.test_file, witness.test_line);
    let macro_location = rust_source_pointer(&witness.test_file, witness.macro_line);
    format!(
        "{}`{}` ({}) reaches the changed owner, then invokes assertion-like macro `{}!` at {}. ripr does not classify that macro as an oracle.",
        crate::domain::TRANSITIVE_REACH_WITNESS_PREFIX,
        witness.test_name,
        test_location,
        witness.macro_name,
        macro_location
    )
}

fn rust_macro_assertion_limitation_detail_lines(
    witness: &RustMacroAssertionWitness,
) -> [String; 4] {
    let test_location = rust_source_pointer(&witness.test_file, witness.test_line);
    let macro_location = rust_source_pointer(&witness.test_file, witness.macro_line);
    [
        format!(
            "{}test `{}` ({}) -> assertion macro `{}!` at {}",
            crate::domain::LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
            witness.test_name,
            test_location,
            witness.macro_name,
            macro_location
        ),
        format!(
            "{}assertion macro `{}!` semantics toward the changed owner",
            crate::domain::LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX,
            witness.macro_name
        ),
        format!(
            "{}analysis/rust-macro-assertion-oracle",
            crate::domain::LIMITATION_ANALYZER_ROUTE_PREFIX
        ),
        format!(
            "{}named limitation only; ripr cannot confirm or deny that the macro assertion discriminates the change",
            crate::domain::LIMITATION_NON_CLAIM_PREFIX
        ),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn property_scope_uses_only_valid_retained_package_authority() {
        use crate::analysis::facts::{UnresolvedPropertyMacroFact, WorkspaceFileAuthority};
        use std::collections::BTreeMap;
        use std::path::{Path, PathBuf};
        let owner = Path::new("crates/owner/src/lib.rs");
        let witness_file = Path::new("custom/nested/src/property.rs");
        let absolute_owner = Path::new("/captured").join(owner);
        let selected_owner = Path::new("selected/root").join(owner);
        let witness = UnresolvedPropertyMacroFact {
            name: "proptest".into(),
            line: 1,
            mentioned_identifiers: vec!["gate".into()],
        };
        for (package, valid, limited) in [
            ("owner-manifest", true, true),
            ("other-manifest", true, false),
            ("other-manifest", false, true),
        ] {
            let authorities = BTreeMap::from([
                (
                    owner.to_path_buf(),
                    WorkspaceFileAuthority {
                        source_digest: String::new(),
                        package_identity: "owner-manifest".into(),
                        valid: true,
                    },
                ),
                (
                    selected_owner.clone(),
                    WorkspaceFileAuthority {
                        source_digest: String::new(),
                        package_identity: "decoy".into(),
                        valid: true,
                    },
                ),
                (
                    witness_file.to_path_buf(),
                    WorkspaceFileAuthority {
                        source_digest: String::new(),
                        package_identity: package.into(),
                        valid,
                    },
                ),
            ]);
            let mentions = super::PropertyMacroMentionIndex {
                captured_root: Some(Path::new("/captured")),
                selected_root: Path::new("selected/root"),
                by_identifier: BTreeMap::from([(
                    "gate",
                    vec![super::PropertyMacroLocation {
                        file: witness_file,
                        witness: &witness,
                    }],
                )]),
                package_authority: Some(&authorities),
            };
            for owner_path in [&absolute_owner, &selected_owner] {
                let mut finding = no_static_path_finding();
                assert_eq!(
                    super::apply_unresolved_property_macro_limit(
                        &mut finding,
                        "gate",
                        owner_path,
                        &mentions
                    ),
                    limited
                );
            }
        }
        let absent = BTreeMap::<PathBuf, WorkspaceFileAuthority>::new();
        for package_authority in [None, Some(&absent)] {
            let mentions = super::PropertyMacroMentionIndex {
                captured_root: Some(Path::new("/captured")),
                selected_root: Path::new("selected/root"),
                by_identifier: BTreeMap::from([(
                    "gate",
                    vec![super::PropertyMacroLocation {
                        file: witness_file,
                        witness: &witness,
                    }],
                )]),
                package_authority,
            };
            assert!(super::apply_unresolved_property_macro_limit(
                &mut no_static_path_finding(),
                "gate",
                &absolute_owner,
                &mentions
            ));
        }
    }

    use super::{
        apply_cross_language_limit, apply_rust_macro_wrapped_assertion_limit,
        apply_wrapper_error_binding_limit, attr_is_ffi_binding, cross_language_limit_kind,
        is_known_rust_assertion_macro, is_unresolved_assertion_like_macro, owner_has_ffi_attr,
        unresolved_assertion_macro_invocations,
    };
    use crate::analysis::facts::{
        CallFact, FunctionSourceRole, FunctionSummary, LiteralFact, RustIndex, TestSummary,
    };
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, StaticLimitKind, SymbolId,
    };
    use std::path::PathBuf;

    fn ffi_function(file: &str, name: &str, attrs: Vec<&str>) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 5,
            body: format!("pub fn {name}(x: i32) -> i32 {{ x }}"),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: attrs.into_iter().map(|s| s.to_string()).collect(),
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            impl_context: Default::default(),
            item: Default::default(),
        }
    }

    fn probe_for_owner(file: &str, name: &str, family: ProbeFamily) -> Probe {
        Probe {
            id: ProbeId(format!("probe:{file}::{name}")),
            location: SourceLocation::new(file, 2, 1),
            owner: Some(SymbolId(format!("{file}::{name}"))),
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some("x > 0".to_string()),
            expression: "x > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        }
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "stage")
    }

    fn reachable_unrevealed_finding_with_related_test(
        test_name: &str,
        test_file: &str,
        test_line: usize,
    ) -> Finding {
        Finding {
            id: "probe:src_lib.rs:predicate:test".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib.rs:predicate:test".to_string()),
                location: SourceLocation::new("src/lib.rs", 2, 1),
                owner: Some(SymbolId("src/lib.rs::inner".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("if a >= b {".to_string()),
                expression: "if a >= b {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::ReachableUnrevealed,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes),
                infect: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                reveal: RevealEvidence {
                    observe: stage(StageState::No),
                    discriminate: stage(StageState::No),
                },
            },
            confidence: 0.48,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests_matched_total: None,
            related_tests: vec![RelatedTest {
                name: test_name.to_string(),
                file: PathBuf::from(test_file),
                line: test_line,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::None,
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
        }
    }

    fn test_summary(name: &str, file: &str, start_line: usize, body: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line,
            end_line: start_line + body.lines().count(),
            body: body.to_string(),
            calls: vec![CallFact {
                line: start_line,
                name: "inner".to_string(),
                text: "inner(10, 3)".to_string(),
            }],
            assertions: Vec::new(),
            literals: vec![
                LiteralFact {
                    line: start_line,
                    value: "10".to_string(),
                },
                LiteralFact {
                    line: start_line + 1,
                    value: "7".to_string(),
                },
            ],
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    #[test]
    fn known_assertion_macros_are_not_unresolved_even_when_path_qualified() {
        for name in [
            "assert",
            "assert_eq",
            "assert_ne",
            "assert_matches",
            "matches",
            "pretty_assertions::assert_eq",
            "insta::assert_snapshot",
            "insta::assert_debug_snapshot",
            "crate::snapshot_assert",
        ] {
            assert!(
                is_known_rust_assertion_macro(name),
                "{name} must remain a known oracle macro"
            );
            assert!(
                !is_unresolved_assertion_like_macro(name),
                "{name} must not be named as an unresolved assertion-like macro"
            );
        }
        // Documented over-credit: any compact name containing "snapshot" is
        // treated as known, so a custom `assert_*snapshot*` macro is not
        // named as unresolved. Pin that rather than silently tightening the
        // matcher in this extraction slice.
        assert!(is_known_rust_assertion_macro("assert_custom_snapshot"));
        assert!(!is_unresolved_assertion_like_macro(
            "assert_custom_snapshot"
        ));
    }

    #[test]
    fn custom_assert_star_macros_are_unresolved_and_std_assert_is_not() {
        assert!(is_unresolved_assertion_like_macro("assert_result"));
        assert!(is_unresolved_assertion_like_macro("assert_ok"));
        assert!(is_unresolved_assertion_like_macro("crate::assert_ok"));
        assert!(!is_unresolved_assertion_like_macro("assert_eq"));
        assert!(!is_unresolved_assertion_like_macro("ensure"));
        assert!(!is_unresolved_assertion_like_macro("println"));
    }

    #[test]
    fn unresolved_assertion_scan_skips_not_equal_and_non_invocation_bangs() {
        let invocations = unresolved_assertion_macro_invocations(
            "let ok = flag != false;\nlet _ = assert_ok!(value);\nmacro_rules! assert_ok { () => {}; }\n",
            4,
        );
        assert_eq!(invocations, vec![("assert_ok".to_string(), 5)]);
    }

    #[test]
    fn macro_wrapped_assertion_limit_names_reachable_unobserved_assertion_macro() {
        let mut finding = reachable_unrevealed_finding_with_related_test(
            "test_inner_with_custom_assertion_macro",
            "tests/it.rs",
            4,
        );
        let index = RustIndex {
            tests: vec![test_summary(
                "test_inner_with_custom_assertion_macro",
                "tests/it.rs",
                4,
                "let result = inner(10, 3);\nassert_result!(result, 7);",
            )],
            ..RustIndex::default()
        };

        apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);

        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved)
        );
        assert!(finding.evidence.iter().any(|line| {
            line.contains("assertion-like macro `assert_result!` at tests/it.rs:5")
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_last_established_edge: test `test_inner_with_custom_assertion_macro` (tests/it.rs:4) -> assertion macro `assert_result!` at tests/it.rs:5"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_first_unresolved_edge: assertion macro `assert_result!` semantics toward the changed owner"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_analyzer_route: analysis/rust-macro-assertion-oracle"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_non_claim: named limitation only; ripr cannot confirm or deny that the macro assertion discriminates the change"
        }));
    }

    #[test]
    fn macro_wrapped_assertion_limit_ignores_known_assertion_macros() {
        let mut finding = reachable_unrevealed_finding_with_related_test(
            "test_inner_with_known_assertion_macro",
            "tests/it.rs",
            4,
        );
        let index = RustIndex {
            tests: vec![test_summary(
                "test_inner_with_known_assertion_macro",
                "tests/it.rs",
                4,
                "let result = inner(10, 3);\nassert_eq!(result, 7);",
            )],
            ..RustIndex::default()
        };

        apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);

        assert_eq!(finding.static_limit_kind, None);
        assert!(finding.evidence.is_empty());
    }

    #[test]
    fn macro_wrapped_assertion_limit_ignores_comments_and_string_literals() {
        let mut finding = reachable_unrevealed_finding_with_related_test(
            "test_inner_with_commented_assertion_macro",
            "tests/it.rs",
            4,
        );
        let index = RustIndex {
            tests: vec![test_summary(
                "test_inner_with_commented_assertion_macro",
                "tests/it.rs",
                4,
                r##"let result = inner(10, 3);
// assert_result!(result, 7);
/* assert_block_result!(result, 7); */
let note = "assert_string_result!(result, 7)";
let raw = r#"assert_raw_result!(result, 7)"#;
let _ = (result, note, raw);"##,
            )],
            ..RustIndex::default()
        };

        apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);

        assert_eq!(finding.static_limit_kind, None);
        assert!(finding.evidence.is_empty());
    }

    #[test]
    fn wrapper_error_binding_limit_names_map_err_error_and_return_families_only() {
        let mut finding = reachable_unrevealed_finding_with_related_test("wraps", "tests/it.rs", 4);
        finding.class = ExposureClass::WeaklyExposed;
        finding.probe.family = ProbeFamily::ErrorPath;
        finding.probe.expression = "try_parse(raw).map_err(Into::into)".to_string();
        let probe = finding.probe.clone();
        apply_wrapper_error_binding_limit(&mut finding, &probe);
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::WrapperErrorBindingUnresolved)
        );

        let mut returned =
            reachable_unrevealed_finding_with_related_test("wraps", "tests/it.rs", 4);
        returned.class = ExposureClass::WeaklyExposed;
        returned.probe.family = ProbeFamily::ReturnValue;
        returned.probe.expression = "try_parse(raw).map_err(Into::into)".to_string();
        let probe = returned.probe.clone();
        apply_wrapper_error_binding_limit(&mut returned, &probe);
        assert_eq!(
            returned.static_limit_kind,
            Some(StaticLimitKind::WrapperErrorBindingUnresolved)
        );

        let mut predicate =
            reachable_unrevealed_finding_with_related_test("wraps", "tests/it.rs", 4);
        predicate.class = ExposureClass::WeaklyExposed;
        predicate.probe.family = ProbeFamily::Predicate;
        predicate.probe.expression = "try_parse(raw).map_err(Into::into)".to_string();
        let probe = predicate.probe.clone();
        apply_wrapper_error_binding_limit(&mut predicate, &probe);
        assert_eq!(predicate.static_limit_kind, None);

        let mut already_named =
            reachable_unrevealed_finding_with_related_test("wraps", "tests/it.rs", 4);
        already_named.class = ExposureClass::WeaklyExposed;
        already_named.probe.family = ProbeFamily::ErrorPath;
        already_named.probe.expression = "try_parse(raw).map_err(Into::into)".to_string();
        already_named.static_limit_kind =
            Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved);
        let probe = already_named.probe.clone();
        apply_wrapper_error_binding_limit(&mut already_named, &probe);
        assert_eq!(
            already_named.static_limit_kind,
            Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved)
        );
    }

    #[test]
    fn owner_with_no_mangle_attr_is_ffi() {
        let owner = ffi_function("src/lib.rs", "ffi_fn", vec!["#[no_mangle]"]);
        assert!(owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_wasm_bindgen_attr_is_ffi() {
        let owner = ffi_function("src/lib.rs", "wasm_fn", vec!["#[wasm_bindgen]"]);
        assert!(owner_has_ffi_attr(&owner));
    }

    #[test]
    fn remaining_ffi_markers_are_recognized_and_plain_attrs_are_not() {
        for marker in [
            "#[export_name = \"x\"]",
            "#[napi]",
            "#[pyo3]",
            "#[uniffi]",
            "#[cxx::bridge]",
        ] {
            let owner = ffi_function("src/lib.rs", "exported", vec![marker]);
            assert!(owner_has_ffi_attr(&owner), "{marker} should be FFI");
        }
        let owner = ffi_function("src/lib.rs", "plain_fn", vec!["#[test]"]);
        assert!(!owner_has_ffi_attr(&owner));
        let empty = ffi_function("src/lib.rs", "pure_fn", vec![]);
        assert!(!owner_has_ffi_attr(&empty));
    }

    #[test]
    fn cross_language_guard_fires_for_weakly_exposed_with_ffi_attr() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        );
    }

    #[test]
    fn cross_language_guard_fires_for_reachable_unrevealed_with_wasm_bindgen() {
        let owner = ffi_function("src/lib.rs", "wasm_fn", vec!["#[wasm_bindgen]"]);
        let probe = probe_for_owner("src/lib.rs", "wasm_fn", ProbeFamily::ReturnValue);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::ReachableUnrevealed);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        );
    }

    #[test]
    fn cross_language_guard_fires_for_infection_unknown_with_ffi_attr() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::InfectionUnknown);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        );
    }

    #[test]
    fn cross_language_guard_does_not_fire_for_pure_rust_owner_weakly_exposed() {
        let owner = ffi_function("src/lib.rs", "pure_fn", vec![]);
        let probe = probe_for_owner("src/lib.rs", "pure_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed);
        assert_eq!(result, None);
    }

    #[test]
    fn cross_language_guard_skips_exposed_and_names_no_static_path_with_ffi() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::ReturnValue);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        assert_eq!(
            cross_language_limit_kind(&probe, &index, &ExposureClass::Exposed),
            None
        );
        // RIPR-SPEC-0062: a binding owner tested only from the other language
        // has no Rust test path by construction, so no_static_path names the
        // cross-language limitation instead of flattening to a bare gap.
        assert_eq!(
            cross_language_limit_kind(&probe, &index, &ExposureClass::NoStaticPath),
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        );
    }

    #[test]
    fn cross_language_guard_does_not_fire_without_an_owner_id() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let mut probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        probe.owner = None;
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        assert_eq!(
            cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed),
            None
        );
    }

    fn no_static_path_finding() -> Finding {
        let mut finding = reachable_unrevealed_finding_with_related_test("t", "tests/it.rs", 1);
        finding.class = ExposureClass::NoStaticPath;
        finding.related_tests.clear();
        finding.probe = probe_for_owner("src/lib.rs", "inner", ProbeFamily::Predicate);
        finding
    }

    #[test]
    fn ffi_attr_matches_the_parsed_path_not_the_attribute_text() {
        for attr in [
            "#[unsafe(no_mangle)]",
            "#[export_name = \"fee\"]",
            "#[wasm_bindgen::prelude::wasm_bindgen(js_name = fee)]",
            "#[pyo3::pyfunction]",
            "#[cfg_attr(feature = \"python\", pyfunction)]",
            "#[cfg_attr(feature = \"python\", pyo3::pymethods)]",
            "#[uniffi::export]",
        ] {
            assert!(attr_is_ffi_binding(attr), "{attr} must mark an FFI owner");
        }
        for attr in [
            "#[doc = \"pyfunction helper\"]",
            "/// exported with no_mangle",
            "#[my_pyfunction_like]",
            "#[cfg_attr(feature = \"pyfunction\", derive(Debug))]",
            "#[cfg(feature = \"napi\")]",
            "#[allow(clippy::pyfunction)]",
            "#[unsafe(link_section = \".napi\")]",
        ] {
            assert!(
                !attr_is_ffi_binding(attr),
                "{attr} must not mark an FFI owner"
            );
        }
    }

    #[test]
    fn pyo3_binding_attrs_are_ffi_on_the_function_or_its_impl() {
        // PyO3's attributes do not contain the string `pyo3`.
        for attr in ["#[pyfunction]", "#[pyclass]", "#[pymodule]"] {
            let owner = ffi_function("src/lib.rs", "fee", vec![attr]);
            assert!(owner_has_ffi_attr(&owner), "{attr} must mark an FFI owner");
        }
        let mut method = ffi_function("src/lib.rs", "charge", vec![]);
        assert!(!owner_has_ffi_attr(&method));
        method.impl_attrs = vec!["#[pymethods]".to_string()];
        assert!(
            owner_has_ffi_attr(&method),
            "a method is exposed through its impl block's binding attribute"
        );
        let mut plain_method = ffi_function("src/lib.rs", "charge", vec![]);
        plain_method.impl_attrs = vec!["#[derive(Debug)]".to_string()];
        assert!(!owner_has_ffi_attr(&plain_method));
    }

    #[test]
    fn cross_language_limit_replaces_the_co_located_test_step_on_no_static_path() {
        let index = RustIndex {
            functions: vec![ffi_function("src/lib.rs", "inner", vec!["#[pyfunction]"])],
            ..RustIndex::default()
        };
        let mut finding = no_static_path_finding();
        finding.recommended_next_step = Some("add a co-located test".to_string());
        let probe = finding.probe.clone();
        apply_cross_language_limit(&mut finding, &probe, &index);
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        );
        assert_eq!(
            finding.recommended_next_step.as_deref(),
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved.describe())
        );

        // A gap class that already has reach keeps its own next step.
        let mut weak = no_static_path_finding();
        weak.class = ExposureClass::WeaklyExposed;
        weak.recommended_next_step = Some("strengthen the assertion".to_string());
        apply_cross_language_limit(&mut weak, &probe, &index);
        assert_eq!(
            weak.static_limit_kind,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        );
        assert_eq!(
            weak.recommended_next_step.as_deref(),
            Some("strengthen the assertion")
        );

        // A no_static_path finding that already names a Rust reach
        // limitation keeps it and its next step.
        let mut witnessed = no_static_path_finding();
        witnessed.static_limit_kind = Some(StaticLimitKind::RustTransitiveReachUnresolved);
        witnessed.recommended_next_step = Some("open the witnessing test".to_string());
        apply_cross_language_limit(&mut witnessed, &probe, &index);
        assert_eq!(
            witnessed.static_limit_kind,
            Some(StaticLimitKind::RustTransitiveReachUnresolved)
        );
        assert_eq!(
            witnessed.recommended_next_step.as_deref(),
            Some("open the witnessing test")
        );

        // A pure-Rust owner is untouched.
        let pure = RustIndex {
            functions: vec![ffi_function("src/lib.rs", "inner", vec![])],
            ..RustIndex::default()
        };
        let mut plain = no_static_path_finding();
        plain.recommended_next_step = Some("add a co-located test".to_string());
        apply_cross_language_limit(&mut plain, &probe, &pure);
        assert_eq!(plain.static_limit_kind, None);
        assert_eq!(
            plain.recommended_next_step.as_deref(),
            Some("add a co-located test")
        );
    }
}
