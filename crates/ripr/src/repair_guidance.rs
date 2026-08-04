//! Versioned producer-owned repair-guidance availability contract.
//!
//! This module answers two questions with closed, machine-stable states:
//! is a producer-owned discriminator available, and is a producer-owned
//! concrete assertion available? Renderers must project these typed facts
//! rather than substitute adjacent prose or a template string when the
//! producer did not derive one. (#2830)
//!
//! The module lives at the crate root rather than under `domain/` because
//! [`RepairGuidance::semantic_digest`] serializes through `serde_json`, and
//! `policy/architecture.txt` forbids `serde_json` inside `crates/ripr/src/domain/**`.
//! This matches the sibling `#2827` contract in [`crate::analysis_outcome`].
//!
//! This module is internal until #2657 and #2658 connect the packet
//! producers. It defines availability only: no state here produces missing
//! guidance, makes a route actionable, authorizes a source edit, or claims a
//! repair would succeed.

use crate::domain::{MissingDiscriminatorFact, StaticLimitKind};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub(crate) const REPAIR_GUIDANCE_SCHEMA_VERSION: &str = "0.1";
pub(crate) const REPAIR_GUIDANCE_CLAIM_BOUNDARY: &str = "Static repair-guidance availability only; no claim that a repair exists, is correct, is authorized, or would pass.";
pub(crate) const MAX_REPAIR_GUIDANCE_TEXT_CHARS: usize = 512;

/// Why a producer-owned guidance fact is not concrete. Every non-concrete
/// state names one of these; silence is not a representable state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuidanceReason {
    /// The producer did not emit the fact at all. Adjacent prose in the same
    /// record is not a substitute.
    ProducerFactAbsent,
    /// The producer ran but derived no behavioral discriminator for the seam.
    NoBehavioralDiscriminatorDerived,
    /// An observer must exist before any assertion can read the behavior.
    ObserverNotStaticallyVisible,
    /// The route offers navigation to a fix site only.
    RouteIsInspectionOnly,
    /// The route offers a verification command only.
    RouteIsVerificationOnly,
    /// A named static limitation blocked derivation.
    StaticLimitationBlocksDerivation,
    /// The analysis inputs changed after this guidance was produced.
    SnapshotStale,
    /// The discriminating oracle belongs to another language and is not
    /// statically visible from this workspace.
    CrossLanguageOracleUnresolved,
}

impl GuidanceReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ProducerFactAbsent => "producer_fact_absent",
            Self::NoBehavioralDiscriminatorDerived => "no_behavioral_discriminator_derived",
            Self::ObserverNotStaticallyVisible => "observer_not_statically_visible",
            Self::RouteIsInspectionOnly => "route_is_inspection_only",
            Self::RouteIsVerificationOnly => "route_is_verification_only",
            Self::StaticLimitationBlocksDerivation => "static_limitation_blocks_derivation",
            Self::SnapshotStale => "snapshot_stale",
            Self::CrossLanguageOracleUnresolved => "cross_language_oracle_unresolved",
        }
    }
}

/// What a consumer can do about a non-concrete guidance state. This is a
/// bounded inspection or refresh route, never an authorization to edit source
/// or execute a verification command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuidanceRecovery {
    /// Open the producer-owned fix site and read the current oracle.
    InspectFixSite,
    /// Run the explain route for the full static evidence.
    RunExplain,
    /// Make the behavior observable, then assert on the observation.
    AddObserverThenAssert,
    /// Re-run the analysis against current inputs.
    RefreshAnalysis,
    /// Review the external-language oracle rather than adding a local test.
    ReviewExternalOracle,
    /// No bounded recovery is available from static evidence.
    NoRecoveryAvailable,
}

impl GuidanceRecovery {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InspectFixSite => "inspect_fix_site",
            Self::RunExplain => "run_explain",
            Self::AddObserverThenAssert => "add_observer_then_assert",
            Self::RefreshAnalysis => "refresh_analysis",
            Self::ReviewExternalOracle => "review_external_oracle",
            Self::NoRecoveryAvailable => "no_recovery_available",
        }
    }
}

/// Producer-owned provenance for a concrete discriminator. Every variant names
/// a fact the analysis produced. There is deliberately no variant for adjacent
/// prose, renderer text, prompt text, comments, or neighboring tests: relabeling
/// those as a discriminator is unrepresentable rather than merely discouraged.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscriminatorBasis {
    /// A [`MissingDiscriminatorFact`] emitted by probe activation evidence.
    ActivationEvidenceFact,
    /// A `RequiredDiscriminator` emitted by seam classification.
    SeamRequiredDiscriminator,
}

/// Producer-owned provenance for a concrete assertion example.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssertionBasis {
    /// Derived from the seam's required discriminator.
    SeamRequiredDiscriminator,
    /// Derived from an observed value fact.
    ObservedValueFact,
    /// Derived from a resolved flow-sink fact.
    ResolvedFlowSink,
}

/// The closed assertion-shape vocabulary. These replace the ad-hoc
/// `&'static str` shape tokens the packet renderer used, so that a shape can
/// only be named when a concrete example exists.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssertionKind {
    ExactReturnValue,
    ExactErrorVariant,
    FieldEquality,
    SideEffectObserver,
    MatchResult,
    CallExpectation,
}

/// The kind of observer a test must establish before it can assert.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObserverKind {
    SideEffectSink,
    CallSite,
    ExternalLanguageOracle,
}

/// Availability of a producer-owned discriminator for one changed behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DiscriminatorAvailability {
    Present {
        text: String,
        basis: DiscriminatorBasis,
    },
    NotProduced {
        reason: GuidanceReason,
        recovery: GuidanceRecovery,
    },
    NotApplicable {
        reason: GuidanceReason,
    },
    StaticLimitation {
        kind: StaticLimitKind,
        reason: GuidanceReason,
    },
    Stale {
        reason: GuidanceReason,
        refresh: GuidanceRecovery,
    },
}

/// Availability of producer-owned concrete assertion guidance for one seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AssertionGuidance {
    Concrete {
        kind: AssertionKind,
        example: String,
        basis: AssertionBasis,
    },
    RequiresObserverSetup {
        observer_kind: ObserverKind,
        reason: GuidanceReason,
    },
    FixSiteOnly {
        reason: GuidanceReason,
    },
    VerificationOnly {
        reason: GuidanceReason,
    },
    Unresolved {
        reason: GuidanceReason,
        recovery: GuidanceRecovery,
    },
    Stale {
        reason: GuidanceReason,
        refresh: GuidanceRecovery,
    },
}

/// Wire state token for [`DiscriminatorAvailability`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscriminatorState {
    Present,
    NotProduced,
    NotApplicable,
    StaticLimitation,
    Stale,
}

impl DiscriminatorState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::NotProduced => "not_produced",
            Self::NotApplicable => "not_applicable",
            Self::StaticLimitation => "static_limitation",
            Self::Stale => "stale",
        }
    }
}

/// Wire state token for [`AssertionGuidance`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssertionState {
    Concrete,
    RequiresObserverSetup,
    FixSiteOnly,
    VerificationOnly,
    Unresolved,
    Stale,
}

