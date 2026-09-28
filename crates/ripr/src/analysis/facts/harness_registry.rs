//! Repository-governed Rust test-harness registry (#3532).
//!
//! This module is the one authority that turns exact
//! `[analysis.test_harnesses]` registrations into typed source facts. It
//! runs once per index build, after the parameterized-test promotion and
//! the test-styles normalizer, so every downstream consumer (diff probes,
//! seam inventory, evidence relation, test selectors, projections) reads
//! the same derived facts.
//!
//! The authority model keeps the required distinctions apart:
//!
//! - a registered custom target is evidence-role, but NOT every function
//!   in it is an executable test — members carry
//!   [`FunctionSourceRole::HarnessHelper`] and only adapter-established
//!   subjects register `TestFact`s;
//! - a registered macro/attribute invocation is a test container, but
//!   RIPR did not expand every generated case — subjects state their
//!   [`HarnessSubjectClaim`];
//! - a known selector route ([`HarnessSelectorCapability::NamedUnexecuted`])
//!   is not a selector that ran — passive analysis never builds or
//!   executes a harness (the #3634 target validation reads `cargo
//!   metadata` for declarations only; it never compiles or runs tests);
//! - unregistered or ambiguous harnesses become typed limitations —
//!   never production reclassification and never executable-test
//!   optimism.
//!
//! Everything here is exact-match on configured inputs. There is no
//! inference from filenames, crate imports, macro suffixes, or function
//! names, and a registration that names a file outside the analyzed
//! index simply grants nothing (fail closed).
//!
//! ## Named-invocation evidence boundary (#3603)
//!
//! A trial subject's identity span is the registration invocation, but
//! the evidence it claims is widened in exactly two bounded directions,
//! both fail-closed:
//!
//! - a bare-identifier callback (`Trial::test("name", helper_fn)`) that
//!   resolves to exactly one function in the registered target file
//!   contributes that function's parsed body evidence (calls, oracles,
//!   literals) one level deep, with real line attribution; closures,
//!   path callbacks, and unresolved or ambiguous names contribute
//!   nothing beyond the invocation span;
//! - method-position `.unwrap()`/`.expect()` calls inside the claimed
//!   span register smoke oracles, the same evidence the ordinary
//!   `#[test]` parser records for `ast::MethodCallExpr`. Non-assertion
//!   macro input is skipped, matching what parsed method-call nodes
//!   could see.
//!
//! No strength over-credit is possible: helper evidence reuses the
//! ordinary parser path, and method oracles are smoke-strength by
//! construction.
//!
//! ## Named-invocation reachability boundary (#3604, #3636)
//!
//! [`HarnessSubjectClaim::NamedInvocation`] is a syntactic claim bounded
//! by the registered target: a named invocation exists in the registered
//! target. It is not a claim that the harness registers or executes the
//! trial. What the invocation contributes to the executable-test
//! denominator is decided by the bounded reachability authority
//! (`reachability.rs`, #3636), which anchors the registered run entry
//! point and resolves its trial argument through supported forms:
//!
//! - a construction provably excluded from every resolved run argument
//!   — or a target with no run entry call at all — keeps its subject
//!   fact and claim but does not enter the executable-test denominator;
//!   a per-trial `registration_unreachable` limitation names it;
//! - a construction the bounded resolver can neither connect nor
//!   exclude stays admitted (the #3604 posture) and is disclosed by one
//!   aggregate `registration_reachability_unknown` limitation naming
//!   the trials — unknown is the bias, because a false unreachable
//!   silently drops a real subject;
//! - admitted-but-unknown subjects still carry only the syntactic
//!   claim; there is no per-subject reachability field, because
//!   per-subject attribution is exactly what the unknown bucket cannot
//!   establish.

use super::model::{FunctionFact, FunctionSourceRole, RustIndex, TestFact};
use super::test_styles::normalized_test_attribute_path as normalized_attribute_path;
use super::{
    HarnessLimitationFact, HarnessSelectorCapability, HarnessSubjectClaim, HarnessSubjectFact,
};
use crate::analysis::rust_index::{
    OracleFact, classify_assertion, extract_assertions, extract_call_facts,
    extract_identifier_tokens, extract_literal_facts,
};
use crate::analysis::syntax::ra::{
    LineIndex, parser_oracles_for_function, shadow_facts_for_body_text, slice_text,
};
use crate::analysis::workspace::{CargoHarnessVerdict, ManifestInventory};
use crate::config::{TestHarnessAdapter, TestHarnessKind, TestHarnessRegistration};
use crate::domain::{OracleKind, OracleStrength};
use ra_ap_syntax::ast::{self, HasName};
use ra_ap_syntax::{AstNode, Edition, SourceFile, TextSize};
use std::collections::BTreeSet;
use std::path::Path;

/// Apply every registration whose target file is present in the index.
/// Registrations for files outside the analyzed set grant nothing.
///
/// Empty registrations leave the index untouched (no-op), so
/// repositories without registrations keep every existing output.
///
/// Before a `custom_harness` registration grants anything, its target is
/// validated against the workspace's Cargo target metadata (#3608): only
/// a declared `[[test]]` target with `harness = false` carries the
/// premise the adapter needs. Membership and target identity come from
/// `cargo metadata` itself, and the `harness` flag from the owning
/// manifest, because metadata output omits the flag (#3634). A target
/// missing from Cargo metadata, still harness-enabled, or an
/// unresolvable validation premise (an unreadable owning manifest,
/// cargo metadata that is unavailable for the analyzed workspace, or
/// owning manifests that disagree on the target's `harness` flag)
/// records a typed limitation and the registration degrades to
/// per-function behavior — the file keeps its ordinary classification
/// instead of receiving file-wide evidence role.
pub(super) fn apply_registrations(
    index: &mut RustIndex,
    workspace_root: &Path,
    registrations: &[TestHarnessRegistration],
) {
    if registrations.is_empty() {
        return;
    }
    let mut subjects = Vec::new();
    let mut limitations = Vec::new();
    // One parsed-manifest inventory for the whole registration batch, so
    // registrations sharing a package parse its manifest once (#3608 review).
    let mut manifests = ManifestInventory::default();
    for registration in registrations {
        // Exact target identity only: a registration whose file is not in
        // this index (stale, wrong package, unanalyzed scope) applies to
        // nothing here.
        let Some(facts) = index.files.get(&registration.target) else {
            continue;
        };
        let source = facts.source.clone();
        match (registration.kind, registration.adapter) {
            (TestHarnessKind::CustomHarnessTarget, TestHarnessAdapter::LibtestMimicV1) => {
                match manifests.verdict(workspace_root, &registration.target) {
                    CargoHarnessVerdict::HarnessDisabled => apply_libtest_mimic_target(
                        index,
                        registration,
                        &source,
                        &mut subjects,
                        &mut limitations,
                    ),
                    verdict => {
                        if let Some(limitation) =
                            cargo_metadata_conflict_limitation(registration, verdict)
                        {
                            limitations.push(limitation);
                        }
                    }
                }
            }
            (TestHarnessKind::RegisteredAttribute, TestHarnessAdapter::ExactAttributeV1) => {
                apply_registered_attribute(
                    index,
                    registration,
                    &source,
                    &mut subjects,
                    &mut limitations,
                )
            }
            // The config parser rejects unknown kind/adapter pairs, so this
            // arm is unreachable today; if it ever becomes reachable the
            // safe behavior is to grant nothing.
            _ => {}
        }
    }
    subjects.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.start_line.cmp(&right.start_line))
            .then(left.end_line.cmp(&right.end_line))
            .then(left.name.cmp(&right.name))
    });
    limitations.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.code.cmp(&right.code))
            .then(left.detail.cmp(&right.detail))
    });
    limitations.dedup();
    index.harness_subjects = subjects;
    index.harness_limitations = limitations;
}

/// The typed limitation recorded when a `custom_harness` registration
/// fails its Cargo target metadata validation (#3608). The target file is
/// named; the detail states exactly which premise is missing and what the
/// registration degrades to. `None` for the confirmed verdict — the
/// adapter runs and no conflict exists.
fn cargo_metadata_conflict_limitation(
    registration: &TestHarnessRegistration,
    verdict: CargoHarnessVerdict,
) -> Option<HarnessLimitationFact> {
    let code = match verdict {
        CargoHarnessVerdict::HarnessEnabled => "harness_flag_conflict",
        CargoHarnessVerdict::NotDeclared => "target_not_declared",
        CargoHarnessVerdict::ManifestUnavailable => "manifest_unavailable",
        CargoHarnessVerdict::HarnessDisabled => return None,
    };
    let target = registration.target.clone();
    let displayed = target.to_string_lossy().replace('\\', "/");
    let detail = match verdict {
        CargoHarnessVerdict::HarnessEnabled => format!(
            "the Cargo target for `{displayed}` still has `harness = true` (explicit or autodiscovered default); the `harness = false` premise of the registration is not established, so no file-wide evidence role, demotion, or trial subjects are granted and the file keeps its ordinary per-function classification"
        ),
        CargoHarnessVerdict::NotDeclared => format!(
            "the registered custom-harness target `{displayed}` does not match any Cargo `[[test]]` target (declared or autodiscovered) in the owning package manifest; no file-wide evidence role, demotion, or trial subjects are granted and the file keeps its ordinary per-function classification"
        ),
        CargoHarnessVerdict::ManifestUnavailable => format!(
            "the Cargo metadata premise for `{displayed}` could not be established (the owning package manifest could not be read or parsed, the workspace's `cargo metadata` was unavailable, or the owning manifests disagree on the target's `harness` flag), so the `harness = false` premise of the registration is not confirmed; no file-wide evidence role, demotion, or trial subjects are granted and the file keeps its ordinary per-function classification"
        ),
        CargoHarnessVerdict::HarnessDisabled => return None,
    };
    Some(HarnessLimitationFact {
        registration_id: registration.registration_id.clone(),
        code: code.to_string(),
        file: target,
        line: 1,
        detail,
    })
}

