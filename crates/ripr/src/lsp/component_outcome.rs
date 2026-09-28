//! Typed bounded component-outcome records for the LSP analysis pipeline
//! (#1997, RIPR-SPEC-0141).
//!
//! Every optional analysis component that can fail or degrade independently
//! of the diff analysis records one [`ComponentOutcome`] on the committed
//! [`super::state::AnalysisSnapshot`]. The snapshot is the single typed
//! authority: diagnostics severity policy, hover, code actions, standard
//! work-done progress, `ripr/analysisStatus`, and workspace status all read
//! the same records, so no degradation is reported only through process
//! stderr (the pre-#1997 hidden-stderr fallback).
//!
//! Vocabulary rules:
//!
//! - component states are the closed set `complete | unavailable | limited |
//!   failed | cancelled | deferred`; a degraded component (`limited` or
//!   `failed`) makes the shared run status `limited` via
//!   [`super::diagnostics::derive_run_status`] — never a silent `full`;
//! - messages are normalized, path-redacted, and size-bounded by
//!   [`bounded_message`] before they can reach any client surface;
//! - `findings_trustworthy` states explicitly whether ordinary diff-scoped
//!   findings remain usable evidence under the degradation; nothing about a
//!   degraded optional component downgrades the findings themselves.

/// One optional analysis component whose outcome is recorded per refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AnalysisComponent {
    Diff,
    SeamInventory,
    GapLedger,
    CausalProjection,
    Cache,
    // NOTE (RIPR-SPEC-0141): `report` is deferred spec vocabulary and
    // intentionally absent from the code until a real producer exists —
    // out-of-scope projection suppression is disclosed typed via
    // `out_of_scope_test_file_findings`, not a fabricated report outcome.
}

impl AnalysisComponent {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Diff => "diff",
            Self::SeamInventory => "seam_inventory",
            Self::GapLedger => "gap_ledger",
            Self::CausalProjection => "causal_projection",
            Self::Cache => "cache",
        }
    }
}

/// Closed outcome-state vocabulary for one component (RIPR-SPEC-0141).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ComponentState {
    Complete,
    Unavailable,
    Limited,
    Failed,
    // NOTE (RIPR-SPEC-0141): `cancelled` is deferred spec vocabulary and
    // intentionally absent from the code until a real producer exists — a
    // cancelled attempt never commits a snapshot, so no producer constructs
    // this state today.
    Deferred,
}

impl ComponentState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Unavailable => "unavailable",
            Self::Limited => "limited",
            Self::Failed => "failed",
            Self::Deferred => "deferred",
        }
    }
}

/// The typed, bounded record of one component's outcome for one snapshot.
#[derive(Clone, Debug)]
pub(super) struct ComponentOutcome {
    pub(super) component: AnalysisComponent,
    pub(super) state: ComponentState,
    /// Stable machine-readable code for the specific outcome (for example
    /// `seam_inventory_failed`, `stale_artifact`). `None` for a plain
    /// `complete` outcome.
    pub(super) kind: Option<&'static str>,
    /// Bounded, path-redacted, single-line detail. `None` when the outcome
    /// needs no explanation beyond `component`/`state`/`kind`.
    pub(super) message: Option<String>,
    /// Whether ordinary diff-scoped findings remain trustworthy evidence
    /// under this outcome. Recorded explicitly so no consumer has to guess.
    pub(super) findings_trustworthy: bool,
    /// The safe recovery action a client may name to the user.
    pub(super) recovery: Option<&'static str>,
}

impl ComponentOutcome {
    pub(super) fn complete(component: AnalysisComponent) -> Self {
        Self {
            component,
            state: ComponentState::Complete,
            kind: None,
            message: None,
            findings_trustworthy: true,
            recovery: None,
        }
    }

