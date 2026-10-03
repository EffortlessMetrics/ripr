//! Canonical gap-item projection for the MCP adapter.
//!
//! A [`GapItem`] is the MCP-facing view of one producer-owned
//! [`crate::domain::Finding`]. The projection is built once when a completed
//! snapshot is committed; MCP never re-runs classification, ranking, or
//! evidence production. List responses carry only the small summary document;
//! the complete bounded evidence document is served lazily by
//! `ripr_get_gap` / `ripr://gap/{canonical_item_id}`.

use crate::domain::Finding;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const GAP_LIST_SCHEMA_VERSION: &str = "ripr-mcp-gap-list-v1";
pub(crate) const GAP_SCHEMA_VERSION: &str = "ripr-mcp-gap-v1";

/// `ripr_get_gap` / `ripr://gap/{canonical_item_id}` never authorizes an
/// edit by itself: the readiness block reports the producer repair-readiness
/// facts projected at snapshot commit time, and `ripr_prepare_repair` (#3090)
/// is the only route that binds a repair transaction — and only when every
/// readiness gate is established.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepairFixSite {
    pub(crate) test_name: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) oracle: Option<String>,
    pub(crate) oracle_kind: &'static str,
}

/// Producer-fact repair readiness for one canonical item, computed once when
/// the snapshot is committed. The evaluation is a fail-closed conjunction of
/// producer facts already on the finding; MCP never upgrades a missing fact
/// and never runs its own classification:
///
/// 1. candidate actionability — the shared `#3281` predicate
///    [`Finding::is_candidate_actionable`];
/// 2. an established discriminator — the canonical gap names a non-empty
///    normalized discriminator, activation names no missing discriminator,
///    and `missing` names no `Missing discriminator value:` entry;
/// 3. an established fix site — a strong, high-confidence directly-related
///    test, on a path the shared edit-cage test-surface predicate accepts.
///
/// The first failing gate in this fixed order is the typed reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepairReadiness {
    pub(crate) ready: bool,
    pub(crate) fix_site: Option<RepairFixSite>,
    /// First failing gate (`not_candidate_actionable`, `missing_discriminator`,
    /// `fix_site_not_established`, `fix_site_not_test_surface`); `None` only
    /// when every gate is established.
    pub(crate) ineligibility: Option<&'static str>,
}

