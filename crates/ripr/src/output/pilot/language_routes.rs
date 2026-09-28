//! Language routes for code that pilot's Rust seam scan does not rank.
//!
//! `ripr pilot` ranks Rust repo seams. A repository whose changed code is
//! TypeScript, JavaScript, Python or Perl produces no Rust seams, and without
//! this model the pilot reported "complete" with no recommendation, which read
//! as a clean result that pilot never computed (#3906).
//!
//! The decision is made once here, from the workspace's preview-language file
//! discovery and the effective language config; the pilot renderers only
//! display it. Wording is reused from its existing owners: the
//! `typescript_diff_first` guidance from the repo-exposure renderer and the
//! unavailable-adapter notice from [`LanguageId`].

use crate::agent::loop_commands::shell_path;
use crate::domain::LanguageId;
use crate::output::repo_exposure::{PythonRepoExposureGuidance, TsFullRepoGuidance};
use std::path::{Path, PathBuf};

/// Languages pilot routes elsewhere, in stable display order.
const ROUTED_LANGUAGE_ORDER: &[LanguageId] = &[
    LanguageId::TypeScript,
    LanguageId::JavaScript,
    LanguageId::Python,
    LanguageId::Perl,
];

/// Whether the language routes change what the pilot's result means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PilotLanguageRoutesState {
    /// No TypeScript, JavaScript, Python or Perl files were found.
    NotDetected,
    /// Rust seams exist, so the pilot ranking stands. Other languages are
    /// listed in `pilot-summary.json` only; the human output is unchanged.
    Supplementary,
    /// Pilot's Rust seam scan produced no seams. An empty ranking says
    /// nothing about these languages, so the human output names each route.
    Required,
}

impl PilotLanguageRoutesState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PilotLanguageRoutesState::NotDetected => "not_detected",
            PilotLanguageRoutesState::Supplementary => "supplementary",
            PilotLanguageRoutesState::Required => "required",
        }
    }
}

/// One language pilot did not rank, and where the user should go instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PilotLanguageRoute {
    pub(crate) language: LanguageId,
    pub(crate) file_count: usize,
    /// The adapter is compiled into this binary.
    pub(crate) available: bool,
    /// The adapter is compiled in and listed in the effective
    /// `[languages] enabled` config.
    pub(crate) enabled: bool,
    /// The diff-first `ripr check` command that analyzes this language, or
    /// `None` when this binary cannot analyze it.
    pub(crate) command: Option<String>,
    /// Stable category of the reused guidance, when one applies.
    pub(crate) guidance_category: Option<&'static str>,
    /// Reused guidance text: the `typescript_diff_first` repair route for
    /// TypeScript/JavaScript, the `python_diff_first` repair route for Python,
    /// or the unavailable-adapter notice.
    pub(crate) guidance: Option<String>,
}

impl PilotLanguageRoute {
    pub(crate) fn language_status(&self) -> &'static str {
        if self.available {
            "preview"
        } else {
            "unavailable"
        }
    }

    pub(crate) fn route(&self) -> &'static str {
        if self.available {
            "check_diff_first"
        } else {
            "unavailable_in_this_binary"
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PilotLanguageRoutes {
    pub(crate) state: PilotLanguageRoutesState,
    pub(crate) routes: Vec<PilotLanguageRoute>,
}

impl PilotLanguageRoutes {
    /// Build the routes from preview-language file discovery
    /// (`analysis::workspace_preview_language_files`), whether pilot's Rust
    /// scan produced any seam, and the effective enabled languages.
    pub(crate) fn from_discovered(
        root: &Path,
        rust_seams_present: bool,
        enabled_languages: &[LanguageId],
        discovered: &[(LanguageId, PathBuf)],
    ) -> Self {
        let mut routes = Vec::new();
        for language in ROUTED_LANGUAGE_ORDER {
            let file_count = discovered
                .iter()
                .filter(|(found, _)| found == language)
                .count();
            if file_count == 0 {
                continue;
            }
            routes.push(route_for(root, *language, file_count, enabled_languages));
        }
        let state = if routes.is_empty() {
            PilotLanguageRoutesState::NotDetected
        } else if rust_seams_present {
            PilotLanguageRoutesState::Supplementary
        } else {
            PilotLanguageRoutesState::Required
        };
        Self { state, routes }
    }

    /// The routes the human output must show: only when pilot found no Rust
    /// seams, so Rust users' output is unchanged.
    pub(crate) fn required(&self) -> Option<&[PilotLanguageRoute]> {
        (self.state == PilotLanguageRoutesState::Required).then_some(self.routes.as_slice())
    }

    /// Distinct runnable route commands, in route order.
    pub(crate) fn commands(routes: &[PilotLanguageRoute]) -> Vec<&String> {
        let mut commands: Vec<&String> = Vec::new();
        for command in routes.iter().filter_map(|route| route.command.as_ref()) {
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
        commands
    }
}

fn route_for(
    root: &Path,
    language: LanguageId,
    file_count: usize,
    enabled_languages: &[LanguageId],
) -> PilotLanguageRoute {
    let available = language.is_available();
    if !available {
        return PilotLanguageRoute {
            language,
            file_count,
            available,
            enabled: false,
            command: None,
            guidance_category: None,
            guidance: language.unavailable_adapter_notice(),
        };
    }
    let typescript_family = matches!(language, LanguageId::TypeScript | LanguageId::JavaScript);
    // JavaScript runs through the TypeScript-family adapter, which the
    // `[languages] enabled = ["typescript"]` entry turns on.
    let config_language = if typescript_family {
        LanguageId::TypeScript
    } else {
        language
    };
    let (guidance_category, guidance) = if typescript_family {
        (
            Some(TsFullRepoGuidance::CATEGORY),
            Some(TsFullRepoGuidance::REPAIR_ROUTE.to_string()),
        )
    } else if language == LanguageId::Python {
        (
            Some(PythonRepoExposureGuidance::CATEGORY),
            Some(PythonRepoExposureGuidance::REPAIR_ROUTE.to_string()),
        )
    } else {
        (None, None)
    };
    PilotLanguageRoute {
        language,
        file_count,
        available,
        enabled: enabled_languages.contains(&config_language),
        command: Some(format!("ripr check --root {}", shell_path(root))),
        guidance_category,
        guidance,
    }
}
