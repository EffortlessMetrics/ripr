//! Typed Git input authority for the LSP refresh path (#2000,
//! RIPR-SPEC-0142).
//!
//! One accepted refresh request resolves its load-bearing Git inputs exactly
//! once and carries the resulting typed record. The input identity, the
//! committed snapshot, and the status projection all consume this record; no
//! consumer re-runs an equivalent resolution. The record is request-local
//! with a session cache bounded to the in-flight refresh episode and
//! explicit invalidation on root transitions, input invalidation, and
//! refresh generation acceptance — it is not a global mutable cache.

use std::path::{Path, PathBuf};

/// The resolution probe behind every `Resolved`/`Unresolved` record. Named
/// so status and spec language can point at one bounded command shape
/// instead of an implied Git interaction. The `LoaderDefault` state probes
/// through the `analysis::diff` default-base candidate search
/// (`resolve_default_base_commit`), which is bounded by the same
/// `rev-parse --verify` shape plus one `symbolic-ref` lookup.
#[cfg(test)]
pub(super) const RESOLUTION_COMMAND: &str = "git rev-parse --verify --quiet <base>^{commit}";

/// Resolver contract version. Bump when the resolution semantics change so
/// records produced under different rules are distinguishable.
#[cfg(test)]
pub(super) const RESOLVER_VERSION: &str = "lsp-git-inputs-v1";

/// Typed outcome of the one Git input resolution performed for a refresh
/// request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GitInputResolution {
    /// A requested base ref resolved to an exact commit.
    Resolved,
    /// No base ref was requested: the diff loader's default-base authority
    /// (`analysis::diff` candidate order) applies inside the analysis run.
    /// The record resolves that default base once here through
    /// `analysis::resolve_default_base_commit` and carries its commit
    /// (#2261, RIPR-SPEC-0142 amendment), so default-base workspaces dedup
    /// on the same commit authority as the explicit-base path. A workspace
    /// with no resolvable default base fails closed with no commit; the
    /// analysis run reports the named default-base failure through the
    /// unchanged `load_diff` error path.
    LoaderDefault,
    /// The requested base ref did not resolve to a commit (missing ref,
    /// non-commit target, or Git unavailable). Fail-closed: the analysis run
    /// reports the named base failure through the unchanged `load_diff`
    /// error path, and the identity records the unresolved state so a later
    /// successful resolution invalidates dedup.
    Unresolved,
}

impl GitInputResolution {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::LoaderDefault => "loader_default",
            Self::Unresolved => "unresolved",
        }
    }
}

/// The load-bearing Git inputs for one refresh request, resolved once and
/// shared by every refresh consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedGitInputs {
    /// Effective workspace root the resolution ran against. Root identity is
    /// part of the record so a stale record can never be applied to a
    /// different repository after a root transition.
    root: PathBuf,
    /// Requested base ref from repository config or session options.
    /// `None` selects the diff loader's default-base authority.
    requested_base: Option<String>,
    /// The effective base resolved to an exact commit: the requested base
    /// when one was requested and resolved, or the diff loader's default
    /// base when none was requested and the workspace has a resolvable
    /// default (#2261, RIPR-SPEC-0142 amendment). This is the same value
    /// `analysis::resolve_base_commit` / `analysis::resolve_default_base_commit`
    /// computes; it is resolved once here instead of once per identity
    /// construction. `None` when nothing resolved — never fabricated.
    resolved_base: Option<String>,
    resolution: GitInputResolution,
}

impl ResolvedGitInputs {
    /// Resolve the Git inputs for one refresh request. This is the single
    /// resolution site for the LSP refresh path: one bounded probe runs per
    /// call — a single `git rev-parse` for a requested base, or the loader's
    /// default-base candidate probe when no base was requested.
    pub(super) fn resolve(root: &Path, requested_base: Option<&str>) -> Self {
        let (resolved_base, resolution) = match requested_base {
            Some(base) => {
                let resolved = crate::analysis::resolve_base_commit(root, Some(base));
                let resolution = if resolved.is_some() {
                    GitInputResolution::Resolved
                } else {
                    GitInputResolution::Unresolved
                };
                (resolved, resolution)
            }
            None => {
                let resolved = crate::analysis::resolve_default_base_commit(root)
                    .ok()
                    .map(|(_base, commit)| commit);
                (resolved, GitInputResolution::LoaderDefault)
            }
        };
        Self {
            root: root.to_path_buf(),
            requested_base: requested_base.map(str::to_string),
            resolved_base,
            resolution,
        }
    }

    #[cfg(test)]
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn requested_base(&self) -> Option<&str> {
        self.requested_base.as_deref()
    }

    pub(super) fn resolved_base(&self) -> Option<&str> {
        self.resolved_base.as_deref()
    }

    pub(super) fn resolution(&self) -> GitInputResolution {
        self.resolution
    }