impl RepairReadiness {
    pub(crate) fn from_finding(finding: &Finding) -> Self {
        if !finding.is_candidate_actionable() {
            return Self {
                ready: false,
                fix_site: None,
                ineligibility: Some("not_candidate_actionable"),
            };
        }
        let discriminator_missing = finding
            .canonical_gap
            .as_ref()
            .is_none_or(|gap| gap.normalized_discriminator.trim().is_empty())
            || !finding.activation.missing_discriminators.is_empty()
            || finding
                .missing
                .iter()
                .any(|entry| entry.starts_with(crate::domain::MISSING_DISCRIMINATOR_VALUE_PREFIX));
        if discriminator_missing {
            return Self {
                ready: false,
                fix_site: None,
                ineligibility: Some("missing_discriminator"),
            };
        }
        // The gate contract is "a strong, high-confidence directly-related
        // test on a shared edit-cage test-surface path", so the path
        // predicate joins the search: a qualifying test-surface candidate
        // later in the list wins over a non-surface candidate earlier.
        let candidates = finding
            .related_tests
            .iter()
            .filter(|test| {
                test.oracle_strength == crate::domain::OracleStrength::Strong
                    && test.relation_confidence == Some(crate::domain::RelationConfidence::High)
            })
            .map(|test| RepairFixSite {
                test_name: test.name.clone(),
                file: crate::analysis::stable_path_text(&test.file),
                line: test.line,
                oracle: test.oracle.clone(),
                oracle_kind: test.oracle_kind.as_str(),
            })
            .collect::<Vec<_>>();
        let Some(first) = candidates.first().cloned() else {
            return Self {
                ready: false,
                fix_site: None,
                ineligibility: Some("fix_site_not_established"),
            };
        };
        let Some(fix_site) = candidates
            .into_iter()
            .find(|site| crate::analysis::is_test_surface_path(&site.file))
        else {
            return Self {
                ready: false,
                fix_site: Some(first),
                ineligibility: Some("fix_site_not_test_surface"),
            };
        };
        Self {
            ready: true,
            fix_site: Some(fix_site),
            ineligibility: None,
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        match self.ineligibility {
            Some("not_candidate_actionable") => {
                "the producer did not establish the changed source as candidate-current, so no repair target is actionable (#3281)"
            }
            Some("missing_discriminator") => {
                "the producer did not establish a discriminator for the changed behavior (no normalized discriminator, or a producer-named missing discriminator)"
            }
            Some("fix_site_not_established") => {
                "no strong, high-confidence directly-related test establishes an exact fix site"
            }
            Some("fix_site_not_test_surface") => {
                "the strongest established fix site is not a test surface; a production file is never the authored edit target"
            }
            Some(_other) => "the producer did not establish every repair-readiness fact",
            None => {
                "every evaluated producer fact is established: candidate-current source, an established discriminator, and a strong directly-related test fix site on a test surface"
            }
        }
    }
}

pub(crate) struct GapItem {
    pub(crate) canonical_id: String,
    pub(crate) finding_id: String,
    pub(crate) file: String,
    /// Small deterministic document used by `ripr_list_gaps`; its serialized
    /// byte length is the item's budget payload bytes. Class, language, and
    /// line live here (and in the evidence core), not as duplicated fields.
    pub(crate) list_summary: Value,
    list_summary_bytes: usize,
    /// Complete bounded evidence document (without the per-request snapshot
    /// binding and links, which are injected at serve time). The repair
    /// projection reads `changed_behavior` and the discriminator from it.
    pub(crate) evidence_core: Value,
    pub(crate) evidence_bytes: usize,
    /// Producer candidate-actionability captured at projection time; the
    /// shared budget maps it to eligibility so MCP never invents its own.
    candidate_actionable: bool,
    /// Deterministic digest of `evidence_core`, bound into the snapshot
    /// identity so an evidence change yields a new snapshot id instead of
    /// serving altered bytes under the old identity.
    pub(crate) evidence_sha256: String,
    /// Producer-fact repair readiness projected at commit time; the evidence
    /// core embeds it so the snapshot identity binds readiness changes too.
    pub(crate) repair_readiness: RepairReadiness,
}

impl GapItem {
    pub(crate) fn from_finding(finding: &Finding) -> Result<Self, String> {
        let canonical_id = finding
            .canonical_gap
            .as_ref()
            .map(|gap| gap.id.clone())
            .unwrap_or_else(|| finding.id.clone());
        let file = crate::analysis::stable_path_text(&finding.probe.location.file);
        let class = finding.class.as_str().to_string();
        let language = finding
            .language
            .as_ref()
            .map(|language| language.as_str().to_string());
        let line = finding.probe.location.line;

        let list_summary = json!({
            "canonical_id": canonical_id,
            "class": class,
            "language": language,
            "file": file,
            "line": line,
        });
        let list_summary_bytes = serialized_bytes(&list_summary)?;

        let evidence_core = gap_evidence_core(finding, &canonical_id)?;
        let evidence_bytes = serialized_bytes(&evidence_core)?;
        let evidence_sha256 = evidence_sha256(&evidence_core)?;
        let repair_readiness = RepairReadiness::from_finding(finding);
        Ok(Self {
            canonical_id,
            finding_id: finding.id.clone(),
            file,
            list_summary,
            list_summary_bytes,
            evidence_core,
            evidence_bytes,
            candidate_actionable: finding.is_candidate_actionable(),
            evidence_sha256,
            repair_readiness,
        })
    }

    pub(crate) fn list_summary_bytes(&self) -> usize {
        self.list_summary_bytes
    }

