use crate::agent::provenance;
use crate::analysis;
use crate::app::agent_brief::{
    AgentBriefChangedOwner, AgentBriefLine, AgentBriefResolvedWorkingSet,
};
use crate::config::{CONFIG_FILE_NAME, config_fingerprint};
use crate::output;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn validate_agent_receipt_verify_path(
    root: &Path,
    path: &Path,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt --verify-json {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent receipt --verify-json {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

pub(super) fn build_agent_receipt_provenance(
    root: &Path,
    verify_display_path: &Path,
    verify_path: &Path,
    input_paths: &output::agent_receipt::AgentReceiptInputPaths,
) -> Result<output::agent_receipt::AgentReceiptProvenance, String> {
    let before_artifact = agent_receipt_artifact_provenance(
        root,
        &input_paths.before,
        "before artifact",
        "before_artifact",
    )?;
    let after_artifact = agent_receipt_artifact_provenance(
        root,
        &input_paths.after,
        "after artifact",
        "after_artifact",
    )?;
    let verify_artifact = output::agent_receipt::AgentReceiptArtifactProvenance {
        path: output::outcome::display_path(verify_display_path),
        sha256: provenance::sha256_file(verify_path)?,
    };

    Ok(output::agent_receipt::AgentReceiptProvenance {
        ripr_version: env!("CARGO_PKG_VERSION").to_string(),
        repo_root: output::outcome::display_path(root),
        config_fingerprint: agent_receipt_config_fingerprint(root)?,
        command_template_version: crate::agent::loop_commands::AGENT_LOOP_COMMAND_TEMPLATE_VERSION
            .to_string(),
        generated_at: agent_receipt_generated_at()?,
        workflow_artifact: None,
        before_artifact,
        after_artifact,
        verify_artifact,
    })
}

fn agent_receipt_artifact_provenance(
    root: &Path,
    display_path: &str,
    role: &str,
    output_name: &str,
) -> Result<output::agent_receipt::AgentReceiptArtifactProvenance, String> {
    let resolved = validate_agent_receipt_artifact_path(root, Path::new(display_path), role)?;
    Ok(output::agent_receipt::AgentReceiptArtifactProvenance {
        path: display_path.replace('\\', "/"),
        sha256: provenance::sha256_file(&resolved).map_err(|err| {
            format!(
                "hash agent receipt {output_name} {} failed: {err}",
                display_path
            )
        })?,
    })
}

fn validate_agent_receipt_artifact_path(
    root: &Path,
    path: &Path,
    role: &str,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt {role} {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent receipt {role} {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

fn agent_receipt_config_fingerprint(root: &Path) -> Result<Option<String>, String> {
    let path = root.join(CONFIG_FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(config_fingerprint(&text))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("read {} failed: {err}", path.display())),
    }
}

fn agent_receipt_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

pub(super) fn validate_agent_verify_snapshot_path(
    root: &Path,
    path: &Path,
    flag: &str,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent verify root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent verify {flag} {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent verify {flag} {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

pub(super) fn read_agent_verify_snapshot(path: &Path, label: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| {
        format!(
            "read agent verify {label} snapshot {} failed: {err}",
            output::outcome::display_path(path)
        )
    })
}

pub(super) fn resolve_agent_brief_working_set(
    root: &Path,
    working_set: &crate::cli::agent::AgentBriefWorkingSet,
) -> Result<AgentBriefResolvedWorkingSet, String> {
    match working_set {
        crate::cli::agent::AgentBriefWorkingSet::Diff(path) => {
            let diff_path = validate_agent_brief_diff_path(root, path)?;
            let diff_text = analysis::load_diff(root, None, Some(&diff_path), None)?;
            let changed_lines = agent_brief_lines_from_diff(root, &diff_text);
            let changed_owners = agent_brief_owners_for_lines(root, &changed_lines);
            Ok(AgentBriefResolvedWorkingSet::diff(
                path.clone(),
                changed_lines,
            ))
            .map(|working_set| working_set.with_changed_owners(changed_owners))
        }
        crate::cli::agent::AgentBriefWorkingSet::Base(base) => {
            let diff_text = analysis::load_diff(root, Some(base.as_str()), None, None)?;
            let changed_lines = agent_brief_lines_from_diff(root, &diff_text);
            let changed_owners = agent_brief_owners_for_lines(root, &changed_lines);
            Ok(AgentBriefResolvedWorkingSet::base(
                base.clone(),
                changed_lines,
            ))
            .map(|working_set| working_set.with_changed_owners(changed_owners))
        }
        crate::cli::agent::AgentBriefWorkingSet::Files(files) => files
            .iter()
            .map(|file| confine_agent_brief_file_path(root, file))
            .collect::<Result<Vec<_>, _>>()
            .map(AgentBriefResolvedWorkingSet::files),
        crate::cli::agent::AgentBriefWorkingSet::SeamId(seam_id) => {
            Ok(AgentBriefResolvedWorkingSet::seam_id(seam_id.clone()))
        }
    }
}

fn validate_agent_brief_diff_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent brief root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() || path.exists() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent brief diff {} failed: {err}",
            path.display()
        )
    })?;
    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent brief --diff {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }
    Ok(candidate)
}

pub(super) fn agent_brief_lines_from_diff(root: &Path, diff_text: &str) -> Vec<AgentBriefLine> {
    analysis::parse_unified_diff(diff_text)
        .into_iter()
        .flat_map(|file| {
            let path = normalize_agent_brief_path(root, &file.path);
            file.added_lines
                .into_iter()
                .map(move |line| AgentBriefLine::new(path.clone(), line.line))
        })
        .collect()
}

pub(super) fn agent_brief_owners_for_lines(
    root: &Path,
    lines: &[AgentBriefLine],
) -> Vec<AgentBriefChangedOwner> {
    let owner_inputs = lines
        .iter()
        .map(|line| (line.file.clone(), line.line))
        .collect::<Vec<_>>();
    let Ok(owners) = analysis::owner_symbols_for_lines(root, &owner_inputs) else {
        return Vec::new();
    };

    owners
        .into_iter()
        .map(|owner| AgentBriefChangedOwner::new(owner.file, owner.line, owner.owner))
        .collect()
}

/// Confine an agent-brief `--files` entry to the workspace (#2100): strip
/// the root prefix when present, then fail closed with a named error when
/// the result still escapes — a `..` component, an absolute path outside
/// root, or a drive prefix. The brief artifact must never embed an
/// unconfined path; this mirrors the lexical confinement the diff parser
/// applies to parsed diff paths (#2099).
fn confine_agent_brief_file_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let normalized = normalize_agent_brief_path(root, path);
    let mut confined = PathBuf::new();
    for component in normalized.components() {
        match component {
            std::path::Component::Normal(part) => confined.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!(
                    "agent brief --files {} must stay under root {}",
                    path.display(),
                    root.display()
                ));
            }
        }
    }
    if confined.as_os_str().is_empty() {
        return Err(format!(
            "agent brief --files {} is empty after normalization",
            path.display()
        ));
    }
    Ok(confined)
}

