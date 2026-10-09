//! Render the repo seam inventory as JSON or Markdown.
//!
//! Schema is documented in `docs/OUTPUT_SCHEMA.md` under
//! `repo-seams.json`. Bumping the JSON shape requires bumping
//! `REPO_SEAMS_SCHEMA_VERSION` and updating that doc and the gold fixtures
//! in lockstep.
//!
//! Producer-generated JSON artifacts carry the same additive top-level
//! `artifact` identity envelope `repo-exposure-json` binds (#6609, ADR 0019
//! shared projection). Additive identity members keep the schema version at
//! `REPO_SEAMS_SCHEMA_VERSION`, per the #2203 repo-exposure and #5474
//! gate-subject precedents.

use crate::agent::artifact::{
    CONTENT_SHA256_PLACEHOLDER, RepoExposureArtifactContext, Sha256Writer,
    repo_seams_artifact_metadata,
};
use crate::analysis::{RepoSeam, RequiredDiscriminator};
use crate::output::json::escape as json_escape;
use crate::output::markdown::table_code_span;

pub(crate) const REPO_SEAMS_SCHEMA_VERSION: &str = "0.1";

/// Render a producer-owned repo seam inventory bound to its subject (#6609).
///
/// The `content_sha256` commitment covers the exact emitted bytes with the
/// digest field replaced by the fixed placeholder, so a consumer can verify
/// the artifact was not edited in flight: first pass hashes the document with
/// the placeholder envelope, second pass re-renders with the resulting digest.
pub(crate) fn render_repo_seams_json_with_context(
    seams: &[RepoSeam],
    context: &RepoExposureArtifactContext,
) -> Result<String, String> {
    use std::io::Write as _;

    let placeholder = repo_seams_artifact_metadata(context, CONTENT_SHA256_PLACEHOLDER)?;
    let hashed = render_repo_seams_json_document(seams, Some(&placeholder));
    let mut hasher = Sha256Writer::new();
    hasher
        .write_all(hashed.as_bytes())
        .map_err(|err| format!("hash repo seams JSON failed: {err}"))?;
    let content_sha256 = hasher.finish();
    let mut metadata = placeholder;
    metadata["content_sha256"] = serde_json::Value::String(content_sha256);
    Ok(render_repo_seams_json_document(seams, Some(&metadata)))
}