/// The file-wide evidence-role grant after Cargo target metadata
/// validation (#3608): the workspace-relative targets of exactly those
/// `custom_harness` registrations whose owning package manifest declares
/// a `[[test]]` target with `harness = false`. Every file-wide evidence
/// surface consumes this validated set, so a misdeclared registration
/// cannot suppress production probing on an unverified premise.
pub(crate) fn validated_file_wide_harness_targets(
    workspace_root: &Path,
    registrations: &[TestHarnessRegistration],
) -> BTreeSet<std::path::PathBuf> {
    let mut manifests = ManifestInventory::default();
    registrations
        .iter()
        .filter(|registration| registration.file_wide_harness_evidence())
        .filter(|registration| {
            manifests.verdict(workspace_root, &registration.target)
                == CargoHarnessVerdict::HarnessDisabled
        })
        .map(|registration| registration.target.clone())
        .collect()
}

/// libtest-mimic adapter, generation 1 (#3532).
///
/// Establishes one subject per exact source-visible trial registration
/// with a stable string-literal name (`Trial::test("name", ...)`), where
/// the `Trial` path is anchored to the registered marker crate path —
/// either spelled fully qualified (`<marker>::Trial::test`) or bound by a
/// top-level `use` whose path resolves to `<marker>::Trial`. Discovery
/// runs over the file's token stream, so trial registrations inside
/// collection macros (`vec![...]`) are still exact token matches, while
/// everything the issue names as a limitation stays one:
///
/// - dynamic (non-literal) trial names;
/// - trial registration inside a loop (runtime-only discovery);
/// - unanchored or ambiguously imported `Trial` paths;
/// - duplicate trial names (two subject claims on one identity).
///
/// Every established subject claims
/// [`HarnessSubjectClaim::NamedInvocation`]: a syntactic claim bounded by
/// the registered target, not a claim that the harness registers or
/// executes the trial (#3604). Whether an established subject enters the
/// executable-test denominator is decided afterwards by the bounded
/// reachability authority (#3636): constructions excluded from every
/// resolved run argument — or a target with no run entry call — keep
/// their subject fact but withhold their `TestFact` under a
/// `registration_unreachable` limitation, unresolved reachability stays
/// admitted under an aggregate `registration_reachability_unknown`
/// disclosure, and everything else is admitted as before.
fn apply_libtest_mimic_target(
    index: &mut RustIndex,
    registration: &TestHarnessRegistration,
    source: &str,
    subjects: &mut Vec<HarnessSubjectFact>,
    limitations: &mut Vec<HarnessLimitationFact>,
) {
    let target = registration.target.clone();
    demote_harness_target_functions(index, &target);
    let parse = SourceFile::parse(source, Edition::CURRENT);
    if !parse.errors().is_empty() {
        limitations.push(HarnessLimitationFact {
            registration_id: registration.registration_id.clone(),
            code: "parse_unavailable".to_string(),
            file: target,
            line: 1,
            detail: "the registered harness target did not parse; no trial subjects were established (fail closed)".to_string(),
        });
        return;
    }
    let line_index = LineIndex::new(source);
    let file_syntax = parse.tree().syntax().clone();
    let trial_bindings = top_level_use_bindings(&file_syntax, "Trial");
    let tokens: Vec<ra_ap_syntax::SyntaxToken> = parse
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .collect();
    let mut claimed_names = BTreeSet::new();
    // One invocation is one open paren. A qualified path
    // (`marker::Trial::test(`) also contains a token-local bare `Trial`
    // position; claiming the paren prevents the bare pass from
    // re-processing the same call and emitting a false
    // duplicate_subject/unanchored_trial_path limitation.
    let mut claimed_invocations = BTreeSet::new();
    // Established subjects are held pending until the scan completes:
    // denominator admission is decided per trial by the reachability
    // authority (#3636), which needs the whole trial set first.
    let mut pending: Vec<PendingSubject> = Vec::new();

    for position in 0..tokens.len() {
        let Some(matched) = match_trial_path(&tokens, position, &registration.marker) else {
            continue;
        };
        if !claimed_invocations.insert(matched.open_paren_index) {
            continue;
        }
        let line = line_index.line(tokens[position].text_range().start());
        // Anchoring: the bare `Trial` form needs a top-level import bound
        // from exactly the marker path; the qualified form carries the
        // marker in the path itself.
        if !matched.qualified {
            match resolve_trial_binding(&trial_bindings, &registration.marker) {
                TrialBindingResolution::MarkerAnchored => {}
                TrialBindingResolution::Ambiguous { conflicting } => {
                    limitations.push(HarnessLimitationFact {
                        registration_id: registration.registration_id.clone(),
                        code: "ambiguous_import".to_string(),
                        file: target.clone(),
                        line,
                        detail: format!(
                            "bare `Trial::test` cannot be tied to marker `{}`; conflicting imports bind Trial from ({conflicting})",
                            registration.marker
                        ),
                    });
                    continue;
                }
                TrialBindingResolution::Unbound => {
                    limitations.push(HarnessLimitationFact {
                        registration_id: registration.registration_id.clone(),
                        code: "unanchored_trial_path".to_string(),
                        file: target.clone(),
                        line,
                        detail: format!(
                            "`Trial::test` has no top-level import bound from marker `{}`; the invocation is not classified",
                            registration.marker
                        ),
                    });
                    continue;
                }
            }
        }

        // A `Trial::test` template inside a `macro_rules!` definition is
        // not a concrete registration; only a real invocation is.
        if inside_macro_rules(tokens[position].parent_ancestors()) {
            continue;
        }
        if in_loop(tokens[position].parent_ancestors()) {
            limitations.push(HarnessLimitationFact {
                registration_id: registration.registration_id.clone(),
                code: "dynamic_trial_registration".to_string(),
                file: target.clone(),
                line,
                detail: "trial registration runs inside a loop; runtime-discovered trials remain unresolved".to_string(),
            });
            continue;
        }
        let Some(name_token) = tokens.get(matched.name_token_index) else {
            continue;
        };
        let Some(name) = string_literal_token_text(name_token) else {
            limitations.push(HarnessLimitationFact {
                registration_id: registration.registration_id.clone(),
                code: "dynamic_trial_name".to_string(),
                file: target.clone(),
                line: line_index.line(name_token.text_range().start()),
                detail:
                    "trial name is not a simple string literal; generated names remain unresolved"
                        .to_string(),
            });
            continue;
        };
        if !claimed_names.insert(name.clone()) {
            limitations.push(HarnessLimitationFact {
                registration_id: registration.registration_id.clone(),
                code: "duplicate_subject".to_string(),
                file: target.clone(),
                line,
                detail: format!(
                    "trial name `{name}` is registered more than once; conflicting subject claims fail closed"
                ),
            });
            continue;
        }
        // The subject span runs from the trial path to the balanced
        // closing parenthesis of the registration call.
        let Some(close_index) = matching_group_close(&tokens, matched.open_paren_index) else {
            continue;
        };
        let end_offset: u32 = tokens[close_index].text_range().end().into();
        let start: TextSize = tokens[position].text_range().start();
        let start_line = line_index.line(start);
        let end_line = line_index.line_for_range_end(TextSize::new(end_offset));
        let body = slice_text(source, start, TextSize::new(end_offset));
        // Dormant `macro_rules!` templates inside the claimed span never
        // execute: their spans are erased from the lexical extraction
        // (spaces keep line attribution exact) so template calls and
        // values never join the subject while live same-line evidence
        // survives (#3603 review).
        let template_spans = dormant_template_token_spans(
            &tokens,
            matched.name_token_index,
            matched.open_paren_index,
            start,
        );
        let masked_body = mask_dormant_template_spans(&body, &template_spans);
        let mut calls = extract_call_facts(&masked_body, start_line);
        let mut literals = extract_literal_facts(&masked_body, start_line);
        let mut assertions =
            parser_oracles_for_node_tokens(source, &tokens, matched.name_token_index, &line_index);
        // #3603: a bare-identifier callback (`Trial::test("name",
        // helper_fn)`) resolves to exactly one top-level function in this
        // file, and that function's body is what the trial exercises —
        // its parsed evidence joins the subject one level deep with real
        // line attribution. Closures already scan through the claimed
        // span; shadowed names, imports, paths, and unresolved or
        // ambiguous names contribute nothing.
        // The enclosing scope for the shadow scan: resolved from the
        // invocation token's Fn ancestor when the trial is written as
        // plain code, and from the innermost line-span function fact
        // only when the tokens carry no Fn ancestor (a trial collected
        // inside another macro's token tree). Resolving by ancestors
        // first keeps two functions sharing one source line from picking
        // the wrong scope (#3603 review, My3M).
        let enclosing_scope: Option<(String, usize)> =
            match tokens[position].parent_ancestors().find_map(ast::Fn::cast) {
                Some(fn_node) => {
                    let fn_start = fn_node
                        .fn_token()
                        .map(|token| token.text_range().start())
                        .unwrap_or_else(|| fn_node.syntax().text_range().start());
                    let fn_end = fn_node.syntax().text_range().end();
                    Some((
                        slice_text(source, fn_start, fn_end),
                        line_index.line(fn_start),
                    ))
                }
                None => enclosing_function(index, &target, start_line)
                    .map(|function| (function.body.clone(), function.start_line)),
            };
        let enclosing_body = enclosing_scope.as_ref().map(|(body, _)| body.as_str());
        if let Some(helper) =
            bare_ident_callback(&tokens, matched.name_token_index, matched.open_paren_index)
                .and_then(|ident_index| {
                    resolve_helper_function(
                        index,
                        &target,
                        &file_syntax,
                        enclosing_body,
                        tokens[ident_index].text(),
                    )
                })
        {
            // A `macro_rules!` definition in the helper body is a dormant
            // token tree: its template never executes, so nothing inside
            // it joins the subject — not oracles, not calls, not literals.
            // The template spans are erased from every merged evidence
            // extraction (spaces keep line attribution exact), so live
            // same-line evidence survives while template facts cannot be
            // extracted at all — including from the lexical fallback,
            // which line-scans raw text (under-emit; #3603 review).
            let dormant_spans = dormant_template_parse_spans(&helper.body);
            let helper_evidence_body = mask_dormant_template_spans(&helper.body, &dormant_spans);
            calls.extend(extract_call_facts(&helper_evidence_body, helper.start_line));
            assertions.extend(
                parser_oracles_for_function(&helper_evidence_body, helper.start_line)
                    .unwrap_or_else(|| {
                        extract_assertions(&helper_evidence_body, helper.start_line)
                    }),
            );
            literals.extend(extract_literal_facts(
                &helper_evidence_body,
                helper.start_line,
            ));
            calls.sort_by(|left, right| {
                left.line
                    .cmp(&right.line)
                    .then(left.name.cmp(&right.name))
                    .then(left.text.cmp(&right.text))
            });
            calls.dedup_by(|left, right| {
                left.line == right.line && left.name == right.name && left.text == right.text
            });
            assertions
                .sort_by(|left, right| left.line.cmp(&right.line).then(left.text.cmp(&right.text)));
            assertions.dedup_by(|left, right| left.line == right.line && left.text == right.text);
            literals.sort_by(|left, right| {
                left.line
                    .cmp(&right.line)
                    .then(left.value.cmp(&right.value))
            });
            literals.dedup_by(|left, right| left.line == right.line && left.value == right.value);
        }
        // #3727 Slice A: parser-backed shadow facts over the merged evidence
        // body text (an invocation span, not a fn item, so the facts are
        // wrap-parse best-effort). The relation-side flag law routes this
        // synthesized test through the fact fields on parser-backed files,
        // so the facts must describe this body, not stay empty.
        let (nested_fn_names, let_bindings) = shadow_facts_for_body_text(&body);
        pending.push(PendingSubject {
            subject: HarnessSubjectFact {
                registration_id: registration.registration_id.clone(),
                harness_kind: registration.kind.as_str().to_string(),
                adapter: registration.adapter.as_str().to_string(),
                marker: registration.marker.clone(),
                name: name.clone(),
                file: target.clone(),
                start_line,
                end_line,
                body: body.clone(),
                calls: calls.clone(),
                assertions: assertions.clone(),
                literals: literals.clone(),
                selector: HarnessSelectorCapability::NamedUnexecuted,
                claim: HarnessSubjectClaim::NamedInvocation,
                provenance: TestHarnessRegistration::provenance().to_string(),
            },
            test: TestFact {
                name,
                file: target.clone(),
                start_line,
                end_line,
                body,
                calls,
                assertions,
                literals,
                attrs: Vec::new(),
                nested_fn_names,
                let_bindings,
            },
            span_start: position,
            span_end: close_index,
        });
    }
    let token_starts: Vec<u32> = tokens
        .iter()
        .map(|token| token.text_range().start().into())
        .collect();
    let scan = reachability::TargetScan {
        target: &target,
        source,
        file_syntax: &file_syntax,
        tokens: &tokens,
        token_starts: &token_starts,
        line_index: &line_index,
    };
    let admission = SubjectAdmission {
        scan,
        registration_id: &registration.registration_id,
        marker: &registration.marker,
    };
    admit_pending_subjects(&admission, index, pending, subjects, limitations);
}