    /// The complete item document for one committed snapshot. The caller
    /// binds the snapshot identity and the resource links; a missing repair
    /// link stays an explicit `null`, never prose invention.
    pub(crate) fn document(&self, snapshot_id: &str) -> Value {
        json!({
            "schema_version": GAP_SCHEMA_VERSION,
            "snapshot_id": snapshot_id,
            "item": serde_json::Value::Object({
                let mut object = self
                    .evidence_core
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                object.insert(
                    "links".to_string(),
                    json!({
                        "snapshot": format!("ripr://snapshot/{snapshot_id}"),
                        "gap": format!("ripr://gap/{}", self.canonical_id),
                        "repair_attempt": Value::Null,
                        "repair_attempt_note": "ripr://repair-attempt/{attempt_id} binds once ripr_prepare_repair creates the session transaction for this item (#3090)",
                    }),
                );
                object
            }),
            "claim_boundary": "Static exposure evidence for one canonical item. This document does not edit source, execute tests or mutation, or authorize a repair transaction.",
            "limitations": [
                "evidence is static analysis only; no runtime, mutation, or test-execution result is claimed",
                "missing or limited fields remain typed limitations and never become repair-ready",
            ],
        })
    }
}

/// Producer-owned evidence fields, projected once per finding. Fields the
/// producer does not populate stay typed `null`/`absent` states in the
/// serialized finding values rather than being back-filled here.
fn gap_evidence_core(finding: &Finding, canonical_id: &str) -> Result<Value, String> {
    let readiness = RepairReadiness::from_finding(finding);
    let activation = serde_json::to_value(&finding.activation)
        .map_err(|error| format!("serialize activation evidence: {error}"))?;
    let related_tests = finding
        .related_tests
        .iter()
        .map(|test| {
            json!({
                "name": test.name,
                "file": crate::analysis::stable_path_text(&test.file),
                "line": test.line,
                "oracle": test.oracle,
                "oracle_kind": test.oracle_kind.as_str(),
                "oracle_strength": test.oracle_strength.as_str(),
                "relation_reason": test.relation_reason.map(|reason| reason.as_str()),
                "relation_confidence": test.relation_confidence.map(|confidence| confidence.as_str()),
            })
        })
        .collect::<Vec<_>>();
    let source_currentness = serde_json::to_value(finding.source_currentness)
        .map_err(|error| format!("serialize source currentness: {error}"))?;
    let static_limitation = finding
        .static_limit_kind
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| format!("serialize static limitation: {error}"))?;
    let canonical_gap = finding
        .canonical_gap
        .as_ref()
        .map(|gap| {
            json!({
                "id": gap.id,
                "language": gap.language,
                "owner": gap.owner,
                "behavior_kind": gap.behavior_kind,
                "probe_kind": gap.probe_kind,
                "normalized_discriminator": gap.normalized_discriminator,
            })
        })
        .unwrap_or(Value::Null);

    Ok(json!({
        "canonical_id": canonical_id,
        "finding_id": finding.id,
        "class": finding.class.as_str(),
        "language": finding.language.as_ref().map(|language| language.as_str()),
        "location": {
            "file": crate::analysis::stable_path_text(&finding.probe.location.file),
            "line": finding.probe.location.line,
            "column": finding.probe.location.column,
        },
        "changed_behavior": {
            "expression": finding.probe.expression,
            "before": finding.probe.before,
            "after": finding.probe.after,
            "delta_kind": finding.probe.delta.as_str(),
            "probe_family": finding.probe.family.as_str(),
            "expected_sinks": finding.probe.expected_sinks,
        },
        "causal_attribution": canonical_gap,
        "discriminator_availability": activation,
        "related_tests": related_tests,
        "evidence": finding.evidence,
        "missing": finding.missing,
        "static_limitation": static_limitation,
        "source_currentness": source_currentness,
        "recommended_next_step": finding.recommended_next_step,
        "readiness": {
            "repair_packet_ready": readiness.ready,
            "reason": readiness.reason(),
            "prepared_by": "#3090",
            "evaluation_basis": "producer facts on the committed finding: candidate actionability (#3281), the canonical gap's normalized discriminator with no producer-named missing discriminator, and the strongest directly-related related-test grip",
        },
        "repair_boundary": {
            "allowed_edit_surface": "none_declared",
            "must_not_change": "none_declared",
            "prepared_by": "#3090",
        },
    }))
}