fn render_repo_seams_json_document(
    seams: &[RepoSeam],
    artifact: Option<&serde_json::Value>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        REPO_SEAMS_SCHEMA_VERSION
    ));
    if let Some(artifact) = artifact {
        out.push_str(&format!("  \"artifact\": {artifact},\n"));
    }
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
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            table_code_span(seam.id().as_str()),
            table_code_span(&seam.file().to_string_lossy()),
            seam.display_line(),
            table_code_span(seam.owner()),
            seam.kind().as_str(),
            seam.expected_sink().as_str(),
            table_code_span(seam.expression()),
        ));
    }

    out.push_str(
        "\nThis repo seam inventory does not classify test grip yet — \
        `analysis/repo-ripr-classification-v1` adds `SeamGripClass`. \
        Static-language constraints from RIPR-SPEC-0005 still apply.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use std::io::Write as _;

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

    fn temp_root() -> Result<std::path::PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "ripr-repo-seams-identity-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
        Ok(root)
    }

    fn remove_temp_root(root: &std::path::Path) -> Result<(), String> {
        match std::fs::remove_dir_all(root) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("remove temp root: {err}")),
        }
    }

    /// A throwaway workspace root for the identity envelope. Outside Git,
    /// `head`/`worktree` render as `unavailable`, the same disclosure the
    /// repo-exposure envelope emits for an unresolvable producer subject.
    fn rendered_with_context(root: &std::path::Path) -> Result<String, String> {
        let context = RepoExposureArtifactContext::for_repo_seams(
            root.to_path_buf(),
            "draft".to_string(),
            None,
            &crate::config::RiprConfig::default(),
        )?;
        render_repo_seams_json_with_context(&[sample_seam()], &context)
    }

    #[test]
    fn json_carries_schema_version_and_repo_scope() -> Result<(), String> {
        let root = temp_root()?;
        let json = rendered_with_context(&root)?;
        assert!(
            json.contains("\"schema_version\": \"0.1\""),
            "missing schema_version: {json}"
        );
        assert!(
            json.contains("\"scope\": \"repo\""),
            "missing scope: {json}"
        );
        remove_temp_root(&root)
    }

    /// #6609: the whole-repo seam inventory is a producer-owned machine
    /// receipt. The rendered artifact must carry the same identity envelope
    /// `repo-exposure-json` binds — producer tool/version, repository
    /// root/head, analysis format/command, worktree state, input identity,
    /// snapshot identity, and a content commitment over the exact bytes —
    /// so a persisted inventory can be bound to the commit it describes.
    #[test]
    fn json_carries_the_producer_identity_envelope() -> Result<(), String> {
        let root = temp_root()?;
        let json = rendered_with_context(&root)?;

        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|err| format!("repo seams JSON must parse: {err}\n{json}"))?;
        let artifact = value
            .get("artifact")
            .ok_or_else(|| format!("missing artifact identity envelope: {json}"))?;

        assert_eq!(artifact["kind"], "repo_seams", "{artifact}");
        assert_eq!(artifact["producer"]["tool"], "ripr", "{artifact}");
        assert!(
            !artifact["producer"]["version"]
                .as_str()
                .unwrap_or("")
                .is_empty(),
            "producer version must be stated: {artifact}"
        );
        assert_eq!(
            artifact["analysis"]["format"], "repo-seams-json",
            "the envelope must name the producing format: {artifact}"
        );
        assert_eq!(
            artifact["analysis"]["command"], "ripr check --format repo-seams-json",
            "{artifact}"
        );
        assert_eq!(artifact["analysis"]["mode"], "draft", "{artifact}");
        assert_eq!(artifact["analysis"]["profile"], "draft", "{artifact}");
        let input_identity = artifact["analysis"]["input_identity"]
            .as_str()
            .ok_or_else(|| format!("missing input identity: {artifact}"))?;
        assert!(
            input_identity.starts_with("input:v4:fnv1a64:"),
            "input identity must carry the versioned digest shape: {input_identity}"
        );
        let snapshot_identity = artifact["snapshot_identity"]
            .as_str()
            .ok_or_else(|| format!("missing snapshot identity: {artifact}"))?;
        assert!(
            snapshot_identity.starts_with(&format!("snapshot:{input_identity};revision:")),
            "snapshot identity must bind input identity to a revision: {snapshot_identity}"
        );
        let declared_root = artifact["repository"]["root"]
            .as_str()
            .ok_or_else(|| format!("missing repository root: {artifact}"))?;
        let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
        assert_eq!(
            declared_root,
            crate::agent::loop_commands::root_path_display(&canonical),
            "the declared root must be the analyzed checkout"
        );
        let worktree = artifact["analysis"]["worktree"]
            .as_str()
            .ok_or_else(|| format!("missing worktree state: {artifact}"))?;
        assert!(
            matches!(worktree, "clean" | "dirty" | "unavailable"),
            "worktree state speaks the disclosed vocabulary: {worktree}"
        );
        assert!(
            !artifact["repository"]["head"]
                .as_str()
                .unwrap_or("")
                .is_empty(),
            "repository head is stated (resolved or unavailable): {artifact}"
        );
        remove_temp_root(&root)
    }

    /// #6609: `content_sha256` commits the exact emitted bytes with the
    /// digest field replaced by the fixed placeholder (the
    /// `raw_json_placeholder_v1` canonicalization), so a consumer can detect
    /// an edited-in-flight inventory without re-running the multi-minute scan.
    #[test]
    fn content_sha256_commits_the_exact_emitted_bytes() -> Result<(), String> {
        let root = temp_root()?;
        let json = rendered_with_context(&root)?;

        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|err| format!("parse: {err}"))?;
        let declared = value["artifact"]["content_sha256"]
            .as_str()
            .ok_or_else(|| format!("content_sha256 present: {json}"))?
            .to_string();
        assert!(declared.starts_with("sha256:"), "{declared}");

        // Recompute: splice the placeholder back in and hash the bytes.
        let spliced = json.replace(
            &declared,
            crate::agent::artifact::CONTENT_SHA256_PLACEHOLDER,
        );
        assert_ne!(spliced, json, "the placeholder splice must change bytes");
        let mut hasher = Sha256Writer::new();
        hasher
            .write_all(spliced.as_bytes())
            .map_err(|err| format!("hash: {err}"))?;
        assert_eq!(hasher.finish(), declared, "content commitment drift");

        remove_temp_root(&root)
    }

    /// #6609: the identity must name its producing format, so a
    /// `repo-seams-json` artifact never shares an input identity with a
    /// `repo-exposure-json` artifact rendered on the same tree.
    #[test]
    fn seams_input_identity_is_distinct_from_the_exposure_format() -> Result<(), String> {
        let root = temp_root()?;
        let config = crate::config::RiprConfig::default();
        let seams_context = RepoExposureArtifactContext::for_repo_seams(
            root.clone(),
            "draft".to_string(),
            None,
            &config,
        )?;
        let exposure_context = RepoExposureArtifactContext::for_repo_exposure(
            root.clone(),
            "draft".to_string(),
            None,
            &config,
        )?;
        assert_ne!(
            seams_context.input_identity, exposure_context.input_identity,
            "different analysis formats must mint different input identities"
        );
        remove_temp_root(&root)
    }

    #[test]
    fn json_carries_full_seam_record() -> Result<(), String> {
        let root = temp_root()?;
        let json = rendered_with_context(&root)?;
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
        remove_temp_root(&root)
    }

    #[test]
    fn json_emits_empty_array_when_no_seams() -> Result<(), String> {
        let root = temp_root()?;
        let context = RepoExposureArtifactContext::for_repo_seams(
            root.clone(),
            "draft".to_string(),
            None,
            &crate::config::RiprConfig::default(),
        )?;
        let json = render_repo_seams_json_with_context(&[], &context)?;
        assert!(json.contains("\"seams\": []"), "got: {json}");
        assert!(
            json.contains("\"artifact\""),
            "empty result keeps identity: {json}"
        );
        remove_temp_root(&root)
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
