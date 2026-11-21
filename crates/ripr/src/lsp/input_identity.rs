use super::config::LspAnalysisConfig;
use crate::app::Mode;
use crate::domain::LanguageId;
use serde_json::Value;
use std::path::PathBuf;

/// Versioned semantic inputs that identify one LSP analysis session state.
///
/// This is deliberately a pure data model. Computing content identities,
/// resolving a base revision, and deciding which filesystem changes matter are
/// separate concerns owned by the reload and analysis layers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LspAnalysisInputIdentity {
    /// The selected root remains part of the session identity even when two
    /// repositories contain equivalent source and configuration.
    pub(super) effective_root: PathBuf,
    pub(super) saved_workspace_revision: u64,
    pub(super) repository_config_identity: Option<String>,
    pub(super) session_options_identity: Option<String>,
    pub(super) requested_base: Option<String>,
    pub(super) resolved_base: Option<String>,
    pub(super) mode: Mode,
    pub(super) profile: String,
    pub(super) enabled_languages: Vec<String>,
    pub(super) manifest_identity: Option<String>,
    pub(super) lockfile_identity: Option<String>,
    pub(super) analyzer_version: String,
    pub(super) schema_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InputIdentityComponents {
    repository_config_identity: Option<String>,
    session_options_identity: Option<String>,
    requested_base: Option<String>,
    resolved_base: Option<String>,
}

