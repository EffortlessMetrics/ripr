use super::{sample_finding, selector_matches_location};
use crate::analysis::AnalysisMode;
use crate::app::Mode;

#[test]
fn mode_labels_match_public_contract() {
    assert_eq!(Mode::Instant.as_str(), "instant");
    assert_eq!(Mode::Draft.as_str(), "draft");
    assert_eq!(Mode::Fast.as_str(), "fast");
    assert_eq!(Mode::Deep.as_str(), "deep");
    assert_eq!(Mode::Ready.as_str(), "ready");
}

#[test]
fn mode_maps_to_internal_profiles() {
    assert_eq!(Mode::Instant.analysis_mode(), AnalysisMode::Instant);
    assert_eq!(Mode::Draft.analysis_mode(), AnalysisMode::Draft);
    assert_eq!(Mode::Fast.analysis_mode(), AnalysisMode::Fast);
    assert_eq!(Mode::Deep.analysis_mode(), AnalysisMode::Deep);
    assert_eq!(Mode::Ready.analysis_mode(), AnalysisMode::Ready);
}

#[test]
fn selector_matches_exact_and_suffix_file_locations() {
    let finding = sample_finding("src/lib.rs", 42);

    assert!(selector_matches_location("src/lib.rs:42", &finding));
    assert!(selector_matches_location(
        "crates/ripr/src/lib.rs:42",
        &finding
    ));
    assert!(selector_matches_location(
        "crates\\ripr\\src/lib.rs:42",
        &finding
    ));
    assert!(!selector_matches_location("src/lib.rs:41", &finding));
    assert!(!selector_matches_location("src/main.rs:42", &finding));
}

#[test]
fn selector_rejects_location_lookalikes_that_only_contain_file_text() {
    let finding = sample_finding("src/lib.rs", 42);

    assert!(!selector_matches_location("src/lib.rs.bak:42", &finding));
    assert!(!selector_matches_location(
        "generated-src/lib.rs:42",
        &finding
    ));
    assert!(!selector_matches_location("src/lib.rs", &finding));
    assert!(!selector_matches_location("src/lib.rs:042", &finding));
}
