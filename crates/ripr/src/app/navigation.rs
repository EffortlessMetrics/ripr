use super::{CheckInput, Mode};
use crate::agent::loop_commands::shell_arg;
use std::path::Path;

/// Copy-pasteable sibling commands for a selected finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FindingNavigation {
    explain_prefix: String,
    context_prefix: String,
}

impl FindingNavigation {
    pub(crate) fn legacy() -> Self {
        Self {
            explain_prefix: "ripr explain".to_string(),
            context_prefix: "ripr context".to_string(),
        }
    }

    pub(crate) fn explain_command(&self, selector: &str) -> String {
        format!("{} {}", self.explain_prefix, shell_arg(selector))
    }

    pub(crate) fn context_command(&self, selector: &str) -> String {
        format!("{} --at {}", self.context_prefix, shell_arg(selector))
    }
}

/// Build sibling commands that preserve the input identity needed to replay a
/// finding. An artifact is authoritative for its diff source; otherwise the
/// explicit diff or base is carried forward.
pub(crate) fn finding_navigation(
    input: &CheckInput,
    artifact_path: Option<&Path>,
    mode_explicit: bool,
) -> FindingNavigation {
    let mut args = vec![format!(
        "--root {}",
        shell_arg(&input.root.display().to_string())
    )];

    if let Some(artifact_path) = artifact_path {
        args.push(format!(
            "--from {}",
            shell_arg(&artifact_path.display().to_string())
        ));
    } else if let Some(diff_file) = input.diff_file.as_deref() {
        args.push(format!(
            "--diff {}",
            shell_arg(&diff_file.display().to_string())
        ));
    } else if let Some(base) = input.base.as_deref() {
        args.push(format!("--base {}", shell_arg(base)));
    }

    if mode_explicit || input.mode != Mode::Draft {
        args.push(format!("--mode {}", shell_arg(input.mode.as_str())));
    }
    if !input.include_unchanged_tests {
        args.push("--no-unchanged-tests".to_string());
    }
    if let Some(perl_facts_path) = input.perl_facts_path.as_deref() {
        args.push(format!(
            "--perl-facts {}",
            shell_arg(&perl_facts_path.display().to_string())
        ));
    }
    if let Some(suppression_policy) = input.suppression_policy.as_deref() {
        args.push(format!(
            "--suppression-policy {}",
            shell_arg(&suppression_policy.display().to_string())
        ));
    }

    let args = args.join(" ");
    FindingNavigation {
        explain_prefix: format!("ripr explain {args}"),
        context_prefix: format!("ripr context {args}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn finding_navigation_preserves_diff_and_quotes_dynamic_values() {
        let input = CheckInput {
            root: PathBuf::from("repo root"),
            diff_file: Some(PathBuf::from("change set.diff")),
            ..CheckInput::default()
        };
        let navigation = finding_navigation(&input, None, false);

        assert_eq!(
            navigation.explain_command("probe:src/lib.rs:error_path:abc123"),
            "ripr explain --root 'repo root' --diff 'change set.diff' probe:src/lib.rs:error_path:abc123"
        );
        assert_eq!(
            navigation.context_command("probe:src/lib.rs:error_path:abc123"),
            "ripr context --root 'repo root' --diff 'change set.diff' --at probe:src/lib.rs:error_path:abc123"
        );
    }

    #[test]
    fn finding_navigation_prefers_artifact_identity_over_diff_source() {
        let input = CheckInput {
            root: PathBuf::from("repo"),
            diff_file: Some(PathBuf::from("old.diff")),
            mode: Mode::Ready,
            ..CheckInput::default()
        };
        let navigation = finding_navigation(&input, Some(Path::new("saved artifact.json")), false);

        assert_eq!(
            navigation.explain_command("probe:id"),
            "ripr explain --root repo --from 'saved artifact.json' --mode ready probe:id"
        );
    }

    #[test]
    fn finding_navigation_preserves_explicit_draft_mode() {
        let input = CheckInput::default();
        let navigation = finding_navigation(&input, None, true);

        assert_eq!(
            navigation.explain_command("probe:id"),
            "ripr explain --root . --base origin/main --mode draft probe:id"
        );
    }
}
