use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::analysis::test_grip_evidence::{RelatedTestGrip, RelationConfidence};
use crate::config::{ConfigSeverity, RiprConfig};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_AGENT_BRIEF_MAX_SEAMS: usize = 3;
pub(crate) const AGENT_BRIEF_HARD_MAX_SEAMS: usize = 10;
const RELATED_TEST_ASSERTION_WINDOW_LINES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentBriefWorkingSetSource {
    Diff,
    Base,
    Files,
    SeamId,
}

impl AgentBriefWorkingSetSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Diff => "diff",
            Self::Base => "base",
            Self::Files => "files",
            Self::SeamId => "seam_id",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentBriefLine {
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
}

impl AgentBriefLine {
    pub(crate) fn new(file: impl Into<PathBuf>, line: usize) -> Self {
        Self {
            file: file.into(),
            line,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentBriefResolvedWorkingSet {
    pub(crate) source: AgentBriefWorkingSetSource,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) changed_lines: Vec<AgentBriefLine>,
    pub(crate) base: Option<String>,
    pub(crate) diff: Option<PathBuf>,
    pub(crate) seam_id: Option<String>,
}

impl AgentBriefResolvedWorkingSet {
    pub(crate) fn diff(diff: impl Into<PathBuf>, changed_lines: Vec<AgentBriefLine>) -> Self {
        let files = files_from_changed_lines(&changed_lines);
        Self {
            source: AgentBriefWorkingSetSource::Diff,
            files,
            changed_lines,
            base: None,
            diff: Some(diff.into()),
            seam_id: None,
        }
    }

    pub(crate) fn base(base: impl Into<String>, changed_lines: Vec<AgentBriefLine>) -> Self {
        let files = files_from_changed_lines(&changed_lines);
        Self {
            source: AgentBriefWorkingSetSource::Base,
            files,
            changed_lines,
            base: Some(base.into()),
            diff: None,
            seam_id: None,
        }
    }

    pub(crate) fn files(files: Vec<PathBuf>) -> Self {
        Self {
            source: AgentBriefWorkingSetSource::Files,
            files,
            changed_lines: Vec::new(),
            base: None,
            diff: None,
            seam_id: None,
        }
    }

    pub(crate) fn seam_id(seam_id: impl Into<String>) -> Self {
        Self {
            source: AgentBriefWorkingSetSource::SeamId,
            files: Vec::new(),
            changed_lines: Vec::new(),
            base: None,
            diff: None,
            seam_id: Some(seam_id.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentBriefWhyNowReason {
    ChangedLineIntersectsSeam,
    ChangedOwnerFunction,
    ChangedTestForRelatedSeam,
    ChangedAssertionNearRelatedTest,
    SameFileSeam,
    ExplicitSeamId,
    RepoActionableFallback,
}

impl AgentBriefWhyNowReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ChangedLineIntersectsSeam => "changed_line_intersects_seam",
            Self::ChangedOwnerFunction => "changed_owner_function",
            Self::ChangedTestForRelatedSeam => "changed_test_for_related_seam",
            Self::ChangedAssertionNearRelatedTest => "changed_assertion_near_related_test",
            Self::SameFileSeam => "same_file_seam",
            Self::ExplicitSeamId => "explicit_seam_id",
            Self::RepoActionableFallback => "repo_actionable_fallback",
        }
    }
}

const AGENT_BRIEF_WHY_NOW_REASON_VOCABULARY: [AgentBriefWhyNowReason; 7] = [
    AgentBriefWhyNowReason::ChangedLineIntersectsSeam,
    AgentBriefWhyNowReason::ChangedOwnerFunction,
    AgentBriefWhyNowReason::ChangedTestForRelatedSeam,
    AgentBriefWhyNowReason::ChangedAssertionNearRelatedTest,
    AgentBriefWhyNowReason::SameFileSeam,
    AgentBriefWhyNowReason::ExplicitSeamId,
    AgentBriefWhyNowReason::RepoActionableFallback,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentBriefWhyNowConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl AgentBriefWhyNowConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }
}

const AGENT_BRIEF_WHY_NOW_CONFIDENCE_VOCABULARY: [AgentBriefWhyNowConfidence; 4] = [
    AgentBriefWhyNowConfidence::High,
    AgentBriefWhyNowConfidence::Medium,
    AgentBriefWhyNowConfidence::Low,
    AgentBriefWhyNowConfidence::Unknown,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentBriefWhyNow {
    pub(crate) reason: AgentBriefWhyNowReason,
    pub(crate) confidence: AgentBriefWhyNowConfidence,
    pub(crate) evidence: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentBriefSelectedSeam<'a> {
    pub(crate) seam: &'a ClassifiedSeam,
    pub(crate) why_now: AgentBriefWhyNow,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentBriefSelection<'a> {
    pub(crate) requested: usize,
    pub(crate) returned: usize,
    pub(crate) default: usize,
    pub(crate) hard_cap: usize,
    pub(crate) top_seams: Vec<AgentBriefSelectedSeam<'a>>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentBriefPolicy<'a> {
    config: &'a RiprConfig,
}

impl<'a> AgentBriefPolicy<'a> {
    pub(crate) fn from_config(config: &'a RiprConfig) -> Self {
        Self { config }
    }

    fn severity_for(self, class: SeamGripClass) -> ConfigSeverity {
        self.config.severity().for_seam(class)
    }

    pub(crate) fn omission_reason_for_class(self, class: SeamGripClass) -> Option<String> {
        if matches!(self.severity_for(class), ConfigSeverity::Off) {
            return Some(format!(
                "is configured off for {} seams and is not included in agent results",
                class.as_str()
            ));
        }

        if !is_agent_actionable(class) {
            return Some(format!(
                "is {} and is not included in agent results",
                class.as_str()
            ));
        }

        None
    }
}

pub(crate) fn select_agent_brief_seams<'a>(
    classified: &'a [ClassifiedSeam],
    working_set: &AgentBriefResolvedWorkingSet,
    requested_max: usize,
    policy: AgentBriefPolicy<'_>,
) -> AgentBriefSelection<'a> {
    debug_assert_eq!(AGENT_BRIEF_WHY_NOW_REASON_VOCABULARY.len(), 7);
    debug_assert_eq!(AGENT_BRIEF_WHY_NOW_CONFIDENCE_VOCABULARY.len(), 4);

    let max_seams = normalize_requested_max(requested_max);
    let mut warnings = Vec::new();
    let mut direct = direct_candidates(classified, working_set, policy, &mut warnings);

    if direct.candidates.is_empty() && direct.allow_fallback {
        direct.candidates = fallback_candidates(classified, policy);
    }

    direct
        .candidates
        .sort_by(|left, right| compare_selected(left, right, policy));
    direct.candidates.truncate(max_seams);

    let returned = direct.candidates.len();
    AgentBriefSelection {
        requested: requested_max,
        returned,
        default: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
        hard_cap: AGENT_BRIEF_HARD_MAX_SEAMS,
        top_seams: direct.candidates,
        warnings,
    }
}

fn normalize_requested_max(requested: usize) -> usize {
    if requested == 0 {
        DEFAULT_AGENT_BRIEF_MAX_SEAMS
    } else {
        requested.min(AGENT_BRIEF_HARD_MAX_SEAMS)
    }
}

struct AgentBriefCandidateSelection<'a> {
    candidates: Vec<AgentBriefSelectedSeam<'a>>,
    allow_fallback: bool,
}

fn direct_candidates<'a>(
    classified: &'a [ClassifiedSeam],
    working_set: &AgentBriefResolvedWorkingSet,
    policy: AgentBriefPolicy<'_>,
    warnings: &mut Vec<String>,
) -> AgentBriefCandidateSelection<'a> {
    if let Some(seam_id) = working_set.seam_id.as_deref() {
        return AgentBriefCandidateSelection {
            candidates: explicit_seam_candidate(classified, seam_id, policy, warnings),
            allow_fallback: false,
        };
    }

    let mut candidates = Vec::new();
    let mut matched_working_set = false;
    let normalized_working_set = NormalizedAgentBriefWorkingSet::new(working_set);
    for entry in classified {
        let Some(why_now) = why_now_for(entry, &normalized_working_set) else {
            continue;
        };
        matched_working_set = true;
        if let Some(reason) = agent_brief_omission_reason(entry, policy) {
            warnings.push(format!(
                "seam {} at {}:{} {reason}",
                entry.seam.id().as_str(),
                display_path(entry.seam.file()),
                entry.seam.display_line()
            ));
            continue;
        }
        candidates.push(selected(entry, why_now));
    }

    AgentBriefCandidateSelection {
        candidates,
        allow_fallback: !matched_working_set,
    }
}