/// Producer facts a gap-ledger repair route can offer.
///
/// `assertion_shape` and `changed_behavior` are carried so that the adapter
/// visibly declines them: they are adjacent prose about the change, not a
/// producer-owned discriminator. See #2657.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GapRouteGuidanceFacts<'a> {
    pub(crate) missing_discriminator: Option<&'a str>,
    pub(crate) assertion_shape: Option<&'a str>,
    pub(crate) changed_behavior: Option<&'a str>,
    pub(crate) static_limit: Option<StaticLimitKind>,
    pub(crate) inspection_only: bool,
    pub(crate) stale: bool,
}

/// Producer facts a classified seam can offer for assertion guidance.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeamAssertionFacts<'a> {
    /// The example the producer derived, if any. A non-empty renderer string
    /// without a producer basis is not concrete guidance.
    pub(crate) derived_example: Option<&'a str>,
    pub(crate) kind: AssertionKind,
    pub(crate) basis: Option<AssertionBasis>,
    pub(crate) observer_required: Option<ObserverKind>,
    pub(crate) verification_only: bool,
    pub(crate) fix_site_only: bool,
    pub(crate) stale: bool,
}

impl DiscriminatorAvailability {
    pub(crate) fn state(&self) -> DiscriminatorState {
        match self {
            Self::Present { .. } => DiscriminatorState::Present,
            Self::NotProduced { .. } => DiscriminatorState::NotProduced,
            Self::NotApplicable { .. } => DiscriminatorState::NotApplicable,
            Self::StaticLimitation { .. } => DiscriminatorState::StaticLimitation,
            Self::Stale { .. } => DiscriminatorState::Stale,
        }
    }

    pub(crate) fn reason(&self) -> Option<GuidanceReason> {
        match self {
            Self::Present { .. } => None,
            Self::NotProduced { reason, .. }
            | Self::NotApplicable { reason }
            | Self::StaticLimitation { reason, .. }
            | Self::Stale { reason, .. } => Some(*reason),
        }
    }

    pub(crate) fn recovery(&self) -> Option<GuidanceRecovery> {
        match self {
            Self::Present { .. } | Self::NotApplicable { .. } | Self::StaticLimitation { .. } => {
                None
            }
            Self::NotProduced { recovery, .. }
            | Self::Stale {
                refresh: recovery, ..
            } => Some(*recovery),
        }
    }

    pub(crate) fn legacy_text(&self) -> Option<&str> {
        match self {
            Self::Present { text, .. } => Some(text.as_str()),
            Self::NotProduced { .. }
            | Self::NotApplicable { .. }
            | Self::StaticLimitation { .. }
            | Self::Stale { .. } => None,
        }
    }

    /// Project the gap-ledger route facts.
    ///
    /// Precedence is fixed: staleness first (authority rule 3), then a named
    /// static limitation, then the producer fact. `assertion_shape` and
    /// `changed_behavior` never reach the result.
    pub(crate) fn from_gap_route(facts: GapRouteGuidanceFacts<'_>) -> Result<Self, String> {
        // Destructured rather than field-accessed so that declining the two
        // prose fields is an explicit, reviewable step instead of an omission
        // a later edit could quietly undo.
        let GapRouteGuidanceFacts {
            missing_discriminator,
            // Adjacent prose about the change, not a producer discriminator.
            // Authority rule 2 makes relabelling these unrepresentable (#2657).
            assertion_shape: _declined_assertion_shape,
            changed_behavior: _declined_changed_behavior,
            static_limit,
            inspection_only,
            stale,
        } = facts;

        if stale {
            return Ok(Self::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::RefreshAnalysis,
            });
        }
        if let Some(kind) = static_limit {
            return Ok(Self::StaticLimitation {
                kind,
                reason: GuidanceReason::StaticLimitationBlocksDerivation,
            });
        }
        match missing_discriminator {
            Some(text) => Ok(Self::Present {
                text: bounded_guidance_text("discriminator text", text)?,
                basis: DiscriminatorBasis::ActivationEvidenceFact,
            }),
            None if inspection_only => Ok(Self::NotApplicable {
                reason: GuidanceReason::RouteIsInspectionOnly,
            }),
            None => Ok(Self::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            }),
        }
    }

    /// Project the classified-seam producer fact. The gap-ledger route and the
    /// classified seam must agree on the semantic state for equivalent facts.
    pub(crate) fn from_missing_discriminator_fact(
        fact: Option<&MissingDiscriminatorFact>,
        stale: bool,
    ) -> Result<Self, String> {
        if stale {
            return Ok(Self::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::RefreshAnalysis,
            });
        }
        match fact {
            Some(fact) => Ok(Self::Present {
                text: bounded_guidance_text("discriminator text", fact.value.as_str())?,
                basis: DiscriminatorBasis::ActivationEvidenceFact,
            }),
            None => Ok(Self::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            }),
        }
    }

    /// Flatten to the additive wire projection. Both the manual-JSON packet
    /// path and the `serde_json` packet path call this, so neither can encode
    /// absence differently.
    pub(crate) fn view(&self) -> DiscriminatorGuidanceView {
        match self {
            Self::Present { text, basis } => DiscriminatorGuidanceView {
                state: DiscriminatorState::Present,
                text: Some(text.clone()),
                basis: Some(*basis),
                reason: None,
                recovery: None,
                static_limit_kind: None,
            },
            Self::NotProduced { reason, recovery } => DiscriminatorGuidanceView {
                state: DiscriminatorState::NotProduced,
                text: None,
                basis: None,
                reason: Some(*reason),
                recovery: Some(*recovery),
                static_limit_kind: None,
            },
            Self::NotApplicable { reason } => DiscriminatorGuidanceView {
                state: DiscriminatorState::NotApplicable,
                text: None,
                basis: None,
                reason: Some(*reason),
                recovery: None,
                static_limit_kind: None,
            },
            Self::StaticLimitation { kind, reason } => DiscriminatorGuidanceView {
                state: DiscriminatorState::StaticLimitation,
                text: None,
                basis: None,
                reason: Some(*reason),
                recovery: None,
                static_limit_kind: Some(*kind),
            },
            Self::Stale { reason, refresh } => DiscriminatorGuidanceView {
                state: DiscriminatorState::Stale,
                text: None,
                basis: None,
                reason: Some(*reason),
                recovery: Some(*refresh),
                static_limit_kind: None,
            },
        }
    }
}

