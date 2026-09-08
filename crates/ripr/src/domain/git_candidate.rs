//! Typed immutable Git candidate subject for diff analysis (#3237 / #3276).
//!
//! A [`GitCandidateSubject`] binds the *identity* of one analysis input:
//! the repository that owns the objects, the exact base treeish (or the
//! repository's own empty tree), and the exact candidate tree. It is the
//! caller's replacement for the split `root` + `base` + `--diff` input,
//! and it exists so that a run can state "analyze exactly this tree" with
//! no possibility of accidentally analyzing the dirty worktree or a later
//! live index instead.
//!
//! This module binds identity only. It performs no Git object reads and no
//! filesystem access: resolving the treeish, deriving the diff, and
//! materializing candidate bytes are the object producer's job (#3277).
//! Until that producer exists, analysis of a subject input fails closed —
//! see [`GitCandidateSubjectError::ExecutionUnsupported`] — rather than
//! falling back to worktree semantics.

use std::fmt;
use std::path::PathBuf;

/// Maximum accepted treeish length. A usable treeish (object ID, ref, or
/// short descendant expression) is far shorter; the bound exists so an
/// absurd token fails validation instead of reaching a Git invocation.
const MAX_TREEISH_LENGTH: usize = 256;

/// Hash format of a Git object ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitHashFormat {
    /// 40 hex-character object IDs (SHA-1 repositories).
    Sha1,
    /// 64 hex-character object IDs (SHA-256 repositories).
    Sha256,
}

/// A well-formed Git object ID, normalized to lowercase hex.
///
/// Construction is the validation boundary: a `GitObjectId` can only exist
/// for a syntactically valid SHA-1 (40 hex) or SHA-256 (64 hex) object ID.
/// Whether the named object actually exists in a repository is the object
/// producer's question (#3277), not this type's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitObjectId {
    hex: String,
}

impl GitObjectId {
    /// Parses an object ID, accepting upper- or lowercase hex and storing
    /// the canonical lowercase form.
    ///
    /// # Errors
    ///
    /// Returns [`GitCandidateSubjectError::ObjectIdMalformed`] for any
    /// input that is not exactly 40 or 64 hexadecimal characters.
    pub fn parse(value: &str) -> Result<Self, GitCandidateSubjectError> {
        let well_formed =
            matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit());
        if !well_formed {
            return Err(GitCandidateSubjectError::ObjectIdMalformed {
                value: value.to_string(),
            });
        }
        Ok(Self {
            hex: value.to_ascii_lowercase(),
        })
    }

    /// The canonical lowercase hex form of the object ID.
    pub fn as_str(&self) -> &str {
        &self.hex
    }

    /// The hash format implied by the object ID's length.
    pub fn hash_format(&self) -> GitHashFormat {
        if self.hex.len() == 64 {
            GitHashFormat::Sha256
        } else {
            GitHashFormat::Sha1
        }
    }
}

impl fmt::Display for GitObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.hex)
    }
}

/// A validated Git treeish (object ID, ref, or descendant expression such
/// as `HEAD~3`) naming the base side of a candidate diff.
///
/// Validation rejects empty input, whitespace, control characters, NUL,
/// leading `-` (so the value can never be confused with a Git option), and
/// `..` (ref-component rule). Resolution against a real object database is
/// the object producer's job (#3277).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitTreeish {
    value: String,
}

impl GitTreeish {
    /// Validates and stores a treeish.
    ///
    /// # Errors
    ///
    /// Returns [`GitCandidateSubjectError::TreeishMalformed`] when the
    /// value is empty, longer than 256 characters, contains whitespace or
    /// another control character, starts with `-`, or contains `..`.
    pub fn new(value: &str) -> Result<Self, GitCandidateSubjectError> {
        let malformed = value.is_empty()
            || value.len() > MAX_TREEISH_LENGTH
            || value.starts_with('-')
            || value.contains("..")
            || value
                .chars()
                .any(|character| character.is_control() || character.is_whitespace());
        if malformed {
            return Err(GitCandidateSubjectError::TreeishMalformed {
                value: value.to_string(),
            });
        }
        Ok(Self {
            value: value.to_string(),
        })
    }

    /// The treeish exactly as supplied by the caller.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for GitTreeish {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

/// The base side of a candidate diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitCandidateBase {
    /// The repository's own empty tree. SHA-1 and SHA-256 repositories name
    /// the empty tree with different object IDs, so the model carries no
    /// hash constant: the object producer resolves this variant against the
    /// subject repository's format (#3277).
    EmptyTree,
    /// An explicit treeish resolved against the subject repository by the
    /// object producer.
    Treeish(GitTreeish),
}

/// The diff semantics a candidate subject requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GitCandidateDiffSemantics {
    /// Direct two-tree comparison: every path difference between the base
    /// tree and the candidate tree is in scope. This is the only semantics
    /// a subject can request; it is a field so the contract is explicit at
    /// the subject rather than implied by consumer convention.
    #[default]
    DirectTreeToTree,
}

