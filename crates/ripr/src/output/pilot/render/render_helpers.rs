use crate::analysis::ClassifiedSeam;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_for_classified_seam,
    targeted_test_brief_outline_for_classified_seam,
};
use crate::output::json::escape as json_escape;
use crate::output::path::{display_path, display_path_text};
use std::path::Path;

/// Human text for a top seam with no repair target. The typed producer reason
/// (`recommended_test.reason`) stays in the JSON packets; people reading the
/// terminal or the brief get what it means for them instead (#4216 row 3).
/// `ripr agent repair` needs a test in the seam's own package, so a test that
/// reaches the seam from another crate is not a target.
pub(super) const NO_REPAIR_TARGET_FOCUSED_TEST: &str = "none: this seam has no test target that `ripr agent repair` can use (for example, the only tests are in another crate, or static evidence names no exact discriminator), so it will not start a repair attempt here";

/// The hand step that replaces a repair start when the top seam has no
/// repair target: a test in the seam's own crate. Never a pilot rerun, which
/// would only rank the same seam again.
pub(super) fn no_repair_target_hand_step(entry: &ClassifiedSeam) -> String {
    format!(
        "add a test for `{}` in the crate that owns {}",
        entry.seam.owner(),
        display_path(entry.seam.file())
    )
}

pub(super) fn push_top_seam_json(out: &mut String, entry: &ClassifiedSeam) {
    out.push_str("    {\n");
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        json_escape(entry.seam.id().as_str())
    ));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        json_escape(&display_path(entry.seam.file()))
    ));
    out.push_str(&format!("      \"line\": {},\n", entry.seam.display_line()));
    out.push_str(&format!(
        "      \"kind\": \"{}\",\n",
        entry.seam.kind().as_str()
    ));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(entry.seam.owner())
    ));
    out.push_str(&format!(
        "      \"grip_class\": \"{}\",\n",
        entry.class.as_str()
    ));
    out.push_str(&format!(
        "      \"why\": \"{}\",\n",
        json_escape(&why_line(entry))
    ));
    out.push_str("      \"missing_discriminator\": ");
    if let Some(missing) = entry.evidence.missing_discriminators.first() {
        out.push_str(&format!(
            "{{\"value\": \"{}\", \"reason\": \"{}\"}}",
            json_escape(&missing.value),
            json_escape(&missing.reason)
        ));
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    out.push_str(&format!(
        "      \"related_test_present\": {},\n",
        !entry.evidence.related_tests.is_empty()
    ));
    out.push_str(&format!(
        "      \"suggested_assertion_present\": {},\n",
        suggested_assertion_for_classified_seam(entry).is_some()
    ));
    out.push_str(&format!(
        "      \"targeted_test_brief\": \"{}\"\n",
        json_escape(&targeted_test_brief_for_classified_seam(entry))
    ));
    out.push_str("    }");
}

pub(super) fn push_markdown_recommendation(out: &mut String, entry: &ClassifiedSeam) {
    let outline = targeted_test_brief_outline_for_classified_seam(entry);
    out.push_str(&format!(
        "- Inspected seam: `{}` {}:{} `{}` in `{}` ({})\n",
        entry.seam.id().as_str(),
        display_path(entry.seam.file()),
        entry.seam.display_line(),
        entry.seam.kind().as_str(),
        entry.seam.owner(),
        entry.class.human_label()
    ));
    out.push_str(&format!("- Why it matters: {}\n", why_line(entry)));
    if outline.is_not_applicable() {
        out.push_str(&format!(
            "- Focused test: {NO_REPAIR_TARGET_FOCUSED_TEST}\n"
        ));
    } else {
        out.push_str(&format!(
            "- Focused test: add `{}` in `{}`\n",
            outline.suggested_name,
            display_path_text(&outline.suggested_file)
        ));
    }
    if let Some(value) = outline.candidate_value.as_ref() {
        out.push_str(&format!("- Candidate value: `{value}`\n"));
    }
    out.push_str(&format!(
        "- Assertion shape: `{}`\n",
        outline.assertion_shape
    ));
    out.push_str("- Detailed work order:\n\n");
    out.push_str("```text\n");
    out.push_str(&targeted_test_brief_for_classified_seam(entry));
    out.push_str("```\n");
}

pub(super) fn push_path_field(out: &mut String, name: &str, path: &Path, trailing: bool) {
    out.push_str(&format!(
        "    \"{}\": \"{}\"{}\n",
        name,
        json_escape(&display_path(path)),
        if trailing { "," } else { "" }
    ));
}

pub(super) fn why_line(entry: &ClassifiedSeam) -> String {
    if let Some(missing) = entry.evidence.missing_discriminators.first() {
        return format!(
            // #4381: name the value with the shared canonical label rather
            // than a per-surface retype of it.
            "{}: {} ({})",
            crate::output::gap_vocabulary::MISSING_DISCRIMINATOR_LABEL,
            missing.value,
            missing.reason
        );
    }
    let summary = entry.evidence.discriminate.summary.trim();
    if !summary.is_empty() {
        return format!("static discriminator summary: {summary}");
    }
    format!("{} static seam evidence", entry.class.as_str())
}

pub(super) fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