/// One established subject awaiting the reachability decision (#3636):
/// the subject fact and its mirrored `TestFact`, plus the trial
/// invocation's inclusive token-index span for containment checks.
struct PendingSubject {
    subject: HarnessSubjectFact,
    test: TestFact,
    span_start: usize,
    span_end: usize,
}

/// Decide denominator admission for the scan's established subjects
/// (#3636) and publish them. Admitted subjects (reachable or unknown
/// reachability) register their `TestFact` as before; provably
/// unreachable subjects keep only the subject fact and record a
/// per-trial `registration_unreachable` limitation; an unknown outcome
/// records one aggregate `registration_reachability_unknown` disclosure
/// naming the unknown trials.
/// The registration context the admission decision reads: the target
/// scan (index, source, tokens) plus the registration identity and
/// marker that limitations are recorded under.
struct SubjectAdmission<'a> {
    scan: reachability::TargetScan<'a>,
    registration_id: &'a str,
    marker: &'a str,
}

fn admit_pending_subjects(
    admission: &SubjectAdmission,
    index: &mut RustIndex,
    pending: Vec<PendingSubject>,
    subjects: &mut Vec<HarnessSubjectFact>,
    limitations: &mut Vec<HarnessLimitationFact>,
) {
    let SubjectAdmission {
        scan,
        registration_id,
        marker,
    } = admission;
    let target = scan.target;
    let marker = *marker;
    let registration_id = *registration_id;
    if pending.is_empty() {
        return;
    }
    let spans: Vec<reachability::PendingTrialSpan> = pending
        .iter()
        .map(|entry| reachability::PendingTrialSpan {
            name: entry.subject.name.clone(),
            start: entry.span_start,
            end: entry.span_end,
        })
        .collect();
    let outcome = reachability::classify_trial_reachability(scan, index, marker, &spans);
    for (entry, verdict) in pending.into_iter().zip(outcome.verdicts) {
        match verdict {
            reachability::TrialReachability::Reachable
            | reachability::TrialReachability::Unknown => {
                subjects.push(entry.subject);
                push_file_test(index, target, entry.test.clone());
                index.tests.push(entry.test);
            }
            reachability::TrialReachability::Unreachable(reason) => {
                let detail_suffix = match reason {
                    reachability::UnreachableReason::RunEntryAbsent => format!(
                        "no call to the registered harness run entry point (`{marker}::run`, or a top-level import of `run` bound from `{marker}`) exists in this target; the construction has no path into a run argument through the supported resolution forms",
                        marker = marker
                    ),
                    reachability::UnreachableReason::ExcludedByResolvedArguments => format!(
                        "every anchored `{marker}::run` argument resolved completely through the supported resolution forms (direct trial collections, `&`/`vec!`/array literals, immutable let-bound chains, one-level builder functions) and the trial does not appear in any of them",
                        marker = marker
                    ),
                };
                limitations.push(HarnessLimitationFact {
                    registration_id: registration_id.to_string(),
                    code: "registration_unreachable".to_string(),
                    file: target.to_path_buf(),
                    line: entry.subject.start_line,
                    detail: format!(
                        "trial `{name}` is constructed in the registered target, but {detail_suffix}; its executable-test fact does not join the denominator (the syntactic subject claim is retained) (#3636)",
                        name = entry.subject.name,
                    ),
                });
                subjects.push(entry.subject);
            }
        }
    }
    if let Some(detail) = outcome.unknown_detail {
        limitations.push(HarnessLimitationFact {
            registration_id: registration_id.to_string(),
            code: "registration_reachability_unknown".to_string(),
            file: target.to_path_buf(),
            line: outcome.disclosure_line,
            detail,
        });
    }
}