pub(super) fn normalize_agent_brief_path(root: &Path, path: &Path) -> PathBuf {
    let path_text = normalized_path_text(path);
    for root_text in normalized_root_prefixes(root) {
        let prefix = format!("{root_text}/");
        if let Some(stripped) = path_text.strip_prefix(&prefix) {
            return PathBuf::from(stripped);
        }
    }
    PathBuf::from(path_text)
}

fn normalized_root_prefixes(root: &Path) -> Vec<String> {
    let mut prefixes = Vec::new();
    push_unique_normalized_path(&mut prefixes, root);
    if let Ok(root) = std::path::absolute(root) {
        push_unique_normalized_path(&mut prefixes, &root);
    }
    if let Ok(root) = root.canonicalize() {
        push_unique_normalized_path(&mut prefixes, &root);
    }
    prefixes
}

fn push_unique_normalized_path(prefixes: &mut Vec<String>, path: &Path) {
    let text = normalized_path_text(path);
    if !text.is_empty() && !prefixes.iter().any(|existing| existing == &text) {
        prefixes.push(text);
    }
}

fn normalized_path_text(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Regression guard for #2449: the agent-brief `--files` and `--brief-diff`
    // confinement helpers must reject paths that escape the workspace root.
    // The helpers are private, so the tests live in this module.

    /// Create a unique temp dir for one test and return its path. Mirrors the
    /// pattern in `analysis/path.rs` tests.
    struct ScratchDir {
        path: PathBuf,
    }

    impl ScratchDir {
        fn new(label: &str) -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("ripr-agent-brief-{label}-{suffix}"));
            let _ = std::fs::create_dir_all(&path);
            Self { path }
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn confine_agent_brief_file_path_accepts_in_root_relative() -> Result<(), String> {
        let dir = ScratchDir::new("in-root");
        let confined = confine_agent_brief_file_path(&dir.path, Path::new("src/lib.rs"))?;
        assert_eq!(confined, PathBuf::from("src/lib.rs"));
        Ok(())
    }

    #[test]
    fn confine_agent_brief_file_path_strips_root_prefix() -> Result<(), String> {
        let dir = ScratchDir::new("prefix");
        // An absolute path under root should be stripped to its relative tail.
        let absolute = dir.path.join("src/lib.rs");
        let confined = confine_agent_brief_file_path(&dir.path, &absolute)?;
        assert_eq!(confined, PathBuf::from("src/lib.rs"));
        Ok(())
    }

    #[test]
    fn confine_agent_brief_file_path_rejects_parent_traversal() -> Result<(), String> {
        let dir = ScratchDir::new("traversal");
        let Err(err) = confine_agent_brief_file_path(&dir.path, Path::new("../outside.rs")) else {
            return Err("parent traversal should be rejected".to_string());
        };
        assert!(
            err.contains("must stay under root"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn confine_agent_brief_file_path_rejects_empty_after_normalization() -> Result<(), String> {
        let dir = ScratchDir::new("empty");
        let Err(err) = confine_agent_brief_file_path(&dir.path, Path::new(".")) else {
            return Err("a path that normalizes to empty should be rejected".to_string());
        };
        assert!(
            err.contains("empty after normalization"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn confine_agent_brief_file_path_root_path_normalizes_to_safe_relative() -> Result<(), String> {
        let dir = ScratchDir::new("root-normalize");
        // Passing the root directory itself: the root prefix is stripped,
        // leaving a safe non-traversal relative (or rejected as empty).
        // The contract under test is: no escape path reaches downstream.
        let result = confine_agent_brief_file_path(&dir.path, &dir.path);
        match result {
            Ok(confined) => {
                let path = confined.to_string_lossy();
                assert!(
                    !path.contains(".."),
                    "root path should not produce an escape: {path}"
                );
                assert!(
                    !path.is_empty(),
                    "root path should not normalize to empty without an error"
                );
            }
            Err(err) => assert!(
                err.contains("empty after normalization") || err.contains("must stay under root"),
                "unexpected error: {err}"
            ),
        }
        Ok(())
    }

    #[test]
    fn validate_agent_brief_diff_path_accepts_in_root_file() -> Result<(), String> {
        let dir = ScratchDir::new("diff-in-root");
        let diff_file = dir.path.join("change.diff");
        std::fs::write(&diff_file, "diff content").map_err(|err| err.to_string())?;
        // The Ok contract is what we test: an in-root diff file must be
        // accepted. We do not assert on the canonicalized path's prefix
        // because canonicalize may rewrite the temp-dir prefix on Windows.
        let confined = validate_agent_brief_diff_path(&dir.path, Path::new("change.diff"))?;
        assert!(
            !confined.to_string_lossy().contains(".."),
            "accepted path should not contain traversal: {confined:?}"
        );
        Ok(())
    }

    #[test]
    fn validate_agent_brief_diff_path_rejects_outside_root() -> Result<(), String> {
        let inside = ScratchDir::new("diff-inside");
        let outside = ScratchDir::new("diff-outside");
        let foreign = outside.path.join("secret.diff");
        std::fs::write(&foreign, "secret").map_err(|err| err.to_string())?;
        let Err(err) = validate_agent_brief_diff_path(&inside.path, &foreign) else {
            return Err("a diff file outside root should be rejected".to_string());
        };
        assert!(
            err.contains("must stay under root"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn validate_agent_brief_diff_path_rejects_symlink_escape() -> Result<(), String> {
        use std::os::unix::fs::symlink;
        let inside = ScratchDir::new("symlink-inside");
        let outside = ScratchDir::new("symlink-outside");
        let target = outside.path.join("secret.diff");
        std::fs::write(&target, "secret").map_err(|err| err.to_string())?;
        let link = inside.path.join("linked.diff");
        symlink(&target, &link).map_err(|err| err.to_string())?;
        let Err(err) = validate_agent_brief_diff_path(&inside.path, Path::new("linked.diff"))
        else {
            return Err("a symlink escaping root should be rejected".to_string());
        };
        assert!(
            err.contains("must stay under root"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn agent_brief_lines_from_diff_does_not_embed_traversal_paths() -> Result<(), String> {
        // A crafted diff with a `+++ b/../escape.rs` marker. The diff parser's
        // confinement (parse_new_path_marker → confine_to_relative_path)
        // strips the `..` component, so the file survives as `escape.rs`.
        // The lines-from-diff helper then normalizes that safe path. This test
        // pins that no `..` traversal component reaches the brief lines,
        // regardless of whether the parser keeps or drops the file.
        let dir = ScratchDir::new("diff-traversal");
        let diff_text = "diff --git a/src/lib.rs b/../escape.rs\n--- a/src/lib.rs\n+++ b/../escape.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let lines = agent_brief_lines_from_diff(&dir.path, diff_text);
        for line in &lines {
            let path_str = line.file.to_string_lossy();
            assert!(
                !path_str.contains(".."),
                "escape path leaked into brief lines: {path_str}"
            );
        }
        Ok(())
    }
}