    /// The component did not run and that is a normal, non-degraded state
    /// (for example seam diagnostics disabled by configuration).
    pub(super) fn unavailable(component: AnalysisComponent, kind: &'static str) -> Self {
        Self {
            component,
            state: ComponentState::Unavailable,
            kind: Some(kind),
            message: None,
            findings_trustworthy: true,
            recovery: None,
        }
    }

    /// The component was intentionally deferred to a later, explicit run
    /// (RIPR-SPEC-0105 seams deferral). Deferred is disclosed but not a
    /// degradation: it must not warn on every interactive refresh.
    pub(super) fn deferred(
        component: AnalysisComponent,
        kind: &'static str,
        recovery: &'static str,
    ) -> Self {
        Self {
            component,
            state: ComponentState::Deferred,
            kind: Some(kind),
            message: None,
            findings_trustworthy: true,
            recovery: Some(recovery),
        }
    }

    /// The component produced partial evidence; ordinary findings remain
    /// trustworthy unless the caller says otherwise.
    pub(super) fn limited(
        component: AnalysisComponent,
        kind: &'static str,
        message: impl Into<String>,
        findings_trustworthy: bool,
        recovery: &'static str,
    ) -> Self {
        Self {
            component,
            state: ComponentState::Limited,
            kind: Some(kind),
            message: Some(bounded_message(&message.into())),
            findings_trustworthy,
            recovery: Some(recovery),
        }
    }

    /// The component failed; ordinary findings remain trustworthy unless the
    /// caller says otherwise. The message is bounded and path-redacted at
    /// construction so no surface can leak unbounded error text.
    pub(super) fn failed(
        component: AnalysisComponent,
        kind: &'static str,
        message: impl Into<String>,
        findings_trustworthy: bool,
        recovery: &'static str,
    ) -> Self {
        Self {
            component,
            state: ComponentState::Failed,
            kind: Some(kind),
            message: Some(bounded_message(&message.into())),
            findings_trustworthy,
            recovery: Some(recovery),
        }
    }

    /// True when the component degraded the run (`limited` or `failed`).
    /// `unavailable`, `deferred`, `cancelled`, and `complete` are disclosed
    /// states, not degradations.
    pub(super) fn is_degraded(&self) -> bool {
        matches!(self.state, ComponentState::Limited | ComponentState::Failed)
    }

    /// One concise, single-line client-facing summary of the degraded
    /// outcome, naming the recovery route without overclaiming.
    pub(super) fn log_summary(&self) -> String {
        let kind = self.kind.unwrap_or(self.state.as_str());
        let detail = self
            .message
            .as_deref()
            .map(|message| format!(": {message}"))
            .unwrap_or_default();
        let recovery = self
            .recovery
            .map(|recovery| format!(" — recovery: {recovery}"))
            .unwrap_or_default();
        format!(
            "{} {} ({kind}){detail}{recovery}",
            self.component.as_str(),
            self.state.as_str()
        )
    }

    /// The typed payload exposed through `ripr/analysisStatus` (and embedded
    /// in workspace status). `snapshot_identity` binds the record to the
    /// snapshot/input identity that produced it.
    pub(super) fn status_payload(&self, snapshot_identity: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "component": self.component.as_str(),
            "state": self.state.as_str(),
            "kind": self.kind,
            "message": self.message,
            "findings_trustworthy": self.findings_trustworthy,
            "recovery": self.recovery,
            "snapshot_identity": snapshot_identity,
        })
    }

    /// The stable dedup fragment for this outcome: identical degradations on
    /// consecutive refreshes produce an identical fragment, which the backend
    /// uses to emit at most one warning per distinct degradation (#1997).
    /// The bounded message is part of the fragment: a NEW failure detail
    /// under the same kind is a different degradation, not a duplicate — its
    /// updated warning must be sent. Messages are already normalized,
    /// path-redacted, and bounded at construction, so a repeated failure
    /// repeats the same fragment.
    fn degradation_fragment(&self) -> Option<String> {
        if !self.is_degraded() {
            return None;
        }
        Some(format!(
            "{}={}:{}:{}",
            self.component.as_str(),
            self.state.as_str(),
            self.kind.unwrap_or("-"),
            self.message.as_deref().unwrap_or("-")
        ))
    }
}