impl LspAnalysisInputIdentity {
    #[expect(
        clippy::too_many_arguments,
        reason = "identity construction keeps the versioned input fields explicit"
    )]
    fn new(
        effective_root: PathBuf,
        saved_workspace_revision: u64,
        components: InputIdentityComponents,
        mode: Mode,
        profile: impl Into<String>,
        enabled_languages: impl IntoIterator<Item = LanguageId>,
        manifest_identity: Option<String>,
        lockfile_identity: Option<String>,
        analyzer_version: impl Into<String>,
        schema_version: impl Into<String>,
    ) -> Self {
        let mut enabled_languages = enabled_languages
            .into_iter()
            .map(|language| language.as_str().to_string())
            .collect::<Vec<_>>();
        enabled_languages.sort_unstable();
        enabled_languages.dedup();

        Self {
            effective_root,
            saved_workspace_revision,
            repository_config_identity: components.repository_config_identity,
            session_options_identity: components.session_options_identity,
            requested_base: components.requested_base,
            resolved_base: components.resolved_base,
            mode,
            profile: profile.into(),
            enabled_languages,
            manifest_identity,
            lockfile_identity,
            analyzer_version: analyzer_version.into(),
            schema_version: schema_version.into(),
        }
    }

    /// Compatibility constructor for test fixtures that do not own a
    /// scheduler-resolved record. Production refresh construction goes
    /// through [`Self::from_refresh_inputs_with_git`] with the scheduler's
    /// one record.
    #[cfg(test)]
    pub(super) fn from_refresh_inputs(
        effective_root: PathBuf,
        saved_workspace_revision: u64,
        config: &LspAnalysisConfig,
    ) -> Self {
        // Resolve the Git inputs once and delegate to the record-consuming
        // constructor (#2000, RIPR-SPEC-0142) so this compatibility path and
        // the scheduler path produce byte-identical identities.
        let git_inputs = super::git_inputs::ResolvedGitInputs::resolve(
            &effective_root,
            config.base_ref.as_deref(),
            None,
        );
        Self::from_refresh_inputs_with_git(
            effective_root,
            saved_workspace_revision,
            config,
            &git_inputs,
        )
    }

    /// Build the identity from the one typed Git-input record resolved for
    /// the refresh request (#2000, RIPR-SPEC-0142). The record's resolved
    /// base is consumed verbatim — this constructor never re-resolves — so
    /// the identity, the scheduler dedup decision, and the committed
    /// snapshot all share exactly one resolution per accepted refresh.
    pub(super) fn from_refresh_inputs_with_git(
        effective_root: PathBuf,
        saved_workspace_revision: u64,
        config: &LspAnalysisConfig,
        git_inputs: &super::git_inputs::ResolvedGitInputs,
    ) -> Self {
        let enabled_languages = config.repo_config().languages().enabled();
        let (manifest_identity, lockfile_identity) =
            crate::analysis::seam_cache::workspace_named_file_identities(&effective_root);
        let session_options_identity = config.session_options.as_ref().map(|_| {
            crate::config::config_fingerprint(&format!(
                "base_ref={:?};mode={};include_unchanged_tests={};seam_diagnostics={}",
                config.base_ref,
                config.mode.as_str(),
                config.include_unchanged_tests,
                config.enable_seam_diagnostics
            ))
        });
        Self::new(
            effective_root.clone(),
            saved_workspace_revision,
            InputIdentityComponents {
                repository_config_identity: config
                    .repo_config()
                    .source_text()
                    .map(crate::config::config_fingerprint),
                session_options_identity,
                requested_base: config.base_ref.clone(),
                resolved_base: git_inputs.resolved_base().map(str::to_string),
            },
            config.mode.clone(),
            config.diagnostic_profile.as_str(),
            enabled_languages.iter().copied(),
            manifest_identity,
            lockfile_identity,
            env!("CARGO_PKG_VERSION"),
            "lsp-analysis-input-v1",
        )
    }

    /// Stable opaque identity for status, snapshot provenance, and client
    /// comparisons. The individual fields remain internal authority; the
    /// published value is deliberately bounded and does not expose config
    /// contents or absolute paths.
    pub(super) fn stable_id(&self) -> String {
        let canonical = format!(
            "root={};saved_revision={};repo_config={:?};session_options={:?};requested_base={:?};resolved_base={:?};mode={};profile={};languages={};manifest={:?};lockfile={:?};analyzer={};schema={}",
            self.effective_root.to_string_lossy().replace('\\', "/"),
            self.saved_workspace_revision,
            self.repository_config_identity,
            self.session_options_identity,
            self.requested_base,
            self.resolved_base,
            self.mode.as_str(),
            self.profile,
            self.enabled_languages.join(","),
            self.manifest_identity,
            self.lockfile_identity,
            self.analyzer_version,
            self.schema_version,
        );
        format!("input:{}", crate::config::config_fingerprint(&canonical))
    }

    /// The Git input resolution state derived from the identity's own
    /// requested/resolved base pair (#2000, RIPR-SPEC-0142). Pure projection
    /// of existing identity fields — no additional resolution state is
    /// stored on the identity.
    fn git_input_resolution(&self) -> &'static str {
        match (
            self.requested_base.as_deref(),
            self.resolved_base.as_deref(),
        ) {
            (None, _) => "loader_default",
            (Some(_), Some(_)) => "resolved",
            (Some(_), None) => "unresolved",
        }
    }

    /// Produce the bounded, producer-owned input view used by workspace
    /// status. This is metadata for lifecycle recovery, not a replacement for
    /// the opaque stable identity used by semantic consumers.
    pub(super) fn status_payload(&self) -> Value {
        let root = self.effective_root.to_string_lossy().replace('\\', "/");
        serde_json::json!({
            "input_identity": self.stable_id(),
            "root_identity": format!("root:{}", crate::config::config_fingerprint(&root)),
            "effective_root": self.effective_root,
            "saved_workspace_revision": self.saved_workspace_revision,
            "repository_config_identity": self.repository_config_identity,
            "session_options_identity": self.session_options_identity,
            "requested_base": self.requested_base,
            "resolved_base": self.resolved_base,
            "git_input_resolution": self.git_input_resolution(),
            "mode": self.mode.as_str(),
            "profile": self.profile,
            "enabled_languages": self.enabled_languages,
            "manifest_identity": self.manifest_identity,
            "lockfile_identity": self.lockfile_identity,
            "analyzer_version": self.analyzer_version,
            "schema_version": self.schema_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LspDiagnosticProfile;
    use serde_json::json;

    fn identity(
        root: &str,
        languages: impl IntoIterator<Item = LanguageId>,
    ) -> LspAnalysisInputIdentity {
        LspAnalysisInputIdentity::new(
            PathBuf::from(root),
            7,
            InputIdentityComponents {
                repository_config_identity: Some("config:abc".to_string()),
                session_options_identity: Some("session:xyz".to_string()),
                requested_base: Some("origin/main".to_string()),
                resolved_base: Some("abc123".to_string()),
            },
            Mode::Draft,
            "default",
            languages,
            Some("manifest:abc".to_string()),
            Some("lockfile:def".to_string()),
            "ripr:0.10.0",
            "lsp-input-v1",
        )
    }

    #[test]
    fn identity_is_deterministic_and_language_set_order_independent() {
        let first = identity("workspace-root", [LanguageId::TypeScript, LanguageId::Rust]);
        let second = identity("workspace-root", [LanguageId::Rust, LanguageId::TypeScript]);

        assert_eq!(first, second);
        assert_eq!(first.enabled_languages, ["rust", "typescript"]);
    }

    #[test]
    fn duplicate_language_inputs_do_not_change_identity() {
        let with_duplicate = identity("workspace-root", [LanguageId::Rust, LanguageId::Rust]);
        let without_duplicate = identity("workspace-root", [LanguageId::Rust]);

        assert_eq!(with_duplicate, without_duplicate);
    }

    #[test]
    fn selected_root_remains_part_of_session_identity() {
        let first = identity("workspace-root-a", [LanguageId::Rust]);
        let second = identity("workspace-root-b", [LanguageId::Rust]);

        assert_ne!(first, second);
    }

    #[test]
    fn every_declared_input_participates_in_identity() {
        let base = identity("workspace-root", [LanguageId::Rust]);
        let changed_revision = LspAnalysisInputIdentity {
            saved_workspace_revision: 8,
            ..base.clone()
        };
        let changed_schema = LspAnalysisInputIdentity {
            schema_version: "lsp-input-v2".to_string(),
            ..base.clone()
        };

        assert_ne!(
            changed_revision,
            identity("workspace-root", [LanguageId::Rust])
        );
        assert_ne!(
            changed_schema,
            identity("workspace-root", [LanguageId::Rust])
        );

        let changed_repository_config = LspAnalysisInputIdentity {
            repository_config_identity: Some("config:other".to_string()),
            ..base.clone()
        };
        let changed_session_options = LspAnalysisInputIdentity {
            session_options_identity: Some("session:other".to_string()),
            ..base.clone()
        };
        let changed_requested_base = LspAnalysisInputIdentity {
            requested_base: Some("origin/develop".to_string()),
            ..base.clone()
        };
        let changed_resolved_base = LspAnalysisInputIdentity {
            resolved_base: Some("def456".to_string()),
            ..base.clone()
        };
        let changed_mode = LspAnalysisInputIdentity {
            mode: Mode::Ready,
            ..base.clone()
        };
        let changed_profile = LspAnalysisInputIdentity {
            profile: "other".to_string(),
            ..base.clone()
        };
        let changed_manifest = LspAnalysisInputIdentity {
            manifest_identity: Some("manifest:other".to_string()),
            ..base.clone()
        };
        let changed_lockfile = LspAnalysisInputIdentity {
            lockfile_identity: Some("lockfile:other".to_string()),
            ..base.clone()
        };
        let changed_analyzer_version = LspAnalysisInputIdentity {
            analyzer_version: "other-version".to_string(),
            ..base
        };

        let baseline = identity("workspace-root", [LanguageId::Rust]);
        assert_ne!(changed_repository_config, baseline);
        assert_ne!(changed_session_options, baseline);
        assert_ne!(changed_requested_base, baseline);
        assert_ne!(changed_resolved_base, baseline);
        assert_ne!(changed_mode, baseline);
        assert_ne!(changed_profile, baseline);
        assert_ne!(changed_manifest, baseline);
        assert_ne!(changed_lockfile, baseline);
        assert_ne!(changed_analyzer_version, baseline);
    }

    #[test]
    fn record_built_identity_matches_legacy_resolution_byte_for_byte() -> Result<(), String> {
        let root = crate::lsp::tests::unique_lsp_test_root("input-identity-git-parity")?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["init"])?;
        crate::lsp::tests::run_lsp_scope_git(
            root.path(),
            &["config", "user.email", "ripr@example.invalid"],
        )?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["config", "user.name", "RIPR Test"])?;
        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn gate() -> bool {\n    true\n}\n",
        )
        .map_err(|error| format!("write fixture: {error}"))?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["add", "lib.rs"])?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["commit", "-m", "base"])?;

        let config = LspAnalysisConfig {
            base_ref: Some("HEAD".to_string()),
            ..LspAnalysisConfig::default()
        };
        let record =
            crate::lsp::git_inputs::ResolvedGitInputs::resolve(root.path(), Some("HEAD"), None);
        let via_record = LspAnalysisInputIdentity::from_refresh_inputs_with_git(
            root.path().to_path_buf(),
            1,
            &config,
            &record,
        );
        let via_legacy =
            LspAnalysisInputIdentity::from_refresh_inputs(root.path().to_path_buf(), 1, &config);

        if via_record.resolved_base.is_none() {
            return Err("fixture must produce a resolved base".to_string());
        }
        if via_record != via_legacy || via_record.stable_id() != via_legacy.stable_id() {
            return Err(
                "record-built identity must be byte-identical to the legacy resolution path"
                    .to_string(),
            );
        }
        if via_record.status_payload()["git_input_resolution"].as_str() != Some("resolved") {
            return Err("resolved identity must project the resolved label".to_string());
        }
        Ok(())
    }

    #[test]
    fn default_base_identity_matches_legacy_resolution_byte_for_byte() -> Result<(), String> {
        // #2261 (RIPR-SPEC-0142 amendment): with no requested base, the
        // record-consuming constructor and the compatibility constructor
        // produce byte-identical identities, and the identity carries the
        // loader's resolved default-base commit.
        let root = crate::lsp::tests::unique_lsp_test_root("input-identity-default-base-parity")?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["init"])?;
        crate::lsp::tests::run_lsp_scope_git(
            root.path(),
            &["config", "user.email", "ripr@example.invalid"],
        )?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["config", "user.name", "RIPR Test"])?;
        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn gate() -> bool {\n    true\n}\n",
        )
        .map_err(|error| format!("write fixture: {error}"))?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["add", "lib.rs"])?;
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["commit", "-m", "base"])?;
        // Pin the default branch name so the loader's default-base fallback
        // resolves deterministically regardless of host git defaults.
        crate::lsp::tests::run_lsp_scope_git(root.path(), &["branch", "-M", "main"])?;

        let config = LspAnalysisConfig {
            base_ref: None,
            ..LspAnalysisConfig::default()
        };
        let record = crate::lsp::git_inputs::ResolvedGitInputs::resolve(root.path(), None, None);
        let via_record = LspAnalysisInputIdentity::from_refresh_inputs_with_git(
            root.path().to_path_buf(),
            1,
            &config,
            &record,
        );
        let via_legacy =
            LspAnalysisInputIdentity::from_refresh_inputs(root.path().to_path_buf(), 1, &config);

        let Some(resolved) = via_record.resolved_base.as_deref() else {
            return Err("default-base fixture must produce a resolved base".to_string());
        };
        let (_base, expected) = crate::analysis::resolve_default_base_commit(root.path(), None)
            .map_err(|error| format!("fixture default base must resolve: {error}"))?;
        if resolved != expected {
            return Err(format!(
                "identity resolved base {resolved} != loader default-base commit {expected}"
            ));
        }
        if via_record != via_legacy || via_record.stable_id() != via_legacy.stable_id() {
            return Err(
                "record-built default-base identity must be byte-identical to the legacy \
                 resolution path"
                    .to_string(),
            );
        }
        if via_record.status_payload()["git_input_resolution"].as_str() != Some("loader_default") {
            return Err("default-base identity must project the loader_default label".to_string());
        }
        Ok(())
    }

    #[test]
    fn status_payload_projects_loader_default_and_unresolved_labels() {
        let mut unresolved = identity("workspace-root", [LanguageId::Rust]);
        unresolved.resolved_base = None;
        assert_eq!(
            unresolved.status_payload()["git_input_resolution"].as_str(),
            Some("unresolved")
        );
        let mut loader_default = identity("workspace-root", [LanguageId::Rust]);
        loader_default.requested_base = None;
        loader_default.resolved_base = None;
        assert_eq!(
            loader_default.status_payload()["git_input_resolution"].as_str(),
            Some("loader_default")
        );
    }

    #[test]
    fn repository_config_source_content_participates_in_refresh_identity() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-lsp-input-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("clock failed: {error}"))?
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).map_err(|error| format!("create root failed: {error}"))?;
        let result = (|| {
            std::fs::write(
                root.join(crate::config::CONFIG_FILE_NAME),
                "[analysis]\nmode = \"draft\"\n",
            )
            .map_err(|error| format!("write first config failed: {error}"))?;
            let first = crate::config::load_for_root(&root)?;
            std::fs::write(
                root.join(crate::config::CONFIG_FILE_NAME),
                "[analysis]\nmode = \"fast\"\n",
            )
            .map_err(|error| format!("write second config failed: {error}"))?;
            let second = crate::config::load_for_root(&root)?;
            let first = LspAnalysisConfig::from_repo_config_and_options(first, None);
            let second = LspAnalysisConfig::from_repo_config_and_options(second, None);

            let first_identity =
                LspAnalysisInputIdentity::from_refresh_inputs(root.clone(), 1, &first);
            let second_identity =
                LspAnalysisInputIdentity::from_refresh_inputs(root.clone(), 1, &second);

            assert_ne!(
                first_identity.repository_config_identity,
                second_identity.repository_config_identity
            );
            assert_ne!(first_identity, second_identity);
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn diagnostic_profile_participates_in_refresh_identity() -> Result<(), String> {
        let root = PathBuf::from("/workspace");
        let actionable = LspAnalysisConfig {
            diagnostic_profile: LspDiagnosticProfile::Actionable,
            ..LspAnalysisConfig::default()
        };
        let mut full = actionable.clone();
        full.diagnostic_profile = LspDiagnosticProfile::Full;

        let actionable_identity =
            LspAnalysisInputIdentity::from_refresh_inputs(root.clone(), 1, &actionable);
        let full_identity = LspAnalysisInputIdentity::from_refresh_inputs(root, 1, &full);

        if actionable_identity.stable_id() == full_identity.stable_id() {
            return Err("diagnostic profile changes must invalidate input identity".to_string());
        }
        if actionable_identity.profile != "actionable" || full_identity.profile != "full" {
            return Err("input identity must retain the resolved diagnostic profile".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_manifest_and_lockfile_content_participate_in_identity() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-lsp-workspace-input-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("clock failed: {error}"))?
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("crates/app"))
            .map_err(|error| format!("create root failed: {error}"))?;
        let result = (|| {
            std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n")
                .map_err(|error| format!("write manifest failed: {error}"))?;
            std::fs::write(
                root.join("crates/app/Cargo.toml"),
                "[package]\nname='app'\n",
            )
            .map_err(|error| format!("write member manifest failed: {error}"))?;
            std::fs::write(root.join("Cargo.lock"), "version = 4\n")
                .map_err(|error| format!("write lockfile failed: {error}"))?;
            let config = LspAnalysisConfig::default();
            let first = LspAnalysisInputIdentity::from_refresh_inputs(root.clone(), 1, &config);
            std::fs::write(root.join("Cargo.lock"), "version = 4\n# changed\n")
                .map_err(|error| format!("rewrite lockfile failed: {error}"))?;
            let second = LspAnalysisInputIdentity::from_refresh_inputs(root.clone(), 1, &config);

            if first.lockfile_identity == second.lockfile_identity {
                return Err("lockfile content change did not change input identity".to_string());
            }
            if first.manifest_identity.is_none() || second.manifest_identity.is_none() {
                return Err("workspace manifests should produce an identity".to_string());
            }
            if first.stable_id() == second.stable_id() {
                return Err("changed workspace input retained the same stable id".to_string());
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn equivalent_session_option_order_has_one_identity() {
        let first = LspAnalysisConfig::from_repo_config_and_options(
            crate::config::RiprConfig::default(),
            Some(&json!({
                "baseRef": "origin/main",
                "checkMode": "fast",
                "includeUnchangedTests": true,
            })),
        );
        let second = LspAnalysisConfig::from_repo_config_and_options(
            crate::config::RiprConfig::default(),
            Some(&json!({
                "includeUnchangedTests": true,
                "checkMode": "fast",
                "baseRef": "origin/main",
            })),
        );
        let first_identity =
            LspAnalysisInputIdentity::from_refresh_inputs(PathBuf::from("/workspace"), 1, &first);
        let second_identity =
            LspAnalysisInputIdentity::from_refresh_inputs(PathBuf::from("/workspace"), 1, &second);

        assert_eq!(first_identity, second_identity);
    }
}
