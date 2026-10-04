use crate::agent::artifact::{
    ArtifactCurrentness, RepoExposureArtifactContext, validate_repo_exposure_artifact,
};
use crate::agent::loop_commands::{
    bound_root, bound_root_path, check_repo_exposure_command, lexically_clean, shell_arg,
};
use crate::output::gap_decision_ledger::{
    self, GapDecisionLedgerInput, GapDecisionLedgerSourceKind, GapRecord,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(super) const CURRENT: &str = "current";
pub(super) const STALE: &str = "stale";
pub(super) const NOT_EVALUATED: &str = "not_evaluated";
pub(super) const QUEUED: &str = "queued";
pub(super) const BLOCKED_STALE: &str = "blocked_stale";
pub(super) const BLOCKED_NOT_EVALUATED: &str = "blocked_not_evaluated";
const DEFAULT_REPO_EXPOSURE_PATH: &str = "target/ripr/reports/repo-exposure.json";
const DEFAULT_CHECK_OUTPUT_PATH: &str = "target/ripr/reports/check.json";
const CHECK_OUTPUT_SOURCE_KIND: &str = "check_output";

pub(crate) struct GapRecordSourceInput<'a> {
    pub(crate) root: &'a Path,
    pub(crate) gap_ledger_path: &'a Path,
    pub(crate) ledger_root: Option<&'a str>,
    pub(crate) source_kind: Option<&'a str>,
    pub(crate) records_path: Option<&'a str>,
    pub(crate) source_identity_error: Option<&'a str>,
    pub(crate) records: &'a [GapRecord],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GapRecordSourceCurrentness {
    pub(crate) status: String,
    pub(crate) queue_state: String,
    pub(crate) reason: String,
    pub(crate) refresh_commands: Vec<String>,
    pub(crate) source_kind: Option<String>,
    pub(crate) source_path: Option<String>,
}

impl GapRecordSourceCurrentness {
    pub(super) fn current(
        reason: impl Into<String>,
        refresh_commands: Vec<String>,
        source_kind: Option<String>,
        source_path: Option<String>,
    ) -> Self {
        Self {
            status: CURRENT.to_string(),
            queue_state: QUEUED.to_string(),
            reason: reason.into(),
            refresh_commands,
            source_kind,
            source_path,
        }
    }

    pub(super) fn stale(
        reason: impl Into<String>,
        refresh_commands: Vec<String>,
        source_kind: Option<String>,
        source_path: Option<String>,
    ) -> Self {
        Self {
            status: STALE.to_string(),
            queue_state: BLOCKED_STALE.to_string(),
            reason: reason.into(),
            refresh_commands,
            source_kind,
            source_path,
        }
    }

    pub(super) fn not_evaluated(
        reason: impl Into<String>,
        refresh_commands: Vec<String>,
        source_kind: Option<String>,
        source_path: Option<String>,
    ) -> Self {
        Self {
            status: NOT_EVALUATED.to_string(),
            queue_state: BLOCKED_NOT_EVALUATED.to_string(),
            reason: reason.into(),
            refresh_commands,
            source_kind,
            source_path,
        }
    }

    pub(crate) fn is_assignable(&self) -> bool {
        self.status == CURRENT && self.queue_state == QUEUED
    }

    pub(super) fn json(&self) -> Value {
        json!({
            "status": self.status.as_str(),
            "queue_state": self.queue_state.as_str(),
            "reason": self.reason.as_str(),
            "refresh_commands": &self.refresh_commands,
            "source_kind": self.source_kind.as_deref(),
            "source_path": self.source_path.as_deref(),
        })
    }
}

