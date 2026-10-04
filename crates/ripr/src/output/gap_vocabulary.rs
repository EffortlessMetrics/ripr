//! One shared authority for the cross-surface presentation of the shared
//! weak gap state (issue #4381).
//!
//! The same gap state — "related tests reach the change, but no
//! discriminator would notice" — is named by two published schema tokens:
//! `weakly_exposed` (the exposure/probe class, `ripr check`) and
//! `weakly_gripped` (the seam grip class, `ripr pilot`, seam packets, and
//! evidence records). Both tokens are consumed by external contracts —
//! JSON packets, evidence records, SARIF, config `[severity]` keys, and LSP
//! diagnostic codes — so **neither token changes** (the decision recorded in
//! `docs/STATIC_EXPOSURE_MODEL.md`, option B of #4381, consistent with the
//! PR #4520 wording decision). The exposure and grip domains remain two
//! contracts; this module is the only presentation mapping between them
//! and the only home of the shared discriminator phrasing. Per-surface
//! translation forks — a local `"weakly_gripped" => "weakly_exposed"` table
//! in a renderer — are forbidden; `no_per_surface_translation_forks` below
//! is the drift guard.
//!
//! The mapping is intentionally one-directional: grip-to-exposure. The
//! reverse is not one-to-one (several grip classes present as
//! `static_unknown`), so no reverse lookup is offered and surfaces must not
//! invent one.

/// The canonical sentence consumer-facing surfaces quote when they describe
/// the shared missing-discriminator *state*. `ripr check` renders it as the
/// `WeaklyExposed` classification hint and `first_useful_action` quotes it in
/// the repair-start rationale, so an agent matching on phrases sees one
/// sentence, not a per-surface variant. Surfaces that name the specific
/// missing value (for example the pilot `why` line) use
/// [`MISSING_DISCRIMINATOR_LABEL`] instead; they do not rephrase the state.
pub(crate) const MISSING_DISCRIMINATOR_SENTENCE: &str =
    "a related test reaches this change but does not observe the exact changed value";

/// The canonical label for the named missing discriminator value, as
/// opposed to the state sentence above.
pub(crate) const MISSING_DISCRIMINATOR_LABEL: &str = "missing discriminator";

/// The grip-to-exposure presentation mapping — the single translation point
/// between the two vocabularies. Returns `None` for anything that is not a
/// published grip class token, so a caller can never silently invent a
/// counterpart.
pub(crate) fn exposure_counterpart(grip_class: &str) -> Option<&'static str> {
    match grip_class {
        "strongly_gripped" => Some("exposed"),
        "weakly_gripped" => Some("weakly_exposed"),
        "reachable_unrevealed" => Some("reachable_unrevealed"),
        "ungripped" => Some("no_static_path"),
        "infection_unknown" => Some("infection_unknown"),
        "propagation_unknown" => Some("propagation_unknown"),
        "static_unknown" => Some("static_unknown"),
        _ => None,
    }
}

/// The published exposure vocabulary tokens (the `ExposureClass` contract).
const EXPOSURE_TOKENS: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];

/// Resolve any class token — grip or exposure — to its exposure vocabulary
/// token. Exposure tokens resolve as themselves; grip tokens map through
/// [`exposure_counterpart`]; anything else fails closed with `None`. This is
/// the mixed-input contract the front panel consumes: its inputs come from
/// both domains.
pub(crate) fn exposure_class_of(class: &str) -> Option<&'static str> {
    if EXPOSURE_TOKENS.contains(&class) {
        return Some(match class {
            "exposed" => "exposed",
            "weakly_exposed" => "weakly_exposed",
            "reachable_unrevealed" => "reachable_unrevealed",
            "no_static_path" => "no_static_path",
            "infection_unknown" => "infection_unknown",
            "propagation_unknown" => "propagation_unknown",
            _ => "static_unknown",
        });
    }
    exposure_counterpart(class)
}