/// The identity of one immutable Git candidate analysis input.
///
/// Constructing a subject binds identity only — it never reads Git objects
/// and never claims analysis ran. Analysis of a subject input fails closed
/// until the object producer lands (#3277); a debug or serialized form of
/// this type states identities, not results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitCandidateSubject {
    /// Repository root whose object database owns the base and candidate
    /// trees. The run must not consult this path's worktree or index for
    /// analysis bytes.
    pub repository_root: PathBuf,
    /// Base side of the requested diff.
    pub base: GitCandidateBase,
    /// Exact candidate tree object ID. The analyzed source, tests, and
    /// config must come from this tree's blobs.
    pub candidate_tree: GitObjectId,
    /// Requested diff semantics.
    pub diff_semantics: GitCandidateDiffSemantics,
}

impl GitCandidateSubject {
    /// Binds a subject with direct tree-to-tree diff semantics.
    pub fn new(
        repository_root: impl Into<PathBuf>,
        base: GitCandidateBase,
        candidate_tree: GitObjectId,
    ) -> Self {
        Self {
            repository_root: repository_root.into(),
            base,
            candidate_tree,
            diff_semantics: GitCandidateDiffSemantics::DirectTreeToTree,
        }
    }
}

/// Typed validation and completeness outcomes for a candidate subject.
///
/// Structural problems (malformed object IDs and treeishes) surface at
/// construction of the value types. The remaining variants report
/// input-level conflicts and the current execution boundary; none of them
/// may ever be translated into an empty diff or a clean zero-finding run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitCandidateSubjectError {
    /// An object ID was not 40 or 64 hexadecimal characters.
    ObjectIdMalformed { value: String },
    /// A treeish was empty, oversized, carried control characters, led
    /// with `-`, or contained `..`.
    TreeishMalformed { value: String },
    /// The subject's repository root does not name an existing directory.
    RepositoryRootInvalid { root: String },
    /// A candidate subject was combined with an external diff file. The
    /// subject is its own diff authority; the two inputs are mutually
    /// exclusive.
    DiffFileConflict { diff_file: String },
    /// A candidate subject was combined with the top-level `base` input.
    /// The subject carries its own base; a second base is ambiguous.
    BaseConflict { base: String },
    /// The subject is structurally valid, but this build has no Git object
    /// producer yet (#3277). Execution fails closed instead of falling
    /// back to worktree analysis.
    ExecutionUnsupported,
    /// The object producer (#3277) resolved the subject's identity but a
    /// Git plumbing or materialization step failed. The detail names the
    /// exact step and identity; execution never falls back to worktree
    /// analysis or an empty result.
    ExecutionFailed { detail: String },
}

impl GitCandidateSubjectError {
    /// Stable, machine-matchable reason label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ObjectIdMalformed { .. } => "object_id_malformed",
            Self::TreeishMalformed { .. } => "treeish_malformed",
            Self::RepositoryRootInvalid { .. } => "repository_root_invalid",
            Self::DiffFileConflict { .. } => "diff_file_conflict",
            Self::BaseConflict { .. } => "base_conflict",
            Self::ExecutionFailed { .. } => "execution_failed",
            Self::ExecutionUnsupported => "execution_unsupported",
        }
    }
}