fn explicit_seam_candidate<'a>(
    classified: &'a [ClassifiedSeam],
    seam_id: &str,
    policy: AgentBriefPolicy<'_>,
    warnings: &mut Vec<String>,
) -> Vec<AgentBriefSelectedSeam<'a>> {
    let Some(entry) = classified
        .iter()
        .find(|entry| entry.seam.id().as_str() == seam_id)
    else {
        warnings.push(format!("requested seam_id {seam_id} was not found"));
        return Vec::new();
    };

    if let Some(reason) = agent_brief_omission_reason(entry, policy) {
        warnings.push(format!("requested seam_id {seam_id} {reason}"));
        return Vec::new();
    }

    vec![selected(
        entry,
        AgentBriefWhyNow {
            reason: AgentBriefWhyNowReason::ExplicitSeamId,
            confidence: AgentBriefWhyNowConfidence::High,
            evidence: format!("caller requested seam_id {seam_id}"),
        },
    )]
}

fn fallback_candidates<'a>(
    classified: &'a [ClassifiedSeam],
    policy: AgentBriefPolicy<'_>,
) -> Vec<AgentBriefSelectedSeam<'a>> {
    classified
        .iter()
        .filter(|entry| agent_brief_omission_reason(entry, policy).is_none())
        .map(|entry| {
            selected(
                entry,
                AgentBriefWhyNow {
                    reason: AgentBriefWhyNowReason::RepoActionableFallback,
                    confidence: AgentBriefWhyNowConfidence::Low,
                    evidence: "no working-set seam matched; selected a repo-actionable seam"
                        .to_string(),
                },
            )
        })
        .collect()
}