impl AssertionGuidance {
    /// Project the classified-seam assertion facts.
    ///
    /// A derived example is concrete only when the producer also names the
    /// basis it came from (authority rule 1). An example without a basis is
    /// `Unresolved`, which is what stops a template string from serializing as
    /// paste-ready guidance. See #2658.
    pub(crate) fn from_seam_facts(facts: SeamAssertionFacts<'_>) -> Result<Self, String> {
        if facts.stale {
            return Ok(Self::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::RefreshAnalysis,
            });
        }
        if let Some(observer_kind) = facts.observer_required {
            return Ok(Self::RequiresObserverSetup {
                observer_kind,
                reason: GuidanceReason::ObserverNotStaticallyVisible,
            });
        }
        if facts.verification_only {
            return Ok(Self::VerificationOnly {
                reason: GuidanceReason::RouteIsVerificationOnly,
            });
        }
        if facts.fix_site_only {
            return Ok(Self::FixSiteOnly {
                reason: GuidanceReason::RouteIsInspectionOnly,
            });
        }
        match (facts.derived_example, facts.basis) {
            (Some(example), Some(basis)) => Ok(Self::Concrete {
                kind: facts.kind,
                example: bounded_guidance_text("assertion example", example)?,
                basis,
            }),
            (Some(_), None) => Ok(Self::Unresolved {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            }),
            (None, _) => Ok(Self::Unresolved {
                reason: GuidanceReason::NoBehavioralDiscriminatorDerived,
                recovery: GuidanceRecovery::InspectFixSite,
            }),
        }
    }

    /// Flatten to the additive wire projection. Every non-concrete state
    /// serializes `example` and `kind` as null.
    pub(crate) fn view(&self) -> AssertionGuidanceView {
        match self {
            Self::Concrete {
                kind,
                example,
                basis,
            } => AssertionGuidanceView {
                state: AssertionState::Concrete,
                example: Some(example.clone()),
                kind: Some(*kind),
                basis: Some(*basis),
                observer_kind: None,
                reason: None,
                recovery: None,
            },
            Self::RequiresObserverSetup {
                observer_kind,
                reason,
            } => AssertionGuidanceView {
                state: AssertionState::RequiresObserverSetup,
                example: None,
                kind: None,
                basis: None,
                observer_kind: Some(*observer_kind),
                reason: Some(*reason),
                recovery: None,
            },
            Self::FixSiteOnly { reason } => AssertionGuidanceView {
                state: AssertionState::FixSiteOnly,
                example: None,
                kind: None,
                basis: None,
                observer_kind: None,
                reason: Some(*reason),
                recovery: None,
            },
            Self::VerificationOnly { reason } => AssertionGuidanceView {
                state: AssertionState::VerificationOnly,
                example: None,
                kind: None,
                basis: None,
                observer_kind: None,
                reason: Some(*reason),
                recovery: None,
            },
            Self::Unresolved { reason, recovery } => AssertionGuidanceView {
                state: AssertionState::Unresolved,
                example: None,
                kind: None,
                basis: None,
                observer_kind: None,
                reason: Some(*reason),
                recovery: Some(*recovery),
            },
            Self::Stale { reason, refresh } => AssertionGuidanceView {
                state: AssertionState::Stale,
                example: None,
                kind: None,
                basis: None,
                observer_kind: None,
                reason: Some(*reason),
                recovery: Some(*refresh),
            },
        }
    }
}

/// Additive wire projection of [`DiscriminatorAvailability`].
///
/// `text` is the nullable legacy value field. Its meaning is unchanged: when
/// present it is still the producer discriminator string. What changes is that
/// absence is now stated by `state` plus `reason` instead of by an omitted or
/// substituted value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DiscriminatorGuidanceView {
    state: DiscriminatorState,
    text: Option<String>,
    basis: Option<DiscriminatorBasis>,
    reason: Option<GuidanceReason>,
    recovery: Option<GuidanceRecovery>,
    static_limit_kind: Option<StaticLimitKind>,
}

/// Additive wire projection of [`AssertionGuidance`].
///
/// `example` is the nullable legacy value field with its original meaning: a
/// concrete, producer-derived assertion. It is null in every non-concrete state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct AssertionGuidanceView {
    state: AssertionState,
    example: Option<String>,
    kind: Option<AssertionKind>,
    basis: Option<AssertionBasis>,
    observer_kind: Option<ObserverKind>,
    reason: Option<GuidanceReason>,
    recovery: Option<GuidanceRecovery>,
}

#[derive(Deserialize)]
struct DiscriminatorGuidanceViewWire {
    state: DiscriminatorState,
    text: Option<String>,
    basis: Option<DiscriminatorBasis>,
    reason: Option<GuidanceReason>,
    recovery: Option<GuidanceRecovery>,
    static_limit_kind: Option<StaticLimitKind>,
}

impl<'de> Deserialize<'de> for DiscriminatorGuidanceView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DiscriminatorGuidanceViewWire::deserialize(deserializer)?;
        let view = Self {
            state: wire.state,
            text: wire.text,
            basis: wire.basis,
            reason: wire.reason,
            recovery: wire.recovery,
            static_limit_kind: wire.static_limit_kind,
        };
        view.validate().map_err(D::Error::custom)?;
        Ok(view)
    }
}

#[derive(Deserialize)]
struct AssertionGuidanceViewWire {
    state: AssertionState,
    example: Option<String>,
    kind: Option<AssertionKind>,
    basis: Option<AssertionBasis>,
    observer_kind: Option<ObserverKind>,
    reason: Option<GuidanceReason>,
    recovery: Option<GuidanceRecovery>,
}

impl<'de> Deserialize<'de> for AssertionGuidanceView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AssertionGuidanceViewWire::deserialize(deserializer)?;
        let view = Self {
            state: wire.state,
            example: wire.example,
            kind: wire.kind,
            basis: wire.basis,
            observer_kind: wire.observer_kind,
            reason: wire.reason,
            recovery: wire.recovery,
        };
        view.validate().map_err(D::Error::custom)?;
        Ok(view)
    }
}

impl DiscriminatorGuidanceView {
    /// Reject any state/field combination a renderer could use to imply
    /// guidance it does not have. This runs on construction from the typed
    /// state and again on deserialization, so a hand-built JSON producer
    /// cannot disagree with the Serde producer.
    fn validate(&self) -> Result<(), String> {
        let concrete = self.text.is_some() || self.basis.is_some();
        match self.state {
            DiscriminatorState::Present => {
                let text = self
                    .text
                    .as_deref()
                    .ok_or("discriminator state present requires text")?;
                check_guidance_text("discriminator text", text)?;
                if self.basis.is_none() {
                    return Err("discriminator state present requires a producer basis".to_string());
                }
                if self.reason.is_some() || self.recovery.is_some() {
                    return Err(
                        "discriminator state present cannot carry a reason or recovery".to_string(),
                    );
                }
            }
            _ if concrete => {
                return Err(format!(
                    "discriminator state {:?} cannot carry text or basis",
                    self.state
                ));
            }
            DiscriminatorState::NotProduced | DiscriminatorState::Stale => {
                if self.reason.is_none() || self.recovery.is_none() {
                    return Err(format!(
                        "discriminator state {:?} requires a reason and recovery",
                        self.state
                    ));
                }
            }
            DiscriminatorState::NotApplicable | DiscriminatorState::StaticLimitation => {
                if self.reason.is_none() {
                    return Err(format!(
                        "discriminator state {:?} requires a reason",
                        self.state
                    ));
                }
                if self.recovery.is_some() {
                    return Err(format!(
                        "discriminator state {:?} cannot carry a recovery",
                        self.state
                    ));
                }
            }
        }
        // Presence alone is not enough: a state paired with someone else's
        // reason or recovery hands the consumer the wrong explanation while
        // still passing contract validation.
        if let Some(reason) = self.reason
            && !allowed_discriminator_reasons(self.state).contains(&reason)
        {
            return Err(format!(
                "discriminator state {:?} cannot carry reason {reason:?}",
                self.state
            ));
        }
        if let Some(recovery) = self.recovery
            && !allowed_discriminator_recoveries(self.state).contains(&recovery)
        {
            return Err(format!(
                "discriminator state {:?} cannot carry recovery {recovery:?}",
                self.state
            ));
        }
        let expects_limit = self.state == DiscriminatorState::StaticLimitation;
        if expects_limit != self.static_limit_kind.is_some() {
            return Err(
                "static_limit_kind is present exactly when the state is static_limitation"
                    .to_string(),
            );
        }
        Ok(())
    }
}