/// Present any class token — grip or exposure — as the exposure vocabulary
/// token a cross-surface consumer should read. Unknown tokens pass through
/// unchanged so an upstream spelling is never silently dropped.
pub(crate) fn present_exposure_class(class: &str) -> String {
    exposure_class_of(class)
        .map(str::to_string)
        .unwrap_or_else(|| class.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        MISSING_DISCRIMINATOR_LABEL, MISSING_DISCRIMINATOR_SENTENCE, exposure_counterpart,
        present_exposure_class,
    };
    use crate::analysis::seams::SeamGripClass;

    /// The headline mapping this claim exists for: one shared weak state,
    /// `weakly_gripped` on the grip side, `weakly_exposed` on the exposure
    /// side, one translation point.
    #[test]
    fn weak_counterpart_is_weakly_exposed() -> Result<(), String> {
        let Some(counterpart) = exposure_counterpart("weakly_gripped") else {
            return Err("weakly_gripped lost its exposure counterpart".to_string());
        };
        if counterpart != "weakly_exposed" {
            return Err(format!("weak counterpart drifted: {counterpart}"));
        }
        if present_exposure_class("weakly_gripped") != "weakly_exposed" {
            return Err("present_exposure_class did not map the weak grip token".to_string());
        }
        Ok(())
    }

    /// The full published table, pinned pair by pair, so a drive-by edit of
    /// one arm fails a focused test instead of drifting another surface.
    #[test]
    fn full_counterpart_table_is_pinned() -> Result<(), String> {
        let expected: [(&str, &str); 7] = [
            ("strongly_gripped", "exposed"),
            ("weakly_gripped", "weakly_exposed"),
            ("reachable_unrevealed", "reachable_unrevealed"),
            ("ungripped", "no_static_path"),
            ("infection_unknown", "infection_unknown"),
            ("propagation_unknown", "propagation_unknown"),
            ("static_unknown", "static_unknown"),
        ];
        for (grip, exposure) in expected {
            if exposure_counterpart(grip) != Some(exposure) {
                return Err(format!("counterpart for {grip} drifted"));
            }
        }
        // Grip classes with no exposure presentation must fail closed.
        for grip in [
            "activation_unknown",
            "observation_unknown",
            "discrimination_unknown",
            "opaque",
            "intentional",
            "suppressed",
            "not_a_class",
        ] {
            if exposure_counterpart(grip).is_some() {
                return Err(format!("{grip} must not invent an exposure counterpart"));
            }
        }
        Ok(())
    }

    /// Every grip class token either maps through the shared table or fails
    /// closed; none may be silently re-spelled.
    #[test]
    fn every_grip_class_resolves_or_fails_closed() -> Result<(), String> {
        for grip in SeamGripClass::ALL {
            let token = grip.as_str();
            match exposure_counterpart(token) {
                Some(exposure) => {
                    if present_exposure_class(token) != exposure {
                        return Err(format!("{token} did not present as {exposure}"));
                    }
                }
                None => {
                    if present_exposure_class(token) != token {
                        return Err(format!(
                            "{token} without a counterpart must pass through unchanged"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Exposure tokens pass through the presentation helper unchanged.
    #[test]
    fn exposure_tokens_pass_through() -> Result<(), String> {
        for exposure in [
            "exposed",
            "weakly_exposed",
            "reachable_unrevealed",
            "no_static_path",
            "infection_unknown",
            "propagation_unknown",
            "static_unknown",
        ] {
            if present_exposure_class(exposure) != exposure {
                return Err(format!("exposure token {exposure} did not pass through"));
            }
        }
        Ok(())
    }

    /// The mixed-input resolver — the front panel's contract: inputs may
    /// arrive from either domain, and both spellings of the shared weak
    /// state resolve to the one exposure token.
    #[test]
    fn mixed_input_resolves_to_exposure_vocabulary() -> Result<(), String> {
        use super::exposure_class_of;
        if exposure_class_of("weakly_gripped") != Some("weakly_exposed") {
            return Err("grip spelling of the weak state did not resolve".to_string());
        }
        if exposure_class_of("weakly_exposed") != Some("weakly_exposed") {
            return Err("exposure spelling of the weak state did not resolve".to_string());
        }
        if exposure_class_of("strongly_gripped") != Some("exposed")
            || exposure_class_of("exposed") != Some("exposed")
        {
            return Err("strong spellings drifted".to_string());
        }
        if exposure_class_of("not_a_class").is_some() {
            return Err("unknown tokens must fail closed".to_string());
        }
        Ok(())
    }

    /// #4381 acceptance 2: the translation fork audit. No file under
    /// `src/output` outside this module may carry a grip-to-exposure
    /// translation arm; the front panel's former local table is the pinned
    /// example of what must not come back.
    #[test]
    fn no_per_surface_translation_forks() -> Result<(), String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/output");
        let mut offenders: Vec<String> = Vec::new();
        collect_forks(&root, &mut offenders)
            .map_err(|error| format!("audit could not read the output tree: {error}"))?;
        if offenders.is_empty() {
            return Ok(());
        }
        Err(format!(
            "per-surface translation forks found (route them through gap_vocabulary): {offenders:?}"
        ))
    }

    fn collect_forks(
        dir: &std::path::Path,
        offenders: &mut Vec<String>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_forks(&path, offenders)?;
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            if path
                .file_name()
                .is_some_and(|name| name == "gap_vocabulary.rs")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path)?;
            if has_translation_fork(&source) {
                offenders.push(path.display().to_string());
            }
        }
        Ok(())
    }

    /// A translation fork is source that maps one published grip token onto
    /// the other vocabulary's exposure token: a match arm (`"weakly_gripped"
    /// => "weakly_exposed"`), a pair-table entry (`("weakly_gripped",
    /// "weakly_exposed")`), or an if-chain guard (`== "weakly_gripped"`).
    /// Ranking tables (`"weakly_gripped" => 5`) and grip-domain data are not
    /// forks and stay outside the audit. The guard is intentionally literal:
    /// it pins the fork shapes that existed or plausibly recur; an exotic
    /// encoding (a constructed table built token-by-token) is review
    /// responsibility, not something a substring audit can prove absent.
    fn has_translation_fork(source: &str) -> bool {
        [
            "\"weakly_gripped\" => \"",
            "\"strongly_gripped\" => \"",
            "\"weakly_gripped\" => Some(\"",
            "\"strongly_gripped\" => Some(\"",
            "\"weakly_gripped\" | \"weakly_exposed\" =>",
            "\"strongly_gripped\" | \"exposed\" =>",
            "(\"weakly_gripped\", \"",
            "(\"strongly_gripped\", \"",
            "== \"weakly_gripped\"",
            "== \"strongly_gripped\"",
        ]
        .iter()
        .any(|pattern| source.contains(pattern))
    }

    /// Phrasing drift guard: the canonical state sentence lives only in this
    /// module; surfaces must quote the constant, never re-type the text.
    #[test]
    fn shared_sentence_is_quoted_not_retyped() -> Result<(), String> {
        if MISSING_DISCRIMINATOR_SENTENCE.is_empty() {
            return Err("the shared sentence must not be empty".to_string());
        }
        if !MISSING_DISCRIMINATOR_SENTENCE.contains("discriminator")
            && !MISSING_DISCRIMINATOR_SENTENCE.contains("observe")
        {
            return Err("the shared sentence lost its state wording".to_string());
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/output");
        let mut retyped: Vec<String> = Vec::new();
        collect_retyped(&root, &mut retyped)
            .map_err(|error| format!("audit could not read the output tree: {error}"))?;
        if retyped.is_empty() {
            return Ok(());
        }
        Err(format!(
            "surfaces re-typed the shared sentence (quote MISSING_DISCRIMINATOR_SENTENCE instead): {retyped:?}"
        ))
    }

    fn collect_retyped(
        dir: &std::path::Path,
        retyped: &mut Vec<String>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_retyped(&path, retyped)?;
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            if path
                .file_name()
                .is_some_and(|name| name == "gap_vocabulary.rs")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path)?;
            if source.contains("does not observe the exact changed value") {
                retyped.push(path.display().to_string());
            }
        }
        Ok(())
    }

    /// The canonical label names the value, not the state; keep it pinned so
    /// "lacks a discriminator" style variants cannot creep back in.
    #[test]
    fn missing_discriminator_label_is_pinned() -> Result<(), String> {
        if MISSING_DISCRIMINATOR_LABEL != "missing discriminator" {
            return Err("the missing-discriminator label drifted".to_string());
        }
        Ok(())
    }
}
