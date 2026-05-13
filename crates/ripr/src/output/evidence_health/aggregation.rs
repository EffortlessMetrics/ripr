use std::collections::BTreeMap;

use crate::analysis::ClassifiedSeam;
use crate::analysis::canonical_gap::{CanonicalGapIdentity, canonical_gap_identities};
use crate::domain::{OracleKind, OracleStrength};
use crate::output::evidence_record::evidence_record_for;

use super::{
    EvidenceHealthCalibration, EvidenceHealthMetrics, EvidenceHealthQualityCounters,
    EvidenceHealthReport, ORACLE_KIND_LABELS, ORACLE_STRENGTH_LABELS, RELATION_CONFIDENCE_LABELS,
    StaticLimitationCounter, VALUE_CONTEXT_LABELS, count_evidence_record_quality, count_for,
    count_stage, evidence_quality_from_counts, grip_class_counts, increment, increment_limitation,
    increment_unknown_stop_reason, labeled_counts, stage_counts, stage_state_counts,
    top_limitations, unknown_stop_reason_counts,
};

pub(super) fn build_evidence_health_report(
    classified: &[ClassifiedSeam],
    root: String,
    calibration: EvidenceHealthCalibration,
) -> EvidenceHealthReport {
    let canonical_gaps = canonical_gap_identities(classified);
    let mut accumulator = EvidenceHealthAccumulator::new();

    for entry in classified {
        let canonical_gap = canonical_gaps.get(entry.seam.id());
        accumulator.record_entry(entry, canonical_gap);
    }

    accumulator.finish(classified.len(), root, calibration)
}

struct EvidenceHealthAccumulator {
    grip_class_counts: BTreeMap<String, usize>,
    stage_state_counts: BTreeMap<String, BTreeMap<String, usize>>,
    unknown_stage_counts: BTreeMap<String, usize>,
    unknown_stop_reason_counts: BTreeMap<String, usize>,
    missing_discriminator_counts: BTreeMap<String, usize>,
    observed_value_context_counts: BTreeMap<String, usize>,
    related_test_confidence_counts: BTreeMap<String, usize>,
    oracle_strength_counts: BTreeMap<String, usize>,
    oracle_kind_counts: BTreeMap<String, usize>,
    limitations: BTreeMap<String, StaticLimitationCounter>,
    quality_counters: EvidenceHealthQualityCounters,
    headline_eligible_total: usize,
    missing_discriminators_total: usize,
    seams_with_missing_discriminators: usize,
    observed_values_total: usize,
    seams_with_observed_values: usize,
    related_tests_total: usize,
    seams_with_related_tests: usize,
    opaque_oracle_count: usize,
}

impl EvidenceHealthAccumulator {
    fn new() -> Self {
        Self {
            grip_class_counts: grip_class_counts(),
            stage_state_counts: stage_state_counts(),
            unknown_stage_counts: stage_counts(),
            unknown_stop_reason_counts: unknown_stop_reason_counts(),
            missing_discriminator_counts: BTreeMap::new(),
            observed_value_context_counts: labeled_counts(VALUE_CONTEXT_LABELS),
            related_test_confidence_counts: labeled_counts(RELATION_CONFIDENCE_LABELS),
            oracle_strength_counts: labeled_counts(ORACLE_STRENGTH_LABELS),
            oracle_kind_counts: labeled_counts(ORACLE_KIND_LABELS),
            limitations: BTreeMap::new(),
            quality_counters: EvidenceHealthQualityCounters::default(),
            headline_eligible_total: 0,
            missing_discriminators_total: 0,
            seams_with_missing_discriminators: 0,
            observed_values_total: 0,
            seams_with_observed_values: 0,
            related_tests_total: 0,
            seams_with_related_tests: 0,
            opaque_oracle_count: 0,
        }
    }

    fn record_entry(
        &mut self,
        entry: &ClassifiedSeam,
        canonical_gap: Option<&CanonicalGapIdentity>,
    ) {
        let record = evidence_record_for(entry, canonical_gap);
        count_evidence_record_quality(&record, canonical_gap, &mut self.quality_counters);
        self.record_classification(entry);
        self.record_stages(entry);
        self.record_related_tests(entry);
        self.record_observed_values(entry);
        self.record_missing_discriminators(entry);
        self.record_oracles(entry);
    }

    fn record_classification(&mut self, entry: &ClassifiedSeam) {
        increment(&mut self.grip_class_counts, entry.class.as_str());
        if entry.class.is_headline_eligible() {
            self.headline_eligible_total += 1;
        }
        increment_unknown_stop_reason(&mut self.unknown_stop_reason_counts, entry.class);
    }