impl AssertionGuidanceView {
    /// Reject any state/field combination that would let a non-concrete state
    /// carry a paste-ready-looking example.
    fn validate(&self) -> Result<(), String> {
        let concrete = self.example.is_some() || self.kind.is_some() || self.basis.is_some();
        match self.state {
            AssertionState::Concrete => {
                let example = self
                    .example
                    .as_deref()
                    .ok_or("assertion state concrete requires an example")?;
                check_guidance_text("assertion example", example)?;
                if self.kind.is_none() || self.basis.is_none() {
                    return Err(
                        "assertion state concrete requires an assertion kind and producer basis"
                            .to_string(),
                    );
                }
                if self.observer_kind.is_some() || self.reason.is_some() || self.recovery.is_some()
                {
                    return Err(
                        "assertion state concrete cannot carry an observer, reason, or recovery"
                            .to_string(),
                    );
                }
            }
            _ if concrete => {
                return Err(format!(
                    "assertion state {:?} cannot carry an example, kind, or basis",
                    self.state
                ));
            }
            AssertionState::RequiresObserverSetup => {
                if self.observer_kind.is_none() || self.reason.is_none() {
                    return Err(
                        "assertion state requires_observer_setup requires an observer kind and reason"
                            .to_string(),
                    );
                }
                if self.recovery.is_some() {
                    return Err(
                        "assertion state requires_observer_setup cannot carry a recovery"
                            .to_string(),
                    );
                }
            }
            AssertionState::FixSiteOnly | AssertionState::VerificationOnly => {
                if self.reason.is_none() {
                    return Err(format!(
                        "assertion state {:?} requires a reason",
                        self.state
                    ));
                }
                if self.recovery.is_some() {
                    return Err(format!(
                        "assertion state {:?} cannot carry a recovery",
                        self.state
                    ));
                }
            }
            AssertionState::Unresolved | AssertionState::Stale => {
                if self.reason.is_none() || self.recovery.is_none() {
                    return Err(format!(
                        "assertion state {:?} requires a reason and recovery",
                        self.state
                    ));
                }
            }
        }
        let expects_observer = self.state == AssertionState::RequiresObserverSetup;
        if expects_observer != self.observer_kind.is_some() {
            return Err(
                "observer_kind is present exactly when the state is requires_observer_setup"
                    .to_string(),
            );
        }
        if let Some(reason) = self.reason
            && !allowed_assertion_reasons(self.state).contains(&reason)
        {
            return Err(format!(
                "assertion state {:?} cannot carry reason {reason:?}",
                self.state
            ));
        }
        if let Some(recovery) = self.recovery
            && !allowed_assertion_recoveries(self.state).contains(&recovery)
        {
            return Err(format!(
                "assertion state {:?} cannot carry recovery {recovery:?}",
                self.state
            ));
        }
        Ok(())
    }
}

/// The reasons each discriminator state is allowed to carry.
///
/// Closed per state so a stored packet cannot pair, say, `stale` with
/// `producer_fact_absent` and still validate.
fn allowed_discriminator_reasons(state: DiscriminatorState) -> &'static [GuidanceReason] {
    match state {
        DiscriminatorState::Present => &[],
        DiscriminatorState::NotProduced => &[
            GuidanceReason::ProducerFactAbsent,
            GuidanceReason::NoBehavioralDiscriminatorDerived,
        ],
        DiscriminatorState::NotApplicable => &[
            GuidanceReason::RouteIsInspectionOnly,
            GuidanceReason::RouteIsVerificationOnly,
        ],
        DiscriminatorState::StaticLimitation => &[
            GuidanceReason::StaticLimitationBlocksDerivation,
            GuidanceReason::CrossLanguageOracleUnresolved,
        ],
        DiscriminatorState::Stale => &[GuidanceReason::SnapshotStale],
    }
}

/// The recoveries each discriminator state is allowed to carry. A stale
/// snapshot can only be refreshed; it cannot route the reader to a fix site
/// whose evidence is known to be out of date.
fn allowed_discriminator_recoveries(state: DiscriminatorState) -> &'static [GuidanceRecovery] {
    match state {
        DiscriminatorState::Present
        | DiscriminatorState::NotApplicable
        | DiscriminatorState::StaticLimitation => &[],
        DiscriminatorState::NotProduced => &[
            GuidanceRecovery::InspectFixSite,
            GuidanceRecovery::RunExplain,
            GuidanceRecovery::ReviewExternalOracle,
            GuidanceRecovery::NoRecoveryAvailable,
        ],
        DiscriminatorState::Stale => &[
            GuidanceRecovery::RefreshAnalysis,
            GuidanceRecovery::NoRecoveryAvailable,
        ],
    }
}

/// The reasons each assertion state is allowed to carry.
fn allowed_assertion_reasons(state: AssertionState) -> &'static [GuidanceReason] {
    match state {
        AssertionState::Concrete => &[],
        AssertionState::RequiresObserverSetup => &[GuidanceReason::ObserverNotStaticallyVisible],
        AssertionState::FixSiteOnly => &[GuidanceReason::RouteIsInspectionOnly],
        AssertionState::VerificationOnly => &[GuidanceReason::RouteIsVerificationOnly],
        AssertionState::Unresolved => &[
            GuidanceReason::ProducerFactAbsent,
            GuidanceReason::NoBehavioralDiscriminatorDerived,
            GuidanceReason::CrossLanguageOracleUnresolved,
            GuidanceReason::StaticLimitationBlocksDerivation,
        ],
        AssertionState::Stale => &[GuidanceReason::SnapshotStale],
    }
}

/// The recoveries each assertion state is allowed to carry.
fn allowed_assertion_recoveries(state: AssertionState) -> &'static [GuidanceRecovery] {
    match state {
        AssertionState::Concrete
        | AssertionState::RequiresObserverSetup
        | AssertionState::FixSiteOnly
        | AssertionState::VerificationOnly => &[],
        AssertionState::Unresolved => &[
            GuidanceRecovery::InspectFixSite,
            GuidanceRecovery::RunExplain,
            GuidanceRecovery::AddObserverThenAssert,
            GuidanceRecovery::ReviewExternalOracle,
            GuidanceRecovery::NoRecoveryAvailable,
        ],
        AssertionState::Stale => &[
            GuidanceRecovery::RefreshAnalysis,
            GuidanceRecovery::NoRecoveryAvailable,
        ],
    }
}

