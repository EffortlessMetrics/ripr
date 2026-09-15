//! Native Python repo-mode evidence producer (#3554 PR B) and its public
//! Finding projection (#3554 PR C).
//!
//! [`build_repo_evidence`] builds facts ONCE over the selected working set
//! (`PythonRepoInput`, PR A) and produces native Python behavior items with
//! related-test/oracle evidence and typed limitations. It reuses the same
//! producer-owned authorities as diff mode and never re-derives Python
//! semantics:
//!
//! - `extract_source_facts`: the single read+parse per file, yielding
//!   owners, tests, and per-statement source facts;
//! - `owner_for_changed_line`: the shared line-to-owner mapping;
//! - `classify_probe_shape`: the native `ProbeFamily`/`DeltaKind`;
//! - `canonical_python_gap_for`: the same canonical gap identity rules as
//!   diff mode;
//! - `related_test_candidates` / `find_related_tests`: the shared matcher
//!   (exact/import/call/helper evidence, uncertain name-similarity links);
//! - `classify_sink_alignment_with_old`: the sink-alignment read-out;
//! - `static_limit_for_change`: typed dynamic-resolution and
//!   unsupported-syntax limits.
//!
//! Boundaries held by this producer:
//!
//! - no Rust projection and no `SeamKind` bridge (#3039/#1937): items carry
//!   the native `ProbeFamily`, exact owner identity, discriminator,
//!   relation, oracle kind/strength, and limitations separately;
//! - production behavior items come only from `production_files`; test and
//!   helper files enter the evidence (test) pool only and never seed
//!   production findings (#3554);
//! - conservative vocabulary only: statuses and limits describe what the
//!   run covered; the mutation-outcome vocabulary is never used here (see
//!   the workspace language rules);
//! - deterministic: iteration follows the sorted selected inputs and parse
//!   order, with no timestamps, so identical input yields identical
//!   evidence (pinned by a byte/digest test below).
//!
//! PR C projection: the same pass also emits the public `Finding`s
//! ([`PythonRepoEvidence::findings`]) by running the diff-mode classifier
//! (`classify_change_with_context`) over each representative behavior line,
//! so repo-mode findings are indistinguishable in shape from diff-mode
//! findings and retain native Python identity. [`partial_disclosure`]
//! renders the typed partial/capped run state for the pipeline's shared
//! `LanguageRun` channel; `analyze_repo` (PR C) is the production caller.
//!
//! Parse timeouts are NOT typed here: the parser substrate
//! (`rustpython_parser::parse`) exposes no per-file timeout mechanism, and
//! a fabricated timeout row would be invented evidence. The deferral is
//! recorded in this doc comment until a real producer condition exists
//! (#3554 PR B); read and parse failures are typed as
//! [`PythonRepoLimitation::ParseFailure`] rows today.

use super::super::classify::{PythonNoBehaviorContext, classify_change_with_context};
use super::super::probe_shape::{canonical_python_gap_for, classify_probe_shape};
use super::super::related_tests::{
    PythonRelatedCandidate, find_related_tests, related_test_candidates,
};
use super::super::sink_alignment::classify_sink_alignment_with_old;
use super::super::source_facts::{
    PythonSourceFact, PythonSourceFactKind, PythonSourceFacts, extract_source_facts,
    source_fact_snapshot_observation, source_facts_parse_error,
};
use super::super::source_utils::normalized_path;
use super::super::static_limits::static_limit_for_change;
use super::super::workspace::owner_for_changed_line;
use super::super::{PythonOwner, PythonTest};
use super::roles::PythonFileRole;
use super::run_status::{PartialRunReason, PythonRepoRunStatus};
use super::{CapRecoveryRoute, DiscoveryCounts, PythonRepoInput, RepoWorkingSetLimit};
use crate::domain::{
    DeltaKind, Finding, FindingCanonicalGap, OracleKind, OracleStrength, ProbeFamily, RelatedTest,
    StaticLimitKind,
};
use std::collections::BTreeMap;
use std::path::Path;

/// Native Python repo-mode evidence for one bounded run (#3554).
///
/// The producer-level shape and the public projection in one deterministic
/// pass: [`Self::findings`] carries the `LanguageRepoResult` payload that
/// `analyze_repo` (PR C) publishes, built by the same diff-mode classifier
/// over the same representative behavior lines, so the two views can never
/// disagree about what the run analyzed.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::analysis::language::python) struct PythonRepoEvidence {
    /// Post-analysis run status. Only [`PythonRepoRunStatus::Complete`] can
    /// back a full-denominator claim.
    pub(in crate::analysis::language::python) status: PythonRepoRunStatus,
    /// Working-set counts with the producer-owned `failed`/`analyzed`
    /// population over the selected files.
    pub(in crate::analysis::language::python) counts: DiscoveryCounts,
    /// The effective working-set limit and its cap source (#2109).
    pub(in crate::analysis::language::python) working_set_limit: RepoWorkingSetLimit,
    /// Operator recovery routes retained for capped runs (#2109).
    pub(in crate::analysis::language::python) recovery_routes: &'static [CapRecoveryRoute],
    /// Test framework detected by the shared producer at the workspace root.
    pub(in crate::analysis::language::python) test_framework: Option<&'static str>,
    /// Per-file input identity for every selected file, sorted by path.
    pub(in crate::analysis::language::python) files: Vec<PythonRepoFileRecord>,
    /// Production-owner evidence (behavior items + related-test evidence),
    /// in production-file order and parse order within each file.
    pub(in crate::analysis::language::python) owners: Vec<PythonRepoOwnerEvidence>,
    /// Typed limitations, in deterministic construction order.
    pub(in crate::analysis::language::python) limitations: Vec<PythonRepoLimitation>,
    /// Public `Finding` projection over the production behavior items,
    /// in production-file order and ascending line order within each file.
    /// Built by `classify_change_with_context` — the same classifier diff
    /// mode uses — so findings are shape-identical to diff-mode findings
    /// with native Python identity (`language: python`, preview status,
    /// `gap:python:` canonical gaps); no Rust `SeamKind` bridge (#3039).
    pub(in crate::analysis::language::python) findings: Vec<Finding>,
}

