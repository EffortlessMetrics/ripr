use super::config::LspAnalysisConfig;
use crate::app::Mode;
use crate::domain::LanguageId;
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

    pub(super) fn from_refresh_inputs(
        effective_root: PathBuf,
        saved_workspace_revision: u64,
        config: &LspAnalysisConfig,
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
                resolved_base: crate::analysis::resolve_base_commit(
                    &effective_root,
                    config.base_ref.as_deref(),
                ),
            },
            config.mode.clone(),
            "default",
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
