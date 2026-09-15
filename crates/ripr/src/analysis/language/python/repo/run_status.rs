//! Typed run status for Python repo-mode analysis (#3554 PR A).
//!
//! Bounded-analysis contract (#2109, #3554): a capped or partial run can
//! never be reported as a clean-complete run. The invariant is encoded in
//! [`PythonRepoRunStatus::can_support_full_denominator`], which is the only
//! method a consumer may use to decide whether the run can back a
//! full-denominator claim.
//!
//! Vocabulary note: statuses describe what a run covered, never whether a
//! mutation would be caught. Conservative exposure language
//! (`exposed`, `weakly_exposed`, `no_static_path`, ...) is owned by the
//! classification layer, not here.

/// Status of one Python repo-mode run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) enum PythonRepoRunStatus {
    /// The full eligible working set was selected and analyzed; nothing
    /// was capped, failed, or unreadable. Produced only by the evidence
    /// producer (PR B) after successful analysis — never by discovery
    /// alone. The only status that can back a full-denominator claim.
    Complete,
    /// Analysis ran, but part of the requested corpus was not analyzed.
    Partial {
        /// Why the run ended partial, with the count that keeps the
        /// denominator explicit.
        reason: PartialRunReason,
    },
    /// The bounded input was selected (full discovery, no cap hit), but
    /// analysis has not run yet. Discovery alone cannot claim a
    /// full-denominator outcome — only post-analysis [`Complete`] can
    /// (#3666 review: a discovery-stage `Complete` would fabricate
    /// completion evidence).
    Selected,
    /// The eligible working set hit the repo working-set cap (#2109).
    /// Files beyond the cap were counted, not analyzed.
    Capped,
    /// The workspace contributed no Python production source, so there is
    /// no analysis subject. An honest zero with a reason — not a failure
    /// and not a clean coverage claim.
    NoPythonSource,
    /// Python analysis is disabled for this run (config/feature policy).
    /// Discovery did not run.
    Disabled,
}

/// Why a run ended partial. The parse-failure reason is populated by the
/// evidence producer (PR B); the discovery-incomplete reason is populated
/// by the input selector when parts of the workspace could not be read
/// (#3666 review).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) enum PartialRunReason {
    /// Selected files that could not be read or parsed; the retained count
    /// keeps the analysis denominator explicit.
    ParseFailures {
        /// Number of selected files that failed to read or parse.
        failed: usize,
    },
    /// Subtrees or entries the walk could not read; the omitted contents
    /// were never discovered, so source absence is unestablished and the
    /// status must not report `NoPythonSource`.
    DiscoveryIncomplete {
        /// Number of unreadable subtrees or entries.
        unreadable: usize,
    },
    /// Selected files whose role was ambiguous were counted but never read
    /// or analyzed, so the run cannot be `Complete` while any exist.
    /// Unreachable while discovery routes only production/test/helper
    /// roles, but typed so the evidence producer can never report
    /// `Complete` over unanalyzed selected files (#3554 PR B).
    UnanalyzedAmbiguous {
        /// Number of selected ambiguous-role files left unanalyzed.
        count: usize,
    },
}

impl PythonRepoRunStatus {
    /// Whether this status can back a full-denominator claim.
    ///
    /// Only [`PythonRepoRunStatus::Complete`] can. A capped or partial run,
    /// a no-subject run, and a disabled run must never be reported as
    /// clean-complete (#3554 bounded-analysis contract; #2109 cap
    /// semantics).
    pub(in crate::analysis::language::python) fn can_support_full_denominator(&self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Stable status label for typed disclosure on output surfaces.
    pub(in crate::analysis::language::python) fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Selected => "selected",
            Self::Partial { .. } => "partial",
            Self::Capped => "capped",
            Self::NoPythonSource => "no_python_source",
            Self::Disabled => "disabled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_supports_a_full_denominator() {
        assert!(PythonRepoRunStatus::Complete.can_support_full_denominator());
    }

    #[test]
    fn capped_partial_no_subject_and_disabled_never_support_a_full_denominator() {
        assert!(!PythonRepoRunStatus::Capped.can_support_full_denominator());
        assert!(
            !PythonRepoRunStatus::Partial {
                reason: PartialRunReason::ParseFailures { failed: 2 },
            }
            .can_support_full_denominator()
        );
        assert!(!PythonRepoRunStatus::NoPythonSource.can_support_full_denominator());
        assert!(!PythonRepoRunStatus::Disabled.can_support_full_denominator());
    }

    #[test]
    fn partial_reason_retains_the_failed_count() {
        let status = PythonRepoRunStatus::Partial {
            reason: PartialRunReason::ParseFailures { failed: 7 },
        };
        assert_eq!(
            status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::ParseFailures { failed: 7 },
            }
        );
        assert_ne!(
            status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::ParseFailures { failed: 8 },
            }
        );
    }

    #[test]
    fn selected_is_the_discovery_stage_state() {
        // Discovery alone cannot claim a full denominator (#3666 review):
        // the evidence producer (PR B) must complete analysis first.
        assert!(!PythonRepoRunStatus::Selected.can_support_full_denominator());
        assert_eq!(PythonRepoRunStatus::Selected.as_str(), "selected");
    }

    #[test]
    fn discovery_incomplete_reason_retains_the_unreadable_count() {
        let status = PythonRepoRunStatus::Partial {
            reason: PartialRunReason::DiscoveryIncomplete { unreadable: 3 },
        };
        assert!(!status.can_support_full_denominator());
        assert_ne!(
            status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::DiscoveryIncomplete { unreadable: 4 },
            }
        );
    }

    #[test]
    fn unanalyzed_ambiguous_reason_retains_the_count_and_blocks_a_full_denominator() {
        let status = PythonRepoRunStatus::Partial {
            reason: PartialRunReason::UnanalyzedAmbiguous { count: 2 },
        };
        assert!(!status.can_support_full_denominator());
        assert_ne!(
            status,
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::UnanalyzedAmbiguous { count: 3 },
            }
        );
    }

    #[test]
    fn status_labels_are_stable() {
        assert_eq!(PythonRepoRunStatus::Complete.as_str(), "complete");
        assert_eq!(PythonRepoRunStatus::Selected.as_str(), "selected");
        assert_eq!(
            PythonRepoRunStatus::Partial {
                reason: PartialRunReason::ParseFailures { failed: 1 },
            }
            .as_str(),
            "partial"
        );
        assert_eq!(PythonRepoRunStatus::Capped.as_str(), "capped");
        assert_eq!(
            PythonRepoRunStatus::NoPythonSource.as_str(),
            "no_python_source"
        );
        assert_eq!(PythonRepoRunStatus::Disabled.as_str(), "disabled");
    }
}