/// Per-file input identity for one selected file (#3554).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct PythonRepoFileRecord {
    /// Workspace-relative normalized path (forward slashes).
    pub(in crate::analysis::language::python) path: String,
    /// Shared role label (`PythonFileRole::as_str`).
    pub(in crate::analysis::language::python) role: &'static str,
    /// Whether the file was read and parsed successfully. `false` for
    /// read/parse failures and for ambiguous-role files, which are counted
    /// but never read.
    pub(in crate::analysis::language::python) analyzed: bool,
    /// Owners harvested (production files only).
    pub(in crate::analysis::language::python) owner_count: usize,
    /// Tests contributed to the evidence pool (evidence files only).
    pub(in crate::analysis::language::python) test_count: usize,
    /// Behavior items produced (production files only).
    pub(in crate::analysis::language::python) behavior_item_count: usize,
}

/// Evidence for one production owner: exact identity, native behavior
/// items, and related-test/oracle evidence (#3554).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct PythonRepoOwnerEvidence {
    /// Workspace-relative normalized path of the owner's file.
    pub(in crate::analysis::language::python) file: String,
    /// Qualified owner name (`Class.method`, function name, or `<module>`).
    pub(in crate::analysis::language::python) owner: String,
    /// Stable owner-kind label from the shared owner authority.
    pub(in crate::analysis::language::python) owner_kind: &'static str,
    /// Exact owner symbol identity (`python:<file>::<qualified>`).
    pub(in crate::analysis::language::python) symbol: String,
    /// Native behavior items, sorted by line.
    pub(in crate::analysis::language::python) behavior_items: Vec<PythonRepoBehaviorItem>,
    /// Related-test evidence via the shared matcher. An empty list means no
    /// related test was established — distinct from related-but-unaligned
    /// or related-but-weak below (#3554 evidence semantics).
    pub(in crate::analysis::language::python) related_tests: Vec<PythonRepoRelatedTestEvidence>,
}

/// One native Python behavior item: the repo-mode analog of a diff-mode
/// probe subject (#3554).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct PythonRepoBehaviorItem {
    /// 1-indexed source line of the behavior statement.
    pub(in crate::analysis::language::python) line: usize,
    /// Native Python probe family — never bridged to a Rust `SeamKind`
    /// (#3039/#1937).
    pub(in crate::analysis::language::python) family: ProbeFamily,
    /// The delta kind from the shared probe-shape authority.
    pub(in crate::analysis::language::python) delta: DeltaKind,
    /// The behavior statement text: the discriminator a test must observe
    /// (the repo-mode analog of the diff-mode changed line).
    pub(in crate::analysis::language::python) discriminator: String,
    /// Canonical gap identity under the same rules as diff mode. `None`
    /// when a static limit applies, mirroring the diff-mode classifier.
    pub(in crate::analysis::language::python) canonical_gap: Option<FindingCanonicalGap>,
    /// Strongest related-oracle kind (`Unknown` when no related test was
    /// established).
    pub(in crate::analysis::language::python) strongest_oracle_kind: OracleKind,
    /// Strongest related-oracle strength (`Unknown` when no related test
    /// was established).
    pub(in crate::analysis::language::python) strongest_oracle_strength: OracleStrength,
    /// Whether any related test reaches the owner through oracle-eligible
    /// evidence (exact/import/call/helper), not just name similarity.
    pub(in crate::analysis::language::python) has_oracle_eligible_relation: bool,
    /// Sink-alignment read-out: `direct | alias | changed_sink_token |
    /// orthogonal | unknown`. `orthogonal` is the sibling-observer case: a
    /// strong oracle that observes a different value never discriminates
    /// the changed behavior (no reach-plus-oracle over-credit).
    pub(in crate::analysis::language::python) oracle_alignment: String,
    /// Why the alignment read-out holds, verbatim from the shared authority.
    pub(in crate::analysis::language::python) alignment_reason: String,
    /// The changed-sink token the item's behavior statement touches.
    pub(in crate::analysis::language::python) changed_sink: Option<String>,
    /// The sink token the strongest related oracle actually observes.
    pub(in crate::analysis::language::python) observed_sink: Option<String>,
    /// Typed static limit for this item (dynamic import/dispatch,
    /// metaprogramming, mocks, decorator indirection, unsupported syntax).
    pub(in crate::analysis::language::python) static_limit: Option<StaticLimitKind>,
}

/// Related-test evidence for one owner relation, retained separately from
/// the oracle and the alignment (#3554 evidence semantics).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct PythonRepoRelatedTestEvidence {
    pub(in crate::analysis::language::python) test_name: String,
    pub(in crate::analysis::language::python) test_file: String,
    pub(in crate::analysis::language::python) test_line: usize,
    /// Shared matcher relation (`syntactic_call`, `import_alias_call`, ...).
    pub(in crate::analysis::language::python) relation: &'static str,
    /// Whether the relation is oracle-eligible (exact/import/call/helper
    /// evidence rather than name similarity).
    pub(in crate::analysis::language::python) relation_uses_oracle: bool,
    /// Whether the relation is uncertain (`same_stem`, test-name
    /// similarity, fixture name) — the matcher's wrong-owner guard surface:
    /// an uncertain relation may belong to a different, similarly named
    /// owner and must never be read as a discriminating reach.
    pub(in crate::analysis::language::python) relation_uncertain: bool,
    pub(in crate::analysis::language::python) oracle_kind: OracleKind,
    pub(in crate::analysis::language::python) oracle_strength: OracleStrength,
    /// The strongest assertion text when the relation is oracle-eligible.
    pub(in crate::analysis::language::python) oracle_text: Option<String>,
    pub(in crate::analysis::language::python) parametrized: bool,
}