/// The degradation signature for one snapshot's component outcomes: `None`
/// when nothing is degraded, otherwise a deterministic string over the
/// sorted degraded fragments. Byte-identical repeated degradations share one
/// signature, so the backend warns once per distinct degradation and logs one
/// recovery line when the signature clears.
pub(super) fn degradation_signature(outcomes: &[ComponentOutcome]) -> Option<String> {
    let mut fragments = outcomes
        .iter()
        .filter_map(ComponentOutcome::degradation_fragment)
        .collect::<Vec<_>>();
    if fragments.is_empty() {
        return None;
    }
    fragments.sort();
    Some(fragments.join("|"))
}

/// The concise warning text for one distinct degradation signature, naming
/// each degraded component and its recovery route.
pub(super) fn degradation_log_message(outcomes: &[ComponentOutcome]) -> Option<String> {
    let degraded = outcomes
        .iter()
        .filter(|outcome| outcome.is_degraded())
        .map(ComponentOutcome::log_summary)
        .collect::<Vec<_>>();
    if degraded.is_empty() {
        return None;
    }
    Some(format!("ripr analysis limited: {}", degraded.join("; ")))
}

/// Normalize, path-redact, and size-bound one free-text detail string before
/// it can reach a client surface. Single source for the LSP path: the
/// backend's health-failure messages delegate here so error text is governed
/// once (#1997).
pub(super) fn bounded_message(message: &str) -> String {
    let normalized = message
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            character => character,
        })
        .collect::<String>();
    let path_safe = normalized
        .split_whitespace()
        .map(|token| {
            if token.starts_with('/')
                || token.starts_with('\\')
                || token
                    .as_bytes()
                    .get(1)
                    .is_some_and(|character| *character == b':')
            {
                "<path>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut bounded = path_safe.chars().take(240).collect::<String>();
    if path_safe.chars().count() > 240 {
        bounded.push('…');
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::{AnalysisComponent, ComponentOutcome, ComponentState, degradation_signature};

    #[test]
    fn degraded_states_are_exactly_limited_and_failed() {
        let mut degraded = ComponentOutcome::complete(AnalysisComponent::Diff);
        assert!(!degraded.is_degraded());
        degraded = ComponentOutcome::unavailable(AnalysisComponent::SeamInventory, "disabled");
        assert!(!degraded.is_degraded());
        degraded =
            ComponentOutcome::deferred(AnalysisComponent::SeamInventory, "defer", "retry refresh");
        assert!(!degraded.is_degraded());
        degraded = ComponentOutcome::limited(
            AnalysisComponent::Cache,
            "stale_artifact",
            "stale",
            true,
            "run ripr check",
        );
        assert!(degraded.is_degraded());
        degraded = ComponentOutcome::failed(
            AnalysisComponent::GapLedger,
            "gap_ledger_parse_failed",
            "parse failed",
            true,
            "run ripr check",
        );
        assert!(degraded.is_degraded());
    }

    #[test]
    fn failed_outcome_bounds_and_redacts_message() -> Result<(), String> {
        let long = format!(
            "read /workspace/example/ledger.json failed: {}",
            "x".repeat(400)
        );
        let outcome = ComponentOutcome::failed(
            AnalysisComponent::GapLedger,
            "gap_ledger_read_failed",
            long,
            true,
            "run ripr check to regenerate the gap decision ledger",
        );
        let message = outcome
            .message
            .as_deref()
            .ok_or_else(|| "failed outcome must carry a message".to_string())?;
        if message.contains("/home/user") {
            return Err(format!("message leaked an absolute path: {message}"));
        }
        if message.chars().count() > 241 {
            return Err(format!("message exceeded the bound: {message}"));
        }
        if !message.contains("<path>") {
            return Err(format!("message must redact the path token: {message}"));
        }
        Ok(())
    }

    #[test]
    fn status_payload_names_component_state_kind_and_recovery() -> Result<(), String> {
        let outcome = ComponentOutcome::failed(
            AnalysisComponent::CausalProjection,
            "causal_projection_unusable",
            "parse canonical delta failed",
            true,
            "run ripr check to regenerate the causal delta artifact",
        );
        let payload = outcome.status_payload(Some("snapshot:7"));
        if payload["component"].as_str() != Some("causal_projection")
            || payload["state"].as_str() != Some("failed")
            || payload["kind"].as_str() != Some("causal_projection_unusable")
            || payload["findings_trustworthy"].as_bool() != Some(true)
            || payload["recovery"].as_str()
                != Some("run ripr check to regenerate the causal delta artifact")
            || payload["snapshot_identity"].as_str() != Some("snapshot:7")
        {
            return Err(format!("unexpected status payload: {payload}"));
        }
        Ok(())
    }

    #[test]
    fn degradation_signature_is_none_without_degradation_and_stable_when_degraded() {
        let clean = vec![
            ComponentOutcome::complete(AnalysisComponent::Diff),
            ComponentOutcome::deferred(
                AnalysisComponent::SeamInventory,
                "interactive_refresh_deferral",
                "run ripr.refreshDiagnostics for the full seam inventory",
            ),
        ];
        assert_eq!(degradation_signature(&clean), None);

        let degraded_a = vec![
            ComponentOutcome::complete(AnalysisComponent::Diff),
            ComponentOutcome::failed(
                AnalysisComponent::SeamInventory,
                "seam_inventory_failed",
                "walk failed",
                true,
                "retry ripr.refreshDiagnostics",
            ),
        ];
        let mut degraded_b = degraded_a.clone();
        degraded_b.reverse();
        let signature_a = degradation_signature(&degraded_a);
        assert_eq!(signature_a, degradation_signature(&degraded_b));
        assert_eq!(
            signature_a.as_deref(),
            Some("seam_inventory=failed:seam_inventory_failed:walk failed")
        );

        // A new failure detail under the same component/state/kind is a
        // different degradation, not a duplicate (#1997 review): its updated
        // warning must be sent.
        let degraded_new_detail = vec![
            ComponentOutcome::complete(AnalysisComponent::Diff),
            ComponentOutcome::failed(
                AnalysisComponent::SeamInventory,
                "seam_inventory_failed",
                "walk timed out",
                true,
                "retry ripr.refreshDiagnostics",
            ),
        ];
        assert_ne!(
            signature_a,
            degradation_signature(&degraded_new_detail),
            "a changed failure detail must change the dedup signature"
        );
    }

    #[test]
    fn degradation_log_message_names_component_and_recovery() -> Result<(), String> {
        let outcomes = vec![ComponentOutcome::failed(
            AnalysisComponent::SeamInventory,
            "seam_inventory_failed",
            "walk failed",
            true,
            "retry ripr.refreshDiagnostics",
        )];
        let message = super::degradation_log_message(&outcomes)
            .ok_or_else(|| "degraded outcomes must produce a log message".to_string())?;
        if !message.contains("seam_inventory failed")
            || !message.contains("retry ripr.refreshDiagnostics")
        {
            return Err(format!(
                "log message must name component and recovery: {message}"
            ));
        }
        Ok(())
    }

    #[test]
    fn deferred_and_unavailable_states_have_distinct_strings() {
        assert_ne!(
            ComponentState::Deferred.as_str(),
            ComponentState::Unavailable.as_str()
        );
        assert_ne!(
            ComponentState::Failed.as_str(),
            ComponentState::Limited.as_str()
        );
    }
}