/// The exact token shapes this adapter matches at `position`:
/// `Trial :: test (` (bare, import-anchored) or
/// `<marker>::Trial :: test (` (qualified). Path separators count as
/// either one `::` token or two `:` tokens — inside macro token trees
/// (`vec![...]`) the raw punctuation form is what the tokenizer emits.
/// Returns the index of the first argument token and whether the
/// qualified form matched.
fn match_trial_path(
    tokens: &[ra_ap_syntax::SyntaxToken],
    position: usize,
    marker: &str,
) -> Option<TrialPathMatch> {
    let text = |index: usize| tokens.get(index).map(|token| token.text());
    let is_ident_eq =
        |index: usize, expected: &str| text(index).is_some_and(|value| value == expected);
    let l_paren = |index: usize| {
        tokens
            .get(index)
            .is_some_and(|token| token.kind() == ra_ap_syntax::SyntaxKind::L_PAREN)
    };
    // Length of the path separator starting at `index`: 1 for `::`, 2 for
    // `:` `:`, or None.
    fn separator(tokens: &[ra_ap_syntax::SyntaxToken], index: usize) -> Option<usize> {
        match tokens.get(index).map(|token| token.kind()) {
            Some(ra_ap_syntax::SyntaxKind::COLON2) => Some(1),
            Some(ra_ap_syntax::SyntaxKind::COLON)
                if tokens
                    .get(index + 1)
                    .is_some_and(|token| token.kind() == ra_ap_syntax::SyntaxKind::COLON) =>
            {
                Some(2)
            }
            _ => None,
        }
    }

    // A lone `:` predecessor only continues into a candidate when it is
    // a record field (`trials: Trial::test(...)`) - either parsed as one,
    // or shaped like one inside a macro's token tree (field name `ident`
    // preceded by `{` or `,`). Macro input like `stringify!(trial:
    // Trial::test(...))` is inert data and must not adopt the exception.
    let lone_colon_is_record_field = tokens[position].parent_ancestors().any(|ancestor| {
        ast::RecordExprField::can_cast(ancestor.kind())
            || ast::RecordExprFieldList::can_cast(ancestor.kind())
    }) || preceded_by_record_literal_shape(tokens, position);
    for qualified in [false, true] {
        // A bare `Trial` match is only a candidate at a path start; the
        // inner `Trial` of `other::Trial::test(` is a foreign path and
        // must not be adopted through an unrelated import binding.
        if !qualified && !at_path_start(tokens, position, lone_colon_is_record_field) {
            continue;
        }
        let mut segments: Vec<&str> = if qualified {
            marker.split("::").collect()
        } else {
            Vec::new()
        };
        segments.push("Trial");
        segments.push("test");
        let mut cursor = position;
        let mut matched_segments = 0usize;
        for segment in &segments {
            if !is_ident_eq(cursor, segment) {
                break;
            }
            cursor += 1;
            matched_segments += 1;
            if matched_segments < segments.len() {
                let Some(width) = separator(tokens, cursor) else {
                    break;
                };
                cursor += width;
            }
        }
        if matched_segments == segments.len() && l_paren(cursor) {
            // The name literal need not be adjacent to the paren:
            // `Trial::test( "name", …)` is legal, so skip trivia before
            // reading the name token.
            let mut name_token_index = cursor + 1;
            while let Some(token) = tokens.get(name_token_index) {
                if matches!(
                    token.kind(),
                    ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
                ) {
                    name_token_index += 1;
                } else {
                    break;
                }
            }
            return Some(TrialPathMatch {
                qualified,
                name_token_index,
                open_paren_index: cursor,
            });
        }
    }
    None
}

struct TrialPathMatch {
    qualified: bool,
    name_token_index: usize,
    open_paren_index: usize,
}

/// Whether the token at `position` can begin a path: scanning back past
/// trivia, its predecessor must not be a path separator (`other_module
/// :: Trial` must not match at its inner `Trial` token). `::` may reach
/// the scanner as one COLON2 token or as two adjacent COLON tokens, so a
/// single COLON only continues a path when another colon precedes it; a
/// lone `:` is struct-field syntax and a field initializer like
/// `trials: Trial::test(...)` still starts a path.
fn at_path_start(
    tokens: &[ra_ap_syntax::SyntaxToken],
    position: usize,
    lone_colon_is_record_field: bool,
) -> bool {
    let mut cursor = position;
    while cursor > 0 {
        cursor -= 1;
        if matches!(
            tokens[cursor].kind(),
            ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
        ) {
            continue;
        }
        return match tokens[cursor].kind() {
            ra_ap_syntax::SyntaxKind::COLON2 => false,
            ra_ap_syntax::SyntaxKind::COLON => {
                !previous_significant_is_colon(tokens, cursor) && lone_colon_is_record_field
            }
            _ => true,
        };
    }
    true
}

/// Whether a lone-colon candidate at `position` carries the record
/// literal shape inside a macro token tree: `field_ident :` preceded by
/// `{` or `,` (`vec![Suite { trials: Trial::test(...) }]`). A macro
/// input label (`stringify!(trial: ...)`) is preceded by the opening
/// delimiter instead and stays excluded.
fn preceded_by_record_literal_shape(tokens: &[ra_ap_syntax::SyntaxToken], position: usize) -> bool {
    let mut cursor = position;
    let mut saw_colon = false;
    let mut saw_field_ident = false;
    while cursor > 0 {
        cursor -= 1;
        let token = &tokens[cursor];
        if matches!(
            token.kind(),
            ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
        ) {
            continue;
        }
        if !saw_colon {
            if token.kind() != ra_ap_syntax::SyntaxKind::COLON {
                return false;
            }
            saw_colon = true;
            continue;
        }
        if !saw_field_ident {
            if token.kind() != ra_ap_syntax::SyntaxKind::IDENT {
                return false;
            }
            saw_field_ident = true;
            continue;
        }
        return matches!(
            token.kind(),
            ra_ap_syntax::SyntaxKind::L_CURLY | ra_ap_syntax::SyntaxKind::COMMA
        );
    }
    false
}

fn previous_significant_is_colon(tokens: &[ra_ap_syntax::SyntaxToken], from: usize) -> bool {
    let mut cursor = from;
    while cursor > 0 {
        cursor -= 1;
        if matches!(
            tokens[cursor].kind(),
            ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
        ) {
            continue;
        }
        return matches!(
            tokens[cursor].kind(),
            ra_ap_syntax::SyntaxKind::COLON | ra_ap_syntax::SyntaxKind::COLON2
        );
    }
    false
}

