//! Bounded full-payload retention for the review-comments consumer.
//! Ranking and omission meaning stay in the parent agent-brief authority.
use super::*;
use std::collections::BTreeSet;

struct OwnedCandidate {
    ordinal: usize,
    entry: ClassifiedSeam,
    why_now: AgentBriefWhyNow,
}

struct OmissionCount {
    kind: AgentBriefOmission,
    count: usize,
    // Ten global named omissions can hide at most ten occurrences of a kind.
    // Eleven positions suffice to recover its first unnamed occurrence.
    first_ordinals: Vec<usize>,
}

pub(crate) struct BoundedAgentBrief<'a> {
    working_set: &'a AgentBriefResolvedWorkingSet,
    normalized: NormalizedAgentBriefWorkingSet<'a>,
    changed_scope: AgentBriefChangedScope<'a>,
    policy: AgentBriefPolicy<'a>,
    requested: usize,
    limit: usize,
    matched_scope: bool,
    visible_candidates: usize,
    candidates: Vec<OwnedCandidate>,
    named_omissions: Vec<(usize, AgentBriefOmission, String)>,
    omission_counts: Vec<OmissionCount>,
    // Only fixed-size identities, never discarded evidence payloads.
    seen_ids: BTreeSet<String>,
}

impl<'a> BoundedAgentBrief<'a> {
    pub(crate) fn new(
        working_set: &'a AgentBriefResolvedWorkingSet,
        requested: usize,
        policy: AgentBriefPolicy<'a>,
    ) -> Result<Self, String> {
        if working_set.seam_id.is_some() {
            return Err(
                "streamed review selection does not accept an explicit seam-id scope".into(),
            );
        }
        Ok(Self {
            working_set,
            normalized: NormalizedAgentBriefWorkingSet::new(working_set),
            changed_scope: AgentBriefChangedScope::new(working_set),
            policy,
            requested,
            limit: normalize_requested_max(requested),
            matched_scope: false,
            visible_candidates: 0,
            candidates: Vec::new(),
            named_omissions: Vec::new(),
            omission_counts: Vec::new(),
            seen_ids: BTreeSet::new(),
        })
    }

    pub(crate) fn observe(&mut self, ordinal: usize, entry: ClassifiedSeam) -> Result<(), String> {
        if !self.seen_ids.insert(entry.seam.id().as_str().to_string()) {
            return Err("duplicate seam identity in streamed review evidence".into());
        }
        let direct = why_now_for(&entry, &self.normalized);
        let why_now = if let Some(why_now) = direct {
            if !self.matched_scope {
                // Fallback never contributes once any scoped seam is represented,
                // even when every matching seam is configured off/non-actionable.
                self.matched_scope = true;
                self.visible_candidates = 0;
                self.candidates.clear();
            }
            if let Some(omission) = AgentBriefOmission::for_entry(&entry, self.policy) {
                self.record_omission(ordinal, &entry, omission)?;
                return Ok(());
            }
            why_now
        } else {
            if self.matched_scope || agent_brief_omission_reason(&entry, self.policy).is_some() {
                return Ok(());
            }
            AgentBriefWhyNow {
                reason: AgentBriefWhyNowReason::RepoActionableFallback,
                confidence: AgentBriefWhyNowConfidence::Low,
                evidence: "no working-set seam matched; selected a repo-actionable seam"
                    .to_string(),
            }
        };
        self.visible_candidates = self
            .visible_candidates
            .checked_add(1)
            .ok_or("streamed review candidate count overflow")?;
        self.candidates.push(OwnedCandidate {
            ordinal,
            entry,
            why_now,
        });
        let policy = self.policy;
        self.candidates.sort_by(|left, right| {
            compare_selected_parts(
                &left.entry,
                &left.why_now,
                &right.entry,
                &right.why_now,
                policy,
            )
            .then(left.ordinal.cmp(&right.ordinal))
        });
        self.candidates.truncate(self.limit);
        Ok(())
    }

    fn record_omission(
        &mut self,
        ordinal: usize,
        entry: &ClassifiedSeam,
        kind: AgentBriefOmission,
    ) -> Result<(), String> {
        let position = self
            .omission_counts
            .iter()
            .position(|item| item.kind == kind);
        let position = match position {
            Some(position) => position,
            None => {
                self.omission_counts.push(OmissionCount {
                    kind,
                    count: 0,
                    first_ordinals: Vec::new(),
                });
                self.omission_counts.len() - 1
            }
        };
        let count = &mut self.omission_counts[position];
        count.count = count
            .count
            .checked_add(1)
            .ok_or("streamed review omission count overflow")?;
        count.first_ordinals.push(ordinal);
        count.first_ordinals.sort_unstable();
        count
            .first_ordinals
            .truncate(AGENT_BRIEF_MAX_NAMED_OMISSIONS + 1);
        self.named_omissions.push((
            ordinal,
            kind,
            format!(
                "seam {} at {}:{} {}",
                entry.seam.id().as_str(),
                display_path(entry.seam.file()),
                entry.seam.display_line(),
                kind.reason()
            ),
        ));
        self.named_omissions.sort_by_key(|item| item.0);
        self.named_omissions
            .truncate(AGENT_BRIEF_MAX_NAMED_OMISSIONS);
        Ok(())
    }

