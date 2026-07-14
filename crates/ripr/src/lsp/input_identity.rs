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
        let session_options_identity = Some(format!(
            "include_unchanged_tests={};seam_diagnostics={}",
            config.include_unchanged_tests, config.enable_seam_diagnostics
        ));
        Self::new(
            effective_root,
            saved_workspace_revision,
            InputIdentityComponents {
                repository_config_identity: None,
                session_options_identity,
                requested_base: config.base_ref.clone(),
                resolved_base: None,
            },
            config.mode.clone(),
            "default",
            enabled_languages.iter().copied(),
            None,
            None,
            env!("CARGO_PKG_VERSION"),
            "lsp-analysis-input-v1",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