/// Establish whether one persisted GapRecord source still describes the
/// selected checkout. Positive authority is deliberately narrow: only an
/// exact, producer-validated repo-exposure artifact whose freshly derived
/// GapRecords match the ledger (apart from receipt lifecycle metadata) can be
/// `current`. Missing or unsupported identity remains `not_evaluated`.
pub(crate) fn evaluate_gap_record_source_currentness(
    input: GapRecordSourceInput<'_>,
) -> GapRecordSourceCurrentness {
    let refresh_commands = refresh_commands(
        input.root,
        input.gap_ledger_path,
        input.source_kind,
        input.records_path,
    );
    let source_kind_owned = input.source_kind.map(ToString::to_string);
    let source_path_owned = input.records_path.map(ToString::to_string);

    let canonical_root = match input.root.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!("selected root could not be canonicalized for live currentness: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    let Some(ledger_root) = input
        .ledger_root
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return GapRecordSourceCurrentness::not_evaluated(
            "gap ledger is missing root provenance; regenerate it before assignment",
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    };
    let declared_root = resolve_declared_path(&canonical_root, ledger_root);
    let canonical_ledger_root = match declared_root.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!("gap ledger root provenance could not be validated: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    if canonical_ledger_root != canonical_root {
        return GapRecordSourceCurrentness::stale(
            format!(
                "gap ledger root {} does not match selected root {}; regenerate the source and ledger for this checkout",
                canonical_ledger_root.display(),
                canonical_root.display()
            ),
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    }

    if let Some(error) = input.source_identity_error {
        return GapRecordSourceCurrentness::not_evaluated(
            format!("gap ledger producer identity is incomplete: {error}"),
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    }
    if input.source_kind == Some(CHECK_OUTPUT_SOURCE_KIND) {
        // The check-output route (the Python/TypeScript preview path) carries
        // no producer snapshot identity, so it stays non-assignable. Its
        // refresh replays the same route instead of steering to the Rust-only
        // repo-exposure route, which would drop the preview records.
        return GapRecordSourceCurrentness::not_evaluated(
            "gap ledger source kind check_output has no live snapshot authority, so this packet cannot be assigned; the ledger record's repair_route stays usable as advisory guidance, and refresh_commands rebuild the check output and ledger after the checkout changes; they replay the recorded Git base, so a check run from --diff must instead be rerun with the same --diff",
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    }
    if input.source_kind != Some("repo_exposure") {
        return GapRecordSourceCurrentness::not_evaluated(
            format!(
                "gap ledger source kind {} has no live snapshot authority; regenerate from a canonical repo-exposure artifact",
                input.source_kind.unwrap_or("missing")
            ),
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    }
    let Some(records_path) = input.records_path else {
        return GapRecordSourceCurrentness::not_evaluated(
            "gap ledger does not identify its repo-exposure source artifact",
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    };

    let source_path = resolve_declared_path(&canonical_root, records_path);
    let canonical_source_path = match source_path.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!(
                    "repo-exposure source {} could not be read for live currentness: {error}",
                    source_path.display()
                ),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    if !canonical_source_path.starts_with(&canonical_root) {
        return GapRecordSourceCurrentness::stale(
            format!(
                "repo-exposure source {} is outside selected root {}; regenerate inside the selected checkout",
                canonical_source_path.display(),
                canonical_root.display()
            ),
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    }
    let raw = match std::fs::read_to_string(&canonical_source_path) {
        Ok(raw) => raw,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!(
                    "repo-exposure source {} could not be read: {error}",
                    canonical_source_path.display()
                ),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    let validated = match validate_repo_exposure_artifact(
        &canonical_root,
        &raw,
        "swarm live-currentness source",
    ) {
        Ok(validated) => validated,
        Err(error) if error.contains("repository root") && error.contains("does not match") => {
            return GapRecordSourceCurrentness::stale(
                format!("repo-exposure source belongs to another root: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!("repo-exposure source identity could not be validated: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };

    // Recompute the producer identity from the selected checkout. The
    // validator's tracked-only dirty check intentionally does not see a newly
    // added untracked ripr.toml, even though its consumed fields affect the
    // repo-exposure seam inventory.
    let current_config = match crate::config::load_for_root(&canonical_root) {
        Ok(config) => config,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!("current ripr.toml configuration could not be loaded: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    let current_identity = match RepoExposureArtifactContext::for_repo_exposure(
        canonical_root.clone(),
        validated.analysis_mode.clone(),
        validated.base_revision.clone(),
        &current_config,
    ) {
        Ok(context) => context.input_identity,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!("current repo-exposure input identity could not be recomputed: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    if current_identity != validated.input_identity {
        return GapRecordSourceCurrentness::stale(
            "repo-exposure source input identity no longer matches the current producer configuration; regenerate the source and ledger before assignment",
            refresh_commands,
            source_kind_owned,
            source_path_owned,
        );
    }

    let source_display = crate::output::outcome::display_path(&canonical_source_path);
    let derived_records = match derive_repo_exposure_records(&canonical_root, &source_display, raw)
    {
        Ok(records) => records,
        Err(error) => {
            return GapRecordSourceCurrentness::not_evaluated(
                format!("repo-exposure source could not reproduce the gap ledger: {error}"),
                refresh_commands,
                source_kind_owned,
                source_path_owned,
            );
        }
    };
    classify_validated_source(
        validated.currentness,
        source_records_match(&derived_records, input.records),
        refresh_commands,
        source_kind_owned,
        source_path_owned,
    )
}

fn classify_validated_source(
    currentness: ArtifactCurrentness,
    records_match: bool,
    refresh_commands: Vec<String>,
    source_kind: Option<String>,
    source_path: Option<String>,
) -> GapRecordSourceCurrentness {
    match currentness {
        ArtifactCurrentness::Current if records_match => GapRecordSourceCurrentness::current(
            "producer-validated repo-exposure source matches the selected canonical root, exact clean HEAD, snapshot identity, content commitment, and persisted GapRecords",
            refresh_commands,
            source_kind,
            source_path,
        ),
        ArtifactCurrentness::Current => GapRecordSourceCurrentness::stale(
            "persisted GapRecords no longer match the records derived from the current producer artifact; regenerate the ledger before assignment",
            refresh_commands,
            source_kind,
            source_path,
        ),
        ArtifactCurrentness::Historical => GapRecordSourceCurrentness::stale(
            "repo-exposure source was produced for a historical HEAD; refresh the source and ledger before assignment",
            refresh_commands,
            source_kind,
            source_path,
        ),
        ArtifactCurrentness::DirtyWorktree => GapRecordSourceCurrentness::stale(
            "repo-exposure source or selected checkout has tracked worktree changes; refresh after the checkout is clean",
            refresh_commands,
            source_kind,
            source_path,
        ),
    }
}

fn derive_repo_exposure_records(
    root: &Path,
    source_path: &str,
    raw: String,
) -> Result<Vec<GapRecord>, String> {
    let report = gap_decision_ledger::build_gap_decision_ledger_report(GapDecisionLedgerInput {
        root: crate::output::outcome::display_path(root),
        generated_at: "live-currentness-replay".to_string(),
        source_kind: GapDecisionLedgerSourceKind::RepoExposure,
        records_path: source_path.to_string(),
        records_json: Ok(raw),
    });
    let rendered = gap_decision_ledger::render_gap_decision_ledger_json(&report)?;
    gap_decision_ledger::parse_gap_records_json(&rendered)
}

fn source_records_match(derived: &[GapRecord], persisted: &[GapRecord]) -> bool {
    derived.len() == persisted.len()
        && derived.iter().zip(persisted).all(|(derived, persisted)| {
            let mut derived = derived.clone();
            let mut persisted = persisted.clone();
            derived.receipt = None;
            persisted.receipt = None;
            derived == persisted
        })
}

fn resolve_declared_path(root: &Path, declared: &str) -> PathBuf {
    let declared = Path::new(declared);
    if declared.is_absolute() {
        declared.to_path_buf()
    } else {
        root.join(declared)
    }
}

fn refresh_commands(
    root: &Path,
    gap_ledger_path: &Path,
    source_kind: Option<&str>,
    source_path: Option<&str>,
) -> Vec<String> {
    let root_display = bound_root(&crate::output::outcome::display_path(root));
    let check_output = source_kind == Some(CHECK_OUTPUT_SOURCE_KIND);
    // The source artifact is anchored to the same bound root as `--root`, so
    // the generated source path stays absolute and names the selected
    // repository from any working directory (#3999). The ledger output path
    // is the caller's own path and is rendered unchanged.
    let command_root = bound_root_path(root);
    let default_source = if check_output {
        DEFAULT_CHECK_OUTPUT_PATH
    } else {
        DEFAULT_REPO_EXPOSURE_PATH
    };
    let source_path = source_path
        // Clean `.`/`..` lexically before the containment check:
        // `Path::starts_with` is component-wise on the raw path, so an
        // un-normalized `<root>/../outside.json` would otherwise pass it.
        .map(|path| lexically_clean(&resolve_declared_path(&command_root, path)))
        .filter(|path| path.starts_with(&command_root))
        .unwrap_or_else(|| command_root.join(default_source));
    let source_display = crate::output::outcome::display_path(&source_path);
    let ledger_display = crate::output::outcome::display_path(gap_ledger_path);
    if check_output {
        // Replay the base the existing check output recorded, so the refresh
        // compares the same diff; without one, `ripr check` resolves its
        // default base as the original first-pr route would. Check output
        // does not record whether its scope came from `--diff`, so that case
        // cannot be replayed here; the blocked reason names it instead.
        let base_arg = recorded_check_base(&source_path)
            .map(|base| format!(" --base {}", shell_arg(&base)))
            .unwrap_or_default();
        return vec![
            format!(
                "ripr check --root {}{} --json > {}",
                shell_arg(&root_display),
                base_arg,
                shell_arg(&source_display)
            ),
            format!(
                "ripr reports gap-ledger --check-output {} --root {} --out {}",
                shell_arg(&source_display),
                shell_arg(&root_display),
                shell_arg(&ledger_display)
            ),
        ];
    }
    vec![
        check_repo_exposure_command(&root_display, "draft", &source_display),
        format!(
            "ripr reports gap-ledger --repo-exposure {} --root {} --out {}",
            shell_arg(&source_display),
            shell_arg(&root_display),
            shell_arg(&ledger_display)
        ),
    ]
}

fn recorded_check_base(check_output: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(check_output).ok()?;
    let json = serde_json::from_str::<Value>(&contents).ok()?;
    json.get("base")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|base| !base.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_currentness_requires_current_artifact_and_matching_records() {
        assert!(
            classify_validated_source(ArtifactCurrentness::Current, true, Vec::new(), None, None,)
                .is_assignable()
        );
        assert_eq!(
            classify_validated_source(ArtifactCurrentness::Current, false, Vec::new(), None, None,)
                .status,
            STALE
        );
        assert_eq!(
            classify_validated_source(
                ArtifactCurrentness::Historical,
                true,
                Vec::new(),
                None,
                None,
            )
            .status,
            STALE
        );
    }

    /// Negative control required by issue #3504: the receipt-less legacy
    /// freshness shape (`queue_state = queued`, `staleness_status =
    /// not_evaluated`) must fail the assignment authority. If the typed stop
    /// ever weakened from `status == current` to `status != stale`, this
    /// receipt-less `not_evaluated` record would be incorrectly assignable
    /// and this assertion would fail.
    #[test]
    fn receipt_less_not_evaluated_freshness_is_never_assignable() {
        let receipt_less_freshness = GapRecordSourceCurrentness {
            status: NOT_EVALUATED.to_string(),
            queue_state: QUEUED.to_string(),
            reason:
                "GapRecord queue rendering does not compare the ledger with current git state yet."
                    .to_string(),
            refresh_commands: Vec::new(),
            source_kind: None,
            source_path: None,
        };
        assert!(
            !receipt_less_freshness.is_assignable(),
            "not_evaluated is stop-and-refresh, never assignable"
        );
        assert_eq!(
            receipt_less_freshness.status, NOT_EVALUATED,
            "not_evaluated must be preserved exactly, never relabeled as current"
        );
    }

    #[test]
    fn unsupported_source_kind_is_not_evaluated() -> Result<(), String> {
        let root = unique_test_dir("unsupported-source");
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        let ledger = root.join("gap-ledger.json");
        let currentness = evaluate_gap_record_source_currentness(GapRecordSourceInput {
            root: &root,
            gap_ledger_path: &ledger,
            ledger_root: Some(&root.display().to_string()),
            source_kind: Some("records"),
            records_path: Some("records.json"),
            source_identity_error: None,
            records: &[],
        });
        assert_eq!(currentness.status, NOT_EVALUATED);
        assert!(!currentness.is_assignable());
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        Ok(())
    }

    #[test]
    fn wrong_root_is_stale() -> Result<(), String> {
        let root = unique_test_dir("selected-root");
        let other = unique_test_dir("other-root");
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        std::fs::create_dir_all(&other).map_err(|error| format!("create other: {error}"))?;
        let currentness = evaluate_gap_record_source_currentness(GapRecordSourceInput {
            root: &root,
            gap_ledger_path: &root.join("gap-ledger.json"),
            ledger_root: Some(&other.display().to_string()),
            source_kind: Some("repo_exposure"),
            records_path: Some("repo-exposure.json"),
            source_identity_error: None,
            records: &[],
        });
        assert_eq!(currentness.status, STALE);
        assert!(!currentness.is_assignable());
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        std::fs::remove_dir_all(&other).map_err(|error| format!("remove other: {error}"))?;
        Ok(())
    }

    #[test]
    fn refresh_commands_anchor_repo_exposure_to_the_bound_root() -> Result<(), String> {
        let cwd = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
        let relative_root = Path::new("selected-root");
        let ledger = Path::new("out/gap-ledger.json");
        let bound = bound_root_path(relative_root);
        assert!(
            bound.is_absolute(),
            "bound root must be absolute: {bound:?}"
        );
        assert_eq!(bound, cwd.join("selected-root"));

        // A declared source that lexically escapes the bound root (`..`) must
        // not survive the containment check: `Path::starts_with` compares
        // un-normalized components, so `<root>/../outside.json` would pass it
        // while naming a file outside the selected repository.
        let default_source = bound.join(DEFAULT_REPO_EXPOSURE_PATH);
        for (declared, expected_source) in [
            (
                Some("reports/repo-exposure.json"),
                bound.join("reports/repo-exposure.json"),
            ),
            (None, default_source.clone()),
            (Some("../outside.json"), default_source.clone()),
            (Some("reports/../../outside.json"), default_source.clone()),
        ] {
            let commands = refresh_commands(relative_root, ledger, Some("repo_exposure"), declared);
            let source_display = crate::output::outcome::display_path(&expected_source);
            let root_display = bound_root(&crate::output::outcome::display_path(relative_root));
            let expected_ledger_command = format!(
                "ripr reports gap-ledger --repo-exposure {} --root {} --out {}",
                shell_arg(&source_display),
                shell_arg(&root_display),
                shell_arg("out/gap-ledger.json")
            );
            assert_eq!(commands.len(), 2, "{commands:?}");
            assert_eq!(commands[1], expected_ledger_command);
            assert!(
                Path::new(&source_display).is_absolute(),
                "--repo-exposure must be absolute under the bound root: {source_display}"
            );
            assert!(
                commands[0].contains(&shell_arg(&source_display)),
                "check command must name the anchored source: {}",
                commands[0]
            );
            assert!(
                !commands[1].contains("--repo-exposure selected-root/")
                    && !commands[1].contains("--repo-exposure target/"),
                "--repo-exposure must not stay relative: {}",
                commands[1]
            );
        }
        Ok(())
    }

    #[test]
    fn check_output_refresh_replays_the_check_output_route() -> Result<(), String> {
        let root = unique_test_dir("check-output-refresh");
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports).map_err(|error| format!("create reports: {error}"))?;
        let ledger = Path::new("out/gap-ledger.json");
        let bound = bound_root_path(&root);
        let check = bound.join(DEFAULT_CHECK_OUTPUT_PATH);
        let check_display = crate::output::outcome::display_path(&check);
        let root_display = bound_root(&crate::output::outcome::display_path(&root));
        let expected_ledger = format!(
            "ripr reports gap-ledger --check-output {} --root {} --out {}",
            shell_arg(&check_display),
            shell_arg(&root_display),
            shell_arg("out/gap-ledger.json")
        );

        // No readable check output yet: no base to replay.
        let commands = refresh_commands(&root, ledger, Some("check_output"), None);
        assert_eq!(
            commands,
            vec![
                format!(
                    "ripr check --root {} --json > {}",
                    shell_arg(&root_display),
                    shell_arg(&check_display)
                ),
                expected_ledger.clone(),
            ]
        );

        // The recorded base is replayed; an escaping declared path falls
        // back to the default check output under the bound root.
        std::fs::write(&check, r#"{"base":"origin/trunk","findings":[]}"#)
            .map_err(|error| format!("write check: {error}"))?;
        let commands =
            refresh_commands(&root, ledger, Some("check_output"), Some("../outside.json"));
        assert_eq!(
            commands,
            vec![
                format!(
                    "ripr check --root {} --base origin/trunk --json > {}",
                    shell_arg(&root_display),
                    shell_arg(&check_display)
                ),
                expected_ledger,
            ]
        );
        assert!(
            commands
                .iter()
                .all(|command| !command.contains("repo-exposure"))
        );
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        Ok(())
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        std::env::temp_dir().join(format!("ripr-live-currentness-{name}-{nanos}"))
    }
}
