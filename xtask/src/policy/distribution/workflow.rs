use std::collections::BTreeMap;

use super::contract::TargetContract;

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkflowTarget {
    target: String,
    executable: String,
    archive: String,
}

pub(super) fn validate_archive_workflow(
    path: &str,
    text: &str,
    targets: &[TargetContract],
    violations: &mut Vec<String>,
) {
    let workflow_targets = match parse_archive_workflow_targets(path, text) {
        Ok(targets) => targets,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let mut expected = BTreeMap::new();
    for target in targets {
        expected.insert(
            target.rust_target.as_str(),
            (target.executable.as_str(), target.archive.as_str()),
        );
    }
    let mut actual = BTreeMap::new();
    for target in &workflow_targets {
        if actual
            .insert(
                target.target.as_str(),
                (target.executable.as_str(), target.archive.as_str()),
            )
            .is_some()
        {
            violations.push(format!(
                "{path}: duplicate server archive matrix target `{}`",
                target.target
            ));
        }
    }
    if actual != expected {
        violations.push(format!(
            "{path}: server archive matrix must match policy/distribution.toml\nexpected: {expected:#?}\nactual: {actual:#?}"
        ));
    }
}

fn parse_archive_workflow_targets(path: &str, text: &str) -> Result<Vec<WorkflowTarget>, String> {
    let build = bounded_section(text, "\n  build:\n", "\n  manifest:\n")
        .ok_or_else(|| format!("{path}: missing build job bounded by build/manifest jobs"))?;
    let matrix = bounded_section(
        build,
        "\n      matrix:\n        include:\n",
        "\n    steps:\n",
    )
    .ok_or_else(|| format!("{path}: missing build.strategy.matrix.include section"))?;

    let mut targets = Vec::new();
    let mut current_target = None;
    let mut executable = None;
    let mut archive = None;
    for line in matrix.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("- target: ") {
            finish_target(
                path,
                &mut targets,
                current_target.take(),
                executable.take(),
                archive.take(),
            )?;
            current_target = Some(unquote(value));
        } else if let Some(value) = line.strip_prefix("executable: ") {
            executable = Some(unquote(value));
        } else if let Some(value) = line.strip_prefix("archive: ") {
            archive = Some(unquote(value));
        }
    }
    finish_target(path, &mut targets, current_target, executable, archive)?;
    if targets.is_empty() {
        return Err(format!(
            "{path}: build.strategy.matrix.include contains no target rows"
        ));
    }
    Ok(targets)
}

fn finish_target(
    path: &str,
    targets: &mut Vec<WorkflowTarget>,
    target: Option<String>,
    executable: Option<String>,
    archive: Option<String>,
) -> Result<(), String> {
    let Some(target) = target else {
        if executable.is_some() || archive.is_some() {
            return Err(format!(
                "{path}: server archive matrix contains executable/archive before a target"
            ));
        }
        return Ok(());
    };
    let executable = executable.ok_or_else(|| {
        format!("{path}: server archive matrix target `{target}` is missing executable")
    })?;
    let archive = archive.ok_or_else(|| {
        format!("{path}: server archive matrix target `{target}` is missing archive")
    })?;
    targets.push(WorkflowTarget {
        target,
        executable,
        archive,
    });
    Ok(())
}

fn bounded_section<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start = text.find(start)? + start.len();
    let tail = &text[start..];
    let end = tail.find(end)?;
    Some(&tail[..end])
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::distribution::{CONTRACT_PATH, parse_distribution_contract};

    const CONTRACT_TEXT: &str = include_str!("../../../../policy/distribution.toml");
    const WORKFLOW_TEXT: &str =
        include_str!("../../../../.github/workflows/server-archive-qualification.yml");

    #[test]
    fn actual_archive_matrix_matches_distribution_contract() -> Result<(), String> {
        let contract = parse_distribution_contract(CONTRACT_PATH, CONTRACT_TEXT)?;
        let mut violations = Vec::new();
        validate_archive_workflow(
            ".github/workflows/server-archive-qualification.yml",
            WORKFLOW_TEXT,
            &contract.target,
            &mut violations,
        );
        assert!(violations.is_empty(), "{violations:#?}");
        Ok(())
    }

    #[test]
    fn matrix_executable_or_archive_drift_is_rejected() -> Result<(), String> {
        let contract = parse_distribution_contract(CONTRACT_PATH, CONTRACT_TEXT)?;
        let mutated = WORKFLOW_TEXT
            .replacen("executable: ripr.exe", "executable: helper.exe", 1)
            .replacen("archive: tar.gz", "archive: zip", 1);
        let mut violations = Vec::new();
        validate_archive_workflow(
            ".github/workflows/server-archive-qualification.yml",
            &mutated,
            &contract.target,
            &mut violations,
        );
        assert!(violations.iter().any(|violation| {
            violation.contains("server archive matrix must match policy/distribution.toml")
        }));
        Ok(())
    }
}