/// Budget items for the shared [`crate::lsp::diagnostic_budget`] authority.
/// Eligibility is the producer's candidate-actionability predicate captured
/// at projection time; MCP does not invent its own filter or ranking. The
/// selection key uses the same unset (128) ranks the LSP delivery bridge
/// uses, so the budget's evidence-owned order (selection key, then canonical
/// id, then document) is the only ordering applied.
pub(crate) fn budget_items(
    items: &[GapItem],
) -> Vec<crate::lsp::diagnostic_budget::DiagnosticBudgetItem> {
    use crate::lsp::diagnostic_budget::{
        DiagnosticBudgetEligibility, DiagnosticBudgetItem, DiagnosticSelectionKey,
    };
    items
        .iter()
        .map(|item| DiagnosticBudgetItem {
            canonical_id: item.canonical_id.clone(),
            document: item.file.clone(),
            payload_bytes: item.list_summary_bytes(),
            inline_detail_bytes: item
                .evidence_bytes
                .saturating_sub(item.list_summary_bytes()),
            eligibility: if item.candidate_actionable {
                DiagnosticBudgetEligibility::Actionable
            } else {
                DiagnosticBudgetEligibility::ProfileFiltered
            },
            selection_key: DiagnosticSelectionKey {
                repair_route_rank: 128,
                causal_rank: 128,
                evidence_rank: 128,
            },
        })
        .collect()
}