    pub(crate) fn selection(&self) -> Result<AgentBriefSelection<'_>, String> {
        let mut warnings = self
            .named_omissions
            .iter()
            .map(|item| item.2.clone())
            .collect::<Vec<_>>();
        let mut more = Vec::new();
        for item in &self.omission_counts {
            let named = self
                .named_omissions
                .iter()
                .filter(|named| named.1 == item.kind)
                .count();
            let count = item
                .count
                .checked_sub(named)
                .ok_or("streamed review omission accounting incomplete")?;
            if count == 0 {
                continue;
            }
            let first = item
                .first_ordinals
                .iter()
                .find(|ordinal| {
                    !self
                        .named_omissions
                        .iter()
                        .any(|named| named.0 == **ordinal)
                })
                .copied()
                .ok_or("streamed review omission ordering incomplete")?;
            more.push((first, item.kind.more(count)));
        }
        more.sort_by_key(|item| item.0);
        warnings.extend(more.into_iter().map(|item| item.1));
        if !self.matched_scope {
            warnings.push(if self.candidates.is_empty() {
                format!(
                    "No seams matched the requested scope (source: {}), and no other \
                 agent-actionable seam in the analyzed inventory is visible under the \
                 current config.",
                    self.working_set.source.as_str()
                )
            } else {
                format!(
                    "No seams matched the requested scope (source: {}); showing all \
                 repo-actionable seams instead — this is NOT a scoped result.",
                    self.working_set.source.as_str()
                )
            });
        }
        let omitted = self
            .visible_candidates
            .checked_sub(self.candidates.len())
            .ok_or("streamed review candidate accounting incomplete")?;
        if omitted > 0 {
            warnings.push(format!(
                "{omitted} additional visible seams were omitted by the brief cap"
            ));
        }
        Ok(AgentBriefSelection {
            requested: self.requested,
            returned: self.candidates.len(),
            default: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            hard_cap: AGENT_BRIEF_HARD_MAX_SEAMS,
            top_seams: self
                .candidates
                .iter()
                .map(|item| AgentBriefSelectedSeam {
                    seam: &item.entry,
                    why_now: item.why_now.clone(),
                })
                .collect(),
            warnings,
        })
    }
}

impl crate::analysis::ScopedEvidenceConsumer for BoundedAgentBrief<'_> {
    fn in_first_stage(&self, seam: &RepoSeam) -> bool {
        self.changed_scope.contains(seam)
    }

    fn observe(&mut self, ordinal: usize, entry: ClassifiedSeam) -> Result<(), String> {
        self.observe(ordinal, entry)
    }

    fn first_stage_sufficient(&self) -> bool {
        self.changed_scope.fills_selection(
            self.candidates.iter().map(|item| &item.entry),
            self.requested,
            self.policy,
        )
    }

    fn retained_payloads(&self) -> usize {
        self.candidates.len()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::classified;
    use super::*;

    #[test]
    fn late_hidden_scope_match_discards_fallback_and_counter_overflow_refuses() -> Result<(), String>
    {
        let config = RiprConfig::default();
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/changed.rs")]);
        let fallback = classified(
            "src/other.rs",
            1,
            "other::a",
            "n > 0",
            SeamGripClass::WeaklyGripped,
        );
        let hidden = classified(
            "src/changed.rs",
            1,
            "changed::a",
            "n > 0",
            SeamGripClass::StronglyGripped,
        );
        let mut sink =
            BoundedAgentBrief::new(&working_set, 10, AgentBriefPolicy::from_config(&config))?;
        sink.observe(1, fallback)?;
        assert_eq!(sink.candidates.len(), 1);
        sink.observe(0, hidden.clone())?;
        let selection = sink.selection()?;
        assert_eq!(selection.returned, 0);
        assert_eq!(selection.warnings.len(), 1);
        assert!(!selection.warnings[0].contains("fallback"));
        sink.omission_counts[0].count = usize::MAX;
        assert_eq!(
            sink.record_omission(2, &hidden, sink.omission_counts[0].kind)
                .err()
                .as_deref(),
            Some("streamed review omission count overflow")
        );
        sink.visible_candidates = usize::MAX;
        let visible = classified(
            "src/changed.rs",
            3,
            "changed::b",
            "n > 1",
            SeamGripClass::WeaklyGripped,
        );
        assert_eq!(
            sink.observe(3, visible).err().as_deref(),
            Some("streamed review candidate count overflow")
        );
        Ok(())
    }
}