    /// True when this record still governs a request for `root` and
    /// `requested_base`: the record's root and requested base match, so the
    /// resolved value was computed against the same repository and the same
    /// requested input. Revision/generation freshness is decided by the
    /// scheduler; this check only binds the record to its inputs.
    pub(super) fn governs(&self, root: &Path, requested_base: Option<&str>) -> bool {
        self.root == root && self.requested_base.as_deref() == requested_base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::tests::{run_lsp_scope_git, unique_lsp_test_root};

    fn init_repo(root: &Path) -> Result<(), String> {
        run_lsp_scope_git(root, &["init"])?;
        run_lsp_scope_git(root, &["config", "user.email", "ripr@example.invalid"])?;
        run_lsp_scope_git(root, &["config", "user.name", "RIPR Test"])?;
        std::fs::write(
            root.join("lib.rs"),
            "pub fn gate() -> bool {\n    true\n}\n",
        )
        .map_err(|error| format!("write fixture: {error}"))?;
        run_lsp_scope_git(root, &["add", "lib.rs"])?;
        run_lsp_scope_git(root, &["commit", "-m", "base"])
    }

    #[test]
    fn resolution_contract_names_the_probe_and_version() {
        assert!(RESOLUTION_COMMAND.contains("rev-parse --verify"));
        assert_eq!(RESOLVER_VERSION, "lsp-git-inputs-v1");
        assert_eq!(GitInputResolution::Resolved.as_str(), "resolved");
        assert_eq!(GitInputResolution::LoaderDefault.as_str(), "loader_default");
        assert_eq!(GitInputResolution::Unresolved.as_str(), "unresolved");
    }

    #[test]
    fn requested_base_resolves_once_to_the_same_commit_the_analysis_layer_reports()
    -> Result<(), String> {
        let root = unique_lsp_test_root("git-inputs-resolved")?;
        init_repo(root.path())?;
        let record = ResolvedGitInputs::resolve(root.path(), Some("HEAD"));
        if record.resolution() != GitInputResolution::Resolved {
            return Err("requested base must resolve".to_string());
        }
        let expected = crate::analysis::resolve_base_commit(root.path(), Some("HEAD"))
            .ok_or_else(|| "analysis resolver must resolve HEAD".to_string())?;
        if record.resolved_base() != Some(expected.as_str()) {
            return Err(format!(
                "record {} != analysis resolver {expected}",
                record.resolved_base().unwrap_or("<none>")
            ));
        }
        if record.root() != root.path() || record.requested_base() != Some("HEAD") {
            return Err("record must bind its root and requested base".to_string());
        }
        Ok(())
    }

    #[test]
    fn unrequested_base_resolves_the_loader_default_commit() -> Result<(), String> {
        // #2261 (RIPR-SPEC-0142 amendment): the loader-default state carries
        // the same commit the diff loader's default-base authority reports,
        // resolved once here rather than inside identity construction.
        let root = unique_lsp_test_root("git-inputs-loader-default")?;
        init_repo(root.path())?;
        // Pin the default branch name so the loader's default-base fallback
        // resolves deterministically regardless of host git defaults.
        run_lsp_scope_git(root.path(), &["branch", "-M", "main"])?;
        let record = ResolvedGitInputs::resolve(root.path(), None);
        if record.resolution() != GitInputResolution::LoaderDefault {
            return Err("unrequested base must record the loader-default state".to_string());
        }
        let (_base, expected) = crate::analysis::resolve_default_base_commit(root.path())
            .map_err(|error| format!("fixture default base must resolve: {error}"))?;
        if record.resolved_base() != Some(expected.as_str()) {
            return Err(format!(
                "record {} != loader default-base commit {expected}",
                record.resolved_base().unwrap_or("<none>")
            ));
        }
        if record.requested_base().is_some() {
            return Err("loader-default record must carry no requested base".to_string());
        }
        Ok(())
    }

    #[test]
    fn unrequested_base_fails_closed_when_no_default_base_resolves() {
        let record = ResolvedGitInputs::resolve(Path::new("/not-a-repo"), None);
        assert_eq!(record.resolution(), GitInputResolution::LoaderDefault);
        assert_eq!(record.resolved_base(), None);
        assert_eq!(record.requested_base(), None);
    }

    #[test]
    fn missing_ref_fails_closed_as_unresolved() {
        let record = ResolvedGitInputs::resolve(Path::new("/not-a-repo"), Some("missing-ref"));
        assert_eq!(record.resolution(), GitInputResolution::Unresolved);
        assert_eq!(record.resolved_base(), None);
    }

    #[test]
    fn governs_binds_record_to_root_and_requested_base() -> Result<(), String> {
        let root = unique_lsp_test_root("git-inputs-governs")?;
        init_repo(root.path())?;
        let record = ResolvedGitInputs::resolve(root.path(), Some("HEAD"));
        if !record.governs(root.path(), Some("HEAD")) {
            return Err("record must govern its own inputs".to_string());
        }
        if record.governs(root.path(), Some("other")) || record.governs(root.path(), None) {
            return Err("requested base change must not be governed".to_string());
        }
        if record.governs(Path::new("/elsewhere"), Some("HEAD")) {
            return Err("root change must not be governed".to_string());
        }
        Ok(())
    }

    #[test]
    fn dirty_tracked_worktree_does_not_change_the_resolved_inputs() -> Result<(), String> {
        let root = unique_lsp_test_root("git-inputs-dirty")?;
        init_repo(root.path())?;
        let clean = ResolvedGitInputs::resolve(root.path(), Some("HEAD"));
        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn gate() -> bool {\n    false\n}\n",
        )
        .map_err(|error| format!("dirty the worktree: {error}"))?;
        let dirty = ResolvedGitInputs::resolve(root.path(), Some("HEAD"));
        if clean != dirty {
            return Err(
                "uncommitted edits must not alter resolved base identity; the diff path \
                 analyzes committed history and the CLI dirty-worktree disclosure is a \
                 separate, unchanged surface"
                    .to_string(),
            );
        }
        Ok(())
    }
}