/// Typed limitations retained by the evidence producer (#3554).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) enum PythonRepoLimitation {
    /// A selected file could not be read or parsed: it leaves the analyzed
    /// denominator (`counts.failed`) and the run is partial. Parse
    /// timeouts will land here as typed rows once the parser substrate
    /// supports a per-file timeout; the current substrate has none
    /// (deferred — see the module docs).
    ParseFailure {
        /// Workspace-relative normalized path of the failed file.
        file: String,
        /// Producer-owned failure reason (`read_error: ...` or the
        /// snapshot's `source_fact_parse_error: ...` evidence).
        reason: String,
    },
    /// A behavior item hit a typed static limit: dynamic resolution
    /// (dynamic import/dispatch, metaprogramming, mocked modules) or an
    /// unsupported syntax shape. The item stays in the inventory with the
    /// limit attached; this row keeps the per-site identity so limitations
    /// can be enumerated without re-deriving them from items.
    StaticLimit {
        /// Workspace-relative normalized path of the site's file.
        file: String,
        /// Qualified owner name of the site.
        owner: String,
        /// 1-indexed source line of the site.
        line: usize,
        /// The shared typed static-limit kind.
        kind: StaticLimitKind,
    },
}

/// Build the native Python repo evidence for one selected input (#3554).
///
/// Deterministic: `build_repo_evidence` over identical workspace content
/// and identical inputs returns identical evidence and identical findings.
pub(in crate::analysis::language::python) fn build_repo_evidence(
    input: &PythonRepoInput,
    root: &Path,
) -> PythonRepoEvidence {
    let mut counts = input.counts;
    let mut files = Vec::new();
    let mut limitations = Vec::new();
    let mut owners = Vec::new();
    let mut findings = Vec::new();

    // No-subject and disabled runs analyze nothing: an honest zero with the
    // status reason, never fabricated limitations (#3554).
    if matches!(
        input.status,
        PythonRepoRunStatus::NoPythonSource | PythonRepoRunStatus::Disabled
    ) {
        return PythonRepoEvidence {
            status: input.status.clone(),
            counts,
            working_set_limit: input.working_set_limit,
            recovery_routes: input.recovery_routes,
            test_framework: input.test_framework,
            files,
            owners,
            limitations,
            findings,
        };
    }

    let mut failed = 0usize;
    let mut analyzed = 0usize;
    let mut all_tests: Vec<PythonTest> = Vec::new();

    // Evidence sources first, so production items can relate against the
    // full test pool. Owners are intentionally NOT harvested from
    // test/helper files: they enter the evidence pool only, so a
    // production-looking `def` in a test file can never seed production
    // findings (#3554).
    for (role, selected) in [
        (PythonFileRole::PhysicalTest, &input.test_files),
        (PythonFileRole::InlineHelper, &input.helper_files),
    ] {
        for relative in selected {
            match load_facts(root, relative) {
                Ok(facts) => {
                    analyzed += 1;
                    files.push(PythonRepoFileRecord {
                        path: normalized_path(relative),
                        role: role.as_str(),
                        analyzed: true,
                        owner_count: 0,
                        test_count: facts.tests.len(),
                        behavior_item_count: 0,
                    });
                    all_tests.extend(facts.tests);
                }
                Err(reason) => {
                    failed += 1;
                    limitations.push(PythonRepoLimitation::ParseFailure {
                        file: normalized_path(relative),
                        reason,
                    });
                    files.push(PythonRepoFileRecord {
                        path: normalized_path(relative),
                        role: role.as_str(),
                        analyzed: false,
                        owner_count: 0,
                        test_count: 0,
                        behavior_item_count: 0,
                    });
                }
            }
        }
    }

    // Production subjects: owners + behavior items, related against the
    // evidence pool built above. The public Finding projection is built in
    // the same pass so both views share one read+parse per file (PR C).
    for relative in &input.production_files {
        match load_facts(root, relative) {
            Ok(facts) => {
                analyzed += 1;
                let owner_evidence = build_production_file_evidence(
                    relative,
                    &facts,
                    &all_tests,
                    &mut limitations,
                    &mut findings,
                );
                let behavior_item_count = owner_evidence
                    .iter()
                    .map(|owner| owner.behavior_items.len())
                    .sum();
                let owner_count = owner_evidence.len();
                files.push(PythonRepoFileRecord {
                    path: normalized_path(relative),
                    role: PythonFileRole::Production.as_str(),
                    analyzed: true,
                    owner_count,
                    test_count: 0,
                    behavior_item_count,
                });
                owners.extend(owner_evidence);
            }
            Err(reason) => {
                failed += 1;
                limitations.push(PythonRepoLimitation::ParseFailure {
                    file: normalized_path(relative),
                    reason,
                });
                files.push(PythonRepoFileRecord {
                    path: normalized_path(relative),
                    role: PythonFileRole::Production.as_str(),
                    analyzed: false,
                    owner_count: 0,
                    test_count: 0,
                    behavior_item_count: 0,
                });
            }
        }
    }

    // Ambiguous-role files are counted, never read: they stay visible as
    // unanalyzed input identity (#3554 PR A role contract).
    for relative in &input.ambiguous_files {
        files.push(PythonRepoFileRecord {
            path: normalized_path(relative),
            role: PythonFileRole::Unknown.as_str(),
            analyzed: false,
            owner_count: 0,
            test_count: 0,
            behavior_item_count: 0,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    counts.failed = failed;
    counts.analyzed = analyzed;

    // Post-analysis status. Discovery incompleteness dominates (the corpus
    // was never fully enumerated), and an existing cap stays the reported
    // structural fact — parse failures remain typed in `counts.failed` and
    // the limitation rows either way.
    let status = if matches!(
        &input.status,
        PythonRepoRunStatus::Partial {
            reason: PartialRunReason::DiscoveryIncomplete { .. }
        }
    ) || input.status == PythonRepoRunStatus::Capped
    {
        input.status.clone()
    } else if failed > 0 {
        PythonRepoRunStatus::Partial {
            reason: PartialRunReason::ParseFailures { failed },
        }
    } else if !input.ambiguous_files.is_empty() {
        PythonRepoRunStatus::Partial {
            reason: PartialRunReason::UnanalyzedAmbiguous {
                count: input.ambiguous_files.len(),
            },
        }
    } else {
        PythonRepoRunStatus::Complete
    };

    PythonRepoEvidence {
        status,
        counts,
        working_set_limit: input.working_set_limit,
        recovery_routes: input.recovery_routes,
        test_framework: input.test_framework,
        files,
        owners,
        limitations,
        findings,
    }
}

/// Typed partial-run disclosure for the pipeline's shared `LanguageRun`
/// channel (#3554, #2109).
///
/// `Some` exactly when the post-analysis status is capped or partial — the
/// same condition that refuses a full-denominator claim — so the pipeline
/// records a `Partial` language run that human/JSON output renders and
/// gates fail closed on. A `Complete` run and an honest zero
/// (`NoPythonSource`) disclose nothing: neither is a partial denominator.
pub(in crate::analysis::language::python) fn partial_disclosure(
    evidence: &PythonRepoEvidence,
) -> Option<String> {
    match &evidence.status {
        PythonRepoRunStatus::Complete
        | PythonRepoRunStatus::Selected
        | PythonRepoRunStatus::NoPythonSource
        | PythonRepoRunStatus::Disabled => None,
        PythonRepoRunStatus::Capped => {
            let routes = evidence
                .recovery_routes
                .iter()
                .map(|route| route.describe())
                .collect::<Vec<_>>()
                .join("; ");
            let recovery_suffix = if routes.is_empty() {
                String::new()
            } else {
                format!(" Recovery: {routes}")
            };
            Some(format!(
                "repo run capped: {} Python file(s) beyond the working-set limit of {}; findings cover the selected files only.{}",
                evidence.counts.capped, evidence.working_set_limit.limit, recovery_suffix
            ))
        }
        PythonRepoRunStatus::Partial { reason } => Some(match reason {
            PartialRunReason::ParseFailures { failed } => format!(
                "repo run partial: {failed} selected Python file(s) failed to read or parse and left the analyzed denominator"
            ),
            PartialRunReason::DiscoveryIncomplete { unreadable } => format!(
                "repo run partial: {unreadable} workspace subtree(s) could not be read, so discovery may have missed Python source"
            ),
            PartialRunReason::UnanalyzedAmbiguous { count } => format!(
                "repo run partial: {count} selected Python file(s) had an ambiguous role and were counted but never analyzed"
            ),
        }),
    }
}

/// Read a selected file once and parse it once via the shared facts
/// producer. A read or parse failure becomes the typed limitation reason.
fn load_facts(root: &Path, relative: &Path) -> Result<PythonSourceFacts, String> {
    let absolute = root.join(relative);
    let source = std::fs::read_to_string(&absolute).map_err(|err| format!("read_error: {err}"))?;
    let facts = extract_source_facts(relative, &source);
    debug_assert!(source_fact_snapshot_observation(&facts) > 0);
    match source_facts_parse_error(&facts) {
        Some(limitation) => Err(limitation.evidence.clone()),
        None => Ok(facts),
    }
}

/// Build owner evidence for one successfully parsed production file:
/// behavior items grouped per owner via the shared line-to-owner mapping,
/// with relations matched once per owner against the evidence pool.
///
/// The public `Finding`s are built in the same pass by
/// [`classify_change_with_context`] — the same classifier diff mode runs on
/// a changed line — so repo-mode findings are shape-identical to diff-mode
/// findings (#3554 PR C). `old_line_text` is `None` (repo mode has no diff
/// side); the classifier's currentness derivation then records
/// `CandidateCurrent`, matching repo mode's live-tree seeding (#3280).
fn build_production_file_evidence(
    relative: &Path,
    facts: &PythonSourceFacts,
    all_tests: &[PythonTest],
    limitations: &mut Vec<PythonRepoLimitation>,
    findings: &mut Vec<Finding>,
) -> Vec<PythonRepoOwnerEvidence> {
    let file_owners = &facts.owners;
    let mut items_per_owner: Vec<Vec<PythonRepoBehaviorItem>> = vec![Vec::new(); file_owners.len()];
    // Relations are matched once per owner (facts built once), then reused
    // by every behavior item of that owner.
    let owner_relations: Vec<(Vec<PythonRelatedCandidate<'_>>, Vec<RelatedTest>)> = file_owners
        .iter()
        .map(|owner| {
            (
                related_test_candidates(owner, all_tests),
                find_related_tests(owner, all_tests),
            )
        })
        .collect();

    for (line, text) in representative_behavior_lines(&facts.facts) {
        let Some(owner) = owner_for_changed_line(relative, line, file_owners) else {
            continue;
        };
        let Some(index) = file_owners
            .iter()
            .position(|candidate| std::ptr::eq(candidate, owner))
        else {
            continue;
        };
        let (candidates, related) = &owner_relations[index];
        let item =
            build_behavior_item(relative, owner, line, &text, candidates, related, all_tests);
        if let Some(kind) = item.static_limit {
            limitations.push(PythonRepoLimitation::StaticLimit {
                file: normalized_path(relative),
                owner: owner.qualified_name.clone(),
                line,
                kind,
            });
        }
        items_per_owner[index].push(item);
        if let Some(mut finding) = classify_change_with_context(
            relative,
            line,
            &text,
            None,
            file_owners,
            all_tests,
            PythonNoBehaviorContext::default(),
        ) {
            // Repo mode classifies every representative behavior line, so
            // identical statements under one owner produce identical probe
            // ids. Suffix the duplicates (.2, .3, ...) the way the repo
            // probe inventory does, so suppression and selection resolve a
            // single finding instead of hiding the rest (#3668 review).
            let base_id = finding.id.clone();
            let count = findings
                .iter()
                .filter(|existing| {
                    existing.id == base_id
                        || existing.id.strip_prefix(&base_id).is_some_and(|suffix| {
                            suffix.starts_with('.')
                                && suffix[1..].chars().all(|c| c.is_ascii_digit())
                        })
                })
                .count()
                + 1;
            if count > 1 {
                let suffixed = format!("{base_id}.{count}");
                finding.id = suffixed.clone();
                finding.probe.id.0 = suffixed;
            }
            findings.push(finding);
        }
    }

    file_owners
        .iter()
        .enumerate()
        .map(|(index, owner)| {
            let (candidates, related) = &owner_relations[index];
            let mut behavior_items = std::mem::take(&mut items_per_owner[index]);
            behavior_items.sort_by_key(|item| item.line);
            let related_tests = candidates
                .iter()
                .zip(related.iter())
                .map(|(candidate, related)| PythonRepoRelatedTestEvidence {
                    test_name: candidate.test.name.clone(),
                    test_file: normalized_path(&candidate.test.file),
                    test_line: candidate.test.line,
                    relation: candidate.relation.as_str(),
                    relation_uses_oracle: candidate.relation.uses_oracle(),
                    relation_uncertain: candidate.relation.is_uncertain(),
                    oracle_kind: related.oracle_kind.clone(),
                    oracle_strength: related.oracle_strength.clone(),
                    oracle_text: related.oracle.clone(),
                    parametrized: candidate.test.parametrized,
                })
                .collect();
            PythonRepoOwnerEvidence {
                file: normalized_path(relative),
                owner: owner.qualified_name.clone(),
                owner_kind: owner.kind_label(),
                symbol: owner.symbol_id().0,
                behavior_items,
                related_tests,
            }
        })
        .collect()
}

/// Build one behavior item from the shared authorities: probe shape, gap
/// identity, sink alignment, static limit, and strongest related oracle.
fn build_behavior_item(
    relative: &Path,
    owner: &PythonOwner,
    line: usize,
    text: &str,
    candidates: &[PythonRelatedCandidate<'_>],
    related: &[RelatedTest],
    all_tests: &[PythonTest],
) -> PythonRepoBehaviorItem {
    let (family, delta) = classify_probe_shape(text);
    let static_limit = static_limit_for_change(text, owner, candidates).map(|limit| limit.kind);
    let canonical_gap = static_limit
        .is_none()
        .then(|| canonical_python_gap_for(relative, owner, &family, text));
    let alignment = classify_sink_alignment_with_old(owner, text, None, related, all_tests);
    let strongest = related
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .cloned();
    let (strongest_oracle_kind, strongest_oracle_strength) = strongest
        .map(|test| (test.oracle_kind, test.oracle_strength))
        .unwrap_or((OracleKind::Unknown, OracleStrength::Unknown));
    PythonRepoBehaviorItem {
        line,
        family,
        delta,
        discriminator: text.to_string(),
        canonical_gap,
        strongest_oracle_kind,
        strongest_oracle_strength,
        has_oracle_eligible_relation: candidates
            .iter()
            .any(|candidate| candidate.relation.uses_oracle()),
        oracle_alignment: alignment.oracle_alignment,
        alignment_reason: alignment.alignment_reason,
        changed_sink: alignment.changed_sink,
        observed_sink: alignment.observed_sink,
        static_limit,
    }
}

/// Reduce one file's source facts to one representative behavior fact per
/// source line, in ascending line order.
///
/// Only behavior-bearing fact kinds are eligible — structural kinds
/// (module/class/function/method/decorator/parameter) and string literals
/// (docstrings) classify no runtime behavior. Per line the representative
/// is the largest-span fact (the outermost statement), with the derived
/// fact-kind order and then the start byte as deterministic tie-breakers.
fn representative_behavior_lines(facts: &[PythonSourceFact]) -> Vec<(usize, String)> {
    let mut per_line: BTreeMap<usize, &PythonSourceFact> = BTreeMap::new();
    for fact in facts {
        if !is_behavior_fact_kind(fact.kind) {
            continue;
        }
        let candidate_key = representative_key(fact);
        match per_line.get(&fact.start_line) {
            Some(current) if representative_key(current) >= candidate_key => {}
            _ => {
                per_line.insert(fact.start_line, fact);
            }
        }
    }
    per_line
        .into_values()
        .map(|fact| (fact.start_line, fact.text.clone()))
        .collect()
}

/// Statement kinds that carry runtime behavior for probe-shape
/// classification.
fn is_behavior_fact_kind(kind: PythonSourceFactKind) -> bool {
    matches!(
        kind,
        PythonSourceFactKind::Return
            | PythonSourceFactKind::Raise
            | PythonSourceFactKind::Predicate
            | PythonSourceFactKind::Comparison
            | PythonSourceFactKind::BooleanExpression
            | PythonSourceFactKind::Call
            | PythonSourceFactKind::Assignment
            | PythonSourceFactKind::AttributeWrite
            | PythonSourceFactKind::DictLiteral
            | PythonSourceFactKind::ListLiteral
            | PythonSourceFactKind::SetLiteral
            | PythonSourceFactKind::PrintCall
            | PythonSourceFactKind::LogCall
    )
}

/// Deterministic representative ranking: largest span first (outermost
/// statement wins), then the smallest fact-kind ordinal (statement kinds
/// before their nested expression kinds), then the largest start byte.
fn representative_key(
    fact: &PythonSourceFact,
) -> (usize, std::cmp::Reverse<PythonSourceFactKind>, usize) {
    (
        fact.end_byte.saturating_sub(fact.start_byte),
        std::cmp::Reverse(fact.kind),
        fact.start_byte,
    )
}

#[cfg(test)]
mod tests {
    use super::super::discovery::{CAP_RECOVERY_ROUTES, RepoWorkingSetCapSource};
    use super::*;
    use std::path::PathBuf;

    fn write_file(path: &Path, contents: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
        }
        std::fs::write(path, contents).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn unique_test_root(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-py-repo-evidence-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn default_limit() -> RepoWorkingSetLimit {
        RepoWorkingSetLimit {
            limit: 800,
            source: RepoWorkingSetCapSource::Default,
        }
    }

    fn fnv1a64(bytes: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    fn find_owner<'a>(
        evidence: &'a PythonRepoEvidence,
        owner: &str,
    ) -> Result<&'a PythonRepoOwnerEvidence, String> {
        evidence
            .owners
            .iter()
            .find(|candidate| candidate.owner == owner)
            .ok_or_else(|| format!("owner {owner} missing from evidence"))
    }

    fn find_file<'a>(
        evidence: &'a PythonRepoEvidence,
        path: &str,
    ) -> Result<&'a PythonRepoFileRecord, String> {
        evidence
            .files
            .iter()
            .find(|candidate| candidate.path == path)
            .ok_or_else(|| format!("file {path} missing from evidence"))
    }

    #[test]
    fn flat_layout_yields_nonzero_evidence_with_relations_and_oracles() -> Result<(), String> {
        let root = unique_test_root("flat");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        write_file(
            &root.join("test_app.py"),
            "from app import run\n\n\ndef test_run():\n    assert run() == 1\n",
        )?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.status, PythonRepoRunStatus::Complete);
        assert!(evidence.status.can_support_full_denominator());
        let counts = &evidence.counts;
        assert_eq!(counts.discovered, 2);
        assert_eq!(counts.selected, 2);
        assert_eq!(counts.analyzed_candidates, 1);
        assert_eq!(counts.analyzed, 2);
        assert_eq!(counts.failed, 0);
        assert_eq!(counts.analyzed + counts.failed, counts.selected);
        assert!(evidence.limitations.is_empty());

        // Per-file input identity.
        let app = find_file(&evidence, "app.py")?;
        assert_eq!(app.role, "production");
        assert!(app.analyzed);
        assert_eq!(app.owner_count, 2, "module owner + run");
        assert_eq!(app.behavior_item_count, 1);
        let test = find_file(&evidence, "test_app.py")?;
        assert_eq!(test.role, "physical_test");
        assert!(test.analyzed);
        assert_eq!(test.test_count, 1);
        assert_eq!(test.owner_count, 0);
        assert_eq!(evidence.files.len(), 2);

        // Native behavior item identity for the `run` owner.
        let run = find_owner(&evidence, "run")?;
        assert_eq!(run.file, "app.py");
        assert_eq!(run.owner_kind, "function");
        assert_eq!(run.symbol, "python:app.py::run");
        assert_eq!(run.behavior_items.len(), 1);
        let item = &run.behavior_items[0];
        assert_eq!(item.line, 2);
        assert_eq!(item.family, ProbeFamily::ReturnValue);
        assert_eq!(item.delta, DeltaKind::Value);
        assert_eq!(item.discriminator, "return 1");
        let gap = item
            .canonical_gap
            .as_ref()
            .ok_or("canonical gap expected without static limits")?;
        assert!(
            gap.id.starts_with("gap:python:app.py:run:return_value:"),
            "{}",
            gap.id
        );
        assert_eq!(gap.language, "python");
        assert_eq!(gap.owner, "run");

        // Related-test evidence through the shared matcher.
        assert_eq!(run.related_tests.len(), 1);
        let related = &run.related_tests[0];
        assert_eq!(related.test_name, "test_run");
        assert_eq!(related.test_file, "test_app.py");
        assert_eq!(related.relation, "syntactic_call");
        assert!(related.relation_uses_oracle);
        assert!(!related.relation_uncertain);
        assert_eq!(related.oracle_kind, OracleKind::ExactValue);
        assert_eq!(related.oracle_strength, OracleStrength::Strong);
        assert_eq!(related.oracle_text.as_deref(), Some("assert run() == 1"));
        assert!(item.has_oracle_eligible_relation);
        assert_eq!(item.strongest_oracle_kind, OracleKind::ExactValue);
        assert_eq!(item.strongest_oracle_strength, OracleStrength::Strong);
        assert!(
            [
                "direct",
                "alias",
                "changed_sink_token",
                "orthogonal",
                "unknown"
            ]
            .contains(&item.oracle_alignment.as_str()),
            "unexpected alignment {}",
            item.oracle_alignment
        );
        assert!(!item.alignment_reason.is_empty());

        // The module owner is inventoried without behavior items.
        let module = find_owner(&evidence, "<module>")?;
        assert_eq!(module.file, "app.py");
        assert_eq!(module.owner_kind, "module_function");
        assert!(module.behavior_items.is_empty());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn src_layout_yields_nonzero_evidence() -> Result<(), String> {
        let root = unique_test_root("src-layout");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("src/pkg/core.py"), "def core():\n    return 1\n")?;
        write_file(
            &root.join("tests/test_core.py"),
            "from pkg.core import core\n\n\ndef test_core():\n    assert core() == 1\n",
        )?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.status, PythonRepoRunStatus::Complete);
        let core_file = find_file(&evidence, "src/pkg/core.py")?;
        assert_eq!(core_file.role, "production");
        assert!(core_file.behavior_item_count > 0);
        let core = find_owner(&evidence, "core")?;
        assert_eq!(core.file, "src/pkg/core.py");
        assert_eq!(core.symbol, "python:src/pkg/core.py::core");
        assert_eq!(core.behavior_items.len(), 1);
        assert_eq!(core.behavior_items[0].family, ProbeFamily::ReturnValue);
        assert_eq!(core.related_tests[0].relation, "syntactic_call");
        assert_eq!(
            core.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn test_and_helper_files_contribute_evidence_but_never_seed_production_findings()
    -> Result<(), String> {
        let root = unique_test_root("no-seed");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def app_value():\n    return 1\n")?;
        // A production-looking def inside a test file must not become an
        // owner, and a conftest helper must stay an evidence source.
        write_file(
            &root.join("test_helper.py"),
            "def seeded():\n    return 42\n\n\ndef test_real():\n    assert True\n",
        )?;
        write_file(
            &root.join("conftest.py"),
            "def test_in_conftest():\n    assert True\n",
        )?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.status, PythonRepoRunStatus::Complete);
        // Production findings come only from production files.
        assert!(evidence.owners.iter().all(|owner| owner.file == "app.py"));
        assert_eq!(find_owner(&evidence, "app_value")?.behavior_items.len(), 1);
        assert!(evidence.owners.iter().all(|owner| owner.owner != "seeded"));
        // The evidence files were analyzed into the test pool, not as owners.
        let helper = find_file(&evidence, "conftest.py")?;
        assert_eq!(helper.role, "inline_helper");
        assert!(helper.analyzed);
        assert_eq!(helper.owner_count, 0);
        assert_eq!(helper.test_count, 1);
        let physical = find_file(&evidence, "test_helper.py")?;
        assert_eq!(physical.role, "physical_test");
        assert_eq!(physical.test_count, 1);
        assert_eq!(physical.owner_count, 0);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn generated_and_vendor_paths_are_excluded_with_counts() -> Result<(), String> {
        let root = unique_test_root("excluded");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        write_file(&root.join("gen_pb2.py"), "# generated\n")?;
        write_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.counts.discovered, 3);
        assert_eq!(evidence.counts.excluded_by_role, 2);
        assert_eq!(evidence.counts.selected, 1);
        assert_eq!(evidence.counts.analyzed, 1);
        assert_eq!(evidence.files.len(), 1);
        assert_eq!(evidence.files[0].path, "app.py");
        assert!(evidence.owners.iter().all(|owner| owner.file == "app.py"));
        assert_eq!(evidence.status, PythonRepoRunStatus::Complete);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn parse_failure_is_typed_and_adjusts_the_denominator() -> Result<(), String> {
        let root = unique_test_root("parse-failure");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("good.py"), "def good():\n    return 1\n")?;
        write_file(&root.join("broken.py"), "def broken(:\n    pass\n")?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        // Typed limitation with the file path and producer-owned reason.
        assert_eq!(evidence.limitations.len(), 1);
        let PythonRepoLimitation::ParseFailure { file, reason } = &evidence.limitations[0] else {
            return Err(format!(
                "expected a ParseFailure limitation, got {:?}",
                evidence.limitations
            ));
        };
        assert_eq!(file, "broken.py");
        assert!(reason.starts_with("source_fact_parse_error"), "{reason}");
        // The failed file leaves the analyzed denominator; the rest stays.
        let counts = &evidence.counts;
        assert_eq!(counts.selected, 2);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.analyzed, 1);
        assert_eq!(counts.analyzed + counts.failed, counts.selected);
        // The run is honestly partial, never clean-complete.
        assert_eq!(
            evidence.status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::ParseFailures { failed: 1 }
            }
        );
        assert!(!evidence.status.can_support_full_denominator());
        // The failed file produced no owners; the good one still did.
        let broken = find_file(&evidence, "broken.py")?;
        assert!(!broken.analyzed);
        assert_eq!(broken.owner_count, 0);
        assert!(evidence.owners.iter().all(|owner| owner.file == "good.py"));
        assert_eq!(find_owner(&evidence, "good")?.behavior_items.len(), 1);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn unreadable_file_is_a_typed_read_failure() -> Result<(), String> {
        let root = unique_test_root("read-failure");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        // Hand-built input naming a production file that does not exist, so
        // the read itself fails (discovery cannot select a vanished file).
        let input = PythonRepoInput {
            status: PythonRepoRunStatus::Selected,
            production_files: vec![PathBuf::from("missing.py")],
            test_files: Vec::new(),
            helper_files: Vec::new(),
            ambiguous_files: Vec::new(),
            counts: DiscoveryCounts {
                discovered: 1,
                selected: 1,
                analyzed_candidates: 1,
                skipped: 0,
                excluded_by_role: 0,
                capped: 0,
                failed: 0,
                analyzed: 0,
                unreadable_subtrees: 0,
            },
            working_set_limit: default_limit(),
            recovery_routes: CAP_RECOVERY_ROUTES,
            test_framework: None,
        };
        let evidence = build_repo_evidence(&input, &root);
        let PythonRepoLimitation::ParseFailure { file, reason } = &evidence.limitations[0] else {
            return Err(format!(
                "expected a ParseFailure limitation, got {:?}",
                evidence.limitations
            ));
        };
        assert!(file.ends_with("missing.py"), "{file}");
        assert!(reason.starts_with("read_error:"), "{reason}");
        assert_eq!(evidence.counts.failed, 1);
        assert_eq!(evidence.counts.analyzed, 0);
        assert_eq!(
            evidence.status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::ParseFailures { failed: 1 }
            }
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn no_python_source_workspace_is_an_honest_zero() -> Result<(), String> {
        let root = unique_test_root("no-python");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(
            &root.join("test_only.py"),
            "def test_only():\n    assert True\n",
        )?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::NoPythonSource);
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.status, PythonRepoRunStatus::NoPythonSource);
        assert!(!evidence.status.can_support_full_denominator());
        assert!(evidence.owners.is_empty());
        assert!(evidence.files.is_empty());
        assert!(evidence.limitations.is_empty());
        assert_eq!(evidence.counts.analyzed, 0);
        assert_eq!(evidence.counts.failed, 0);
        // The discovery identities survive untouched.
        assert_eq!(evidence.counts.discovered, 1);
        assert_eq!(evidence.counts.selected, 1);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn capped_workspace_preserves_the_capped_status_and_denominator() -> Result<(), String> {
        let root = unique_test_root("capped");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        for name in ["alpha.py", "beta.py", "gamma.py"] {
            write_file(&root.join(name), "def value():\n    return 1\n")?;
        }
        let limit = RepoWorkingSetLimit {
            limit: 2,
            source: RepoWorkingSetCapSource::EnvOverride,
        };
        let input = super::super::select_repo_input_with_limit(&root, limit);
        assert_eq!(input.status, PythonRepoRunStatus::Capped);
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.status, PythonRepoRunStatus::Capped);
        assert!(!evidence.status.can_support_full_denominator());
        assert_eq!(evidence.working_set_limit, limit);
        assert_eq!(evidence.recovery_routes.len(), 2);
        let counts = &evidence.counts;
        assert_eq!(counts.selected, 2);
        assert_eq!(counts.capped, 1);
        assert_eq!(counts.analyzed, 2);
        assert_eq!(counts.failed, 0);
        assert_eq!(counts.analyzed + counts.failed, counts.selected);
        // Only the selected files produced evidence.
        assert_eq!(evidence.files.len(), 2);
        assert!(
            evidence
                .owners
                .iter()
                .all(|owner| { owner.file == "alpha.py" || owner.file == "beta.py" })
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn ambiguous_selected_files_block_a_complete_status() -> Result<(), String> {
        let root = unique_test_root("ambiguous");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        // Hand-built input with an ambiguous-role selected file: counted,
        // never read, and it must keep the run partial (fail-closed).
        let input = PythonRepoInput {
            status: PythonRepoRunStatus::Selected,
            production_files: vec![PathBuf::from("app.py")],
            test_files: Vec::new(),
            helper_files: Vec::new(),
            ambiguous_files: vec![PathBuf::from("mystery.py")],
            counts: DiscoveryCounts {
                discovered: 2,
                selected: 2,
                analyzed_candidates: 1,
                skipped: 0,
                excluded_by_role: 0,
                capped: 0,
                failed: 0,
                analyzed: 0,
                unreadable_subtrees: 0,
            },
            working_set_limit: default_limit(),
            recovery_routes: CAP_RECOVERY_ROUTES,
            test_framework: None,
        };
        let evidence = build_repo_evidence(&input, &root);
        assert_eq!(
            evidence.status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::UnanalyzedAmbiguous { count: 1 }
            }
        );
        assert!(!evidence.status.can_support_full_denominator());
        let mystery = find_file(&evidence, "mystery.py")?;
        assert_eq!(mystery.role, "unknown");
        assert!(!mystery.analyzed);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn weak_oracle_relation_is_distinguishable_from_no_relation() -> Result<(), String> {
        let root = unique_test_root("weak-vs-none");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        // `observed` reaches a smoke oracle; `lonely` has no related test.
        write_file(
            &root.join("app.py"),
            "def observed():\n    return 1\n\n\ndef lonely():\n    return 2\n",
        )?;
        write_file(
            &root.join("test_app.py"),
            "from app import observed\n\n\ndef test_observed():\n    assert observed()\n",
        )?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        let observed = find_owner(&evidence, "observed")?;
        assert_eq!(observed.related_tests.len(), 1);
        assert!(observed.related_tests[0].relation_uses_oracle);
        assert_eq!(
            observed.related_tests[0].oracle_strength,
            OracleStrength::Smoke
        );
        let item = &observed.behavior_items[0];
        assert!(item.has_oracle_eligible_relation);
        assert_eq!(item.strongest_oracle_strength, OracleStrength::Smoke);

        // No oracle-eligible relation at all — a different fact from a weak
        // oracle. The shared matcher still reports the same-stem link
        // (`test_app` stem == owner file stem), preserved as UNCERTAIN so a
        // similarly named owner can never be read as a discriminating reach
        // (the wrong-owner guard surface).
        let lonely = find_owner(&evidence, "lonely")?;
        assert_eq!(lonely.related_tests.len(), 1);
        assert_eq!(lonely.related_tests[0].relation, "same_stem");
        assert!(!lonely.related_tests[0].relation_uses_oracle);
        assert!(lonely.related_tests[0].relation_uncertain);
        assert!(!lonely.behavior_items[0].has_oracle_eligible_relation);
        assert_eq!(
            lonely.behavior_items[0].strongest_oracle_strength,
            OracleStrength::Unknown
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn dynamic_import_site_yields_a_typed_static_limit_row() -> Result<(), String> {
        let root = unique_test_root("static-limit");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(
            &root.join("loader.py"),
            "import importlib\n\n\ndef load(name):\n    return importlib.import_module(name)\n",
        )?;
        let input = super::super::select_repo_input_with_limit(&root, default_limit());
        let evidence = build_repo_evidence(&input, &root);

        assert_eq!(evidence.status, PythonRepoRunStatus::Complete);
        let load = find_owner(&evidence, "load")?;
        let item = load
            .behavior_items
            .iter()
            .find(|item| item.static_limit.is_some())
            .ok_or("expected a statically limited behavior item")?;
        assert_eq!(item.static_limit, Some(StaticLimitKind::MissingImportGraph));
        // A limited item carries no canonical gap, mirroring diff mode.
        assert!(item.canonical_gap.is_none());
        let [
            PythonRepoLimitation::StaticLimit {
                file,
                owner,
                line,
                kind,
            },
        ] = &evidence.limitations[..]
        else {
            return Err(format!(
                "expected one StaticLimit limitation, got {:?}",
                evidence.limitations
            ));
        };
        assert_eq!(file, "loader.py");
        assert_eq!(owner, "load");
        assert_eq!(*line, item.line);
        assert_eq!(*kind, StaticLimitKind::MissingImportGraph);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn repeated_runs_are_byte_and_digest_deterministic() -> Result<(), String> {
        let root = unique_test_root("determinism");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(
            &root.join("app.py"),
            "def run(flag):\n    if flag:\n        return 1\n    return 2\n",
        )?;
        write_file(
            &root.join("test_app.py"),
            "from app import run\n\n\ndef test_run():\n    assert run(True) == 1\n",
        )?;
        write_file(&root.join("gen_pb2.py"), "# generated\n")?;
        write_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;
        let first = build_repo_evidence(
            &super::super::select_repo_input_with_limit(&root, default_limit()),
            &root,
        );
        let second = build_repo_evidence(
            &super::super::select_repo_input_with_limit(&root, default_limit()),
            &root,
        );

        // Identical evidence, identical serialization, identical digest.
        assert_eq!(first, second);
        let first_bytes = format!("{first:?}");
        let second_bytes = format!("{second:?}");
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            fnv1a64(first_bytes.as_bytes()),
            fnv1a64(second_bytes.as_bytes())
        );
        // Non-vacuous: the run carries owners, items, and excluded counts.
        assert!(!first.owners.is_empty());
        assert!(
            first
                .owners
                .iter()
                .any(|owner| !owner.behavior_items.is_empty())
        );
        assert_eq!(first.counts.excluded_by_role, 2);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn repeated_identical_statements_in_owner_get_sequential_finding_and_probe_ids()
    -> Result<(), String> {
        let root = unique_test_root("repeated_stmt_ids");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        // Three identical statements under the same function
        write_file(
            &root.join("app.py"),
            "def run():\n    x = 1\n    x = 1\n    x = 1\n",
        )?;
        let evidence = build_repo_evidence(
            &super::super::select_repo_input_with_limit(&root, default_limit()),
            &root,
        );
        assert_eq!(evidence.findings.len(), 3);
        let id0 = &evidence.findings[0].id;
        let id1 = &evidence.findings[1].id;
        let id2 = &evidence.findings[2].id;
        assert_ne!(id0, id1);
        assert_ne!(id1, id2);
        assert_ne!(id0, id2);
        assert_eq!(id1, &format!("{id0}.2"));
        assert_eq!(id2, &format!("{id0}.3"));
        assert_eq!(&evidence.findings[0].probe.id.0, id0);
        assert_eq!(&evidence.findings[1].probe.id.0, id1);
        assert_eq!(&evidence.findings[2].probe.id.0, id2);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