/// The canonical DTO both packet paths project. It carries the schema version
/// and claim boundary so a consumer never has to infer either from field shape.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RepairGuidance {
    schema_version: String,
    discriminator: DiscriminatorGuidanceView,
    assertion: AssertionGuidanceView,
    claim_boundary: String,
}

#[derive(Deserialize)]
struct RepairGuidanceWire {
    schema_version: String,
    discriminator: DiscriminatorGuidanceView,
    assertion: AssertionGuidanceView,
    claim_boundary: String,
}

impl<'de> Deserialize<'de> for RepairGuidance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RepairGuidanceWire::deserialize(deserializer)?;
        if wire.schema_version != REPAIR_GUIDANCE_SCHEMA_VERSION {
            return Err(D::Error::custom(format!(
                "repair guidance schema_version must be {REPAIR_GUIDANCE_SCHEMA_VERSION}, got {}",
                wire.schema_version
            )));
        }
        if wire.claim_boundary != REPAIR_GUIDANCE_CLAIM_BOUNDARY {
            return Err(D::Error::custom(
                "repair guidance claim_boundary does not match the versioned contract",
            ));
        }
        RepairGuidance::from_views(wire.discriminator, wire.assertion).map_err(D::Error::custom)
    }
}

impl RepairGuidance {
    /// Build from the typed states. `schema_version` and `claim_boundary` are
    /// stamped by the constructor and never taken from a caller.
    pub(crate) fn new(
        discriminator: &DiscriminatorAvailability,
        assertion: &AssertionGuidance,
    ) -> Result<Self, String> {
        Self::from_views(discriminator.view(), assertion.view())
    }

    fn from_views(
        discriminator: DiscriminatorGuidanceView,
        assertion: AssertionGuidanceView,
    ) -> Result<Self, String> {
        discriminator.validate()?;
        assertion.validate()?;
        if discriminator.state == DiscriminatorState::Stale
            && assertion.state == AssertionState::Concrete
        {
            return Err(
                "stale discriminator state cannot accompany concrete assertion guidance"
                    .to_string(),
            );
        }
        Ok(Self {
            schema_version: REPAIR_GUIDANCE_SCHEMA_VERSION.to_string(),
            discriminator,
            assertion,
            claim_boundary: REPAIR_GUIDANCE_CLAIM_BOUNDARY.to_string(),
        })
    }

    /// Stable semantic commitment over the versioned typed guidance. The DTO
    /// carries no path, timestamp, duration, or map, so the digest is portable
    /// across checkout roots and traversal orders.
    pub(crate) fn semantic_digest(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("serialize repair guidance for digest failed: {error}"))?;
        let digest = Sha256::digest(bytes);
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut hex, "{byte:02x}")
                .map_err(|error| format!("format repair guidance digest failed: {error}"))?;
        }
        Ok(format!("sha256:{hex}"))
    }
}

/// Validate bounds on a borrowed slice. The `validate` paths only need the
/// verdict, so they must not allocate a `String` they immediately discard.
fn check_guidance_text(field: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    let count = trimmed.chars().count();
    if count > MAX_REPAIR_GUIDANCE_TEXT_CHARS {
        return Err(format!(
            "{field} has {count} characters; maximum is {MAX_REPAIR_GUIDANCE_TEXT_CHARS}"
        ));
    }
    Ok(())
}