/// Assertion evidence for one trial subject: the exact assertion-macro and
/// `unwrap`/`expect` method tokens inside the registration's own argument
/// span. Only tokens between the name argument and the balanced close
/// count, so adjacent code never credits this subject. Each oracle carries
/// the real source line of its invocation (#3603).
///
/// Method-position `.unwrap()`/`.expect()` calls register smoke oracles
/// with the receiver expression as text — the same evidence the ordinary
/// `#[test]` parser records for `ast::MethodCallExpr`. Macro input is
/// skipped wholesale (assertion macros still classify themselves), so a
/// method token inside `println!(...)` never classifies, matching what
/// parsed method-call nodes could see on the ordinary path.
fn parser_oracles_for_node_tokens(
    source: &str,
    tokens: &[ra_ap_syntax::SyntaxToken],
    name_token_index: usize,
    line_index: &LineIndex,
) -> Vec<OracleFact> {
    let mut assertions = Vec::new();
    let mut depth: usize = 1;
    let mut index = name_token_index + 1;
    while index < tokens.len() && depth > 0 {
        let token = &tokens[index];
        match token.kind() {
            ra_ap_syntax::SyntaxKind::L_PAREN => depth += 1,
            ra_ap_syntax::SyntaxKind::R_PAREN => depth = depth.saturating_sub(1),
            // A `macro_rules!` definition inside the claimed span is a
            // dormant token tree: its template never executes, so no token
            // inside it — assertion macro, method call, or otherwise —
            // classifies as oracle evidence. Skip the whole definition.
            // The token-level shape walk is required because a definition
            // inside another macro's token tree (a trial collected in
            // `vec![...]`) carries no `MacroRules` node to query; the
            // ancestor check inside the identifier arm still guards the
            // file-level shapes.
            ra_ap_syntax::SyntaxKind::IDENT if token.text() == "macro_rules" => {
                match skip_macro_rules_definition(tokens, index) {
                    Some(next) => {
                        index = next;
                        continue;
                    }
                    None => {
                        index += 1;
                        continue;
                    }
                }
            }
            ra_ap_syntax::SyntaxKind::IDENT => {
                let name = token.text();
                // A `macro_rules!` definition inside the claimed span is a
                // dormant token tree: its template never executes, so no
                // token inside it — assertion macro, method call, or
                // otherwise — classifies as oracle evidence. The same
                // ancestor authority that keeps dormant templates from
                // becoming subjects keeps their tokens from becoming
                // oracles.
                if inside_macro_rules(tokens[index].parent_ancestors()) {
                    index += 1;
                    continue;
                }
                // Method-position unwrap/expect smoke oracles (#3603): a
                // `.name(` shape only. A path-shaped `Result::unwrap(...)`,
                // a bare binding, or a field read never classifies — the
                // ordinary parser only sees `ast::MethodCallExpr` here
                // either. The argument group is consumed so nothing inside
                // it reclassifies.
                let method_span = if name == "unwrap" || name == "expect" {
                    previous_significant(tokens, index)
                        .filter(|previous| {
                            tokens[*previous].kind() == ra_ap_syntax::SyntaxKind::DOT
                        })
                        .and_then(|dot_index| {
                            next_significant(tokens, index + 1)
                                .filter(|open| {
                                    tokens[*open].kind() == ra_ap_syntax::SyntaxKind::L_PAREN
                                })
                                .and_then(|open_index| {
                                    matching_group_close(tokens, open_index)
                                        .map(|close_index| (dot_index, close_index))
                                })
                        })
                } else {
                    None
                };
                if let Some((dot_index, close_index)) = method_span {
                    let receiver_start = receiver_start_index(tokens, dot_index);
                    let text = slice_text(
                        source,
                        tokens[receiver_start].text_range().start(),
                        tokens[close_index].text_range().end(),
                    )
                    .trim()
                    .trim_end_matches(';')
                    .to_string();
                    assertions.push(OracleFact {
                        line: line_index.line(tokens[receiver_start].text_range().start()),
                        kind: OracleKind::SmokeOnly,
                        strength: OracleStrength::Smoke,
                        observed_tokens: extract_identifier_tokens(&text),
                        text,
                        ok_value_observed: None,
                    });
                    index = close_index;
                    index += 1;
                    continue;
                }
                // Shared leaf predicate; the bang gate below restores
                // MacroCall semantics, and the leaf boundary keeps
                // `snapshot_helper!`-style names out of the oracle set.
                if !crate::analysis::syntax::ra::is_assertion_macro_leaf(name) {
                    // Non-assertion macro invocation: skip its input token
                    // tree so method tokens inside macro input never
                    // classify — the ordinary parser sees no method-call
                    // nodes inside macro input either.
                    if let Some(group_end) = next_significant(tokens, index + 1)
                        .filter(|next| tokens[*next].kind() == ra_ap_syntax::SyntaxKind::BANG)
                        .and_then(|bang_index| {
                            next_significant(tokens, bang_index + 1)
                                .and_then(|open| matching_group_close(tokens, open))
                        })
                    {
                        index = group_end;
                    }
                    index += 1;
                    continue;
                }
                // Macro semantics: an assertion is an invocation, so a
                // bang must follow the path segment (trivia allowed). A
                // plain identifier that merely contains an assertion
                // keyword (`let snapshots = …`) never classifies.
                let bang_index = match next_significant(tokens, index + 1) {
                    Some(next) if tokens[next].kind() == ra_ap_syntax::SyntaxKind::BANG => next,
                    _ => {
                        index += 1;
                        continue;
                    }
                };
                // assert!(..) / assert_eq![..] — the token tree follows; the
                // assertion text is the complete macro invocation — full
                // qualified path, every delimiter Rust permits — classified
                // and token-extracted exactly like the ordinary parser
                // slices it for `#[test]` functions (including a same-line
                // trailing semicolon).
                //
                // Qualified forms (`insta::assert_snapshot![..]`) classify
                // on the leaf but slice from the whole contiguous path:
                // walk back over `::`-separated identifiers without
                // crossing an expression boundary. Inside another macro's
                // token tree the separator reaches the walk as two COLON
                // tokens, so both spellings participate.
                let mut text_start_token = index;
                while let Some(last_separator) = previous_significant(tokens, text_start_token) {
                    let double = match tokens[last_separator].kind() {
                        ra_ap_syntax::SyntaxKind::COLON2 => true,
                        ra_ap_syntax::SyntaxKind::COLON => {
                            matches!(
                                previous_significant(tokens, last_separator),
                                Some(first) if tokens[first].kind() == ra_ap_syntax::SyntaxKind::COLON
                            )
                        }
                        _ => false,
                    };
                    if !double && tokens[last_separator].kind() != ra_ap_syntax::SyntaxKind::COLON2
                    {
                        break;
                    }
                    let segment = if double {
                        previous_significant(tokens, last_separator)
                            .and_then(|first| previous_significant(tokens, first))
                    } else {
                        previous_significant(tokens, last_separator)
                    };
                    match segment {
                        // `crate`, `self`, and `super` are keyword path
                        // segments, not identifiers.
                        Some(segment)
                            if matches!(
                                tokens[segment].kind(),
                                ra_ap_syntax::SyntaxKind::IDENT
                                    | ra_ap_syntax::SyntaxKind::CRATE_KW
                                    | ra_ap_syntax::SyntaxKind::SELF_KW
                                    | ra_ap_syntax::SyntaxKind::SUPER_KW
                            ) =>
                        {
                            text_start_token = segment;
                        }
                        _ => break,
                    }
                }
                let group = next_significant(tokens, bang_index + 1)
                    .filter(|next| {
                        matches!(
                            tokens[*next].kind(),
                            ra_ap_syntax::SyntaxKind::L_PAREN
                                | ra_ap_syntax::SyntaxKind::L_BRACK
                                | ra_ap_syntax::SyntaxKind::L_CURLY
                        )
                    })
                    .and_then(|open_index| {
                        matching_group_close(tokens, open_index)
                            .map(|close_index| (open_index, close_index))
                    });
                let text = match group {
                    Some((_open_index, close_index)) => {
                        // Same-line trailing semicolon parity: the ordinary
                        // parser's macro-call slice extends over a `;` on
                        // the same line. The probe walks the raw trivia so
                        // a newline — in whitespace or a comment — before
                        // the semicolon keeps the text at the group close.
                        let mut end = tokens[close_index].text_range().end();
                        let mut probe = close_index + 1;
                        while let Some(inner) = tokens.get(probe) {
                            if is_trivia(inner.kind()) {
                                if inner.text().contains('\n') {
                                    break;
                                }
                                probe += 1;
                                continue;
                            }
                            if inner.kind() == ra_ap_syntax::SyntaxKind::SEMICOLON {
                                end = inner.text_range().end();
                            }
                            break;
                        }
                        slice_text(source, tokens[text_start_token].text_range().start(), end)
                    }
                    None => format!("{name}!"),
                };
                // The group has been consumed; resuming inside it would
                // rescan nested idents as separate top-level oracles.
                // Without a group (`assert!` alone), resume after the bang.
                let cursor = match group {
                    Some((_open_index, close_index)) => close_index + 1,
                    None => bang_index + 1,
                };
                let classification = classify_assertion(&text);
                assertions.push(OracleFact {
                    line: line_index.line(tokens[text_start_token].text_range().start()),
                    kind: classification.kind,
                    strength: classification.strength,
                    observed_tokens: extract_identifier_tokens(&text),
                    text,
                    ok_value_observed: None,
                });
                // The macro body has been consumed; resuming inside it
                // would rescan nested idents as separate top-level
                // oracles.
                index = cursor.saturating_sub(1);
            }
            _ => {}
        }
        index += 1;
    }
    assertions.sort_by(|left, right| left.line.cmp(&right.line).then(left.text.cmp(&right.text)));
    assertions
}

/// Index of the previous non-trivia token before `from`.
fn previous_significant(tokens: &[ra_ap_syntax::SyntaxToken], from: usize) -> Option<usize> {
    if from == 0 {
        return None;
    }
    (0..from).rev().find(|index| {
        !matches!(
            tokens[*index].kind(),
            ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
        )
    })
}

/// Whether one token kind is trivia.
fn is_trivia(kind: ra_ap_syntax::SyntaxKind) -> bool {
    matches!(
        kind,
        ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
    )
}

/// Index of the close token balancing the bracket-like open at
/// `open_index` (`()`, `[]`, `{}`), or `None` when unbalanced.
fn matching_group_close(tokens: &[ra_ap_syntax::SyntaxToken], open_index: usize) -> Option<usize> {
    let close_kind = match tokens.get(open_index)?.kind() {
        ra_ap_syntax::SyntaxKind::L_PAREN => ra_ap_syntax::SyntaxKind::R_PAREN,
        ra_ap_syntax::SyntaxKind::L_BRACK => ra_ap_syntax::SyntaxKind::R_BRACK,
        ra_ap_syntax::SyntaxKind::L_CURLY => ra_ap_syntax::SyntaxKind::R_CURLY,
        _ => return None,
    };
    let open_kind = tokens[open_index].kind();
    let mut depth: usize = 0;
    for (offset, token) in tokens[open_index..].iter().enumerate() {
        if token.kind() == open_kind {
            depth += 1;
        } else if token.kind() == close_kind {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(open_index + offset);
            }
        }
    }
    None
}

/// Index of the open token balancing the bracket-like close at
/// `close_index`, or `None` when unbalanced.
fn matching_group_open(tokens: &[ra_ap_syntax::SyntaxToken], close_index: usize) -> Option<usize> {
    let open_kind = match tokens.get(close_index)?.kind() {
        ra_ap_syntax::SyntaxKind::R_PAREN => ra_ap_syntax::SyntaxKind::L_PAREN,
        ra_ap_syntax::SyntaxKind::R_BRACK => ra_ap_syntax::SyntaxKind::L_BRACK,
        ra_ap_syntax::SyntaxKind::R_CURLY => ra_ap_syntax::SyntaxKind::L_CURLY,
        _ => return None,
    };
    let close_kind = tokens[close_index].kind();
    let mut depth: usize = 0;
    for (offset, token) in tokens[..=close_index].iter().rev().enumerate() {
        if token.kind() == close_kind {
            depth += 1;
        } else if token.kind() == open_kind {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(close_index - offset);
            }
        }
    }
    None
}