struct NormalizedAgentBriefWorkingSet<'a> {
    changed_lines: Vec<(String, &'a AgentBriefLine)>,
    files: Vec<String>,
}

impl<'a> NormalizedAgentBriefWorkingSet<'a> {
    fn new(working_set: &'a AgentBriefResolvedWorkingSet) -> Self {
        let changed_lines = working_set
            .changed_lines
            .iter()
            .map(|line| (normalized_path(&line.file), line))
            .collect();
        let files = working_set
            .files
            .iter()
            .map(|file| normalized_path(file))
            .collect();

        Self {
            changed_lines,
            files,
        }
    }
}

fn why_now_for(
    entry: &ClassifiedSeam,
    working_set: &NormalizedAgentBriefWorkingSet<'_>,
) -> Option<AgentBriefWhyNow> {
    if let Some(line) = matching_changed_line(entry, working_set) {
        return Some(AgentBriefWhyNow {
            reason: AgentBriefWhyNowReason::ChangedLineIntersectsSeam,
            confidence: AgentBriefWhyNowConfidence::High,
            evidence: format!("changed line {} intersects the seam origin", line.line),
        });
    }

    if let Some((test, line)) = matching_related_test_exact_line(entry, working_set) {
        return Some(AgentBriefWhyNow {
            reason: AgentBriefWhyNowReason::ChangedTestForRelatedSeam,
            confidence: AgentBriefWhyNowConfidence::High,
            evidence: format!(
                "changed line {} intersects related test {} at {}:{}",
                line.line,
                test.test_name,
                display_path(&test.file),
                test.line
            ),
        });
    }

    if let Some((test, line)) = matching_related_test_assertion_line(entry, working_set) {
        return Some(AgentBriefWhyNow {
            reason: AgentBriefWhyNowReason::ChangedAssertionNearRelatedTest,
            confidence: AgentBriefWhyNowConfidence::Medium,
            evidence: format!(
                "changed line {} is near related test {} at {}:{}",
                line.line,
                test.test_name,
                display_path(&test.file),
                test.line
            ),
        });
    }

    if let Some(test) = matching_related_test_file(entry, working_set) {
        return Some(AgentBriefWhyNow {
            reason: AgentBriefWhyNowReason::ChangedTestForRelatedSeam,
            confidence: AgentBriefWhyNowConfidence::Medium,
            evidence: format!(
                "working set includes related test {} at {}:{}",
                test.test_name,
                display_path(&test.file),
                test.line
            ),
        });
    }

    let seam_file = normalized_path(entry.seam.file());
    if working_set.files.iter().any(|file| file == &seam_file) {
        return Some(AgentBriefWhyNow {
            reason: AgentBriefWhyNowReason::SameFileSeam,
            confidence: AgentBriefWhyNowConfidence::Medium,
            evidence: format!("working set includes {}", display_path(entry.seam.file())),
        });
    }

    None
}

fn matching_changed_line<'a>(
    entry: &ClassifiedSeam,
    working_set: &'a NormalizedAgentBriefWorkingSet<'a>,
) -> Option<&'a AgentBriefLine> {
    let seam_file = normalized_path(entry.seam.file());
    working_set
        .changed_lines
        .iter()
        .find_map(|(line_file, line)| {
            (line_file == &seam_file && line.line == entry.seam.display_line()).then_some(*line)
        })
}

fn matching_related_test_exact_line<'a>(
    entry: &'a ClassifiedSeam,
    working_set: &'a NormalizedAgentBriefWorkingSet<'a>,
) -> Option<(&'a RelatedTestGrip, &'a AgentBriefLine)> {
    matching_related_test_line(entry, working_set, |test, line| line.line == test.line)
}

fn matching_related_test_assertion_line<'a>(
    entry: &'a ClassifiedSeam,
    working_set: &'a NormalizedAgentBriefWorkingSet<'a>,
) -> Option<(&'a RelatedTestGrip, &'a AgentBriefLine)> {
    matching_related_test_line(entry, working_set, |test, line| {
        line.line > test.line
            && line
                .line
                .checked_sub(test.line)
                .is_some_and(|distance| distance <= RELATED_TEST_ASSERTION_WINDOW_LINES)
    })
}

fn matching_related_test_line<'a>(
    entry: &'a ClassifiedSeam,
    working_set: &'a NormalizedAgentBriefWorkingSet<'a>,
    matches_line: impl Fn(&RelatedTestGrip, &AgentBriefLine) -> bool,
) -> Option<(&'a RelatedTestGrip, &'a AgentBriefLine)> {
    if entry.evidence.related_tests.is_empty() {
        return None;
    }

    entry.evidence.related_tests.iter().find_map(|test| {
        let test_file = normalized_path(&test.file);
        working_set
            .changed_lines
            .iter()
            .find_map(|(line_file, line)| {
                (line_file == &test_file && matches_line(test, line)).then_some((test, *line))
            })
    })
}