    fn record_stages(&mut self, entry: &ClassifiedSeam) {
        for (stage, state) in [
            ("reach", &entry.evidence.reach.state),
            ("activate", &entry.evidence.activate.state),
            ("propagate", &entry.evidence.propagate.state),
            ("observe", &entry.evidence.observe.state),
            ("discriminate", &entry.evidence.discriminate.state),
        ] {
            count_stage(
                &mut self.stage_state_counts,
                &mut self.unknown_stage_counts,
                &mut self.limitations,
                stage,
                state,
                entry.seam.id().as_str(),
            );
        }
    }

    fn record_related_tests(&mut self, entry: &ClassifiedSeam) {
        if entry.evidence.related_tests.is_empty() {
            increment_limitation(
                &mut self.limitations,
                "no_related_tests",
                "No related test was associated with the seam.",
                entry.seam.id().as_str(),
            );
        } else {
            self.seams_with_related_tests += 1;
        }
        self.related_tests_total += entry.evidence.related_tests.len();
    }

    fn record_observed_values(&mut self, entry: &ClassifiedSeam) {
        if !entry.evidence.observed_values.is_empty() {
            self.seams_with_observed_values += 1;
        }
        self.observed_values_total += entry.evidence.observed_values.len();
        for value in &entry.evidence.observed_values {
            increment(
                &mut self.observed_value_context_counts,
                value.context.as_str(),
            );
        }
    }

    fn record_missing_discriminators(&mut self, entry: &ClassifiedSeam) {
        if !entry.evidence.missing_discriminators.is_empty() {
            self.seams_with_missing_discriminators += 1;
            increment_limitation(
                &mut self.limitations,
                "missing_discriminator",
                "At least one discriminator remains missing for the seam.",
                entry.seam.id().as_str(),
            );
        }
        self.missing_discriminators_total += entry.evidence.missing_discriminators.len();
        for missing in &entry.evidence.missing_discriminators {
            increment(
                &mut self.missing_discriminator_counts,
                missing.value.as_str(),
            );
        }
    }

    fn record_oracles(&mut self, entry: &ClassifiedSeam) {
        for related in &entry.evidence.related_tests {
            increment(
                &mut self.related_test_confidence_counts,
                related.relation_confidence.as_str(),
            );
            increment(
                &mut self.oracle_strength_counts,
                related.oracle_strength.as_str(),
            );
            increment(&mut self.oracle_kind_counts, related.oracle_kind.as_str());
            if related.oracle_kind == OracleKind::Unknown
                || related.oracle_strength == OracleStrength::Unknown
            {
                self.opaque_oracle_count += 1;
                increment_limitation(
                    &mut self.limitations,
                    "opaque_oracle",
                    "A related test contains an assertion shape ripr cannot classify.",
                    entry.seam.id().as_str(),
                );
            }
        }
    }

    fn finish(
        self,
        seams_total: usize,
        root: String,
        calibration: EvidenceHealthCalibration,
    ) -> EvidenceHealthReport {
        let metrics = EvidenceHealthMetrics {
            seams_total,
            headline_eligible_total: self.headline_eligible_total,
            weakly_gripped_total: count_for(&self.grip_class_counts, "weakly_gripped"),
            ungripped_total: count_for(&self.grip_class_counts, "ungripped"),
            grip_class_counts: self.grip_class_counts,
            stage_state_counts: self.stage_state_counts,
            unknown_stage_counts: self.unknown_stage_counts,
            unknown_stop_reason_counts: self.unknown_stop_reason_counts,
            missing_discriminators_total: self.missing_discriminators_total,
            seams_with_missing_discriminators: self.seams_with_missing_discriminators,
            missing_discriminator_counts: self.missing_discriminator_counts,
            observed_values_total: self.observed_values_total,
            seams_with_observed_values: self.seams_with_observed_values,
            observed_value_context_counts: self.observed_value_context_counts,
            related_tests_total: self.related_tests_total,
            seams_with_related_tests: self.seams_with_related_tests,
            related_test_confidence_counts: self.related_test_confidence_counts,
            oracle_strength_counts: self.oracle_strength_counts,
            oracle_kind_counts: self.oracle_kind_counts,
            opaque_oracle_count: self.opaque_oracle_count,
        };
        let evidence_quality = evidence_quality_from_counts(self.quality_counters, &metrics);

        EvidenceHealthReport {
            root,
            metrics,
            evidence_quality,
            calibration,
            top_static_limitations: top_limitations(self.limitations),
        }
    }
}