/// Validate and normalize into the owned form the typed states store.
fn bounded_guidance_text(field: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    check_guidance_text(field, trimmed)?;
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{Debug, Display};

    fn expect_error<T: Debug, E: Display>(
        result: Result<T, E>,
        expected: &str,
    ) -> Result<(), String> {
        match result {
            Ok(value) => Err(format!(
                "expected error containing `{expected}`, got success: {value:?}"
            )),
            Err(error) if error.to_string().contains(expected) => Ok(()),
            Err(error) => Err(format!(
                "expected error containing `{expected}`, got `{error}`"
            )),
        }
    }

    fn seam_facts<'a>(derived_example: Option<&'a str>) -> SeamAssertionFacts<'a> {
        SeamAssertionFacts {
            derived_example,
            kind: AssertionKind::ExactReturnValue,
            basis: None,
            observer_required: None,
            verification_only: false,
            fix_site_only: false,
            stale: false,
        }
    }

    fn unresolved_assertion() -> AssertionGuidance {
        AssertionGuidance::Unresolved {
            reason: GuidanceReason::NoBehavioralDiscriminatorDerived,
            recovery: GuidanceRecovery::InspectFixSite,
        }
    }

    #[test]
    fn typed_guidance_tokens_cover_every_wire_variant() -> Result<(), String> {
        let reasons = [
            (GuidanceReason::ProducerFactAbsent, "producer_fact_absent"),
            (
                GuidanceReason::NoBehavioralDiscriminatorDerived,
                "no_behavioral_discriminator_derived",
            ),
            (
                GuidanceReason::ObserverNotStaticallyVisible,
                "observer_not_statically_visible",
            ),
            (
                GuidanceReason::RouteIsInspectionOnly,
                "route_is_inspection_only",
            ),
            (
                GuidanceReason::RouteIsVerificationOnly,
                "route_is_verification_only",
            ),
            (
                GuidanceReason::StaticLimitationBlocksDerivation,
                "static_limitation_blocks_derivation",
            ),
            (GuidanceReason::SnapshotStale, "snapshot_stale"),
            (
                GuidanceReason::CrossLanguageOracleUnresolved,
                "cross_language_oracle_unresolved",
            ),
        ];
        for (reason, expected) in reasons {
            if reason.as_str() != expected {
                return Err(format!("guidance reason token changed: {reason:?}"));
            }
        }

        let recoveries = [
            (GuidanceRecovery::InspectFixSite, "inspect_fix_site"),
            (GuidanceRecovery::RunExplain, "run_explain"),
            (
                GuidanceRecovery::AddObserverThenAssert,
                "add_observer_then_assert",
            ),
            (GuidanceRecovery::RefreshAnalysis, "refresh_analysis"),
            (
                GuidanceRecovery::ReviewExternalOracle,
                "review_external_oracle",
            ),
            (
                GuidanceRecovery::NoRecoveryAvailable,
                "no_recovery_available",
            ),
        ];
        for (recovery, expected) in recoveries {
            if recovery.as_str() != expected {
                return Err(format!("guidance recovery token changed: {recovery:?}"));
            }
        }

        let states = [
            (DiscriminatorState::Present, "present"),
            (DiscriminatorState::NotProduced, "not_produced"),
            (DiscriminatorState::NotApplicable, "not_applicable"),
            (DiscriminatorState::StaticLimitation, "static_limitation"),
            (DiscriminatorState::Stale, "stale"),
        ];
        for (state, expected) in states {
            if state.as_str() != expected {
                return Err(format!("discriminator state token changed: {state:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn concrete_discriminator_round_trips_with_its_producer_basis() -> Result<(), String> {
        let facts = GapRouteGuidanceFacts {
            missing_discriminator: Some("boundary value 0"),
            ..GapRouteGuidanceFacts::default()
        };
        let availability = DiscriminatorAvailability::from_gap_route(facts)?;
        assert_eq!(
            availability,
            DiscriminatorAvailability::Present {
                text: "boundary value 0".to_string(),
                basis: DiscriminatorBasis::ActivationEvidenceFact,
            }
        );

        let expected = availability.view();
        let json = serde_json::to_string(&expected)
            .map_err(|error| format!("serialize discriminator view failed: {error}"))?;
        let actual: DiscriminatorGuidanceView = serde_json::from_str(&json)
            .map_err(|error| format!("parse discriminator view failed: {error}"))?;
        assert_eq!(actual, expected);
        assert_eq!(
            actual.basis,
            Some(DiscriminatorBasis::ActivationEvidenceFact)
        );
        Ok(())
    }

    #[test]
    fn adjacent_prose_cannot_become_a_discriminator() -> Result<(), String> {
        let facts = GapRouteGuidanceFacts {
            missing_discriminator: None,
            assertion_shape: Some("assert the returned value"),
            changed_behavior: Some("the predicate boundary moved"),
            ..GapRouteGuidanceFacts::default()
        };
        let availability = DiscriminatorAvailability::from_gap_route(facts)?;
        assert_eq!(
            availability,
            DiscriminatorAvailability::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            }
        );

        let view = availability.view();
        assert_eq!(view.text, None);
        assert_eq!(view.basis, None);

        let json = serde_json::to_value(&view)
            .map_err(|error| format!("serialize discriminator view failed: {error}"))?;
        assert_eq!(json["text"], serde_json::Value::Null);
        assert_eq!(json["state"], serde_json::json!("not_produced"));
        Ok(())
    }

    #[test]
    fn absent_discriminator_cannot_serialize_as_a_concrete_string() -> Result<(), String> {
        let forged = serde_json::json!({
            "state": "not_produced",
            "text": "assert the returned value",
            "basis": null,
            "reason": "producer_fact_absent",
            "recovery": "inspect_fix_site",
            "static_limit_kind": null
        });
        expect_error(
            serde_json::from_value::<DiscriminatorGuidanceView>(forged),
            "cannot carry text or basis",
        )?;

        let missing_basis = serde_json::json!({
            "state": "present",
            "text": "boundary value 0",
            "basis": null,
            "reason": null,
            "recovery": null,
            "static_limit_kind": null
        });
        expect_error(
            serde_json::from_value::<DiscriminatorGuidanceView>(missing_basis),
            "requires a producer basis",
        )
    }

    #[test]
    fn concrete_assertion_requires_code_plus_producer_basis() -> Result<(), String> {
        let without_basis =
            AssertionGuidance::from_seam_facts(seam_facts(Some("assert_eq!(actual, 0)")))?;
        assert_eq!(
            without_basis,
            AssertionGuidance::Unresolved {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            }
        );
        assert_eq!(without_basis.view().example, None);

        let with_basis = AssertionGuidance::from_seam_facts(SeamAssertionFacts {
            basis: Some(AssertionBasis::SeamRequiredDiscriminator),
            ..seam_facts(Some("assert_eq!(actual, 0)"))
        })?;
        assert_eq!(
            with_basis,
            AssertionGuidance::Concrete {
                kind: AssertionKind::ExactReturnValue,
                example: "assert_eq!(actual, 0)".to_string(),
                basis: AssertionBasis::SeamRequiredDiscriminator,
            }
        );

        let forged = serde_json::json!({
            "state": "concrete",
            "example": "assert_eq!(actual, expected)",
            "kind": null,
            "basis": null,
            "observer_kind": null,
            "reason": null,
            "recovery": null
        });
        expect_error(
            serde_json::from_value::<AssertionGuidanceView>(forged),
            "requires an assertion kind and producer basis",
        )
    }

    #[test]
    fn concrete_guidance_round_trips_with_example_kind_and_basis() -> Result<(), String> {
        // The one case where a packet legitimately emits paste-ready guidance.
        // Everything else in this module is about refusing to; this pins that
        // the permitted path still projects and survives a round trip intact.
        let discriminator = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            missing_discriminator: Some("amount == discount_threshold"),
            ..GapRouteGuidanceFacts::default()
        })?;
        let assertion = AssertionGuidance::from_seam_facts(SeamAssertionFacts {
            basis: Some(AssertionBasis::ObservedValueFact),
            ..seam_facts(Some("assert_eq!(discounted_total(100), 90)"))
        })?;

        let view = assertion.view();
        assert_eq!(view.state, AssertionState::Concrete);
        assert_eq!(
            view.example.as_deref(),
            Some("assert_eq!(discounted_total(100), 90)")
        );
        assert_eq!(view.kind, Some(AssertionKind::ExactReturnValue));
        assert_eq!(view.basis, Some(AssertionBasis::ObservedValueFact));
        assert_eq!(view.observer_kind, None);
        assert_eq!(view.reason, None);
        assert_eq!(view.recovery, None);

        let expected = RepairGuidance::new(&discriminator, &assertion)?;
        let json = serde_json::to_string(&expected)
            .map_err(|error| format!("serialize concrete guidance failed: {error}"))?;
        let actual: RepairGuidance = serde_json::from_str(&json)
            .map_err(|error| format!("parse concrete guidance failed: {error}"))?;
        assert_eq!(actual, expected);
        assert_eq!(actual.semantic_digest()?, expected.semantic_digest()?);
        Ok(())
    }

    #[test]
    fn every_assertion_kind_has_a_distinct_stable_wire_token() -> Result<(), String> {
        let kinds = [
            (AssertionKind::ExactReturnValue, "exact_return_value"),
            (AssertionKind::ExactErrorVariant, "exact_error_variant"),
            (AssertionKind::FieldEquality, "field_equality"),
            (AssertionKind::SideEffectObserver, "side_effect_observer"),
            (AssertionKind::MatchResult, "match_result"),
            (AssertionKind::CallExpectation, "call_expectation"),
        ];
        for (kind, token) in kinds {
            let guidance = AssertionGuidance::from_seam_facts(SeamAssertionFacts {
                kind,
                basis: Some(AssertionBasis::SeamRequiredDiscriminator),
                ..seam_facts(Some("assert_eq!(actual, 0)"))
            })?;
            let json = serde_json::to_value(guidance.view())
                .map_err(|error| format!("serialize assertion view failed: {error}"))?;
            assert_eq!(json["kind"], serde_json::json!(token));
        }
        Ok(())
    }

    #[test]
    fn a_stale_snapshot_suppresses_the_classified_seam_fact_too() -> Result<(), String> {
        // The gap-ledger adapter's staleness precedence is covered elsewhere;
        // the classified-seam adapter has the same rule and must not diverge.
        let fact = MissingDiscriminatorFact {
            value: "boundary value 0".to_string(),
            reason: "no test observes the boundary".to_string(),
            flow_sink: None,
        };
        let stale = DiscriminatorAvailability::from_missing_discriminator_fact(Some(&fact), true)?;
        assert_eq!(
            stale,
            DiscriminatorAvailability::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::RefreshAnalysis,
            }
        );
        assert_eq!(stale.view().text, None);

        let stale_via_gap = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            missing_discriminator: Some("boundary value 0"),
            stale: true,
            ..GapRouteGuidanceFacts::default()
        })?;
        assert_eq!(stale, stale_via_gap);
        Ok(())
    }

    #[test]
    fn non_concrete_assertion_states_serialize_example_as_null() -> Result<(), String> {
        let cases = [
            AssertionGuidance::from_seam_facts(SeamAssertionFacts {
                observer_required: Some(ObserverKind::SideEffectSink),
                ..seam_facts(Some("assert_eq!(actual, expected)"))
            })?,
            AssertionGuidance::from_seam_facts(SeamAssertionFacts {
                fix_site_only: true,
                ..seam_facts(Some("assert_eq!(actual, expected)"))
            })?,
            AssertionGuidance::from_seam_facts(SeamAssertionFacts {
                verification_only: true,
                ..seam_facts(Some("assert_eq!(actual, expected)"))
            })?,
            AssertionGuidance::from_seam_facts(seam_facts(None))?,
            AssertionGuidance::from_seam_facts(SeamAssertionFacts {
                stale: true,
                basis: Some(AssertionBasis::SeamRequiredDiscriminator),
                ..seam_facts(Some("assert_eq!(actual, expected)"))
            })?,
        ];

        let states = cases
            .iter()
            .map(|guidance| guidance.view().state)
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![
                AssertionState::RequiresObserverSetup,
                AssertionState::FixSiteOnly,
                AssertionState::VerificationOnly,
                AssertionState::Unresolved,
                AssertionState::Stale,
            ]
        );

        for guidance in &cases {
            let view = guidance.view();
            let json = serde_json::to_value(&view)
                .map_err(|error| format!("serialize assertion view failed: {error}"))?;
            assert_eq!(json["example"], serde_json::Value::Null);
            assert_eq!(json["kind"], serde_json::Value::Null);
            assert_eq!(json["basis"], serde_json::Value::Null);

            let parsed: AssertionGuidanceView = serde_json::from_value(json)
                .map_err(|error| format!("parse assertion view failed: {error}"))?;
            assert_eq!(parsed, view);
        }
        Ok(())
    }

    #[test]
    fn equivalent_seam_and_gap_record_facts_produce_the_same_state() -> Result<(), String> {
        let fact = MissingDiscriminatorFact {
            value: "boundary value 0".to_string(),
            reason: "no test observes the boundary".to_string(),
            flow_sink: None,
        };
        let from_seam =
            DiscriminatorAvailability::from_missing_discriminator_fact(Some(&fact), false)?;
        let from_gap = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            missing_discriminator: Some("boundary value 0"),
            ..GapRouteGuidanceFacts::default()
        })?;
        assert_eq!(from_seam, from_gap);
        assert_eq!(from_seam.view(), from_gap.view());

        let absent_seam = DiscriminatorAvailability::from_missing_discriminator_fact(None, false)?;
        let absent_gap = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            assertion_shape: Some("assert the returned value"),
            ..GapRouteGuidanceFacts::default()
        })?;
        assert_eq!(absent_seam, absent_gap);
        assert_eq!(absent_seam.view(), absent_gap.view());
        Ok(())
    }

    #[test]
    fn manual_and_serde_projections_cannot_disagree_on_nullability() -> Result<(), String> {
        let availability = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            static_limit: Some(StaticLimitKind::RustMacroReachUnresolved),
            ..GapRouteGuidanceFacts::default()
        })?;
        let view = availability.view();

        // A hand-built producer that names the state but omits the typed limit
        // kind is rejected by the same validation the Serde path runs.
        let hand_built = serde_json::json!({
            "state": "static_limitation",
            "text": null,
            "basis": null,
            "reason": "static_limitation_blocks_derivation",
            "recovery": null,
            "static_limit_kind": null
        });
        expect_error(
            serde_json::from_value::<DiscriminatorGuidanceView>(hand_built),
            "static_limit_kind is present exactly when",
        )?;

        let faithful = serde_json::to_value(&view)
            .map_err(|error| format!("serialize discriminator view failed: {error}"))?;
        let parsed: DiscriminatorGuidanceView = serde_json::from_value(faithful)
            .map_err(|error| format!("parse discriminator view failed: {error}"))?;
        assert_eq!(parsed, view);
        assert_eq!(
            parsed.static_limit_kind,
            Some(StaticLimitKind::RustMacroReachUnresolved)
        );
        Ok(())
    }

    #[test]
    fn stale_state_suppresses_current_concrete_guidance() -> Result<(), String> {
        let stale = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            missing_discriminator: Some("boundary value 0"),
            static_limit: Some(StaticLimitKind::RustMacroReachUnresolved),
            stale: true,
            ..GapRouteGuidanceFacts::default()
        })?;
        assert_eq!(
            stale,
            DiscriminatorAvailability::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::RefreshAnalysis,
            }
        );
        assert_eq!(stale.view().text, None);

        let stale_assertion = AssertionGuidance::from_seam_facts(SeamAssertionFacts {
            stale: true,
            basis: Some(AssertionBasis::SeamRequiredDiscriminator),
            ..seam_facts(Some("assert_eq!(actual, 0)"))
        })?;
        assert_eq!(stale_assertion.view().example, None);

        let concrete = AssertionGuidance::Concrete {
            kind: AssertionKind::ExactReturnValue,
            example: "assert_eq!(actual, 0)".to_string(),
            basis: AssertionBasis::SeamRequiredDiscriminator,
        };
        expect_error(
            RepairGuidance::new(&stale, &concrete),
            "stale discriminator state cannot accompany concrete assertion guidance",
        )
    }

    #[test]
    fn adding_renderer_prose_does_not_change_the_domain_state() -> Result<(), String> {
        let bare = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts::default())?;
        let with_prose = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            assertion_shape: Some("assert the returned value"),
            changed_behavior: Some("the predicate boundary moved"),
            ..GapRouteGuidanceFacts::default()
        })?;
        assert_eq!(bare, with_prose);

        let left = RepairGuidance::new(&bare, &unresolved_assertion())?;
        let right = RepairGuidance::new(&with_prose, &unresolved_assertion())?;
        assert_eq!(left.semantic_digest()?, right.semantic_digest()?);
        Ok(())
    }

    #[test]
    fn inspection_only_routes_are_not_applicable_rather_than_missing() -> Result<(), String> {
        let inspection = DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
            inspection_only: true,
            ..GapRouteGuidanceFacts::default()
        })?;
        assert_eq!(
            inspection,
            DiscriminatorAvailability::NotApplicable {
                reason: GuidanceReason::RouteIsInspectionOnly,
            }
        );
        let view = inspection.view();
        assert_eq!(view.recovery, None);

        let forged = serde_json::json!({
            "state": "not_applicable",
            "text": null,
            "basis": null,
            "reason": "route_is_inspection_only",
            "recovery": "inspect_fix_site",
            "static_limit_kind": null
        });
        expect_error(
            serde_json::from_value::<DiscriminatorGuidanceView>(forged),
            "cannot carry a recovery",
        )
    }

    #[test]
    fn every_state_pair_round_trips_through_the_canonical_dto() -> Result<(), String> {
        let discriminators = [
            DiscriminatorAvailability::Present {
                text: "boundary value 0".to_string(),
                basis: DiscriminatorBasis::SeamRequiredDiscriminator,
            },
            DiscriminatorAvailability::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            },
            DiscriminatorAvailability::NotApplicable {
                reason: GuidanceReason::RouteIsInspectionOnly,
            },
            DiscriminatorAvailability::StaticLimitation {
                kind: StaticLimitKind::CrossLanguageOracleVisibilityUnresolved,
                reason: GuidanceReason::CrossLanguageOracleUnresolved,
            },
            DiscriminatorAvailability::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::RefreshAnalysis,
            },
        ];
        let assertions = [
            AssertionGuidance::RequiresObserverSetup {
                observer_kind: ObserverKind::ExternalLanguageOracle,
                reason: GuidanceReason::ObserverNotStaticallyVisible,
            },
            AssertionGuidance::FixSiteOnly {
                reason: GuidanceReason::RouteIsInspectionOnly,
            },
            AssertionGuidance::VerificationOnly {
                reason: GuidanceReason::RouteIsVerificationOnly,
            },
            unresolved_assertion(),
            AssertionGuidance::Stale {
                reason: GuidanceReason::SnapshotStale,
                refresh: GuidanceRecovery::NoRecoveryAvailable,
            },
        ];

        for discriminator in &discriminators {
            for assertion in &assertions {
                let expected = RepairGuidance::new(discriminator, assertion)?;
                let json = serde_json::to_string(&expected)
                    .map_err(|error| format!("serialize repair guidance failed: {error}"))?;
                let actual: RepairGuidance = serde_json::from_str(&json)
                    .map_err(|error| format!("parse repair guidance failed: {error}"))?;
                assert_eq!(actual, expected);
            }
        }
        Ok(())
    }

    #[test]
    fn distinct_states_have_distinct_semantic_identity() -> Result<(), String> {
        let absent = RepairGuidance::new(
            &DiscriminatorAvailability::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            },
            &unresolved_assertion(),
        )?;
        let limited = RepairGuidance::new(
            &DiscriminatorAvailability::StaticLimitation {
                kind: StaticLimitKind::RustMacroReachUnresolved,
                reason: GuidanceReason::StaticLimitationBlocksDerivation,
            },
            &unresolved_assertion(),
        )?;
        assert_ne!(absent.semantic_digest()?, limited.semantic_digest()?);

        let repeated = RepairGuidance::new(
            &DiscriminatorAvailability::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            },
            &unresolved_assertion(),
        )?;
        assert_eq!(absent.semantic_digest()?, repeated.semantic_digest()?);
        Ok(())
    }

    #[test]
    fn guidance_text_is_bounded_and_non_empty() -> Result<(), String> {
        let too_long = "x".repeat(MAX_REPAIR_GUIDANCE_TEXT_CHARS + 1);
        expect_error(
            DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
                missing_discriminator: Some(too_long.as_str()),
                ..GapRouteGuidanceFacts::default()
            }),
            "maximum",
        )?;
        expect_error(
            DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts {
                missing_discriminator: Some("   "),
                ..GapRouteGuidanceFacts::default()
            }),
            "must not be empty",
        )?;
        expect_error(
            AssertionGuidance::from_seam_facts(SeamAssertionFacts {
                basis: Some(AssertionBasis::ObservedValueFact),
                ..seam_facts(Some(too_long.as_str()))
            }),
            "maximum",
        )
    }

    #[test]
    fn a_state_cannot_borrow_another_states_reason_or_recovery() -> Result<(), String> {
        // A stale snapshot explained as an absent producer fact, routed to a
        // fix site whose evidence is known to be out of date. Presence-only
        // validation accepted this; the consumer got the wrong explanation
        // and the wrong recovery while the contract still reported valid.
        let mismatched_stale = serde_json::json!({
            "state": "stale",
            "text": null,
            "basis": null,
            "reason": "producer_fact_absent",
            "recovery": "inspect_fix_site",
            "static_limit_kind": null
        });
        expect_error(
            serde_json::from_value::<DiscriminatorGuidanceView>(mismatched_stale),
            "cannot carry reason",
        )?;

        let stale_with_wrong_recovery = serde_json::json!({
            "state": "stale",
            "text": null,
            "basis": null,
            "reason": "snapshot_stale",
            "recovery": "inspect_fix_site",
            "static_limit_kind": null
        });
        expect_error(
            serde_json::from_value::<DiscriminatorGuidanceView>(stale_with_wrong_recovery),
            "cannot carry recovery",
        )?;

        let observer_setup_with_stale_reason = serde_json::json!({
            "state": "requires_observer_setup",
            "example": null,
            "kind": null,
            "basis": null,
            "observer_kind": "side_effect_sink",
            "reason": "snapshot_stale",
            "recovery": null
        });
        expect_error(
            serde_json::from_value::<AssertionGuidanceView>(observer_setup_with_stale_reason),
            "cannot carry reason",
        )?;

        let assertion_stale_with_wrong_recovery = serde_json::json!({
            "state": "stale",
            "example": null,
            "kind": null,
            "basis": null,
            "observer_kind": null,
            "reason": "snapshot_stale",
            "recovery": "add_observer_then_assert"
        });
        expect_error(
            serde_json::from_value::<AssertionGuidanceView>(assertion_stale_with_wrong_recovery),
            "cannot carry recovery",
        )?;

        // Every state the adapters actually produce still validates.
        let produced = RepairGuidance::new(
            &DiscriminatorAvailability::from_gap_route(GapRouteGuidanceFacts::default())?,
            &AssertionGuidance::from_seam_facts(seam_facts(None))?,
        )?;
        assert_eq!(
            produced.discriminator.state,
            DiscriminatorState::NotProduced
        );
        Ok(())
    }

    #[test]
    fn deserialization_revalidates_the_versioned_claim() -> Result<(), String> {
        let valid = RepairGuidance::new(
            &DiscriminatorAvailability::NotProduced {
                reason: GuidanceReason::ProducerFactAbsent,
                recovery: GuidanceRecovery::InspectFixSite,
            },
            &unresolved_assertion(),
        )?;
        let mut value = serde_json::to_value(valid)
            .map_err(|error| format!("serialize repair guidance value failed: {error}"))?;
        value["schema_version"] = serde_json::json!("9.9");
        expect_error(
            serde_json::from_value::<RepairGuidance>(value.clone()),
            "schema_version must be",
        )?;

        value["schema_version"] = serde_json::json!(REPAIR_GUIDANCE_SCHEMA_VERSION);
        value["claim_boundary"] = serde_json::json!("Repair correctness established.");
        expect_error(
            serde_json::from_value::<RepairGuidance>(value),
            "claim_boundary does not match",
        )
    }
}