fn matching_related_test_file<'a>(
    entry: &'a ClassifiedSeam,
    working_set: &NormalizedAgentBriefWorkingSet<'_>,
) -> Option<&'a RelatedTestGrip> {
    if entry.evidence.related_tests.is_empty() {
        return None;
    }

    entry.evidence.related_tests.iter().find(|test| {
        let test_file = normalized_path(&test.file);
        working_set.files.iter().any(|file| file == &test_file)
    })
}

fn selected<'a>(seam: &'a ClassifiedSeam, why_now: AgentBriefWhyNow) -> AgentBriefSelectedSeam<'a> {
    AgentBriefSelectedSeam { seam, why_now }
}

fn compare_selected(
    left: &AgentBriefSelectedSeam<'_>,
    right: &AgentBriefSelectedSeam<'_>,
    policy: AgentBriefPolicy<'_>,
) -> Ordering {
    why_now_sort_priority(&left.why_now)
        .cmp(&why_now_sort_priority(&right.why_now))
        .then_with(|| {
            severity_priority(policy.severity_for(left.seam.class))
                .cmp(&severity_priority(policy.severity_for(right.seam.class)))
        })
        .then_with(|| grip_priority(left.seam.class).cmp(&grip_priority(right.seam.class)))
        .then_with(|| {
            best_relation_confidence_priority(left.seam)
                .cmp(&best_relation_confidence_priority(right.seam))
        })
        .then_with(|| {
            normalized_path(left.seam.seam.file()).cmp(&normalized_path(right.seam.seam.file()))
        })
        .then_with(|| {
            left.seam
                .seam
                .display_line()
                .cmp(&right.seam.seam.display_line())
        })
        .then_with(|| {
            left.seam
                .seam
                .id()
                .as_str()
                .cmp(right.seam.seam.id().as_str())
        })
}

fn why_now_sort_priority(why_now: &AgentBriefWhyNow) -> u8 {
    match (why_now.reason, why_now.confidence) {
        (AgentBriefWhyNowReason::ExplicitSeamId, _) => 0,
        (AgentBriefWhyNowReason::ChangedLineIntersectsSeam, _) => 1,
        (AgentBriefWhyNowReason::ChangedOwnerFunction, _) => 2,
        (AgentBriefWhyNowReason::ChangedTestForRelatedSeam, AgentBriefWhyNowConfidence::High) => 3,
        (AgentBriefWhyNowReason::ChangedAssertionNearRelatedTest, _) => 4,
        (AgentBriefWhyNowReason::ChangedTestForRelatedSeam, _) => 5,
        (AgentBriefWhyNowReason::SameFileSeam, _) => 6,
        (AgentBriefWhyNowReason::RepoActionableFallback, _) => 7,
    }
}

fn grip_priority(class: SeamGripClass) -> u8 {
    match class {
        SeamGripClass::WeaklyGripped => 0,
        SeamGripClass::Ungripped => 1,
        SeamGripClass::ReachableUnrevealed => 2,
        SeamGripClass::ActivationUnknown => 3,
        SeamGripClass::PropagationUnknown => 4,
        SeamGripClass::ObservationUnknown => 5,
        SeamGripClass::DiscriminationUnknown => 6,
        SeamGripClass::Opaque => 7,
        SeamGripClass::StronglyGripped => 8,
        SeamGripClass::Intentional => 9,
        SeamGripClass::Suppressed => 10,
    }
}

fn severity_priority(severity: ConfigSeverity) -> u8 {
    match severity {
        ConfigSeverity::Warning => 0,
        ConfigSeverity::Info => 1,
        ConfigSeverity::Note => 2,
        ConfigSeverity::Off => 3,
    }
}

fn best_relation_confidence_priority(entry: &ClassifiedSeam) -> u8 {
    entry
        .evidence
        .related_tests
        .iter()
        .map(|test| relation_confidence_priority(test.relation_confidence))
        .min()
        .unwrap_or(4)
}

fn relation_confidence_priority(confidence: RelationConfidence) -> u8 {
    match confidence {
        RelationConfidence::High => 0,
        RelationConfidence::Medium => 1,
        RelationConfidence::Low => 2,
        RelationConfidence::Opaque => 3,
    }
}

fn is_agent_actionable(class: SeamGripClass) -> bool {
    class.is_headline_eligible() || matches!(class, SeamGripClass::Opaque)
}

fn agent_brief_omission_reason(
    entry: &ClassifiedSeam,
    policy: AgentBriefPolicy<'_>,
) -> Option<String> {
    if matches!(policy.severity_for(entry.class), ConfigSeverity::Off) {
        return Some(format!(
            "is configured off for {} seams and is not included in agent brief results",
            entry.class.as_str()
        ));
    }

    if !is_agent_actionable(entry.class) {
        return Some(format!(
            "is {} and is not included in agent brief results",
            entry.class.as_str()
        ));
    }

    None
}

