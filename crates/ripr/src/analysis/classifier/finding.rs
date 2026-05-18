use super::evidence::ClassifiedProbeEvidence;
use crate::analysis::classify::{
    ProbeContext, ensure_unknown_stop_reason, missing_evidence, recommended_next_step, stop_reasons,
};
use crate::domain::*;

pub(in crate::analysis) fn build_finding(
    context: &ProbeContext<'_>,
    class: ExposureClass,
    evidence: ClassifiedProbeEvidence,
) -> Finding {
    let missing = missing_evidence(
        context.probe,
        &class,
        &evidence.infect,
        &evidence.observe,
        &evidence.discriminate,
        &evidence.activation,
    );
    let mut stop_reasons = stop_reasons(context.probe, context.owner_fn, &context.related_tests);
    ensure_unknown_stop_reason(&class, &mut stop_reasons);
    let recommended_next_step = recommended_next_step(context.probe, &class);
    let confidence = evidence.confidence(&class);

    Finding {
        id: context.probe.id.0.clone(),
        probe: context.probe.clone(),
        class,
        ripr: evidence.ripr,
        confidence,
        evidence: evidence.evidence,
        missing,
        flow_sinks: evidence.flow_sinks,
        activation: evidence.activation,
        stop_reasons,
        related_tests: evidence.related_tests,
        recommended_next_step,
        // Language metadata is populated by the per-language adapter
        // (e.g. `analysis::language::RustAdapter::analyze_diff`) after
        // classification. The classifier itself stays language-neutral.
        language: None,
        language_status: None,
        owner_kind: None,
        static_limit_kind: None,
    }
}