/// Index of the `<` balancing a `>` at `angle_index` (generic arguments
/// such as `parse::<u16>(...)`), or `None` when unbalanced. Compound
/// shift tokens never match, so expression contexts stop the walk.
fn matching_angle_open(tokens: &[ra_ap_syntax::SyntaxToken], angle_index: usize) -> Option<usize> {
    if tokens.get(angle_index)?.kind() != ra_ap_syntax::SyntaxKind::R_ANGLE {
        return None;
    }
    let mut depth: usize = 0;
    for (offset, token) in tokens[..=angle_index].iter().rev().enumerate() {
        match token.kind() {
            ra_ap_syntax::SyntaxKind::R_ANGLE => depth += 1,
            ra_ap_syntax::SyntaxKind::L_ANGLE => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(angle_index - offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Index of the first token of the receiver expression for the method
/// invocation whose `.` sits at `dot_index`. The walk runs backwards and
/// only crosses balanced bracket groups, generic argument groups,
/// literals, identifiers, keyword receiver participants (`self`,
/// `Self`, `await`, `crate`, `super`), and the `.`/`::` connectors of
/// one postfix expression; anything else (operators, statement
/// punctuation, other keywords, a `!` not preceded by a macro-path
/// identifier) ends the receiver, so the text never reaches across a
/// statement boundary. Returns `dot_index` itself when nothing can be
/// consumed.
fn receiver_start_index(tokens: &[ra_ap_syntax::SyntaxToken], dot_index: usize) -> usize {
    let mut receiver_start = dot_index;
    let mut cursor = dot_index;
    while cursor > 0 {
        cursor -= 1;
        if is_trivia(tokens[cursor].kind()) {
            continue;
        }
        match tokens[cursor].kind() {
            ra_ap_syntax::SyntaxKind::R_PAREN
            | ra_ap_syntax::SyntaxKind::R_BRACK
            | ra_ap_syntax::SyntaxKind::R_CURLY => {
                let Some(open) = matching_group_open(tokens, cursor) else {
                    return receiver_start;
                };
                receiver_start = open;
                cursor = open;
            }
            ra_ap_syntax::SyntaxKind::R_ANGLE => {
                let Some(open) = matching_angle_open(tokens, cursor) else {
                    return receiver_start;
                };
                // Only a turbofish group (`parse::<u16>(..)`) is a generic
                // argument group in receiver position, and its `<` is
                // always preceded by `::`. Without the path separators the
                // angles are comparison operators, and consuming the group
                // would reach across the expression and over-credit the
                // receiver. Inside a macro token tree the separators reach
                // the walk as two COLON tokens, so both spellings
                // participate.
                let turbofish = match previous_significant(tokens, open) {
                    Some(separator) => match tokens[separator].kind() {
                        ra_ap_syntax::SyntaxKind::COLON2 => true,
                        ra_ap_syntax::SyntaxKind::COLON => {
                            matches!(
                                previous_significant(tokens, separator),
                                Some(first) if tokens[first].kind() == ra_ap_syntax::SyntaxKind::COLON
                            )
                        }
                        _ => false,
                    },
                    None => false,
                };
                if !turbofish {
                    // Comparison operators end the receiver at their
                    // operand; consuming the angle group would reach
                    // across the expression.
                    return receiver_start;
                }
                receiver_start = open;
                cursor = open;
            }
            ra_ap_syntax::SyntaxKind::IDENT
            | ra_ap_syntax::SyntaxKind::STRING
            | ra_ap_syntax::SyntaxKind::INT_NUMBER
            | ra_ap_syntax::SyntaxKind::FLOAT_NUMBER
            | ra_ap_syntax::SyntaxKind::CHAR
            | ra_ap_syntax::SyntaxKind::BYTE_STRING
            // Keyword receiver participants (#3603 review): `self.value
            // .unwrap()`, `future.await.unwrap()`, and path roots like
            // `crate::config.value.unwrap()` are ordinary postfix
            // receivers; dropping the keyword would truncate the receiver
            // text and misattribute the observed receiver.
            | ra_ap_syntax::SyntaxKind::SELF_KW
            | ra_ap_syntax::SyntaxKind::SELF_TYPE_KW
            | ra_ap_syntax::SyntaxKind::AWAIT_KW
            | ra_ap_syntax::SyntaxKind::CRATE_KW
            | ra_ap_syntax::SyntaxKind::SUPER_KW => {
                receiver_start = cursor;
            }
            ra_ap_syntax::SyntaxKind::DOT | ra_ap_syntax::SyntaxKind::COLON2 => {
                // Postfix connectors: the current receiver start stands.
            }
            ra_ap_syntax::SyntaxKind::COLON => {
                // A raw `::` inside a macro token tree reaches the walk as
                // two COLON tokens: accept the pair when the neighbor
                // colon sits on the other side. A lone colon (a
                // struct-field separator) ends the receiver.
                let previous_colon = matches!(
                    previous_significant(tokens, cursor),
                    Some(previous) if tokens[previous].kind() == ra_ap_syntax::SyntaxKind::COLON
                );
                let next_colon = matches!(
                    next_significant(tokens, cursor + 1),
                    Some(next) if tokens[next].kind() == ra_ap_syntax::SyntaxKind::COLON
                );
                if previous_colon {
                    if let Some(first) = previous_significant(tokens, cursor) {
                        cursor = first;
                    } else {
                        return receiver_start;
                    }
                } else if next_colon {
                    if let Some(second) = next_significant(tokens, cursor + 1) {
                        cursor = second;
                    } else {
                        return receiver_start;
                    }
                } else {
                    return receiver_start;
                }
            }
            ra_ap_syntax::SyntaxKind::BANG => {
                // A macro-call receiver (`println!("x").unwrap()`)
                // continues through `!` only when the macro path's last
                // identifier precedes it; a negation operator ends the
                // receiver.
                let Some(path_ident) = previous_significant(tokens, cursor) else {
                    return receiver_start;
                };
                if tokens[path_ident].kind() != ra_ap_syntax::SyntaxKind::IDENT {
                    return receiver_start;
                }
                receiver_start = path_ident;
                cursor = path_ident;
            }
            _ => return receiver_start,
        }
    }
    receiver_start
}

/// Token index of a bare-identifier trial callback (`Trial::test("name",
/// helper_fn)`): between the name argument and the balanced close, the
/// only significant tokens are one separating comma, the identifier, and
/// optionally a trailing comma. Closures, path callbacks, and any
/// multi-token argument do not resolve (fail closed, #3603).
fn bare_ident_callback(
    tokens: &[ra_ap_syntax::SyntaxToken],
    name_token_index: usize,
    open_paren_index: usize,
) -> Option<usize> {
    let close_index = matching_group_close(tokens, open_paren_index)?;
    let significant: Vec<usize> = (name_token_index + 1..close_index)
        .filter(|index| !is_trivia(tokens[*index].kind()))
        .collect();
    if significant.len() != 2 && significant.len() != 3 {
        return None;
    }
    let comma = significant[0];
    let ident = significant[1];
    if tokens[comma].kind() != ra_ap_syntax::SyntaxKind::COMMA
        || tokens[ident].kind() != ra_ap_syntax::SyntaxKind::IDENT
    {
        return None;
    }
    match significant.len() {
        2 => Some(ident),
        // A trailing comma after the identifier is still a bare callback.
        3 if tokens[significant[2]].kind() == ra_ap_syntax::SyntaxKind::COMMA => Some(ident),
        _ => None,
    }
}

/// The one file-level function the callback can be shown to name, or
/// `None` whenever binding identity is not provable (#3603 review). The
/// resolution is fail-closed in three directions:
///
/// - a local binding of the name inside the trial's enclosing body
///   (`let`, parameter, closure or loop pattern, or a fn-local `use`)
///   shadows any file-level fn — the callback would run the local, not
///   the file-level fn, so nothing is credited;
/// - a top-level `use` binding the same leaf name makes the bare
///   callback ambiguous with the import — nothing is credited;
/// - the name must name exactly one `FunctionFact` AND exactly one
///   top-level `fn` item of the file; a same-named fn that lives only
///   inside a nested module or impl is not name-visible to the
///   invocation and never admits. Function facts are producer-owned
///   parsed structure, so resolution is never a name heuristic on raw
///   text.
fn resolve_helper_function<'a>(
    index: &'a RustIndex,
    target: &Path,
    file_syntax: &ra_ap_syntax::SyntaxNode,
    enclosing_body: Option<&str>,
    name: &str,
) -> Option<&'a FunctionFact> {
    if let Some(body) = enclosing_body
        && enclosing_body_binds_name(body, name)
    {
        return None;
    }
    if !top_level_use_bindings(file_syntax, name).is_empty() {
        return None;
    }
    // A top-level `const`/`static` binding the callback's leaf name makes
    // the bare callback ambiguous with the item binding — parse-only
    // spellings can carry both a same-named const and fn, and a local
    // const/static shadow is separately covered by the enclosing-body
    // gate.
    let top_level_same_named_item = file_syntax
        .children()
        .any(|item| node_binding_name(&item).is_some_and(|item_name| item_name == name));
    if top_level_same_named_item {
        return None;
    }
    let top_level_same_named = file_syntax
        .children()
        .filter_map(ast::Fn::cast)
        .filter(|function| {
            function
                .name()
                .map(|item| item.text() == name)
                .unwrap_or(false)
        })
        .count();
    if top_level_same_named != 1 {
        return None;
    }
    let facts = index.files.get(target)?;
    let mut matches = facts
        .functions
        .iter()
        .filter(|function| function.name == name);
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

/// The innermost function fact whose span contains `line`, if any — the
/// line-based fallback for invocations whose tokens carry no Fn ancestor
/// (a trial collected inside another macro's token tree). Ancestor-based
/// resolution is preferred; this heuristic can pick the wrong scope when
/// two functions share one source line.
fn enclosing_function<'a>(
    index: &'a RustIndex,
    target: &Path,
    line: usize,
) -> Option<&'a FunctionFact> {
    index
        .files
        .get(target)?
        .functions
        .iter()
        .filter(|function| function.start_line <= line && line <= function.end_line)
        .min_by_key(|function| function.end_line.saturating_sub(function.start_line))
}

/// Whether the enclosing body binds `name` locally: any identifier
/// pattern (`let`, parameter, closure parameter, loop pattern), any
/// `const`/`static` item binding inside the body (a local const or static
/// shadows the file-level fn just like a `let`), or any `use` item inside
/// the body that binds the name. An unparseable body counts as bound
/// (fail closed).
fn enclosing_body_binds_name(body_text: &str, name: &str) -> bool {
    let parse = SourceFile::parse(body_text, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return true;
    }
    let tree = parse.tree();
    let syntax = tree.syntax();
    let pattern_binds = syntax
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .any(|pattern| {
            pattern
                .name()
                .map(|item| item.text() == name)
                .unwrap_or(false)
        });
    if pattern_binds {
        return true;
    }
    let item_binds = syntax
        .descendants()
        .any(|node| node_binding_name(&node).is_some_and(|item_name| item_name == name));
    if item_binds {
        return true;
    }
    let mut bindings = BTreeSet::new();
    for use_item in syntax.descendants().filter_map(ast::Use::cast) {
        let Some(use_tree) = use_item.use_tree() else {
            continue;
        };
        collect_use_bindings(&use_tree, String::new(), name, &mut bindings);
    }
    !bindings.is_empty()
}

/// The bound name of a `const` or `static` item node, if the node is one.
/// Item bindings carry a `Name`, not an identifier pattern, so the
/// identifier-pattern scan cannot see them.
fn node_binding_name(node: &ra_ap_syntax::SyntaxNode) -> Option<String> {
    let name = ast::Const::cast(node.clone())
        .and_then(|item| item.name())
        .or_else(|| ast::Static::cast(node.clone()).and_then(|item| item.name()))?;
    Some(name.text().to_string())
}

/// Byte spans of the dormant `macro_rules!` definitions in `text`, from
/// the parsed `ast::MacroRules` nodes — real parsed structure, no brace
/// balancing. An unparseable body yields no spans: the oracle producers
/// already fail closed on their own.
fn dormant_template_parse_spans(text: &str) -> Vec<(usize, usize)> {
    let parse = SourceFile::parse(text, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return Vec::new();
    }
    parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::MacroRules::cast)
        .map(|definition| {
            let range = definition.syntax().text_range();
            (
                u32::from(range.start()) as usize,
                u32::from(range.end()) as usize,
            )
        })
        .collect()
}

