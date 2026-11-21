//! Input-level validation for the immutable Git candidate subject
//! (#3237 / #3276 R1).
//!
//! The subject value types in `domain::git_candidate` carry their own
//! structural validity. This module owns the remaining judgments at the
//! `CheckInput` boundary: the subject is mutually exclusive with the
//! external-diff and top-level-base inputs, its repository root must name
//! an existing directory, and — until the Git object producer lands
//! (#3277) — a subject input binds and defers execution to the object producer instead of ever reaching
//! worktree diff analysis.

use crate::app::CheckInput;
use crate::domain::GitCandidateSubjectError;

/// Validate a `CheckInput`'s candidate-subject dimension.
///
/// Returns `Ok(())` when no subject is set (every existing caller). When a
/// subject is set, returns the first named failure: an input conflict, an
/// invalid repository root, or — for a fully valid subject —
/// [`GitCandidateSubjectError::ExecutionUnsupported`], because this build
/// binds and validates subjects but cannot execute them. A subject input
/// must never be translated into ordinary worktree analysis or an empty
/// diff.
pub(crate) fn validate_input_subject(input: &CheckInput) -> Result<(), GitCandidateSubjectError> {
    let Some(subject) = input.git_candidate.as_ref() else {
        return Ok(());
    };
    if let Some(diff_file) = input.diff_file.as_ref() {
        return Err(GitCandidateSubjectError::DiffFileConflict {
            diff_file: diff_file.to_string_lossy().to_string(),
        });
    }
    if let Some(base) = input.base.as_ref() {
        return Err(GitCandidateSubjectError::BaseConflict { base: base.clone() });
    }
    if subject.repository_root.as_os_str().is_empty() || !subject.repository_root.is_dir() {
        return Err(GitCandidateSubjectError::RepositoryRootInvalid {
            root: subject.repository_root.to_string_lossy().to_string(),
        });
    }
    // #3277: binding is complete and the object producer now executes
    // the subject in the diff pipeline; no rejection here.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_input_subject;
    use crate::app::CheckInput;
    use crate::domain::{
        GitCandidateBase, GitCandidateSubject, GitCandidateSubjectError, GitObjectId, GitTreeish,
    };
    use std::path::PathBuf;

    fn subject(repository_root: PathBuf) -> Result<GitCandidateSubject, String> {
        Ok(GitCandidateSubject::new(
            repository_root,
            GitCandidateBase::Treeish(
                GitTreeish::new("HEAD~1").map_err(|error| error.to_string())?,
            ),
            GitObjectId::parse(&"a".repeat(40)).map_err(|error| error.to_string())?,
        ))
    }

    fn input_with(subject: Option<GitCandidateSubject>) -> CheckInput {
        CheckInput {
            // Clear the default `base` and absent diff so the subject, not
            // a leftover default, is what validation judges.
            base: None,
            diff_file: None,
            git_candidate: subject,
            ..CheckInput::default()
        }
    }

    #[test]
    fn input_without_subject_validates_ok() {
        assert_eq!(validate_input_subject(&input_with(None)), Ok(()));
    }

    #[test]
    fn subject_conflicts_with_external_diff_file() -> Result<(), String> {
        let mut input = input_with(Some(subject(PathBuf::from("."))?));
        input.diff_file = Some(PathBuf::from("change.diff"));
        assert_eq!(
            validate_input_subject(&input),
            Err(GitCandidateSubjectError::DiffFileConflict {
                diff_file: "change.diff".to_string()
            })
        );
        Ok(())
    }

    #[test]
    fn subject_conflicts_with_top_level_base() -> Result<(), String> {
        let mut input = input_with(Some(subject(PathBuf::from("."))?));
        input.base = Some("origin/main".to_string());
        assert_eq!(
            validate_input_subject(&input),
            Err(GitCandidateSubjectError::BaseConflict {
                base: "origin/main".to_string()
            })
        );
        Ok(())
    }

    #[test]
    fn subject_requires_an_existing_repository_root_directory() -> Result<(), String> {
        let missing = subject(PathBuf::from("no/such/repo/exists"))?;
        assert!(matches!(
            validate_input_subject(&input_with(Some(missing))),
            Err(GitCandidateSubjectError::RepositoryRootInvalid { .. })
        ));
        let empty = subject(PathBuf::from(""))?;
        assert!(matches!(
            validate_input_subject(&input_with(Some(empty))),
            Err(GitCandidateSubjectError::RepositoryRootInvalid { .. })
        ));
        Ok(())
    }

    #[test]
    fn structurally_valid_subject_binds_and_defers_to_the_object_producer() -> Result<(), String> {
        // #3277: binding validates; execution moved to the object
        // producer in the diff pipeline. A structurally valid subject
        // passes validation here — the producer owns the identity
        // checks (and their named failures) at run time.
        let current_dir =
            std::env::current_dir().map_err(|error| format!("current_dir failed: {error}"))?;
        assert_eq!(
            validate_input_subject(&input_with(Some(subject(current_dir)?))),
            Ok(())
        );
        Ok(())
    }
}