impl fmt::Display for GitCandidateSubjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectIdMalformed { value } => write!(
                formatter,
                "git candidate subject: malformed object ID {value:?}: \
                 expected 40 or 64 hexadecimal characters"
            ),
            Self::TreeishMalformed { value } => write!(
                formatter,
                "git candidate subject: malformed treeish {value:?}: must be \
                 non-empty, at most 256 characters, free of control characters \
                 and '..', and must not start with '-'"
            ),
            Self::RepositoryRootInvalid { root } => write!(
                formatter,
                "git candidate subject: repository root {root:?} is missing \
                 or not a directory"
            ),
            Self::DiffFileConflict { diff_file } => write!(
                formatter,
                "git candidate subject conflicts with the external diff file \
                 {diff_file:?}: the subject derives its own diff; supply one \
                 input authority, not two"
            ),
            Self::BaseConflict { base } => write!(
                formatter,
                "git candidate subject conflicts with the top-level base \
                 {base:?}: the subject names its own base; unset the \
                 top-level base"
            ),
            Self::ExecutionFailed { detail } => write!(
                formatter,
                "git candidate subject: object producer failed: {detail}"
            ),
            Self::ExecutionUnsupported => write!(
                formatter,
                "git candidate subjects are not executable on this \
                 path: the object producer executes diff-mode subjects only \
                 (#3277); refusing to fall back to worktree or repo analysis"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1_OID: &str = "0123456789abcdef0123456789abcdef01234567";
    const SHA256_OID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn object_id_parse_accepts_sha1_and_sha256_and_normalizes_case() -> Result<(), String> {
        let sha1 = GitObjectId::parse(SHA1_OID).map_err(|error| error.to_string())?;
        assert_eq!(sha1.as_str(), SHA1_OID);
        assert_eq!(sha1.hash_format(), GitHashFormat::Sha1);
        let sha256 = GitObjectId::parse(SHA256_OID).map_err(|error| error.to_string())?;
        assert_eq!(sha256.hash_format(), GitHashFormat::Sha256);
        let upper = GitObjectId::parse(&SHA1_OID.to_ascii_uppercase())
            .map_err(|error| error.to_string())?;
        assert_eq!(upper.as_str(), SHA1_OID, "hex case is not identity");
        Ok(())
    }

    #[test]
    fn object_id_parse_rejects_malformed_ids() {
        for value in [
            "",
            "abc",
            &"a".repeat(39),
            &"a".repeat(41),
            &"g".repeat(40),
            "not an oid",
        ] {
            assert_eq!(
                GitObjectId::parse(value),
                Err(GitCandidateSubjectError::ObjectIdMalformed {
                    value: value.to_string()
                }),
                "value {value:?} must not parse"
            );
        }
    }

    #[test]
    fn treeish_accepts_ref_expressions_and_object_ids() -> Result<(), String> {
        for value in ["HEAD~3", "refs/tags/v1.2.3", SHA1_OID, "main"] {
            let treeish = GitTreeish::new(value).map_err(|error| error.to_string())?;
            assert_eq!(treeish.as_str(), value, "value {value:?} must parse");
        }
        Ok(())
    }

    #[test]
    fn treeish_rejects_empty_option_and_component_shapes() {
        for value in [
            "",
            "-o",
            "--global-option",
            "two words",
            "a..b",
            "a\tb",
            &"x".repeat(257),
        ] {
            assert_eq!(
                GitTreeish::new(value),
                Err(GitCandidateSubjectError::TreeishMalformed {
                    value: value.to_string()
                }),
                "value {value:?} must not parse"
            );
        }
    }

    #[test]
    fn empty_tree_base_carries_no_hash_constant() -> Result<(), String> {
        // The model must not hardcode one empty-tree OID: SHA-1 and SHA-256
        // repositories name the empty tree differently, and resolution is
        // #3277's job. `EmptyTree` is a unit variant, so a subject built on
        // it cannot carry any tree identity but the marker itself.
        let candidate_tree = GitObjectId::parse(SHA1_OID).map_err(|error| error.to_string())?;
        let subject = GitCandidateSubject::new(
            std::path::Path::new("/repo"),
            GitCandidateBase::EmptyTree,
            candidate_tree.clone(),
        );
        assert_eq!(subject.base, GitCandidateBase::EmptyTree);
        assert_eq!(
            subject,
            GitCandidateSubject {
                repository_root: std::path::PathBuf::from("/repo"),
                base: GitCandidateBase::EmptyTree,
                candidate_tree,
                diff_semantics: GitCandidateDiffSemantics::DirectTreeToTree,
            }
        );
        Ok(())
    }

    #[test]
    fn error_display_names_each_failure_class() {
        let cases = [
            (
                GitCandidateSubjectError::ObjectIdMalformed {
                    value: "zz".to_string(),
                },
                "malformed object ID",
            ),
            (
                GitCandidateSubjectError::TreeishMalformed {
                    value: "-x".to_string(),
                },
                "malformed treeish",
            ),
            (
                GitCandidateSubjectError::RepositoryRootInvalid {
                    root: "/gone".to_string(),
                },
                "missing or not a directory",
            ),
            (
                GitCandidateSubjectError::DiffFileConflict {
                    diff_file: "a.diff".to_string(),
                },
                "conflicts with the external diff file",
            ),
            (
                GitCandidateSubjectError::BaseConflict {
                    base: "origin/main".to_string(),
                },
                "conflicts with the top-level base",
            ),
            (
                GitCandidateSubjectError::ExecutionUnsupported,
                "not executable",
            ),
        ];
        for (error, fragment) in cases {
            let text = error.to_string();
            assert!(
                text.contains(fragment),
                "display {text:?} missing {fragment:?}"
            );
            assert!(!error.as_str().is_empty());
        }
    }

    #[test]
    fn debug_output_states_identity_not_results() -> Result<(), String> {
        let subject = GitCandidateSubject::new(
            std::path::Path::new("/repo"),
            GitCandidateBase::Treeish(
                GitTreeish::new("HEAD~1").map_err(|error| error.to_string())?,
            ),
            GitObjectId::parse(SHA256_OID).map_err(|error| error.to_string())?,
        );
        let debug = format!("{subject:?}");
        assert!(debug.contains("repository_root"));
        assert!(
            !debug.to_lowercase().contains("ran"),
            "debug form must not claim analysis ran: {debug:?}"
        );
        Ok(())
    }
}
