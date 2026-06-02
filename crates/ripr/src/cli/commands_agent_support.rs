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
            let diff_text = analysis::load_diff(root, None, Some(&diff_path))?;
            let changed_lines = agent_brief_lines_from_diff(root, &diff_text);
            let changed_owners = agent_brief_owners_for_lines(root, &changed_lines);
            Ok(AgentBriefResolvedWorkingSet::diff(
                path.clone(),
                changed_lines,
            ))
            .map(|working_set| working_set.with_changed_owners(changed_owners))
        }
        crate::cli::agent::AgentBriefWorkingSet::Base(base) => {
            let diff_text = analysis::load_diff(root, Some(base.as_str()), None)?;
            let changed_lines = agent_brief_lines_from_diff(root, &diff_text);
            let changed_owners = agent_brief_owners_for_lines(root, &changed_lines);
            Ok(AgentBriefResolvedWorkingSet::base(
                base.clone(),
                changed_lines,
            ))
            .map(|working_set| working_set.with_changed_owners(changed_owners))
        }
        crate::cli::agent::AgentBriefWorkingSet::Files(files) => {
            Ok(AgentBriefResolvedWorkingSet::files(
                files
                    .iter()
                    .map(|file| normalize_agent_brief_path(root, file))
                    .collect(),
            ))
        }
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