fn files_from_changed_lines(changed_lines: &[AgentBriefLine]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for line in changed_lines {
        if !files.iter().any(|file| same_file(file, &line.file)) {
            files.push(line.file.clone());
        }
    }
    files
}

fn same_file(left: &Path, right: &Path) -> bool {
    normalized_path(left) == normalized_path(right)
}

fn normalized_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_string()
}

fn display_path(path: &Path) -> String {
    normalized_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::config::{RiprConfig, tests_only_parse};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueFact,
    };

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn classified(
        file: &str,
        line: usize,
        owner: &str,
        expression: &str,
        class: SeamGripClass,
    ) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            file,
            owner,
            SeamKind::PredicateBoundary,
            line * 10,
            line,
            expression,
            RequiredDiscriminator::BoundaryValue {
                description: expression.to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            class,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: vec![RelatedTestGrip {
                    test_name: format!("{owner}_test"),
                    file: PathBuf::from("tests/sample.rs"),
                    line: 5,
                    oracle_kind: OracleKind::ExactValue,
                    oracle_strength: OracleStrength::Strong,
                    evidence_summary: "exact value assertion".to_string(),
                    relation_reason: RelationReason::DirectOwnerCall,
                    relation_confidence: RelationConfidence::High,
                }],
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                observe: stage(StageState::Yes),
                discriminate: stage(StageState::Weak),
                observed_values: Vec::<ValueFact>::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "boundary value".to_string(),
                    reason: "missing equality boundary".to_string(),
                    flow_sink: None,
                }],
            },
        }
    }

    fn related_test(
        entry: &mut ClassifiedSeam,
        file: &str,
        line: usize,
        confidence: RelationConfidence,
    ) {
        entry.evidence.related_tests[0].file = PathBuf::from(file);
        entry.evidence.related_tests[0].line = line;
        entry.evidence.related_tests[0].relation_confidence = confidence;
    }

    fn selected_ids(selection: &AgentBriefSelection<'_>) -> Vec<String> {
        selection
            .top_seams
            .iter()
            .map(|entry| entry.seam.seam.id().as_str().to_string())
            .collect()
    }

    fn select<'a>(
        seams: &'a [ClassifiedSeam],
        working_set: &AgentBriefResolvedWorkingSet,
        requested_max: usize,
    ) -> AgentBriefSelection<'a> {
        let config = RiprConfig::default();
        select_agent_brief_seams(
            seams,
            working_set,
            requested_max,
            AgentBriefPolicy::from_config(&config),
        )
    }

    fn select_with_config<'a>(
        seams: &'a [ClassifiedSeam],
        working_set: &AgentBriefResolvedWorkingSet,
        requested_max: usize,
        config: &RiprConfig,
    ) -> AgentBriefSelection<'a> {
        select_agent_brief_seams(
            seams,
            working_set,
            requested_max,
            AgentBriefPolicy::from_config(config),
        )
    }

    #[test]
    fn agent_brief_why_now_vocabulary_matches_spec_contract() {
        assert_eq!(AgentBriefWorkingSetSource::Diff.as_str(), "diff");
        assert_eq!(AgentBriefWorkingSetSource::Base.as_str(), "base");
        assert_eq!(AgentBriefWorkingSetSource::Files.as_str(), "files");
        assert_eq!(AgentBriefWorkingSetSource::SeamId.as_str(), "seam_id");

        assert_eq!(
            AGENT_BRIEF_WHY_NOW_REASON_VOCABULARY.map(AgentBriefWhyNowReason::as_str),
            [
                "changed_line_intersects_seam",
                "changed_owner_function",
                "changed_test_for_related_seam",
                "changed_assertion_near_related_test",
                "same_file_seam",
                "explicit_seam_id",
                "repo_actionable_fallback",
            ]
        );
        let priority_cases = [
            (
                AgentBriefWhyNowReason::ChangedLineIntersectsSeam,
                AgentBriefWhyNowConfidence::High,
                1,
            ),
            (
                AgentBriefWhyNowReason::ChangedOwnerFunction,
                AgentBriefWhyNowConfidence::High,
                2,
            ),
            (
                AgentBriefWhyNowReason::ChangedTestForRelatedSeam,
                AgentBriefWhyNowConfidence::High,
                3,
            ),
            (
                AgentBriefWhyNowReason::ChangedAssertionNearRelatedTest,
                AgentBriefWhyNowConfidence::Medium,
                4,
            ),
            (
                AgentBriefWhyNowReason::ChangedTestForRelatedSeam,
                AgentBriefWhyNowConfidence::Medium,
                5,
            ),
            (
                AgentBriefWhyNowReason::SameFileSeam,
                AgentBriefWhyNowConfidence::Medium,
                6,
            ),
            (
                AgentBriefWhyNowReason::ExplicitSeamId,
                AgentBriefWhyNowConfidence::High,
                0,
            ),
            (
                AgentBriefWhyNowReason::RepoActionableFallback,
                AgentBriefWhyNowConfidence::Low,
                7,
            ),
        ];
        for (reason, confidence, expected) in priority_cases {
            assert_eq!(
                why_now_sort_priority(&AgentBriefWhyNow {
                    reason,
                    confidence,
                    evidence: String::new(),
                }),
                expected
            );
        }

        assert_eq!(
            AGENT_BRIEF_WHY_NOW_CONFIDENCE_VOCABULARY.map(AgentBriefWhyNowConfidence::as_str),
            ["high", "medium", "low", "unknown"]
        );
    }

    #[test]
    fn agent_brief_base_working_set_derives_files_from_changed_lines() {
        let working_set = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![
                AgentBriefLine::new("src/pricing.rs", 88),
                AgentBriefLine::new("src/pricing.rs", 89),
                AgentBriefLine::new("src/tax.rs", 12),
            ],
        );

        assert_eq!(working_set.source, AgentBriefWorkingSetSource::Base);
        assert_eq!(working_set.base.as_deref(), Some("main"));
        assert_eq!(working_set.diff, None);
        assert_eq!(working_set.seam_id, None);
        assert_eq!(
            working_set.files,
            vec![PathBuf::from("src/pricing.rs"), PathBuf::from("src/tax.rs")]
        );
    }

    #[test]
    fn agent_brief_selector_ranks_explicit_seam_id_first() {
        let unrelated = classified(
            "src/pricing.rs",
            12,
            "pricing::unrelated",
            "amount > 0",
            SeamGripClass::WeaklyGripped,
        );
        let requested = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::ReachableUnrevealed,
        );
        let seam_id = requested.seam.id().as_str().to_string();
        let seams = vec![unrelated, requested];

        let selection = select(&seams, &AgentBriefResolvedWorkingSet::seam_id(&seam_id), 3);

        assert_eq!(selected_ids(&selection), vec![seam_id]);
        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::ExplicitSeamId
        );
        assert_eq!(
            selection.top_seams[0].why_now.confidence,
            AgentBriefWhyNowConfidence::High
        );
        assert!(selection.warnings.is_empty());
    }

    #[test]
    fn agent_brief_selector_prefers_changed_line_intersection_over_same_file() {
        let same_file = classified(
            "src/pricing.rs",
            12,
            "pricing::below_threshold",
            "amount < discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let touched = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::ReachableUnrevealed,
        );
        let touched_id = touched.seam.id().as_str().to_string();
        let seams = vec![same_file, touched];
        let working_set = AgentBriefResolvedWorkingSet::diff(
            "change.diff",
            vec![AgentBriefLine::new("src/pricing.rs", 88)],
        );

        let selection = select(&seams, &working_set, 3);

        assert_eq!(selection.top_seams[0].seam.seam.id().as_str(), touched_id);
        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::ChangedLineIntersectsSeam
        );
        assert_eq!(
            selection.top_seams[1].why_now.reason,
            AgentBriefWhyNowReason::SameFileSeam
        );
    }

    #[test]
    fn agent_brief_selector_caps_file_scope_deterministically() {
        let seams = vec![
            classified(
                "src/pricing.rs",
                10,
                "pricing::a",
                "a",
                SeamGripClass::ActivationUnknown,
            ),
            classified(
                "src/pricing.rs",
                11,
                "pricing::b",
                "b",
                SeamGripClass::WeaklyGripped,
            ),
            classified(
                "src/pricing.rs",
                12,
                "pricing::c",
                "c",
                SeamGripClass::Ungripped,
            ),
        ];
        let weak_id = seams[1].seam.id().as_str().to_string();
        let ungripped_id = seams[2].seam.id().as_str().to_string();
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/pricing.rs")]);

        let selection = select(&seams, &working_set, 2);

        assert_eq!(selection.requested, 2);
        assert_eq!(selection.returned, 2);
        assert_eq!(selection.default, DEFAULT_AGENT_BRIEF_MAX_SEAMS);
        assert_eq!(selection.hard_cap, AGENT_BRIEF_HARD_MAX_SEAMS);
        assert_eq!(selected_ids(&selection), vec![weak_id, ungripped_id]);
        assert!(
            selection
                .top_seams
                .iter()
                .all(|entry| entry.why_now.reason == AgentBriefWhyNowReason::SameFileSeam)
        );
    }

    #[test]
    fn agent_brief_selector_routes_file_scoped_related_test_to_seam() {
        let mut seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(&mut seam, "tests/pricing.rs", 20, RelationConfidence::High);
        let seams = vec![seam];
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("tests/pricing.rs")]);

        let selection = select(&seams, &working_set, 3);

        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::ChangedTestForRelatedSeam
        );
        assert_eq!(
            selection.top_seams[0].why_now.confidence,
            AgentBriefWhyNowConfidence::Medium
        );
        assert!(
            selection.top_seams[0]
                .why_now
                .evidence
                .contains("working set includes related test")
        );
    }

    #[test]
    fn agent_brief_selector_routes_changed_related_test_line_to_seam() {
        let mut seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(&mut seam, "tests/pricing.rs", 20, RelationConfidence::High);
        let seams = vec![seam];
        let working_set = AgentBriefResolvedWorkingSet::diff(
            "change.diff",
            vec![AgentBriefLine::new("tests/pricing.rs", 20)],
        );

        let selection = select(&seams, &working_set, 3);

        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::ChangedTestForRelatedSeam
        );
        assert_eq!(
            selection.top_seams[0].why_now.confidence,
            AgentBriefWhyNowConfidence::High
        );
        assert!(
            selection.top_seams[0]
                .why_now
                .evidence
                .contains("intersects related test")
        );
    }

    #[test]
    fn agent_brief_selector_routes_changed_assertion_near_related_test_to_seam() {
        let mut seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(&mut seam, "tests/pricing.rs", 20, RelationConfidence::High);
        let seams = vec![seam];
        let working_set = AgentBriefResolvedWorkingSet::diff(
            "change.diff",
            vec![AgentBriefLine::new("tests/pricing.rs", 24)],
        );

        let selection = select(&seams, &working_set, 3);

        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::ChangedAssertionNearRelatedTest
        );
        assert_eq!(
            selection.top_seams[0].why_now.confidence,
            AgentBriefWhyNowConfidence::Medium
        );
        assert!(
            selection.top_seams[0]
                .why_now
                .evidence
                .contains("is near related test")
        );
    }

    #[test]
    fn agent_brief_selector_ranks_assertion_near_related_test_before_file_only_related_test() {
        let mut broad = classified(
            "src/a.rs",
            88,
            "pricing::broad_file_match",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(&mut broad, "tests/shared.rs", 1, RelationConfidence::High);
        let mut assertion = classified(
            "src/z.rs",
            88,
            "pricing::assertion_match",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(
            &mut assertion,
            "tests/shared.rs",
            20,
            RelationConfidence::High,
        );
        let assertion_id = assertion.seam.id().as_str().to_string();
        let seams = vec![broad, assertion];
        let working_set = AgentBriefResolvedWorkingSet::diff(
            "change.diff",
            vec![AgentBriefLine::new("tests/shared.rs", 24)],
        );

        let selection = select(&seams, &working_set, 2);

        assert_eq!(selection.top_seams[0].seam.seam.id().as_str(), assertion_id);
        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::ChangedAssertionNearRelatedTest
        );
    }

    #[test]
    fn agent_brief_selector_uses_default_limit_when_zero_is_requested() {
        let seams = vec![
            classified(
                "src/pricing.rs",
                10,
                "pricing::a",
                "a",
                SeamGripClass::WeaklyGripped,
            ),
            classified(
                "src/pricing.rs",
                11,
                "pricing::b",
                "b",
                SeamGripClass::Ungripped,
            ),
            classified(
                "src/pricing.rs",
                12,
                "pricing::c",
                "c",
                SeamGripClass::ReachableUnrevealed,
            ),
            classified(
                "src/pricing.rs",
                13,
                "pricing::d",
                "d",
                SeamGripClass::ActivationUnknown,
            ),
        ];
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/pricing.rs")]);

        let selection = select(&seams, &working_set, 0);

        assert_eq!(selection.requested, 0);
        assert_eq!(selection.returned, DEFAULT_AGENT_BRIEF_MAX_SEAMS);
        assert_eq!(selection.top_seams.len(), DEFAULT_AGENT_BRIEF_MAX_SEAMS);
    }

    #[test]
    fn agent_brief_grip_priority_matches_spec_order() {
        let priorities = [
            (SeamGripClass::WeaklyGripped, 0),
            (SeamGripClass::Ungripped, 1),
            (SeamGripClass::ReachableUnrevealed, 2),
            (SeamGripClass::ActivationUnknown, 3),
            (SeamGripClass::PropagationUnknown, 4),
            (SeamGripClass::ObservationUnknown, 5),
            (SeamGripClass::DiscriminationUnknown, 6),
            (SeamGripClass::Opaque, 7),
            (SeamGripClass::StronglyGripped, 8),
            (SeamGripClass::Intentional, 9),
            (SeamGripClass::Suppressed, 10),
        ];

        for (class, expected) in priorities {
            assert_eq!(grip_priority(class), expected);
        }
    }

    #[test]
    fn agent_brief_severity_priority_prefers_warning_before_info_note_and_off() {
        assert_eq!(severity_priority(ConfigSeverity::Warning), 0);
        assert_eq!(severity_priority(ConfigSeverity::Info), 1);
        assert_eq!(severity_priority(ConfigSeverity::Note), 2);
        assert_eq!(severity_priority(ConfigSeverity::Off), 3);
    }

    #[test]
    fn agent_brief_selector_uses_related_test_confidence_before_path() {
        let mut low = classified(
            "src/a.rs",
            88,
            "pricing::low_confidence",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(&mut low, "tests/shared.rs", 20, RelationConfidence::Low);
        let mut high = classified(
            "src/z.rs",
            88,
            "pricing::high_confidence",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        related_test(&mut high, "tests/shared.rs", 20, RelationConfidence::High);
        let high_id = high.seam.id().as_str().to_string();
        let seams = vec![low, high];
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("tests/shared.rs")]);

        let selection = select(&seams, &working_set, 2);

        assert_eq!(selection.top_seams[0].seam.seam.id().as_str(), high_id);
    }

    #[test]
    fn agent_brief_selector_uses_repo_fallback_when_working_set_has_no_match() {
        let seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let seam_id = seam.seam.id().as_str().to_string();
        let seams = vec![seam];
        let working_set = AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/other.rs")]);

        let selection = select(&seams, &working_set, 3);

        assert_eq!(selected_ids(&selection), vec![seam_id]);
        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::RepoActionableFallback
        );
        assert_eq!(
            selection.top_seams[0].why_now.confidence,
            AgentBriefWhyNowConfidence::Low
        );
    }

    #[test]
    fn agent_brief_selector_uses_configured_severity_before_grip_priority() -> Result<(), String> {
        let weak = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let ungripped = classified(
            "src/pricing.rs",
            89,
            "pricing::taxed_total",
            "tax > 0",
            SeamGripClass::Ungripped,
        );
        let ungripped_id = ungripped.seam.id().as_str().to_string();
        let seams = vec![weak, ungripped];
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/pricing.rs")]);
        let config = tests_only_parse(
            r#"
[severity.seams]
weakly_gripped = "note"
ungripped = "warning"
"#,
        )?;

        let selection = select_with_config(&seams, &working_set, 2, &config);

        assert_eq!(selection.top_seams[0].seam.seam.id().as_str(), ungripped_id);
        Ok(())
    }

    #[test]
    fn agent_brief_selector_omits_configured_off_working_set_seam() -> Result<(), String> {
        let seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let fallback = classified(
            "src/tax.rs",
            12,
            "tax::total",
            "tax > 0",
            SeamGripClass::Ungripped,
        );
        let seams = vec![seam, fallback];
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/pricing.rs")]);
        let config = tests_only_parse(
            r#"
[severity.seams]
weakly_gripped = "off"
"#,
        )?;

        let selection = select_with_config(&seams, &working_set, 3, &config);

        assert!(selection.top_seams.is_empty());
        assert_eq!(
            selection.warnings,
            vec![format!(
                "seam {} at src/pricing.rs:88 is configured off for weakly_gripped seams and is not included in agent brief results",
                seams[0].seam.id().as_str()
            )]
        );
        Ok(())
    }

    #[test]
    fn agent_brief_selector_omits_configured_off_explicit_seam_without_fallback()
    -> Result<(), String> {
        let hidden = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let hidden_id = hidden.seam.id().as_str().to_string();
        let fallback = classified(
            "src/tax.rs",
            12,
            "tax::total",
            "tax > 0",
            SeamGripClass::Ungripped,
        );
        let seams = vec![hidden, fallback];
        let config = tests_only_parse(
            r#"
[severity.seams]
weakly_gripped = "off"
"#,
        )?;

        let selection = select_with_config(
            &seams,
            &AgentBriefResolvedWorkingSet::seam_id(&hidden_id),
            3,
            &config,
        );

        assert!(selection.top_seams.is_empty());
        assert_eq!(
            selection.warnings,
            vec![format!(
                "requested seam_id {hidden_id} is configured off for weakly_gripped seams and is not included in agent brief results"
            )]
        );
        Ok(())
    }

    #[test]
    fn agent_brief_selector_warns_when_explicit_seam_is_missing() {
        let seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let seams = vec![seam];

        let selection = select(
            &seams,
            &AgentBriefResolvedWorkingSet::seam_id("missing-seam"),
            3,
        );

        assert!(selection.top_seams.is_empty());
        assert_eq!(
            selection.warnings,
            vec!["requested seam_id missing-seam was not found"]
        );
    }

    #[test]
    fn agent_brief_selector_warns_when_explicit_seam_is_hidden() {
        let hidden = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::StronglyGripped,
        );
        let seam_id = hidden.seam.id().as_str().to_string();
        let seams = vec![hidden];

        let selection = select(&seams, &AgentBriefResolvedWorkingSet::seam_id(&seam_id), 3);

        assert!(selection.top_seams.is_empty());
        assert_eq!(
            selection.warnings,
            vec![format!(
                "requested seam_id {seam_id} is configured off for strongly_gripped seams and is not included in agent brief results"
            )]
        );
    }

    #[test]
    fn agent_brief_selector_normalizes_windows_style_paths() {
        let seam = classified(
            "src/pricing.rs",
            88,
            "pricing::discounted_total",
            "amount >= discount_threshold",
            SeamGripClass::WeaklyGripped,
        );
        let seams = vec![seam];
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from(".\\src\\pricing.rs")]);

        let selection = select(&seams, &working_set, 3);

        assert_eq!(
            selection.top_seams[0].why_now.reason,
            AgentBriefWhyNowReason::SameFileSeam
        );
    }
}