/// Erase the dormant-template byte spans from `text` by replacing their
/// non-newline bytes with spaces: template evidence can no longer be
/// extracted, while line offsets and live same-line evidence outside the
/// spans stay exact. Span edges are character boundaries, so only whole
/// characters are replaced.
fn mask_dormant_template_spans(text: &str, spans: &[(usize, usize)]) -> String {
    if spans.is_empty() {
        return text.to_string();
    }
    let mut masked = text.as_bytes().to_vec();
    for (start, end) in spans {
        let range = (*start).min(masked.len())..(*end).min(masked.len());
        for byte in &mut masked[range] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    // See the note above: masked regions cover whole characters, so the
    // masked bytes are always valid UTF-8; the fallback keeps the input
    // rather than panicking on a defect.
    String::from_utf8(masked).unwrap_or_else(|_| text.to_string())
}

/// Byte spans (relative to the claimed-span body text, which starts at
/// `body_start`) of the dormant `macro_rules!` definitions between the
/// trial's name argument and its balanced close, for trials registered
/// inside another macro's token tree where no `MacroRules` nodes exist.
fn dormant_template_token_spans(
    tokens: &[ra_ap_syntax::SyntaxToken],
    name_token_index: usize,
    open_paren_index: usize,
    body_start: TextSize,
) -> Vec<(usize, usize)> {
    let Some(close_index) = matching_group_close(tokens, open_paren_index) else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut walk = name_token_index + 1;
    while walk < close_index {
        if tokens[walk].kind() == ra_ap_syntax::SyntaxKind::IDENT
            && tokens[walk].text() == "macro_rules"
            && let Some(next) = skip_macro_rules_definition(tokens, walk)
        {
            let span_start =
                u32::from(tokens[walk].text_range().start()).saturating_sub(u32::from(body_start));
            let span_end = u32::from(tokens[next - 1].text_range().end())
                .saturating_sub(u32::from(body_start));
            spans.push((span_start as usize, span_end as usize));
            walk = next;
            continue;
        }
        walk += 1;
    }
    spans
}

/// Index of the next non-trivia token at or after `from`.
fn next_significant(tokens: &[ra_ap_syntax::SyntaxToken], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|index| {
        !matches!(
            tokens[*index].kind(),
            ra_ap_syntax::SyntaxKind::WHITESPACE | ra_ap_syntax::SyntaxKind::COMMENT
        )
    })
}

/// Exact registered test-producing attribute adapter, generation 1
/// (#3532). Promotes functions carrying the exact registered attribute
/// path through the shared #3531 role authority. The attribute must
/// either be spelled as the full registered path (`#[<marker>]`) or bound
/// in the target file by a top-level `use` resolving to the marker; a
/// lookalike spelling or a same-named unrelated import never classifies.
fn apply_registered_attribute(
    index: &mut RustIndex,
    registration: &TestHarnessRegistration,
    source: &str,
    subjects: &mut Vec<HarnessSubjectFact>,
    limitations: &mut Vec<HarnessLimitationFact>,
) {
    let target = registration.target.clone();
    let parse = SourceFile::parse(source, Edition::CURRENT);
    if !parse.errors().is_empty() {
        // Fail closed: subject promotion over an unparseable target is
        // unsound, so nothing beyond the typed limitation is established.
        limitations.push(HarnessLimitationFact {
            registration_id: registration.registration_id.clone(),
            code: "parse_unavailable".to_string(),
            file: target,
            line: 1,
            detail: "the registered attribute target did not parse; no registered subjects were established (fail closed)".to_string(),
        });
        return;
    }
    let use_bindings =
        top_level_use_bindings(parse.tree().syntax(), marker_leaf(&registration.marker));
    let promoted_functions: Vec<(usize, String, TestFact, HarnessSubjectFact)> = {
        let Some(facts) = index.files.get(&target) else {
            return;
        };
        let mut promoted = Vec::new();
        for function in &facts.functions {
            let Some(resolution) =
                resolve_registered_attribute(&function.attrs, &registration.marker, &use_bindings)
            else {
                continue;
            };
            let line = match resolution {
                RegisteredAttributeResolution::Matched => function.start_line,
                RegisteredAttributeResolution::Ambiguous { detail } => {
                    limitations.push(HarnessLimitationFact {
                        registration_id: registration.registration_id.clone(),
                        code: "ambiguous_import".to_string(),
                        file: target.clone(),
                        line: function.start_line,
                        detail,
                    });
                    continue;
                }
                RegisteredAttributeResolution::Unresolved { detail } => {
                    limitations.push(HarnessLimitationFact {
                        registration_id: registration.registration_id.clone(),
                        code: "unresolved_marker_import".to_string(),
                        file: target.clone(),
                        line: function.start_line,
                        detail,
                    });
                    continue;
                }
            };
            // Built-in exact test attributes keep precedence: the function
            // is already an executable test and must not register twice
            // (#3499's families continue to work without duplicate
            // registry entries).
            if function.source_role.registers_executable_test() {
                continue;
            }
            let test = registered_attribute_test_fact(function);
            let subject = HarnessSubjectFact {
                registration_id: registration.registration_id.clone(),
                harness_kind: registration.kind.as_str().to_string(),
                adapter: registration.adapter.as_str().to_string(),
                marker: registration.marker.clone(),
                name: function.name.clone(),
                file: target.clone(),
                start_line: function.start_line,
                end_line: function.end_line,
                body: function.body.clone(),
                calls: function.calls.clone(),
                assertions: test.assertions.clone(),
                literals: function.literals.clone(),
                selector: HarnessSelectorCapability::NamedUnexecuted,
                claim: HarnessSubjectClaim::NamedFunction,
                provenance: TestHarnessRegistration::provenance().to_string(),
            };
            promoted.push((line, function.name.clone(), test, subject));
        }
        promoted
    };
    if promoted_functions.is_empty() {
        return;
    }

    let Some(facts) = index.files.get_mut(&target) else {
        return;
    };
    for function in &mut facts.functions {
        if promoted_functions
            .iter()
            .any(|(line, name, _, _)| *line == function.start_line && *name == function.name)
            && !function.source_role.registers_executable_test()
        {
            function.source_role = FunctionSourceRole::RegisteredTestAttribute;
        }
    }
    for (_, name, test, subject) in &promoted_functions {
        let _ = name;
        subjects.push(subject.clone());
        push_file_test(index, &target, test.clone());
        index.tests.push(test.clone());
    }
    // Mirror the promotion into the flat function list so every consumer
    // of `index.functions` sees the same producer-owned role.
    for function in &mut index.functions {
        if function.file != target {
            continue;
        }
        if promoted_functions
            .iter()
            .any(|(line, name, _, _)| *line == function.start_line && *name == function.name)
            && !function.source_role.registers_executable_test()
        {
            function.source_role = FunctionSourceRole::RegisteredTestAttribute;
        }
    }
}

fn registered_attribute_test_fact(function: &FunctionFact) -> TestFact {
    TestFact {
        name: function.name.clone(),
        file: function.file.clone(),
        start_line: function.start_line,
        end_line: function.end_line,
        body: function.body.clone(),
        calls: function.calls.clone(),
        assertions: parser_oracles_for_function(&function.body, function.start_line)
            .unwrap_or_else(|| {
                crate::analysis::rust_index::extract_assertions(&function.body, function.start_line)
            }),
        literals: function.literals.clone(),
        attrs: function.attrs.clone(),
        // #3727 Slice A: the promotion mirrors the source function's
        // parser-produced shadow facts — same body, same decisions.
        nested_fn_names: function.nested_fn_names.clone(),
        let_bindings: function.let_bindings.clone(),
    }
}

fn push_file_test(index: &mut RustIndex, target: &Path, test: TestFact) {
    let Some(facts) = index.files.get_mut(target) else {
        return;
    };
    if facts
        .tests
        .iter()
        .all(|existing| existing.name != test.name || existing.start_line != test.start_line)
    {
        facts.tests.push(test);
        facts.tests.sort_by(|left, right| {
            left.start_line
                .cmp(&right.start_line)
                .then(left.end_line.cmp(&right.end_line))
                .then(left.name.cmp(&right.name))
        });
    }
}