/// Extract the canonical item id from a `ripr://gap/{id}` resource URI.
/// Returns `None` for any other URI, including the exact status resource.
pub(crate) fn gap_resource_id(uri: &str) -> Option<&str> {
    uri.strip_prefix("ripr://gap/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

/// Extract the snapshot id from a `ripr://snapshot/{id}` resource URI.
pub(crate) fn snapshot_resource_id(uri: &str) -> Option<&str> {
    uri.strip_prefix("ripr://snapshot/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

fn serialized_bytes(value: &Value) -> Result<usize, String> {
    struct LengthWriter(usize);

    impl std::io::Write for LengthWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0 += buffer.len();
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = LengthWriter(0);
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| format!("serialize gap item: {error}"))?;
    Ok(writer.0)
}

/// Deterministic digest of the complete evidence document. Snapshot identity
/// binds this so a same-outcome refresh that changes evidence text produces a
/// new snapshot id instead of serving altered bytes under the old identity.
fn evidence_sha256(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("serialize evidence digest input: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// One fully-established candidate finding shared by the `gaps` and `repair`
/// test suites: candidate-current source, a normalized discriminator, and one
/// strong, high-confidence directly-related test on a test surface.
#[cfg(test)]
pub(crate) fn test_finding() -> Result<Finding, String> {
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, Probe, ProbeFamily, ProbeId, RelatedTest,
        RiprEvidence, SourceLocation, StageEvidence, StageState,
    };
    let stage = || StageEvidence::new(StageState::Unknown, Confidence::Unknown, "test");
    Ok(Finding {
        id: "finding:test:1".to_string(),
        canonical_gap: Some(crate::domain::FindingCanonicalGap {
            id: "gap:test:1".to_string(),
            language: "rust".to_string(),
            file: "src/lib.rs".to_string(),
            owner: "checkout".to_string(),
            behavior_kind: "predicate".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "total == 10000".to_string(),
        }),
        probe: Probe {
            id: ProbeId("probe:test:1".to_string()),
            location: SourceLocation::new("src/lib.rs", 12, 5),
            owner: Some(crate::domain::SymbolId("symbol:checkout".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Value,
            before: Some("total > 10000".to_string()),
            after: Some("total >= 10000".to_string()),
            expression: "total >= 10000".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        },
        class: crate::domain::ExposureClass::WeaklyExposed,
        ripr: RiprEvidence {
            reach: stage(),
            infect: stage(),
            propagate: stage(),
            reveal: crate::domain::RevealEvidence {
                observe: stage(),
                discriminate: stage(),
            },
        },
        confidence: 0.5,
        evidence: vec!["the changed comparison reaches the checkout return".to_string()],
        missing: vec!["No strong discriminator was detected".to_string()],
        flow_sinks: Vec::new(),
        activation: ActivationEvidence::default(),
        stop_reasons: Vec::new(),
        related_tests_matched_total: Some(1),
        related_tests: vec![RelatedTest {
            name: "checkout_totals".to_string(),
            file: std::path::PathBuf::from("tests/checkout.rs"),
            line: 40,
            oracle: Some("assert_eq!(total, 10001)".to_string()),
            oracle_kind: crate::domain::OracleKind::ExactValue,
            oracle_strength: crate::domain::OracleStrength::Strong,
            relation_reason: Some(crate::domain::RelationReason::DirectOwnerCall),
            relation_confidence: Some(crate::domain::RelationConfidence::High),
        }],
        recommended_next_step: Some("add a boundary assertion for 10000".to_string()),
        language: Some(crate::domain::LanguageId::Rust),
        language_status: None,
        owner_kind: None,
        static_limit_kind: None,
        changed_sink: Some("total".to_string()),
        observed_sink: None,
        oracle_alignment: None,
        alignment_reason: None,
        source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding() -> Result<Finding, String> {
        test_finding()
    }

    #[test]
    fn canonical_id_prefers_the_producer_gap_identity() -> Result<(), String> {
        let item = GapItem::from_finding(&finding()?)?;
        if item.canonical_id != "gap:test:1" || item.finding_id != "finding:test:1" {
            return Err(format!(
                "canonical identity must prefer the producer gap id: {item:?}",
                item = (item.canonical_id.clone(), item.finding_id.clone())
            ));
        }
        if item.list_summary.pointer("/class").and_then(Value::as_str) != Some("weakly_exposed") {
            return Err(format!(
                "unexpected class wire token: {}",
                item.list_summary
            ));
        }
        if item.file != "src/lib.rs"
            || item.list_summary.pointer("/line").and_then(Value::as_u64) != Some(12)
        {
            return Err(format!(
                "location projection drifted: {}:{}",
                item.file, item.list_summary
            ));
        }
        Ok(())
    }

    #[test]
    fn fallback_identity_uses_the_finding_id() -> Result<(), String> {
        let mut finding = finding()?;
        finding.canonical_gap = None;
        let item = GapItem::from_finding(&finding)?;
        if item.canonical_id != "finding:test:1" {
            return Err(format!(
                "without a canonical gap the finding id must be the item identity: {}",
                item.canonical_id
            ));
        }
        Ok(())
    }

    #[test]
    fn eligibility_follows_the_producer_actionability_predicate() -> Result<(), String> {
        use crate::lsp::diagnostic_budget::DiagnosticBudgetEligibility;
        let actionable = budget_items(&[GapItem::from_finding(&finding()?)?]);
        let Some(actionable_item) = actionable.first() else {
            return Err("budget items must not be empty".to_string());
        };
        if actionable_item.eligibility != DiagnosticBudgetEligibility::Actionable {
            return Err("candidate-current findings must stay actionable".to_string());
        }
        let mut base_deleted = finding()?;
        base_deleted.source_currentness = crate::domain::SourceCurrentness::BaseDeleted;
        let filtered = budget_items(&[GapItem::from_finding(&base_deleted)?]);
        let Some(filtered_item) = filtered.first() else {
            return Err("budget items must not be empty".to_string());
        };
        if filtered_item.eligibility != DiagnosticBudgetEligibility::ProfileFiltered {
            return Err(
                "non-candidate-current findings must map to the producer's filtered state"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn evidence_document_reports_producer_repair_readiness() -> Result<(), String> {
        let item = GapItem::from_finding(&finding()?)?;
        let document = item.document("snapshot:sha256:abc");
        let readiness = document
            .pointer("/item/readiness/repair_packet_ready")
            .ok_or_else(|| "gap document lost its readiness block".to_string())?;
        if readiness != &serde_json::json!(true) {
            return Err(format!(
                "the fully established fixture finding must project repair readiness: {document}"
            ));
        }
        let reason = document
            .pointer("/item/readiness/reason")
            .and_then(Value::as_str)
            .ok_or_else(|| "readiness needs its evaluation reason".to_string())?;
        if reason.is_empty() {
            return Err("readiness reason must not be empty".to_string());
        }
        if document
            .pointer("/item/readiness/prepared_by")
            .and_then(Value::as_str)
            != Some("#3090")
        {
            return Err("readiness lost the repair-slice owner".to_string());
        }
        if document.pointer("/item/repair_boundary/allowed_edit_surface")
            != Some(&serde_json::json!("none_declared"))
        {
            return Err("gap evidence must not invent an allowed edit surface".to_string());
        }
        if document.pointer("/item/links/repair_attempt") != Some(&Value::Null) {
            return Err("the repair-attempt link must stay an explicit null".to_string());
        }
        let snapshot_link = document
            .pointer("/item/links/snapshot")
            .and_then(Value::as_str)
            .ok_or_else(|| "gap document lost its snapshot link".to_string())?;
        if snapshot_link != "ripr://snapshot/snapshot:sha256:abc" {
            return Err(format!("snapshot link drifted: {snapshot_link}"));
        }
        Ok(())
    }

    #[test]
    fn readiness_stays_fail_closed_on_missing_producer_facts() -> Result<(), String> {
        let mut no_discriminator = finding()?;
        no_discriminator.canonical_gap = None;
        let item = GapItem::from_finding(&no_discriminator)?;
        if item.repair_readiness.ready
            || item.repair_readiness.ineligibility != Some("missing_discriminator")
        {
            return Err(format!(
                "a finding without a canonical gap must not be repair-ready: {:?}",
                item.repair_readiness.ineligibility
            ));
        }

        let mut base_deleted = finding()?;
        base_deleted.source_currentness = crate::domain::SourceCurrentness::BaseDeleted;
        let item = GapItem::from_finding(&base_deleted)?;
        if item.repair_readiness.ready
            || item.repair_readiness.ineligibility != Some("not_candidate_actionable")
        {
            return Err(format!(
                "base-side evidence must not be repair-ready: {:?}",
                item.repair_readiness.ineligibility
            ));
        }

        let mut weak_grip = finding()?;
        for test in &mut weak_grip.related_tests {
            test.oracle_strength = crate::domain::OracleStrength::Weak;
        }
        let item = GapItem::from_finding(&weak_grip)?;
        if item.repair_readiness.ready
            || item.repair_readiness.ineligibility != Some("fix_site_not_established")
        {
            return Err(format!(
                "a finding without a strong directly-related test must not be repair-ready: {:?}",
                item.repair_readiness.ineligibility
            ));
        }
        Ok(())
    }

    #[test]
    fn complete_and_incomplete_evidence_stay_distinct() -> Result<(), String> {
        let mut with_gap = finding()?;
        with_gap.missing = Vec::new();
        let complete = GapItem::from_finding(&with_gap)?;
        let incomplete = GapItem::from_finding(&finding()?)?;
        if complete.evidence_bytes == incomplete.evidence_bytes {
            return Err(
                "missing-evidence and complete-evidence items must not project identically"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn resource_uri_parsing_is_strict() {
        assert_eq!(gap_resource_id("ripr://gap/gap:test:1"), Some("gap:test:1"));
        assert_eq!(
            snapshot_resource_id("ripr://snapshot/snapshot:sha256:x"),
            Some("snapshot:sha256:x")
        );
        for other in [
            "ripr://workspace/status",
            "ripr://gap/",
            "ripr://gap/a/b",
            "ripr://snapshot/",
            "https://example.com/gap/x",
        ] {
            assert_eq!(gap_resource_id(other), None, "{other}");
        }
        assert_eq!(snapshot_resource_id("ripr://gap/x"), None);
    }
}
