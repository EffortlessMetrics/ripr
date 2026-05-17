use crate::analysis::classify::{
    ProbeContext, activation_evidence, classify, confidence_score, infection_evidence,
    local_flow_sinks, propagation_evidence, reach_evidence, reveal_evidence,
};
use crate::domain::*;

pub(in crate::analysis) struct ClassifiedProbeEvidence {
    pub(in crate::analysis) ripr: RiprEvidence,
    pub(in crate::analysis) evidence: Vec<String>,
    pub(in crate::analysis) flow_sinks: Vec<FlowSinkFact>,
    pub(in crate::analysis) activation: ActivationEvidence,
    pub(in crate::analysis) related_tests: Vec<RelatedTest>,
    pub(in crate::analysis) reach: StageEvidence,
    pub(in crate::analysis) infect: StageEvidence,
    pub(in crate::analysis) propagate: StageEvidence,
    pub(in crate::analysis) observe: StageEvidence,
    pub(in crate::analysis) discriminate: StageEvidence,
}

impl ClassifiedProbeEvidence {
    pub(in crate::analysis) fn gather(context: &ProbeContext<'_>) -> Self {
        let reach = reach_evidence(&context.related_tests, context.owner_fn);
        let flow_sinks = local_flow_sinks(context.probe, context.owner_fn);
        let activation = activation_evidence(
            context.probe,
            context.owner_fn,
            &context.related_tests,
            &flow_sinks,
        );
        let infect = infection_evidence(context.probe, &context.related_tests, &activation);
        let propagate = propagation_evidence(context.probe, &flow_sinks);
        let (observe, discriminate, related_tests) =
            reveal_evidence(context.probe, &context.related_tests);

        let ripr = RiprEvidence {
            reach: reach.clone(),
            infect: infect.clone(),
            propagate: propagate.clone(),
            reveal: RevealEvidence {
                observe: observe.clone(),
                discriminate: discriminate.clone(),
            },
        };
        let evidence = evidence_summaries([&reach, &infect, &propagate, &observe, &discriminate]);

        Self {
            ripr,
            evidence,
            flow_sinks,
            activation,
            related_tests,
            reach,
            infect,
            propagate,
            observe,
            discriminate,
        }
    }

    pub(in crate::analysis) fn classify(&self, probe: &Probe) -> ExposureClass {
        classify(
            &self.reach,
            &self.infect,
            &self.propagate,
            &self.observe,
            &self.discriminate,
            probe,
        )
    }

    pub(in crate::analysis) fn confidence(&self, class: &ExposureClass) -> f32 {
        confidence_score(
            &self.reach,
            &self.infect,
            &self.propagate,
            &self.observe,
            &self.discriminate,
            class,
        )
    }
}

fn evidence_summaries<'e>(stages: impl IntoIterator<Item = &'e StageEvidence>) -> Vec<String> {
    let mut summaries = stages
        .into_iter()
        .filter_map(|stage| (!stage.summary.is_empty()).then_some(stage.summary.clone()))
        .collect::<Vec<_>>();
    summaries.sort();
    summaries.dedup();
    summaries
}

#[cfg(test)]
mod tests {
    use super::evidence_summaries;
    use crate::domain::{Confidence, StageEvidence, StageState};

    #[test]
    fn evidence_summaries_drop_empty_and_deduplicate_in_sorted_order() {
        let stages = [
            StageEvidence::new(StageState::Yes, Confidence::High, "z evidence"),
            StageEvidence::new(StageState::No, Confidence::Low, ""),
            StageEvidence::new(StageState::Weak, Confidence::Medium, "a evidence"),
            StageEvidence::new(StageState::Yes, Confidence::High, "z evidence"),
        ];

        assert_eq!(
            evidence_summaries(stages.iter()),
            vec!["a evidence".to_string(), "z evidence".to_string()]
        );
    }
}