/// Demote every function in a registered `harness = false` target to the
/// evidence-only helper role and drop the target's executable
/// `TestFact`s. With `harness = false` the libtest harness never
/// collects this file, so a `#[test]` attribute alone establishes nothing and
/// no member enters the executable-test denominator on its own: the
/// whole target is helper-evidence, and executable subjects come only
/// from adapter-established trial registrations.
///
/// Demotion cleans the target by file and span overlap (#3602) rather than
/// by function name alone: any `TestFact` whose declared span overlaps a
/// demoted function in the target file (or belongs to the target) is dropped,
/// preventing any future producer that names `TestFact`s differently from its
/// source function from leaking phantom tests into the harness target.
pub(crate) fn demote_harness_target_functions(index: &mut RustIndex, target: &Path) {
    let demoted_spans: Vec<(usize, usize)> = {
        let Some(facts) = index.files.get(target) else {
            return;
        };
        facts
            .functions
            .iter()
            .filter(|function| function.source_role != FunctionSourceRole::HarnessHelper)
            .map(|function| (function.start_line, function.end_line))
            .collect()
    };
    if demoted_spans.is_empty() {
        return;
    }
    let Some(facts) = index.files.get_mut(target) else {
        return;
    };
    for function in &mut facts.functions {
        function.source_role = FunctionSourceRole::HarnessHelper;
    }
    let overlaps_demoted = |test: &TestFact| -> bool {
        demoted_spans.iter().any(|&(start, end)| {
            // Span overlap between test [test.start_line, test.end_line] and
            // demoted function [start, end].
            test.start_line <= end && test.end_line >= start
        })
    };
    facts.tests.retain(|test| !overlaps_demoted(test));
    index
        .tests
        .retain(|test| test.file != target || !overlaps_demoted(test));
    for function in &mut index.functions {
        if function.file == target {
            function.source_role = FunctionSourceRole::HarnessHelper;
        }
    }
}

enum RegisteredAttributeResolution {
    Matched,
    Ambiguous { detail: String },
    Unresolved { detail: String },
}

/// Resolve one function's attributes against the registered marker.
fn resolve_registered_attribute(
    attrs: &[String],
    marker: &str,
    use_bindings: &BTreeSet<String>,
) -> Option<RegisteredAttributeResolution> {
    // Exact full-path spellings win regardless of attribute order: an
    // earlier bare spelling must not mask a later exact `#[<marker>]`.
    for attribute in attrs {
        if normalized_attribute_path(attribute).as_deref() == Some(marker) {
            return Some(RegisteredAttributeResolution::Matched);
        }
    }
    let (_prefix, name) = marker.rsplit_once("::")?;
    for attribute in attrs {
        let Some(path) = normalized_attribute_path(attribute) else {
            continue;
        };
        if path == marker {
            continue;
        }
        if path != name {
            continue;
        }
        // Bare spelling: the file must bind the name from exactly the
        // registered path. Any other binding of the same name is the
        // same-named-unrelated-import conflict.
        let anchored = use_bindings.contains(marker);
        let conflicting = use_bindings
            .iter()
            .any(|binding| binding != marker && binding.ends_with(&format!("::{name}")));
        if anchored && !conflicting {
            return Some(RegisteredAttributeResolution::Matched);
        }
        if conflicting {
            return Some(RegisteredAttributeResolution::Ambiguous {
                detail: format!(
                    "`#[{name}]` cannot be tied to registered marker `{marker}`; the file's imports bind that name from more than one path"
                ),
            });
        }
        return Some(RegisteredAttributeResolution::Unresolved {
            detail: format!(
                "`#[{name}]` matches the last segment of registered marker `{marker}`, but no top-level import in this file binds that name from the registered path"
            ),
        });
    }
    None
}

fn marker_leaf(marker: &str) -> &str {
    marker.rsplit_once("::").map_or(marker, |(_, leaf)| leaf)
}

/// Collect the full binding paths for `name` from top-level `use` items
/// (e.g. `use libtest_mimic::Trial;` binds `libtest_mimic::Trial`;
/// `use myco::{contract_test, other};` binds `myco::contract_test`).
/// Bounded to the shapes this walker can establish: glob imports, re-exports
/// through nested modules, and aliases away from the bound name grant no
/// binding, so the authority fails closed instead of guessing.
fn top_level_use_bindings(syntax: &ra_ap_syntax::SyntaxNode, name: &str) -> BTreeSet<String> {
    let mut bindings = BTreeSet::new();
    for use_item in syntax.children().filter_map(ast::Use::cast) {
        let Some(use_tree) = use_item.use_tree() else {
            continue;
        };
        collect_use_bindings(&use_tree, String::new(), name, &mut bindings);
    }
    bindings
}

fn collect_use_bindings(
    tree: &ast::UseTree,
    prefix: String,
    name: &str,
    out: &mut BTreeSet<String>,
) {
    let path_text = tree.path().map(|path| {
        path.syntax()
            .text()
            .to_string()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    });
    let alias = tree.rename().and_then(|rename| rename.name());
    if let Some(list) = tree.use_tree_list() {
        let child_prefix = match (&path_text, prefix.is_empty()) {
            (Some(path), false) => format!("{prefix}::{path}"),
            (Some(path), true) => path.clone(),
            (None, _) => prefix,
        };
        for nested in list.use_trees() {
            collect_use_bindings(&nested, child_prefix.clone(), name, out);
        }
        return;
    }
    let Some(tree_name) = alias.map(|named| named.text().to_string()).or_else(|| {
        path_text
            .as_ref()
            .and_then(|path| path.rsplit("::").next())
            .map(ToString::to_string)
    }) else {
        return;
    };
    if tree_name != name {
        return;
    }
    let full = match (&path_text, prefix.is_empty()) {
        (Some(path), false) => format!("{prefix}::{path}"),
        (Some(path), true) => path.clone(),
        (None, false) => prefix,
        (None, true) => tree_name,
    };
    out.insert(full);
}

enum TrialBindingResolution {
    MarkerAnchored,
    Ambiguous { conflicting: String },
    Unbound,
}

fn resolve_trial_binding(bindings: &BTreeSet<String>, marker: &str) -> TrialBindingResolution {
    let anchored_full = format!("{marker}::Trial");
    let matches_marker = |binding: &String| {
        binding == &anchored_full || binding.ends_with(&format!("::{anchored_full}"))
    };
    let anchored = bindings.iter().any(matches_marker);
    let conflicting = bindings
        .iter()
        .filter(|binding| !matches_marker(binding))
        .cloned()
        .collect::<Vec<_>>();
    if anchored && conflicting.is_empty() {
        return TrialBindingResolution::MarkerAnchored;
    }
    if anchored || conflicting.len() > 1 {
        return TrialBindingResolution::Ambiguous {
            conflicting: conflicting.join(", "),
        };
    }
    // No binding at all, or exactly one binding from another path: the
    // invocation cannot be tied to the registered marker.
    TrialBindingResolution::Unbound
}

/// Whether the ancestor chain of a token sits inside a loop expression —
/// the bounded signal for runtime-only trial discovery. Works through
/// macro token trees too, because tokens keep their ancestor nodes.
fn in_loop(mut ancestors: impl Iterator<Item = ra_ap_syntax::SyntaxNode>) -> bool {
    ancestors.any(|ancestor| {
        ast::WhileExpr::can_cast(ancestor.kind())
            || ast::ForExpr::can_cast(ancestor.kind())
            || ast::LoopExpr::can_cast(ancestor.kind())
    })
}

/// Whether the token sits inside a `macro_rules!` definition body: the
/// tokens there are a template, not an executed registration.
fn inside_macro_rules(mut ancestors: impl Iterator<Item = ra_ap_syntax::SyntaxNode>) -> bool {
    ancestors.any(|ancestor| ast::MacroRules::can_cast(ancestor.kind()))
}

/// Index just past the `macro_rules! name <group>` definition whose
/// `macro_rules` token sits at `index`, or `None` when the shape is not a
/// definition. Trivia-tolerant — `macro_rules` and `!` may be separated
/// by whitespace or comments — and balanced at the token level over every
/// delimiter Rust permits for the template body (`(...)`, `[...]`,
/// `{...}`), because a definition inside another macro's token tree
/// carries no `MacroRules` node to query.
fn skip_macro_rules_definition(
    tokens: &[ra_ap_syntax::SyntaxToken],
    index: usize,
) -> Option<usize> {
    let bang = next_significant(tokens, index + 1)?;
    if tokens[bang].kind() != ra_ap_syntax::SyntaxKind::BANG {
        return None;
    }
    let name = next_significant(tokens, bang + 1)?;
    if tokens[name].kind() != ra_ap_syntax::SyntaxKind::IDENT {
        return None;
    }
    let open = next_significant(tokens, name + 1)?;
    let close = matching_group_close(tokens, open)?;
    Some(close + 1)
}

/// Exact simple string-literal text of one token. Escaped spellings,
/// raw strings, and empty names are not demonstrated by this bounded parser and
/// stay unresolved (fail closed).
fn string_literal_token_text(token: &ra_ap_syntax::SyntaxToken) -> Option<String> {
    if token.kind() != ra_ap_syntax::SyntaxKind::STRING {
        return None;
    }
    let text = token.text().trim();
    let body = text.strip_prefix('"')?.strip_suffix('"')?;
    if body.is_empty() || body.contains(BACKSLASH) {
        return None;
    }
    Some(body.to_string())
}

/// The one escape character this bounded parser refuses to guess about.
const BACKSLASH: char = '\\';

/// The bounded fail-closed reachability authority for trial subjects
/// (#3636): run-entry anchoring, argument resolution, and the
/// reachable/unknown/unreachable verdicts.
mod reachability;

#[cfg(test)]
mod tests;
