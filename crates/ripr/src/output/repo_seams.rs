//! Render the repo seam inventory as JSON or Markdown.
//!
//! Schema is documented in `docs/OUTPUT_SCHEMA.md` under
//! `repo-seams.json`. Bumping the JSON shape requires bumping
//! `REPO_SEAMS_SCHEMA_VERSION` and updating that doc and the gold fixtures
//! in lockstep.

use crate::analysis::{RepoSeam, RequiredDiscriminator};
use crate::output::json::escape as json_escape;

pub(crate) const REPO_SEAMS_SCHEMA_VERSION: &str = "0.1";

pub(crate) fn render_repo_seams_json(seams: &[RepoSeam]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        REPO_SEAMS_SCHEMA_VERSION
    ));
    out.push_str("  \"scope\": \"repo\",\n");
    out.push_str("  \"seams\": [");

    for (idx, seam) in seams.iter().enumerate() {
        if idx == 0 {
            out.push('\n');
        }
        push_seam_json(&mut out, seam);
        if idx + 1 != seams.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    if !seams.is_empty() {
        out.push_str("  ");
    }
    out.push_str("]\n");
    out.push_str("}\n");
    out
}

fn push_seam_json(out: &mut String, seam: &RepoSeam) {
    out.push_str("    {\n");
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        json_escape(seam.id().as_str())
    ));
    out.push_str(&format!("      \"kind\": \"{}\",\n", seam.kind().as_str()));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        json_escape(&seam.file().to_string_lossy())
    ));
    out.push_str(&format!("      \"line\": {},\n", seam.display_line()));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(seam.owner())
    ));
    out.push_str(&format!(
        "      \"expression\": \"{}\",\n",
        json_escape(seam.expression())
    ));
    out.push_str("      \"required_discriminator\": {\n");
    out.push_str(&format!(
        "        \"kind\": \"{}\",\n",
        seam.required_discriminator().as_str()
    ));
    out.push_str(&format!(
        "        \"description\": \"{}\"\n",
        json_escape(discriminator_payload(seam.required_discriminator()))
    ));
    out.push_str("      },\n");
    out.push_str("      \"expected_sink\": {\n");
    out.push_str(&format!(
        "        \"kind\": \"{}\"\n",
        seam.expected_sink().as_str()
    ));
    out.push_str("      }\n");
    out.push_str("    }");
}

fn discriminator_payload(d: &RequiredDiscriminator) -> &str {
    match d {
        RequiredDiscriminator::BoundaryValue { description } => description,
        RequiredDiscriminator::ErrorVariant { variant } => variant,
        RequiredDiscriminator::ReturnValue { description } => description,
        RequiredDiscriminator::FieldValue { field } => field,
        RequiredDiscriminator::Effect { sink } => sink,
        RequiredDiscriminator::MatchArmTaken { arm } => arm,
        RequiredDiscriminator::CallSite { target } => target,
    }
}

pub(crate) fn render_repo_seams_md(seams: &[RepoSeam]) -> String {
    let mut out = String::new();
    out.push_str("# Repo Seam Inventory\n\n");
    out.push_str(&format!("Schema version: {}\n", REPO_SEAMS_SCHEMA_VERSION));
    out.push_str("Scope: repo\n");
    out.push_str(&format!("Total seams: {}\n\n", seams.len()));

    if seams.is_empty() {
        out.push_str(
            "No production seams currently inventoried. \
             Diff-scoped findings remain available via `ripr check`.\n",
        );
        return out;
    }

    out.push_str("| Seam ID | File | Line | Owner | Kind | Expected sink | Expression |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for seam in seams {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | {} | {} | `{}` |\n",
            seam.id().as_str(),
            md_escape(&seam.file().to_string_lossy()),
            seam.display_line(),
            md_escape(seam.owner()),
            seam.kind().as_str(),
            seam.expected_sink().as_str(),
            md_escape_inline_code(seam.expression()),
        ));
    }

    out.push_str(
        "\nThis repo seam inventory does not classify test grip yet — \
        `analysis/repo-ripr-classification-v1` adds `SeamGripClass`. \
        Static-language constraints from RIPR-SPEC-0005 still apply.\n",
    );
    out
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn md_escape_inline_code(value: &str) -> String {
    // Replace backticks (would close the inline code span) and pipe
    // characters (would split the table cell) with safe equivalents.
    value
        .replace('`', "\u{2018}")
        .replace('|', "\\|")
        .replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};

    fn sample_seam() -> RepoSeam {
        RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            1234,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount == discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    #[test]
    fn json_carries_schema_version_and_repo_scope() {
        let json = render_repo_seams_json(&[sample_seam()]);
        assert!(
            json.contains("\"schema_version\": \"0.1\""),
            "missing schema_version: {json}"
        );
        assert!(
            json.contains("\"scope\": \"repo\""),
            "missing scope: {json}"
        );
    }

    #[test]
    fn json_carries_full_seam_record() {
        let json = render_repo_seams_json(&[sample_seam()]);
        for needle in [
            "\"seam_id\":",
            "\"kind\": \"predicate_boundary\"",
            "\"file\": \"src/pricing.rs\"",
            "\"line\": 88",
            "\"owner\": \"pricing::discounted_total\"",
            "\"expression\": \"amount >= discount_threshold\"",
            "\"required_discriminator\":",
            "\"kind\": \"boundary_value\"",
            "\"description\": \"amount == discount_threshold\"",
            "\"expected_sink\":",
            "\"kind\": \"return_value\"",
        ] {
            assert!(json.contains(needle), "missing {needle:?} in: {json}");
        }
    }

    #[test]
    fn json_emits_empty_array_when_no_seams() {
        let json = render_repo_seams_json(&[]);
        assert!(json.contains("\"seams\": []"), "got: {json}");
    }

    #[test]
    fn markdown_renders_table_when_seams_exist() {
        let md = render_repo_seams_md(&[sample_seam()]);
        assert!(md.contains("# Repo Seam Inventory"));
        assert!(md.contains("| Seam ID | File | Line |"));
        assert!(md.contains("predicate_boundary"));
        assert!(md.contains("pricing::discounted_total"));
    }

    #[test]
    fn markdown_explains_when_inventory_is_empty() {
        let md = render_repo_seams_md(&[]);
        assert!(md.contains("Total seams: 0"));
        assert!(md.contains("No production seams"));
    }

    #[test]
    fn markdown_uses_static_exposure_vocabulary() {
        // The check-static-language xtask gate enforces forbidden-token
        // absence repo-wide. This test pins that the renderer's boilerplate
        // uses the approved seam evidence vocabulary so a future edit
        // cannot regress the wording without breaking a unit test too.
        let md = render_repo_seams_md(&[sample_seam()]);
        assert!(
            md.contains("This repo seam inventory does not classify test grip"),
            "boilerplate footer drift: {md}"
        );
        assert!(
            md.contains("Static-language constraints from RIPR-SPEC-0005"),
            "boilerplate footer drift: {md}"
        );
    }
}
