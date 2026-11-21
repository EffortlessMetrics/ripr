//! Cargo target enumeration for source-role confirmation (#3283).
//!
//! ripr never ran Cargo target discovery: `[[test]]` and `[[bench]]`
//! entries were invisible, so an explicit `path = "src/contract_test.rs"`
//! target could not confirm evidence role outside the default `tests/`
//! and `benches/` layouts. This module reads the package manifests that
//! own the analyzed files and extracts exactly the target declarations
//! the source-role model consumes.
//!
//! Bounded scope, stated so the gate does not overclaim:
//!
//! - Only explicit `path = ...` entries on `[[test]]` and `[[bench]]`
//!   targets are extracted. Autodiscovered targets live under `tests/`
//!   and `benches/` by construction, which the layout rules already
//!   classify.
//! - `autotests = false` / `autobenches = false` only suppress
//!   *autodiscovery*; explicit targets remain valid, so the flags are
//!   read but do not change classification.
//! - Malformed manifests never fail analysis: an unreadable or invalid
//!   `Cargo.toml` yields no declared targets for that package, and the
//!   layout rules keep applying (fail-closed toward production, which is
//!   the conservative direction — over-excluding production files is the
//!   named risk of #3283).
//! - Paths are workspace-relative-resolved against the manifest's
//!   directory, then normalized so Windows and POSIX compare equal.
//!
//! ## Metadata-sourced harness validation (#3608, reworked by #3634)
//!
//! The `custom_harness` verdict sources its workspace membership and its
//! test-target inventory from `cargo metadata` itself instead of a
//! manifest TOML emulation: one bounded `cargo metadata --no-deps
//! --offline` probe per batch reports cargo's own member resolution
//! (glob and character-class member patterns, exclude handling, and
//! every path-dependency form including `[workspace.dependencies]`
//! inheritance) and every test target cargo would compile. The `harness`
//! flag is absent from metadata output by construction (verified on the
//! pinned toolchain), so the flag premise still comes from parsing the
//! owning package manifest. An unavailable probe — no cargo binary, a
//! workspace cargo rejects, or an unreadable probe output — fails closed
//! to `manifest_unavailable`: a registration grants nothing, never
//! over-credits.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::source_role::SourceRoleContext;

/// Declared explicit target paths for one workspace, keyed by nothing —
/// a flat set is all the role model needs.
#[derive(Clone, Debug, Default)]
pub(crate) struct DeclaredCargoTargets {
    pub(crate) tests: BTreeSet<PathBuf>,
    pub(crate) benches: BTreeSet<PathBuf>,
}

/// Read `path = ...` entries from the `[[test]]` and `[[bench]]` arrays
/// of one manifest. `manifest_dir` is the directory containing the
/// `Cargo.toml`; declared paths resolve relative to it.
pub(crate) fn declared_targets_from_manifest(
    manifest_text: &str,
    manifest_dir: &Path,
) -> DeclaredCargoTargets {
    let mut targets = DeclaredCargoTargets::default();
    let Ok(value) = toml::from_str::<toml::Value>(manifest_text) else {
        return targets;
    };
    collect_explicit_paths(value.get("test"), manifest_dir, &mut targets.tests);
    collect_explicit_paths(value.get("bench"), manifest_dir, &mut targets.benches);
    targets
}

fn collect_explicit_paths(
    target_entries: Option<&toml::Value>,
    manifest_dir: &Path,
    out: &mut BTreeSet<PathBuf>,
) {
    let Some(entries) = target_entries.and_then(|value| value.as_array()) else {
        return;
    };
    for entry in entries {
        let Some(path) = entry.get("path").and_then(|value| value.as_str()) else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        out.insert(normalize(&manifest_dir.join(path)));
    }
}

/// One declared Cargo test target with its effective `harness` flag
/// (#3608): the parsed Cargo target metadata the harness-registry
/// validation consumes. `harness` carries Cargo's default (`true`) when
/// the manifest omits the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeclaredCargoTestTarget {
    pub(crate) path: PathBuf,
    pub(crate) harness: bool,
    /// The declared target name for a name-only entry. The cargo target
    /// name is the identity that proves cargo retained this entry: a
    /// name-only declaration whose ambiguous dual layout made cargo drop
    /// the target never reaches the flag matcher, even when another
    /// explicit entry claims one of the same layout paths (#3637 review).
    pub(crate) name: Option<String>,
}

/// Enumerate every `[[test]]` target of one manifest with its effective
/// `harness` flag (#3608). Two entry shapes contribute:
///
/// - an explicit `path = ...` entry contributes exactly its resolved path
///   and matches the metadata target by that path;
/// - a name-only entry (no `path`) carries its declared name and matches
///   the metadata target by that name (#3637 review): cargo resolves the
///   entry itself to `tests/<name>.rs` or `tests/<name>/main.rs` — with
///   only the directory layout on disk it reports the directory shape as
///   the entry's source path (verified against `cargo metadata` 1.95.0) —
///   and with both layouts on disk it drops the target entirely, so only
///   the name match can prove cargo retained the entry.
///
/// The flag is the entry's `harness` key when present, Cargo's `true`
/// default otherwise. Entries with neither a name nor a path contribute
/// nothing (fail closed). Malformed manifests yield no targets.
/// Text-taking test form of the parsed-value core; the verdict path
/// parses each manifest once and calls
/// [`declared_test_targets_with_harness_from_value`] directly.
#[cfg(test)]
pub(crate) fn declared_test_targets_with_harness_from_manifest(
    manifest_text: &str,
    manifest_dir: &Path,
) -> Vec<DeclaredCargoTestTarget> {
    match toml::from_str::<toml::Value>(manifest_text) {
        Ok(value) => declared_test_targets_with_harness_from_value(&value, manifest_dir),
        Err(_) => Vec::new(),
    }
}

/// The parsed-manifest core of [`declared_test_targets_with_harness_from_manifest`].
/// The verdict path parses each owning manifest once and reuses the value
/// here (#3608 review): the manifest is only the `harness`-flag source —
/// target identity itself comes from the `cargo metadata` inventory
/// (#3634) — so both explicit `path = ...` entries and name-only entries
/// resolved to their autodiscovery shape contribute.
fn declared_test_targets_with_harness_from_value(
    value: &toml::Value,
    manifest_dir: &Path,
) -> Vec<DeclaredCargoTestTarget> {
    let Some(entries) = value.get("test").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for entry in entries {
        let harness = entry
            .get("harness")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if let Some(path) = entry.get("path").and_then(|value| value.as_str()) {
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            targets.push(DeclaredCargoTestTarget {
                path: lexical(&normalize(&manifest_dir.join(path))),
                harness,
                name: None,
            });
            continue;
        }
        let Some(name) = entry.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        // A name-only entry matches cargo's retained target by NAME, not
        // by a synthesized layout path (#3637 review): cargo resolves the
        // entry to `tests/<name>.rs` or `tests/<name>/main.rs` itself and
        // reports that src_path; with both on disk it drops the target
        // entirely, and only the metadata-side name proves the entry was
        // retained. The path field stays the conventional file layout for
        // display and test ergonomics only.
        targets.push(DeclaredCargoTestTarget {
            path: lexical(&normalize(&manifest_dir.join(format!("tests/{name}.rs")))),
            harness,
            name: Some(name.to_string()),
        });
    }
    targets
}

/// The verdict of one registered harness target against the workspace's
/// Cargo target metadata (#3608; metadata-sourced since #3634).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CargoHarnessVerdict {
    /// The path is a declared `[[test]]` target whose effective
    /// `harness` flag is `false`: the registration's premise holds.
    HarnessDisabled,
    /// The path is a known Cargo test target (explicit entry or package
    /// autodiscovery) whose effective `harness` flag is `true`: the
    /// libtest harness still collects the file, so the `harness = false`
    /// premise of a custom-harness registration does not hold.
    HarnessEnabled,
    /// The workspace's Cargo metadata declares no test target for this
    /// path: nothing in the workspace compiles the file as a test.
    NotDeclared,
    /// No premise about the target can be established: the metadata
    /// probe was unavailable (no cargo binary, a workspace cargo rejects,
    /// or an unreadable probe output), an owning manifest could not be
    /// read or parsed, or the same path is claimed by manifests with
    /// conflicting `harness` flags.
    ManifestUnavailable,
}

/// One analysis pass's parsed owning manifests plus the metadata probe:
/// `cargo metadata` runs at most once per batch and each manifest is read
/// and parsed at most once, so a batch of registrations in one workspace
/// costs one probe, not one per registration.
#[derive(Default)]
pub(crate) struct ManifestInventory {
    manifests: BTreeMap<PathBuf, OwnedManifest>,
    metadata: MetadataState,
}

/// The lazily initialized `cargo metadata` view of the analyzed workspace
/// (#3634). `Failed` records an unavailable probe so the whole batch
/// fails closed instead of re-spawning per verdict.
#[derive(Default)]
enum MetadataState {
    #[default]
    Unloaded,
    Failed,
    /// Workspace test targets keyed by their lexically resolved source
    /// path; each entry lists the owning package manifest directories
    /// with the cargo target name (#3637 review).
    Loaded(BTreeMap<PathBuf, Vec<(PathBuf, String)>>),
}

/// The memoized parse outcome for one owning manifest directory.
#[derive(Clone)]
enum OwnedManifest {
    /// No manifest exists at this directory.
    Absent,
    /// The manifest exists but could not be read or parsed.
    Unresolvable,
    Parsed {
        root: PathBuf,
        value: toml::Value,
    },
}

impl ManifestInventory {
    /// The Cargo target metadata verdict for one registered harness
    /// target path. `workspace_root` anchors the metadata probe; the
    /// registration target is workspace-relative.
    ///
    /// Ownership is cargo's own (#3634): the batch runs one bounded
    /// `cargo metadata --no-deps --offline` probe against the analysis
    /// root, and its `packages[].targets[]` inventory is the authority
    /// for both workspace membership and test-target identity. This
    /// resolves exactly the shapes the previous manifest TOML emulation
    /// approximated — `[workspace.members]` globs including
    /// character classes, `[workspace.exclude]` (cargo treats exclude
    /// patterns as literal path prefixes; a wildcard component matches no
    /// member), `[workspace.dependencies]` inheritance, and dev- and
    /// build-path dependencies. A target missing from the inventory is
    /// `NotDeclared`; a target present in it resolves its `harness` flag
    /// from the owning manifest's `[[test]]` entries (explicit `path`
    /// spellings and name-only defaults alike), because metadata output
    /// omits the `harness` field by construction. Conflicting flags
    /// across owning manifests — the same path claimed by two packages —
    /// are ambiguous and fail closed.
    pub(crate) fn verdict(
        &mut self,
        workspace_root: &Path,
        registration_target: &Path,
    ) -> CargoHarnessVerdict {
        // The inventory keys are cargo's absolute source paths, so the
        // anchored target must be resolved in the same terms. Two forms
        // are tried: the as-given absolute path, and the real path
        // behind any symlinks. Cargo echoes the path family it was
        // given on Windows (a junction root yields alias-form keys,
        // verified 1.95.0) but canonicalizes symlinks on Unix (#3637
        // review, demonstrated by the Linux CI run of the alias pin), so one
        // form alone cannot cover both hosts. Missing on both forms is
        // the fail-closed NotDeclared.
        let joined = workspace_root.join(registration_target);
        let absolute = std::path::absolute(&joined).unwrap_or(joined);
        let anchored = lexical(&normalize(&absolute));
        self.ensure_workspace_metadata(workspace_root);
        let owners = match &self.metadata {
            MetadataState::Loaded(targets) => {
                targets.get(&anchored).cloned().or_else(|| {
                    // `fs::canonicalize` returns verbatim `\\?\`
                    // paths on Windows; those can only match
                    // verbatim keys cargo never echoes, so a miss
                    // there is harmless — the alias-form lookup
                    // above already covered the Windows behavior.
                    let real = std::fs::canonicalize(&absolute).ok()?;
                    targets.get(&lexical(&normalize(&real))).cloned()
                })
            }
            // No metadata premise is available (probe failed or was never
            // loadable): the registration grants nothing. Fail closed —
            // under-credit, never over-credit (#3634).
            MetadataState::Unloaded | MetadataState::Failed => {
                return CargoHarnessVerdict::ManifestUnavailable;
            }
        };
        let Some(owners) = owners else {
            // Cargo's own target inventory has no test target for this
            // path: nothing in the workspace compiles it as a test.
            return CargoHarnessVerdict::NotDeclared;
        };
        let mut flags: Vec<bool> = Vec::new();
        for (manifest_dir, target_name) in &owners {
            match self.manifest_at(manifest_dir) {
                OwnedManifest::Parsed { root, value } => {
                    for target in declared_test_targets_with_harness_from_value(&value, &root) {
                        // Identity, not tokens (#3637 review): a
                        // name-only entry matches by its declared name —
                        // the cargo target name proves cargo retained it
                        // — while an explicit-path entry matches by its
                        // exact path (the metadata src_path of an
                        // explicit target is its declared path). A
                        // declaration cargo dropped from the inventory
                        // (ambiguous dual layout) therefore cannot
                        // contribute its flag to a live target that
                        // another entry owns.
                        let matched = match &target.name {
                            Some(name) => name == target_name,
                            None => target.path == anchored,
                        };
                        if matched {
                            flags.push(target.harness);
                        }
                    }
                }
                // The owning manifest cannot be read or parsed: the
                // `harness` premise is unestablishable even though
                // metadata names the target.
                OwnedManifest::Absent | OwnedManifest::Unresolvable => {
                    return CargoHarnessVerdict::ManifestUnavailable;
                }
            }
        }
        let Some(first) = flags.first().copied() else {
            // Metadata credits the target through package autodiscovery
            // and no manifest entry names it: the libtest harness default
            // applies, so the `harness = false` premise does not hold.
            return CargoHarnessVerdict::HarnessEnabled;
        };
        if flags.iter().all(|flag| *flag == first) {
            if first {
                CargoHarnessVerdict::HarnessEnabled
            } else {
                CargoHarnessVerdict::HarnessDisabled
            }
        } else {
            // Two owning manifests declare the same path with different
            // `harness` flags; which compilation unit collects the file
            // is not statically decidable here.
            CargoHarnessVerdict::ManifestUnavailable
        }
    }

    /// Initialize the metadata probe once per batch. A failed probe is
    /// recorded as [`MetadataState::Failed`] so every verdict in the
    /// batch fails closed deterministically instead of re-spawning.
    fn ensure_workspace_metadata(&mut self, workspace_root: &Path) {
        if matches!(self.metadata, MetadataState::Unloaded) {
            self.metadata = match run_workspace_cargo_metadata(workspace_root) {
                Some(targets) => MetadataState::Loaded(targets),
                None => MetadataState::Failed,
            };
        }
    }

    fn manifest_at(&mut self, dir: &Path) -> OwnedManifest {
        if !dir.join("Cargo.toml").is_file() {
            return OwnedManifest::Absent;
        }
        if let Some(cached) = self.manifests.get(dir) {
            return cached.clone();
        }
        let parsed = match std::fs::read_to_string(dir.join("Cargo.toml"))
            .ok()
            .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
        {
            Some(value) => OwnedManifest::Parsed {
                root: dir.to_path_buf(),
                value,
            },
            None => OwnedManifest::Unresolvable,
        };
        self.manifests.insert(dir.to_path_buf(), parsed.clone());
        parsed
    }
}

/// Process-wide probe sequence: parallel batches (test suites, LSP
/// refreshes) must never share a stdout capture file even when their
/// timestamps land in the same clock tick.
static METADATA_PROBE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The deadline for the one bounded `cargo metadata` probe per batch.
/// `--no-deps --offline` parses workspace manifests only — no network,
/// no dependency resolution, no build scripts — so the deadline exists
/// to fail closed instead of stalling an interactive refresh on a
/// pathological workspace.
const CARGO_METADATA_PROBE_DEADLINE: std::time::Duration = std::time::Duration::from_mins(2);

/// Run `cargo metadata --no-deps --offline` against the analysis root and
/// extract the workspace test-target inventory (#3634). `None` on any
/// unresolvable state: no root manifest, a cargo binary that cannot be
/// spawned, a workspace cargo rejects, a probe that outlives the
/// deadline (terminated and reaped by the shared deadline-aware wait),
/// or output that cannot be read or parsed. Stdout is captured through a
/// temp file rather than a pipe so a large workspace cannot fill the OS
/// pipe buffer and deadlock against the poll; the file is removed on
/// every path.
fn run_workspace_cargo_metadata(
    workspace_root: &Path,
) -> Option<BTreeMap<PathBuf, Vec<(PathBuf, String)>>> {
    let manifest_path = workspace_root.join("Cargo.toml");
    if !manifest_path.is_file() {
        return None;
    }
    let stdout_path = std::env::temp_dir().join(format!(
        "ripr-cargo-metadata-{}-{}-{}.json",
        std::process::id(),
        METADATA_PROBE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    let parsed = std::fs::File::create(&stdout_path)
        .ok()
        .and_then(|stdout_file| {
            std::process::Command::new("cargo")
                .args([
                    "metadata",
                    "--no-deps",
                    "--format-version",
                    "1",
                    "--offline",
                ])
                .arg("--manifest-path")
                // Bare `Cargo.toml`: the process directory below is the
                // workspace root, and a root-prefixed `manifest_path`
                // would be re-resolved against that new directory — a
                // relative analysis root (`some/dir`) would probe
                // `some/dir/some/dir/Cargo.toml` and fail every
                // registration's premise (#3637 review). An absolute path
                // would work, but canonicalize emits `\\?\` verbatim
                // paths on Windows that would leak into the inventory
                // keys.
                .arg("Cargo.toml")
                // Cargo resolves the workspace from the process directory
                // too: without this anchor, a probe for a bare-package
                // root inherits the caller's enclosing workspace and cargo
                // rejects the manifest as "believes it's in a workspace
                // when it's not" (#3634).
                .current_dir(workspace_root)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::from(stdout_file))
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()
        })
        .and_then(|mut child| {
            let outcome = crate::git::poll_child(
                &mut child,
                Some(CARGO_METADATA_PROBE_DEADLINE),
                "cargo metadata",
            );
            match outcome {
                crate::git::ChildWait::Exited(status) if status.success() => {
                    std::fs::read(&stdout_path)
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                        .map(|value| workspace_test_target_owners(&value))
                }
                _ => None,
            }
        });
    let _ = std::fs::remove_file(&stdout_path);
    parsed
}

/// The workspace test-target inventory from parsed `cargo metadata`
/// (#3634). With `--no-deps`, `packages` is exactly the workspace member
/// set — cargo's own membership resolution. Each `kind: ["test"]` target
/// contributes its lexically resolved `src_path` (cargo keeps declared
/// `..` segments as spelled, so both sides resolve lexically) mapped to
/// the owning package's manifest directory and the cargo target name —
/// the name is the identity that ties a metadata target back to the
/// manifest entry cargo retained, so a declaration cargo dropped from
/// its inventory cannot contribute its flag to a live target (#3637
/// review).
fn workspace_test_target_owners(
    value: &serde_json::Value,
) -> BTreeMap<PathBuf, Vec<(PathBuf, String)>> {
    let mut owners: BTreeMap<PathBuf, Vec<(PathBuf, String)>> = BTreeMap::new();
    let Some(packages) = value.get("packages").and_then(serde_json::Value::as_array) else {
        return owners;
    };
    for package in packages {
        let manifest_dir = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .map(|manifest_path| normalize(Path::new(manifest_path)))
            .and_then(|manifest_path| manifest_path.parent().map(Path::to_path_buf));
        let Some(manifest_dir) = manifest_dir else {
            continue;
        };
        let Some(targets) = package.get("targets").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for target in targets {
            let is_test = target
                .get("kind")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|kinds| {
                    kinds
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .any(|kind| kind == "test")
                });
            if !is_test {
                continue;
            }
            let Some(src_path) = target.get("src_path").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(name) = target.get("name").and_then(serde_json::Value::as_str) else {
                continue;
            };
            owners
                .entry(lexical(&normalize(Path::new(src_path))))
                .or_default()
                .push((manifest_dir.clone(), name.to_string()));
        }
    }
    for entry in owners.values_mut() {
        entry.sort();
        entry.dedup();
    }
    owners
}

/// Resolve the Cargo target metadata verdict for one registered harness
/// target path (#3608). Single-target form: constructs a throwaway
/// [`ManifestInventory`]. Batch callers (the registry, the role-grant
/// filter) reuse one inventory so a registration set in one package
/// parses its manifest once. Test-only: production consumers go through
/// [`ManifestInventory`].
#[cfg(test)]
pub(crate) fn cargo_test_target_harness_verdict(
    workspace_root: &Path,
    registration_target: &Path,
) -> CargoHarnessVerdict {
    ManifestInventory::default().verdict(workspace_root, registration_target)
}

/// Crate-root paths declared by one manifest, relative to `manifest_dir`
/// (#3533 review): `path = ...` on `[lib]`, `[[bin]]`, `[[test]]`,
/// `[[bench]]`, and `[[example]]` targets. Rust resolves out-of-line
/// `mod name;` in these files relative to the file's containing directory
/// regardless of its filename, so module composition needs this identity.
/// Entries whose declared path escapes the package directory are dropped
/// rather than trusted. Malformed manifests yield no declared roots.
pub(crate) fn declared_crate_root_paths_from_manifest(
    manifest_text: &str,
    manifest_dir: &Path,
) -> Vec<PathBuf> {
    let Ok(value) = toml::from_str::<toml::Value>(manifest_text) else {
        return Vec::new();
    };
    let normalized_dir = normalize(manifest_dir);
    let mut normalized = BTreeSet::new();
    for key in ["lib", "bin", "test", "bench", "example"] {
        let entries = match value.get(key) {
            // `[lib]` is a single table; the targets are arrays of tables.
            Some(toml::Value::Array(entries)) => entries.iter().collect::<Vec<_>>(),
            Some(table @ toml::Value::Table(_)) => vec![table],
            _ => continue,
        };
        for entry in entries {
            let Some(path) = entry.get("path").and_then(|path| path.as_str()) else {
                continue;
            };
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            normalized.insert(normalize(&manifest_dir.join(path)));
        }
    }
    normalized
        .into_iter()
        .filter_map(|target| {
            target
                .strip_prefix(&normalized_dir)
                .ok()
                .map(|relative| relative.to_path_buf())
        })
        .filter(|relative| {
            // ParentDir (or absolute/prefix) components escape the package
            // directory: drop the target rather than trust it.
            relative.components().all(|component| {
                matches!(
                    component,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        })
        .collect()
}

/// Build the source-role context for a set of analyzed files: read each
/// distinct owning package manifest once. `files` are workspace-relative
/// discovery identities; `workspace_root` anchors the package-root walk
/// (a relative path alone has no anchor, and reading a bare
/// `Cargo.toml` would consult the process working directory instead of
/// the analyzed workspace). Declared target paths are normalized back to
/// workspace-relative so they compare against discovery identities.
pub(crate) fn context_for_files<'a, I>(workspace_root: &Path, files: I) -> SourceRoleContext
where
    I: IntoIterator<Item = &'a Path>,
{
    let mut manifests: BTreeMap<PathBuf, DeclaredCargoTargets> = BTreeMap::new();
    let mut context = SourceRoleContext::empty();
    for file in files {
        let anchored = workspace_root.join(file);
        let Some(root) = package_root_of(&anchored) else {
            continue;
        };
        if !manifests.contains_key(&root) {
            let targets = std::fs::read_to_string(root.join("Cargo.toml"))
                .map(|text| declared_targets_from_manifest(&text, &root))
                .unwrap_or_default();
            manifests.insert(root.clone(), targets);
        }
        if let Some(targets) = manifests.get(&root) {
            context
                .declared_test_targets
                .extend(strip_root(workspace_root, &targets.tests));
            context
                .declared_bench_targets
                .extend(strip_root(workspace_root, &targets.benches));
        }
    }
    context
}

/// Normalize absolute declared-target paths back to workspace-relative
/// identities (forward-slashed), dropping anything that escapes the
/// workspace root rather than trusting it.
fn strip_root(workspace_root: &Path, targets: &BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
    targets
        .iter()
        .filter_map(|target| {
            let normalized = normalize(target);
            normalized
                .strip_prefix(normalize(workspace_root))
                .ok()
                .map(normalize)
        })
        .collect()
}

/// The package root owning `file`: the nearest ancestor directory that
/// starts a Cargo source layout (`src/`, `tests/`, `benches/`,
/// `examples/` — mirroring `workspace::classify::package_root`).
fn package_root_of(file: &Path) -> Option<PathBuf> {
    let mut parent = file.parent()?;
    loop {
        let name = parent
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if matches!(name.as_str(), "src" | "tests" | "benches" | "examples") {
            return parent.parent().map(Path::to_path_buf);
        }
        parent = parent.parent()?;
    }
}

fn normalize(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

/// Lexically resolve one normalized path (#3608 review): collapse CurDir
/// components and ParentDir/preceding-segment pairs without touching the
/// filesystem, so a declared path like `generated/../qa/mimic.rs` (or a
/// `../shared/x.rs` sibling declaration) compares equal to its
/// registration spelling. A leading ParentDir chain — the path escaping
/// above its base — is kept as spelled, so outside-root declarations
/// resolve consistently without being silently clamped into the root.
fn lexical(path: &Path) -> PathBuf {
    let mut resolved: Vec<std::path::Component> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => match resolved.last() {
                Some(std::path::Component::Normal(_)) => {
                    resolved.pop();
                }
                // Nothing to pop (base root or a leading `..` chain): keep
                // the escape as spelled; only a prefix/root below it is
                // dropped, since `..` cannot rise above a filesystem root.
                Some(std::path::Component::Prefix(_) | std::path::Component::RootDir) => {}
                _ => resolved.push(component),
            },
            other => resolved.push(other),
        }
    }
    resolved.into_iter().collect()
}

#[cfg(test)]
mod extraction {
    use super::*;

    /// Crate-root identity extraction (#3533 review): `[lib]` tables and
    /// `[[bin]]`/`[[test]]`/`[[bench]]`/`[[example]]` arrays contribute
    /// package-relative paths; entries without `path` and escaping paths
    /// contribute nothing.
    #[test]
    fn crate_root_paths_cover_lib_table_and_target_arrays() {
        let manifest = "[package]\nname='x'\nversion='0.1.0'\n\
            \n[lib]\npath='source/root.rs'\n\
            \n[[bin]]\nname='tool'\npath='src/tool.rs'\n\
            \n[[bin]]\nname='autodiscovered'\n\
            \n[[test]]\npath='../escaped.rs'\n\
            \n[[bench]]\npath='benches/perf.rs'\n";
        let roots = declared_crate_root_paths_from_manifest(manifest, Path::new("/ws/pkg-a"));
        assert_eq!(
            roots,
            vec![
                PathBuf::from("benches/perf.rs"),
                PathBuf::from("source/root.rs"),
                PathBuf::from("src/tool.rs"),
            ]
        );
        assert!(
            declared_crate_root_paths_from_manifest("not [ valid toml", Path::new("/ws/pkg-a"))
                .is_empty()
        );
    }

    #[test]
    fn explicit_test_and_bench_paths_are_extracted() {
        let manifest = "[package]\nname='x'\nversion='0.1.0'\n\
            \n[[test]]\nname='contract'\npath='src/contract_test.rs'\n\
            \n[[bench]]\nname='perf'\npath='src/perf.rs'\n\
            \n[[test]]\nname='autodiscovered'\n";
        let targets = declared_targets_from_manifest(manifest, Path::new("/ws/pkg-a"));
        assert!(
            targets
                .tests
                .contains(&PathBuf::from("/ws/pkg-a/src/contract_test.rs"))
        );
        assert!(
            targets
                .benches
                .contains(&PathBuf::from("/ws/pkg-a/src/perf.rs"))
        );
        // Entries without an explicit path contribute nothing: their
        // files live under tests/ by autodiscovery, which layout covers.
        assert_eq!(targets.tests.len(), 1);
        assert_eq!(targets.benches.len(), 1);
    }

    #[test]
    fn malformed_manifests_yield_no_targets() {
        let targets = declared_targets_from_manifest("not [ valid toml", Path::new("/ws"));
        assert!(targets.tests.is_empty());
        assert!(targets.benches.is_empty());
    }

    /// #3608: the harness-flag extraction names every `[[test]]` target and
    /// carries the effective flag (absent key = Cargo's `true` default).
    /// The manifest is only the flag source (#3634): target identity comes
    /// from the metadata inventory, so this extraction must list every
    /// entry shape the inventory can resolve back to.
    #[test]
    fn test_targets_with_harness_cover_explicit_name_resolved_and_defaults() {
        let manifest = "[package]\nname='x'\nversion='0.1.0'\n\
        \n[[test]]\nname='contract'\npath='src/contract_test.rs'\nharness=false\n\
        \n[[test]]\nname='plain'\nharness=false\n\
        \n[[test]]\nname='flagged'\nharness=true\n\
        \n[[test]]\nname='defaults_on'\n\
        \n[[test]]\npath='tests/explicit_only.rs'\n";
        let targets =
            declared_test_targets_with_harness_from_manifest(manifest, Path::new("/ws/pkg"));
        let rendered = targets
            .iter()
            .map(|target| {
                (
                    target.path.to_string_lossy().replace('\\', "/"),
                    target.harness,
                    target.name.clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                ("/ws/pkg/src/contract_test.rs".to_string(), false, None),
                (
                    "/ws/pkg/tests/plain.rs".to_string(),
                    false,
                    Some("plain".to_string()),
                ),
                (
                    "/ws/pkg/tests/flagged.rs".to_string(),
                    true,
                    Some("flagged".to_string()),
                ),
                (
                    "/ws/pkg/tests/defaults_on.rs".to_string(),
                    true,
                    Some("defaults_on".to_string()),
                ),
                ("/ws/pkg/tests/explicit_only.rs".to_string(), true, None,),
            ],
            "explicit entries carry no name (path-matched); name-only entries carry their declared name (name-matched, #3637 review)"
        );
        assert!(
            declared_test_targets_with_harness_from_manifest("not [ valid toml", Path::new("/ws"))
                .is_empty()
        );
    }

    #[test]
    fn autodiscovery_flags_do_not_drop_explicit_targets() {
        let manifest = "[package]\nname='x'\nautotests=false\nautobenches=false\n\
            \n[[test]]\npath='tests/contract.rs'\n[[bench]]\npath='benches/perf.rs'\n";
        let targets = declared_targets_from_manifest(manifest, Path::new("/ws"));
        assert_eq!(targets.tests.len(), 1);
        assert_eq!(targets.benches.len(), 1);
    }

    #[test]
    fn package_root_resolution_covers_source_layouts() {
        assert_eq!(
            package_root_of(Path::new("/ws/pkg-a/src/lib.rs")),
            Some(PathBuf::from("/ws/pkg-a"))
        );
        assert_eq!(
            package_root_of(Path::new("/ws/pkg-a/tests/it.rs")),
            Some(PathBuf::from("/ws/pkg-a"))
        );
        assert_eq!(
            package_root_of(Path::new("/ws/pkg-a/benches/perf.rs")),
            Some(PathBuf::from("/ws/pkg-a"))
        );
        // A file with no Cargo source-layout ancestor has no package root.
        assert_eq!(package_root_of(Path::new("/ws/loose.rs")), None);
    }
}

/// A unique temp directory outside the repo tree for one fixture
/// workspace.
#[cfg(test)]
fn unique_workspace(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ripr-harness-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ))
}

#[cfg(test)]
mod context {
    use super::*;

    #[test]
    fn context_for_files_collects_declared_targets_from_disk() -> Result<(), String> {
        // #3283 discriminating test for the aggregation itself: a real
        // manifest on disk contributes its declared targets, workspace
        // relative, while an unrelated package contributes nothing.
        let dir = unique_workspace("targets-ctx");
        std::fs::create_dir_all(dir.join("pkg-a/src")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(dir.join("pkg-b/src")).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("pkg-a/Cargo.toml"),
            "[package]\nname='a'\nversion='0.1.0'\n\n[[test]]\npath='src/contract_test.rs'\n[[bench]]\npath='src/perf.rs'\n",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            dir.join("pkg-b/Cargo.toml"),
            "[package]\nname='b'\nversion='0.1.0'\n",
        )
        .map_err(|error| error.to_string())?;
        let files = [dir.join("pkg-a/src/lib.rs"), dir.join("pkg-b/src/lib.rs")]
            .iter()
            .map(|path| {
                path.strip_prefix(&dir)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|_| path.clone())
            })
            .collect::<Vec<_>>();
        let context = context_for_files(
            &dir,
            files.iter().map(|path| path.as_path()).collect::<Vec<_>>(),
        );
        assert!(
            context
                .declared_test_targets
                .contains(&PathBuf::from("pkg-a/src/contract_test.rs"))
        );
        assert!(
            context
                .declared_bench_targets
                .contains(&PathBuf::from("pkg-a/src/perf.rs"))
        );
        assert!(context.production_like_targets.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}

/// Harness-verdict fixtures. Every fixture is a real Cargo workspace:
/// the verdict sources workspace membership and the test-target
/// inventory from `cargo metadata` (#3634), so member packages carry the
/// `src/lib.rs` target file cargo requires and the asserted outcomes are
/// cargo's own resolution, not an emulation of it. Autodiscovered and
/// name-only targets appear in the inventory only when their files exist
/// (verified against `cargo metadata`, #3634), so fixtures write those
/// files; explicit `path = ...` targets are reported regardless.
#[cfg(test)]
mod harness_verdict {
    use super::*;

    /// One member package: a manifest plus the `src/lib.rs` target file
    /// cargo requires of every workspace package.
    fn write_member_package(dir: &Path, relative: &str, manifest: &str) -> Result<(), String> {
        let package_dir = dir.join(relative);
        std::fs::create_dir_all(package_dir.join("src")).map_err(|error| error.to_string())?;
        std::fs::write(package_dir.join("Cargo.toml"), manifest)
            .map_err(|error| error.to_string())?;
        std::fs::write(package_dir.join("src/lib.rs"), "").map_err(|error| error.to_string())
    }

    /// One directory without a manifest or target file (the
    /// broken-workspace shapes).
    fn make_dir(dir: &Path, relative: &str) -> Result<(), String> {
        std::fs::create_dir_all(dir.join(relative)).map_err(|error| error.to_string())
    }

    fn write_file(dir: &Path, relative: &str, contents: &str) -> Result<(), String> {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, contents).map_err(|error| error.to_string())
    }

    /// #3608: the verdict discriminates declared-harness-false targets from
    /// harness-enabled targets (explicit or autodiscovered), undeclared
    /// paths, and non-test target kinds — sourced from `cargo metadata`'s
    /// own target inventory (#3634).
    #[test]
    fn harness_verdict_discriminates_declared_enabled_and_missing_targets() -> Result<(), String> {
        let dir = unique_workspace("verdict");
        // The analyzed root declares a real workspace whose member is pkg;
        // metadata's member set and target inventory are the authority
        // (review HAkg, #3634).
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['pkg']\n")?;
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        make_dir(&dir, "pkg/tests")?;
        write_file(&dir, "pkg/tests/enabled.rs", "")?;
        write_file(&dir, "pkg/tests/discovered.rs", "")?;
        make_dir(&dir, "orphan/src")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // orphan is not a member: metadata's inventory names no target for
        // the path.
        assert_eq!(
            verdict("orphan/src/mimic.rs"),
            CargoHarnessVerdict::NotDeclared
        );

        // Manifest A: autotests disabled, one explicit harness = false target
        // with a custom path, one explicit harness = true name target.
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nautotests=false\n\n\
         [[test]]\nname='custom'\npath='src/contract_test.rs'\nharness=false\n\n\
         [[test]]\nname='enabled'\nharness=true\n",
        )?;
        // Explicit [[test]] with harness = false and a custom path: the
        // custom-harness premise holds.
        assert_eq!(
            verdict("pkg/src/contract_test.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        // Name-resolved autodiscovery shape with an explicit harness = true
        // entry: the target is known and its harness stays enabled.
        assert_eq!(
            verdict("pkg/tests/enabled.rs"),
            CargoHarnessVerdict::HarnessEnabled
        );
        // Conventional tests/ layout without any entry while autotests =
        // false: cargo's inventory has no such target.
        assert_eq!(
            verdict("pkg/tests/undiscovered.rs"),
            CargoHarnessVerdict::NotDeclared
        );
        // A typo'd or swapped path matches nothing.
        assert_eq!(
            verdict("pkg/src/contract_tset.rs"),
            CargoHarnessVerdict::NotDeclared
        );
        // loose.rs matches no declaration and no autodiscovery shape at
        // the workspace root.
        assert_eq!(verdict("loose.rs"), CargoHarnessVerdict::NotDeclared);
        // The package's lib target is in metadata but is not a test
        // target: kind filtering keeps the registration uncredited.
        assert_eq!(verdict("pkg/src/lib.rs"), CargoHarnessVerdict::NotDeclared);

        // Manifest B: plain package, no [[test]] entries — autodiscovery on.
        write_member_package(&dir, "pkg", "[package]\nname='p'\nversion='0.1.0'\n")?;
        assert_eq!(
            verdict("pkg/tests/discovered.rs"),
            CargoHarnessVerdict::HarnessEnabled
        );
        // Without the explicit entry the custom-path target is no longer declared.
        assert_eq!(
            verdict("pkg/src/contract_test.rs"),
            CargoHarnessVerdict::NotDeclared
        );

        // Manifest C: an explicit harness = false declaration on the
        // conventional layout (name-only entry) confirms the premise.
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\n\n\
         [[test]]\nname='discovered'\nharness=false\n",
        )?;
        assert_eq!(
            verdict("pkg/tests/discovered.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608: a virtual workspace manifest (no `[package]` table) declares no
    /// autodiscovered targets; cargo metadata succeeds on the empty member
    /// set and the inventory is empty.
    #[test]
    fn virtual_manifest_root_declares_no_autodiscovered_targets() -> Result<(), String> {
        let dir = unique_workspace("virtual");
        std::fs::create_dir_all(dir.join("tests")).map_err(|error| error.to_string())?;
        std::fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .map_err(|error| error.to_string())?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("tests/it.rs")),
            CargoHarnessVerdict::NotDeclared
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3634: a virtual manifest carrying a `[[test]]` table is rejected by
    /// cargo itself ("this virtual manifest specifies a `test` section"),
    /// so the metadata probe is unavailable and every verdict fails closed
    /// to ManifestUnavailable — never to a fabricated declaration.
    #[test]
    fn virtual_manifest_with_a_test_section_fails_metadata_closed() -> Result<(), String> {
        let dir = unique_workspace("virtual-decl");
        std::fs::create_dir_all(dir.join("tests")).map_err(|error| error.to_string())?;
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        std::fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = ['pkg']\n\n[[test]]\nname = 'mimic'\nharness = false\n",
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("tests/mimic.rs")),
            CargoHarnessVerdict::ManifestUnavailable,
            "cargo rejects the manifest outright: no metadata premise exists"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3634: no workspace manifest at the analysis root means no metadata
    /// probe is possible — every registration fails closed to
    /// ManifestUnavailable instead of guessing from stray manifests.
    #[test]
    fn absent_workspace_manifest_fails_closed() -> Result<(), String> {
        let dir = unique_workspace("no-root");
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='target.rs'\nharness=false\n",
        )?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("pkg/target.rs"),
            CargoHarnessVerdict::ManifestUnavailable,
            "no root manifest: the metadata premise cannot be established"
        );
        assert_eq!(
            verdict("pkg/tests/other.rs"),
            CargoHarnessVerdict::ManifestUnavailable
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review (Fe25): declaration ownership is cargo's own — a
    /// workspace-root package's explicit `[[test]] path = ...` entry claims
    /// its target even when the path sits below a directory containing
    /// another Cargo.toml, and two packages claiming one path with
    /// conflicting `harness` flags stay ambiguous and fail closed (the
    /// inventory lists the path under both owning packages, #3634).
    #[test]
    fn workspace_root_declaration_claims_a_target_below_a_nested_manifest() -> Result<(), String> {
        let dir = unique_workspace("root-decl");
        make_dir(&dir, "below/nested/manifest/dir")?;
        make_dir(&dir, "below/nested/manifest/tests")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\nmembers = ['below/nested/manifest']\n\n\
             [[test]]\nname='mimic'\npath='below/nested/manifest/dir/mimic.rs'\nharness=false\n",
        )?;
        // The root package and the nested member both need target files.
        make_dir(&dir, "src")?;
        write_file(&dir, "src/lib.rs", "")?;
        write_member_package(
            &dir,
            "below/nested/manifest",
            "[package]\nname='nested'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_file(&dir, "below/nested/manifest/tests/other.rs", "")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // The root declaration wins for the explicit target.
        assert_eq!(
            verdict("below/nested/manifest/dir/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        // Nearest-manifest resolution still governs autodiscovery credit:
        // the nested package's own conventional tests/ file is discovered.
        assert_eq!(
            verdict("below/nested/manifest/tests/other.rs"),
            CargoHarnessVerdict::HarnessEnabled
        );

        // Round 4 (Gajt): two manifests declaring the same path with
        // conflicting `harness` flags is ambiguous ownership — which
        // compilation unit collects the file is not statically decidable —
        // so the verdict fails closed instead of picking a winner.
        write_member_package(
            &dir,
            "below/nested/manifest",
            "[package]\nname='nested'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='dir/mimic.rs'\nharness=true\n",
        )?;
        assert_eq!(
            verdict("below/nested/manifest/dir/mimic.rs"),
            CargoHarnessVerdict::ManifestUnavailable,
            "conflicting declarations fail closed"
        );

        // Agreeing declarations remain deterministic.
        write_member_package(
            &dir,
            "below/nested/manifest",
            "[package]\nname='nested'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='dir/mimic.rs'\nharness=false\n",
        )?;
        assert_eq!(
            verdict("below/nested/manifest/dir/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "agreeing declarations keep the deterministic verdict"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round four (Gajt): Cargo permits an explicit
    /// `[[test]]` path to resolve outside the declaring package's
    /// directory, so a sibling package's `../shared/mimic.rs`
    /// harness = false declaration claims the shared target even though
    /// the declaring manifest is not an ancestor of it; the metadata
    /// inventory records the target under the declaring package (#3634).
    #[test]
    fn shared_target_declared_from_a_sibling_package_resolves() -> Result<(), String> {
        let dir = unique_workspace("shared");
        write_member_package(
            &dir,
            "crates/a",
            "[package]\nname='a'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=false\n",
        )?;
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\nmembers = ['crates/a']\n",
        )?;
        make_dir(&dir, "src")?;
        write_file(&dir, "src/lib.rs", "")?;
        make_dir(&dir, "shared")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        // The sibling package's declaration claims the shared target.
        assert_eq!(
            verdict("shared/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        // Undeclared sibling paths keep the ordinary fallback.
        assert_eq!(verdict("shared/other.rs"), CargoHarnessVerdict::NotDeclared);

        // Ambiguity: a second package declaring the same shared path with
        // a conflicting harness flag fails closed. The second package is
        // declared as a member when its manifest appears.
        make_dir(&dir, "crates/b")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\nmembers = ['crates/a', 'crates/b']\n",
        )?;
        write_member_package(
            &dir,
            "crates/b",
            "[package]\nname='b'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=true\n",
        )?;
        assert_eq!(
            verdict("shared/mimic.rs"),
            CargoHarnessVerdict::ManifestUnavailable,
            "conflicting declarations of one shared path are ambiguous"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round four (GalA): declared paths and registration
    /// targets compare on the lexically resolved identity — ParentDir and
    /// CurDir segments collapse without touching the filesystem, and a
    /// leading escape chain stays as spelled. `cargo metadata` keeps `..`
    /// segments in `src_path` as spelled too (verified #3634), so both
    /// sides resolve the same way.
    #[test]
    fn parent_segments_lexically_resolve_on_both_sides() -> Result<(), String> {
        let dir = unique_workspace("lexical");
        make_dir(&dir, "pkg")?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\nmembers = ['pkg']\n",
        )?;
        make_dir(&dir, "src")?;
        write_file(&dir, "src/lib.rs", "")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // In-package `..` declaration: `generated/../qa/mimic.rs` resolves
        // to `pkg/qa/mimic.rs` (the generated/ directory need not exist;
        // metadata still reports the declared target, verified #3634).
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='generated/../qa/mimic.rs'\nharness=false\n",
        )?;
        assert_eq!(
            verdict("pkg/qa/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        // The un-collapsed spelling of the same target matches too.
        assert_eq!(
            verdict("pkg/generated/../qa/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );

        // The reviewer's shape: a sibling `../shared/x.rs` declaration
        // matches a target that spells the same location with a `..`
        // segment.
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='shared'\npath='../shared/x.rs'\nharness=false\n",
        )?;
        assert_eq!(
            verdict("shared/../shared/x.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        assert_eq!(verdict("shared/x.rs"), CargoHarnessVerdict::HarnessDisabled);

        // An escape above the workspace root stays outside: the
        // declaration resolves outside and never claims an in-workspace
        // target spelled as if it were inside.
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='escape'\npath='../../outside/x.rs'\nharness=false\n",
        )?;
        assert_eq!(
            verdict("outside/x.rs"),
            CargoHarnessVerdict::NotDeclared,
            "a root-escaping declaration does not clamp onto an in-workspace path"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review (Fe4d), metadata-sourced (#3634): a member declaring
    /// `edition.workspace = true` autodiscovers through cargo's own
    /// edition resolution — workspace edition 2024 keeps a second
    /// conventional `tests/*.rs` in metadata's target inventory. When the
    /// inherited edition cannot resolve, cargo rejects the manifest and
    /// the whole workspace fails closed instead of approximating a
    /// default.
    #[test]
    fn workspace_inherited_edition_governs_autodiscovery() -> Result<(), String> {
        let dir = unique_workspace("ws-edition");
        make_dir(&dir, "member/tests")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['member']\n\n[workspace.package]\nedition = '2024'\n",
        )?;
        write_member_package(
            &dir,
            "member",
            "[package]\nname='m'\nversion='0.1.0'\nedition.workspace=true\n\n\
             [[test]]\nname='declared'\nharness=false\n",
        )?;
        write_file(&dir, "member/tests/declared.rs", "")?;
        write_file(&dir, "member/tests/other.rs", "")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("member/tests/declared.rs"),
            CargoHarnessVerdict::HarnessDisabled
        );
        assert_eq!(
            verdict("member/tests/other.rs"),
            CargoHarnessVerdict::HarnessEnabled,
            "the member inherits edition 2024, so autodiscovery stays on"
        );

        // Without the workspace edition the member manifest is invalid
        // (`edition.workspace = true` dangles): cargo metadata fails and
        // every verdict fails closed.
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['member']\n")?;
        assert_eq!(
            verdict("member/tests/other.rs"),
            CargoHarnessVerdict::ManifestUnavailable,
            "an unresolvable workspace fails closed rather than guessing an edition"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review: edition 2015 with a manual `[[test]]` entry — the
    /// declared entry still matches while the sibling conventional-layout
    /// file is no longer discovered. Cargo's own autodiscovery rule
    /// decides: the sibling is absent from metadata's inventory (#3634).
    #[test]
    fn edition_2015_manual_declaration_keeps_explicit_match_and_drops_discovery()
    -> Result<(), String> {
        let dir = unique_workspace("edition");
        make_dir(&dir, "pkg/tests")?;
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2015'\n\n[[test]]\nname='declared'\nharness=false\n",
        )?;
        write_file(&dir, "pkg/tests/declared.rs", "")?;
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['pkg']\n")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("pkg/tests/declared.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the explicit declaration matches regardless of the autodiscovery default"
        );
        assert_eq!(
            verdict("pkg/tests/other.rs"),
            CargoHarnessVerdict::NotDeclared,
            "edition 2015 with a manual [[test]] disables autodiscovery of siblings"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review: a readable but malformed owning manifest cannot
    /// establish any premise about its targets — the verdict is
    /// ManifestUnavailable, never a target-typo NotDeclared. With no root
    /// manifest at all the metadata probe is unavailable first (#3634).
    #[test]
    fn malformed_readable_manifest_is_manifest_unavailable() -> Result<(), String> {
        let dir = unique_workspace("malformed");
        make_dir(&dir, "pkg/src")?;
        make_dir(&dir, "pkg/tests")?;
        write_file(&dir, "pkg/Cargo.toml", "not [ valid toml")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("pkg/src/mimic.rs"),
            CargoHarnessVerdict::ManifestUnavailable
        );
        assert_eq!(
            verdict("pkg/tests/mimic.rs"),
            CargoHarnessVerdict::ManifestUnavailable,
            "even the autodiscovery premise cannot be established from a malformed manifest"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3634: ownership follows cargo metadata's member set, not manifest
    /// presence. A `[[test]]` declaration in a manifest that is NOT a
    /// workspace member grants no authority (the pre-#3634 emulation
    /// credited it through a nearest-manifest walk — an over-credit); once
    /// the same package joins the workspace its declaration claims the
    /// target.
    #[test]
    fn metadata_membership_governs_target_ownership() -> Result<(), String> {
        let dir = unique_workspace("nonconventional");
        make_dir(&dir, "crates/a/qa")?;
        make_dir(&dir, "qa_root")?;
        // The empty [workspace] table makes the root package a
        // standalone workspace root: under a test-process temp dir that
        // sits inside an enclosing workspace, cargo would otherwise
        // reject the bare package outright.
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\n",
        )?;
        make_dir(&dir, "src")?;
        write_file(&dir, "src/lib.rs", "")?;
        write_member_package(
            &dir,
            "crates/a",
            "[package]\nname='a'\nversion='0.1.0'\nedition='2024'\n\n\
             [[test]]\nname='mimic'\npath='qa/mimic.rs'\nharness=false\n",
        )?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // crates/a is not a member (the root package forms its own
        // single-package workspace): its declaration grants nothing.
        assert_eq!(
            verdict("crates/a/qa/mimic.rs"),
            CargoHarnessVerdict::NotDeclared,
            "a nonmember manifest's declaration grants no harness authority (#3634)"
        );
        // The root package's own nonconventional path matches nothing and
        // is not an autodiscovery shape either.
        assert_eq!(
            verdict("qa_root/mimic.rs"),
            CargoHarnessVerdict::NotDeclared
        );

        // Once crates/a joins the workspace its declaration claims the
        // target, and a root declaration claims the root-level path.
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\nmembers = ['crates/a']\n\n\
             [[test]]\nname='root_mimic'\npath='qa_root/mimic.rs'\nharness=false\n",
        )?;
        assert_eq!(
            verdict("crates/a/qa/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the member's declaration is honored through metadata membership"
        );
        assert_eq!(
            verdict("qa_root/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the root package's declaration is honored"
        );

        // Dropping the nested declaration: the member no longer declares
        // the target and the inventory drops it.
        write_member_package(
            &dir,
            "crates/a",
            "[package]\nname='a'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        assert_eq!(
            verdict("crates/a/qa/mimic.rs"),
            CargoHarnessVerdict::NotDeclared
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review: Cargo never autodiscovers tests below `src/tests/`;
    /// cargo's inventory simply has no such target, so the module file
    /// stays uncredited without changing the shared layout predicate's
    /// source-role behavior.
    #[test]
    fn nested_src_tests_module_file_is_not_an_autodiscovered_target() -> Result<(), String> {
        let dir = unique_workspace("nested-tests");
        make_dir(&dir, "pkg/src/tests")?;
        make_dir(&dir, "pkg/tests")?;
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_file(&dir, "pkg/tests/case.rs", "")?;
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['pkg']\n")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("pkg/src/tests/case.rs"),
            CargoHarnessVerdict::NotDeclared,
            "a module file below src/tests/ is not a package-root test target"
        );
        assert_eq!(
            verdict("pkg/tests/case.rs"),
            CargoHarnessVerdict::HarnessEnabled,
            "the package-root tests/ shape stays autodiscovered"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round five (HAla), corrected by the #3637 review: a
    /// name-only `[[test]]` entry governs both of cargo's name-only
    /// layouts. With only `tests/<name>.rs` on disk cargo reports that
    /// path; with only `tests/<name>/main.rs` cargo reports the directory
    /// shape as the entry's target (verified against `cargo metadata`
    /// 1.95.0) and it inherits the entry's flag; with both, cargo drops
    /// the ambiguous target from its inventory entirely, so neither
    /// layout is credited.
    #[test]
    fn name_only_entry_credits_both_cargo_layouts() -> Result<(), String> {
        let dir = unique_workspace("name-only");
        make_dir(&dir, "pkg/tests/suite")?;
        write_file(&dir, "pkg/src/lib.rs", "")?;
        write_file(&dir, "pkg/tests/suite/main.rs", "")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]
members = ['pkg']
",
        )?;
        write_member_package(
            &dir,
            "pkg",
            "[package]
name='p'
version='0.1.0'
edition='2024'

[[test]]
name='suite'
harness=false
",
        )?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // Sole directory layout (#3637 review): cargo reports
        // `tests/suite/main.rs` as the name-only entry's target, so the
        // `harness = false` premise holds for it. The file layout is not
        // in cargo's inventory at all and stays NotDeclared.
        assert_eq!(
            verdict("pkg/tests/suite/main.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the sole directory layout is the name-only entry's target and inherits its flag"
        );
        assert_eq!(
            verdict("pkg/tests/suite.rs"),
            CargoHarnessVerdict::NotDeclared,
            "a layout cargo did not compile is not credited even though the entry governs it"
        );

        write_file(&dir, "pkg/tests/suite.rs", "")?;
        assert_eq!(
            verdict("pkg/tests/suite.rs"),
            CargoHarnessVerdict::NotDeclared,
            "with both layouts on disk cargo drops the ambiguous target entirely"
        );

        // With BOTH layouts on disk, cargo's inventory drops the
        // ambiguous `suite` target entirely (verified against
        // `cargo metadata`, #3634): neither layout inherits the flag.
        write_file(&dir, "pkg/tests/suite/main.rs", "")?;
        assert_eq!(
            verdict("pkg/tests/suite.rs"),
            CargoHarnessVerdict::NotDeclared,
            "the dual file/directory shape is cargo-rejected, so nothing is credited"
        );
        assert_eq!(
            verdict("pkg/tests/suite/main.rs"),
            CargoHarnessVerdict::NotDeclared,
            "the directory layout does not inherit the entry's flag"
        );

        // Without the entry, once the conflicting file is gone the
        // directory layout is its own autodiscovered target with no
        // flag to inherit.
        write_member_package(
            &dir,
            "pkg",
            "[package]\nname='p'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        std::fs::remove_file(dir.join("pkg/tests/suite.rs")).map_err(|error| error.to_string())?;
        assert_eq!(
            verdict("pkg/tests/suite/main.rs"),
            CargoHarnessVerdict::HarnessEnabled,
            "without the declaration the autodiscovered directory layout is harness-enabled"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3637 review (devin): with both name-only layouts on disk cargo
    /// drops the ambiguous `suite` target entirely, but an explicit entry
    /// claiming the directory path stays in cargo's inventory. The
    /// dropped declaration must not contribute its flag to the live
    /// target — name-based matching skips it, so the explicit entry's
    /// `harness = true` alone decides and the verdict is HarnessEnabled,
    /// never a false flag conflict.
    #[test]
    fn dropped_dual_declaration_cannot_conflict_with_a_live_explicit_target() -> Result<(), String>
    {
        let dir = unique_workspace("dual-explicit");
        make_dir(&dir, "pkg/tests/suite")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]
members = ['pkg']
",
        )?;
        write_member_package(
            &dir,
            "pkg",
            "[package]
name='p'
version='0.1.0'
edition='2024'

[[test]]
name='suite'
harness=false

[[test]]
name='suite_main'
path='tests/suite/main.rs'
harness=true
",
        )?;
        write_file(&dir, "pkg/src/lib.rs", "")?;
        write_file(&dir, "pkg/tests/suite.rs", "")?;
        write_file(&dir, "pkg/tests/suite/main.rs", "")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("pkg/tests/suite/main.rs"),
            CargoHarnessVerdict::HarnessEnabled,
            "only the explicit entry cargo retained decides the live target's flag"
        );
        assert_eq!(
            verdict("pkg/tests/suite.rs"),
            CargoHarnessVerdict::NotDeclared,
            "the dropped dual-layout declaration compiles nothing"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3637 review (devin): a relative analysis root must not invalidate
    /// the metadata probe. The probe anchors the child process in the
    /// workspace root, so the manifest path it passes must be resolved in
    /// that root, not re-prefixed with it; the fixture lives under the
    /// test process cwd (`target/`, gitignored) and the verdict is driven
    /// with a purely relative root.
    #[test]
    fn relative_analysis_root_still_resolves_the_workspace() -> Result<(), String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let relative_root = PathBuf::from("target").join(format!("harness-relative-{stamp}"));
        let pkg_tests = relative_root.join("pkg/tests");
        std::fs::create_dir_all(&pkg_tests).map_err(|error| error.to_string())?;
        std::fs::write(
            relative_root.join("Cargo.toml"),
            "[workspace]
members = ['pkg']
",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            relative_root.join("pkg/Cargo.toml"),
            "[package]
name='p'
version='0.1.0'
edition='2024'

[[test]]
name='mimic'
harness=false
",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(relative_root.join("pkg/src.rs"), "").map_err(|error| error.to_string())?;
        std::fs::write(pkg_tests.join("mimic.rs"), "").map_err(|error| error.to_string())?;

        let verdict =
            cargo_test_target_harness_verdict(&relative_root, Path::new("pkg/tests/mimic.rs"));
        assert_eq!(
            verdict,
            CargoHarnessVerdict::HarnessDisabled,
            "a relative root resolves the same workspace premise as an absolute one"
        );
        let _ = std::fs::remove_dir_all(&relative_root);
        Ok(())
    }

    /// #3637 review (devin), pinned across both cargo behaviors: cargo
    /// metadata echoes the path family it was given on Windows (a
    /// junction root yields alias-form keys, verified 1.95.0) but
    /// canonicalizes symlinks on Unix (demonstrated by the Linux CI run of the
    /// first version of this pin). The verdict therefore resolves the
    /// anchored target under BOTH forms, and the alias root stays valid
    /// on either host. Canonicalizing unconditionally would break the
    /// Windows side (verbatim \?\ output never matches cargo's plain
    /// echo); skipping canonicalization would break Unix.
    /// Create a directory alias (Windows junction, Unix symlink) for the
    /// alias-form inventory pin. Returns the OS error when aliases are
    /// unavailable on this host so the caller can skip without a false
    /// pass.
    fn create_filesystem_alias(
        real: &std::path::Path,
        alias: &std::path::Path,
    ) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(alias)
                .arg(real)
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other("mklink junction creation failed"))
            }
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(real, alias)
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (real, alias);
            Err(std::io::Error::other("no filesystem alias support"))
        }
    }

    #[test]
    fn alias_form_roots_still_match_cargo_alias_form_output() -> Result<(), String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let temp = std::env::temp_dir();
        let real = temp.join(format!("harness-alias-real-{stamp}"));
        let alias = temp.join(format!("harness-alias-link-{stamp}"));
        std::fs::create_dir_all(real.join("pkg/tests")).map_err(|error| error.to_string())?;
        std::fs::write(
            real.join("Cargo.toml"),
            "[workspace]
members = ['pkg']
",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            real.join("pkg/Cargo.toml"),
            "[package]
name='p'
version='0.1.0'
edition='2024'

[[test]]
name='mimic'
harness=false
",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(real.join("pkg/src.rs"), "").map_err(|error| error.to_string())?;
        std::fs::write(real.join("pkg/tests/mimic.rs"), "").map_err(|error| error.to_string())?;
        let alias_created = create_filesystem_alias(&real, &alias);
        if alias_created.is_err() {
            // Alias creation can be unavailable (no symlink privilege on
            // this host): the invariant is still pinned wherever aliases
            // exist, and skipping is not a pass for the alias path itself.
            let _ = std::fs::remove_dir_all(&real);
            return Ok(());
        }
        let verdict = cargo_test_target_harness_verdict(&alias, Path::new("pkg/tests/mimic.rs"));
        let _ = std::fs::remove_dir_all(&alias);
        let _ = std::fs::remove_dir_all(&real);
        assert_eq!(
            verdict,
            CargoHarnessVerdict::HarnessDisabled,
            "an alias-form root compares against cargo's alias-form inventory output"
        );
        Ok(())
    }

    /// #3608 review round five (HAkg): membership gating. A malformed
    /// manifest that is NOT a workspace member is ignored by cargo
    /// metadata — it neither rejects a valid member registration nor
    /// conflicts with it — while a malformed MEMBER manifest makes the
    /// whole workspace unresolvable and every verdict fails closed.
    #[test]
    fn membership_gates_the_declaration_map() -> Result<(), String> {
        let dir = unique_workspace("membership");
        make_dir(&dir, "member")?;
        make_dir(&dir, "stray")?;
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['member']\n")?;
        write_member_package(
            &dir,
            "member",
            "[package]\nname='m'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='target.rs'\nharness=false\n",
        )?;
        // A standalone nested manifest outside the member set is
        // malformed: it is not a member, so cargo metadata ignores it.
        write_file(&dir, "stray/Cargo.toml", "not [ valid toml")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("member/target.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "a malformed nonmember manifest neither rejects nor conflicts"
        );

        // Even a nonmember declaring the same path with a conflicting
        // flag cannot create ambiguity: it is not part of the workspace.
        write_member_package(
            &dir,
            "stray",
            "[package]\nname='s'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../member/target.rs'\nharness=true\n",
        )?;
        assert_eq!(
            verdict("member/target.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "nonmember declarations cannot conflict with member targets"
        );

        // A malformed MEMBER manifest leaves the whole workspace
        // unresolvable: cargo metadata rejects it.
        write_file(&dir, "member/Cargo.toml", "not [ valid toml")?;
        assert_eq!(
            verdict("member/target.rs"),
            CargoHarnessVerdict::ManifestUnavailable,
            "a malformed member manifest fails closed"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round five (HAkg): regular path dependencies of
    /// members join the member set, so a dependency package's shared
    /// harness declaration is honored.
    #[test]
    fn path_dependency_members_join_the_declaration_map() -> Result<(), String> {
        let dir = unique_workspace("pathdep");
        write_member_package(
            &dir,
            "crates/a",
            "[package]\nname='a'\nversion='0.1.0'\nedition='2024'\n\n[dependencies]\nb = { path = '../b' }\n",
        )?;
        write_member_package(
            &dir,
            "crates/b",
            "[package]\nname='b'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['crates/a']\n")?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the path-dependency member's declaration is honored"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round five (HAkg): `[workspace.exclude]` removes
    /// members and glob expansion (`crates/*`) honors declared members.
    #[test]
    fn glob_members_honor_excludes() -> Result<(), String> {
        let dir = unique_workspace("glob");
        write_member_package(
            &dir,
            "crates/kept",
            "[package]\nname='kept'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "crates/dropped")?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['crates/*']\nexclude = ['crates/dropped']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the glob-kept member's declaration is honored"
        );
        // The excluded package declares the same path with a conflicting
        // flag: excluded from the member set, it cannot create ambiguity.
        write_member_package(
            &dir,
            "crates/dropped",
            "[package]\nname='dropped'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=true\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the excluded member's conflicting declaration is ignored"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round five (IZb_): a mid-pattern `**` spans
    /// arbitrarily deep directories — a harness-declaring package several
    /// directories below a `crates/**/pkg` member pattern resolves. Cargo
    /// rejects a glob match without a manifest exactly as the
    /// pre-#3634 emulation did: metadata fails closed.
    #[test]
    fn recursive_member_glob_reaches_deep_packages() -> Result<(), String> {
        let dir = unique_workspace("deep-glob");
        write_member_package(
            &dir,
            "crates/x/y/pkg",
            "[package]\nname='deep'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../../../shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['crates/**/pkg']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the deep package's declaration is honored through the recursive glob"
        );
        // A trailing `**` reaches every manifest-carrying directory
        // below its base — including member `src/` directories, which
        // never carry manifests. Cargo rejects the whole shape (verified
        // against `cargo metadata`, #3634), so the verdict fails closed
        // even though `crates/x/y/pkg` itself is a valid declaring
        // member; the pre-#3634 emulation only failed closed once a
        // manifest-less directory was created by hand.
        write_member_package(
            &dir,
            "crates/x",
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_member_package(
            &dir,
            "crates/x/y",
            "[package]\nname='y'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        make_dir(&dir, "crates/x/y/nested")?;
        write_member_package(
            &dir,
            "crates/x/y/nested",
            "[package]\nname='nested'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['crates/x/**']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::ManifestUnavailable,
            "a trailing ** glob over real packages matches manifest-less src/ directories: cargo rejects the shape"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round five (IZc5): a declared member whose manifest
    /// does not exist is a broken workspace — cargo metadata rejects it
    /// and every custom-harness verdict fails closed, even when another
    /// member declares the registered target.
    #[test]
    fn missing_member_manifest_fails_closed() -> Result<(), String> {
        let dir = unique_workspace("absent-member");
        make_dir(&dir, "ghost")?;
        write_member_package(
            &dir,
            "real",
            "[package]\nname='real'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='target.rs'\nharness=false\n",
        )?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['ghost', 'real']\n",
        )?;
        // ghost/ is deliberately left without a Cargo.toml.
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("real/target.rs")),
            CargoHarnessVerdict::ManifestUnavailable,
            "the missing member manifest leaves the workspace incomplete"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round five (IZdo): dev- and build-dependency path
    /// packages join the member set exactly as `cargo metadata` reports,
    /// so their harness declarations are honored even when that
    /// dependency is the only route to the package.
    #[test]
    fn dev_and_build_path_dependencies_join_the_member_set() -> Result<(), String> {
        let dir = unique_workspace("devbuild");
        write_member_package(
            &dir,
            "main",
            "[package]\nname='main'\nversion='0.1.0'\nedition='2024'\n\n[dev-dependencies]\ndevdep = { path = '../devdep' }\n\n[build-dependencies]\nbuilddep = { path = '../builddep' }\n",
        )?;
        write_member_package(
            &dir,
            "devdep",
            "[package]\nname='devdep'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='dev_mimic'\npath='../shared/dev.rs'\nharness=false\n",
        )?;
        write_member_package(
            &dir,
            "builddep",
            "[package]\nname='builddep'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='build_mimic'\npath='../shared/build.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(&dir, "Cargo.toml", "[workspace]\nmembers = ['main']\n")?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));
        assert_eq!(
            verdict("shared/dev.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the dev-dependency member's declaration is honored"
        );
        assert_eq!(
            verdict("shared/build.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the build-dependency member's declaration is honored"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round six (I4dv), pinned as cargo's own behavior
    /// (#3634; empirically verified on the pinned toolchain against
    /// `cargo metadata`: an excluded direct path dependency stays outside
    /// `workspace_members`). A member depending on an excluded package by
    /// path does not give that package harness authority: its declarations
    /// validate nothing and conflict with nothing. The exclude x path-dep
    /// precedence is contested upstream; cargo's exclude-wins resolution
    /// is also the under-credit direction, so the pin is safe.
    #[test]
    fn excluded_path_dependency_gains_no_harness_authority() -> Result<(), String> {
        let dir = unique_workspace("excl-dep");
        write_member_package(
            &dir,
            "main",
            "[package]\nname='main'\nversion='0.1.0'\nedition='2024'\n\n[dependencies]\ndep = { path = '../dep' }\n",
        )?;
        write_member_package(
            &dir,
            "dep",
            "[package]\nname='dep'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../shared/mimic.rs'\nharness=true\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['main']\nexclude = ['dep']\n",
        )?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // The excluded dependency's harness = true declaration validates
        // nothing: the shared target matches no member declaration.
        assert_eq!(
            verdict("shared/mimic.rs"),
            CargoHarnessVerdict::NotDeclared,
            "the excluded dependency's declaration grants no authority"
        );

        // And it conflicts with nothing: a member declaration with the
        // opposite flag stays deterministic instead of becoming ambiguous.
        // The root here is a package so its own [[test]] declaration is
        // one a broken exclusion would have to clash with.
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n\
             [workspace]\nmembers = ['main']\nexclude = ['dep']\n\n\
             [[test]]\nname='mimic'\npath='shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "src")?;
        write_file(&dir, "src/lib.rs", "")?;
        assert_eq!(
            verdict("shared/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the excluded dependency's conflicting flag creates no ambiguity"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round seven (Jhjw/Jlb0), pinned as cargo's own
    /// behavior (#3634; empirically verified against `cargo metadata`):
    /// exclusion is directory-prefix aware, so `exclude = ['dep']` keeps a
    /// nested `dep/sub` path dependency outside the member set and its
    /// harness declarations grant no authority.
    #[test]
    fn exclusion_prefix_covers_nested_path_dependencies() -> Result<(), String> {
        let dir = unique_workspace("excl-prefix");
        write_member_package(
            &dir,
            "main",
            "[package]\nname='main'\nversion='0.1.0'\nedition='2024'\n\n[dependencies]\nsub = { path = '../dep/sub' }\n",
        )?;
        write_member_package(
            &dir,
            "dep/sub",
            "[package]\nname='sub'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=true\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['main']\nexclude = ['dep']\n",
        )?;
        let verdict = |relative: &str| cargo_test_target_harness_verdict(&dir, Path::new(relative));

        // The nested excluded dependency's declaration validates nothing.
        assert_eq!(
            verdict("shared/mimic.rs"),
            CargoHarnessVerdict::NotDeclared,
            "dep/sub is excluded through its parent directory prefix"
        );

        // And it conflicts with nothing: a member declaration with the
        // opposite flag stays deterministic instead of becoming ambiguous.
        write_file(
            &dir,
            "Cargo.toml",
            "[package]\nname='ws'\nversion='0.1.0'\nedition='2024'\n\n\
             [workspace]\nmembers = ['main']\nexclude = ['dep']\n\n\
             [[test]]\nname='mimic'\npath='shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "src")?;
        write_file(&dir, "src/lib.rs", "")?;
        assert_eq!(
            verdict("shared/mimic.rs"),
            CargoHarnessVerdict::HarnessDisabled,
            "the nested excluded dependency's conflicting flag creates no ambiguity"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round eight, shape A — explicit literal entry:
    /// `dep/sub` stays a member under `exclude = ['dep']` (verified
    /// against `cargo metadata`), so its `harness = false` declaration
    /// claims the target.
    #[test]
    fn explicit_literal_member_beats_parent_prefix_exclusion() -> Result<(), String> {
        let dir = unique_workspace("literal-member");
        write_member_package(
            &dir,
            "main",
            "[package]\nname='main'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_member_package(
            &dir,
            "dep/sub",
            "[package]\nname='sub'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['dep/sub']\nexclude = ['dep']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the explicit literal member is retained: its declaration claims the target"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3608 review round eight, shape B — glob-discovered member: the
    /// nested package reached through `dep/*` yields to the parent-prefix
    /// exclusion `dep`, so its declaration grants no authority. The
    /// declared path resolves inside the workspace
    /// (`../../shared/mimic.rs`), so the assertion actually discriminates:
    /// a regression that kept the member would surface its harness-enabled
    /// target as HarnessEnabled (round-eight review; fixture path fixed
    /// for #3634).
    #[test]
    fn glob_discovered_member_yields_to_parent_prefix_exclusion() -> Result<(), String> {
        let dir = unique_workspace("glob-member");
        write_member_package(
            &dir,
            "main",
            "[package]\nname='main'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_member_package(
            &dir,
            "dep/sub",
            "[package]\nname='sub'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=true\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['dep/*']\nexclude = ['dep']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::NotDeclared,
            "the glob-discovered member is dropped: its declaration grants no authority"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3634 edge case 1: a member depending on a path through
    /// `[workspace.dependencies]` inheritance is a workspace member per
    /// `cargo metadata`, so the inherited dependency's `harness = false`
    /// declaration claims its target. The pre-#3634 emulation left the
    /// inherited dependency unresolved and under-credited the
    /// registration.
    #[test]
    fn workspace_inherited_path_dependency_joins_the_member_set() -> Result<(), String> {
        let dir = unique_workspace("ws-inherited");
        write_member_package(
            &dir,
            "crates/app",
            "[package]\nname='app'\nversion='0.1.0'\nedition='2024'\n\n[dependencies]\ndep = { workspace = true }\n",
        )?;
        write_member_package(
            &dir,
            "dep",
            "[package]\nname='dep'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['crates/app']\n\n[workspace.dependencies]\ndep = { path = 'dep' }\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the workspace-inherited path dependency is a member: its declaration claims the target"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3634 edge case 2 (round-eight review): cargo treats
    /// `[workspace.exclude]` patterns as literal path prefixes — a
    /// wildcard component matches no member directory, verified against
    /// `cargo metadata` on the pinned toolchain. `exclude = ['dep/*']`
    /// therefore excludes nothing: the glob-matched member keeps its
    /// harness authority (shape: `members = ['dep/*']`), and so does the
    /// literal member under the same exclusion (shape:
    /// `members = ['dep/sub']`). The pre-#3634 emulation glob-matched
    /// exclude patterns and wrongly dropped both.
    #[test]
    fn exclude_wildcard_pattern_excludes_no_member() -> Result<(), String> {
        let dir = unique_workspace("exclude-wildcard");
        write_member_package(
            &dir,
            "main",
            "[package]\nname='main'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write_member_package(
            &dir,
            "dep/sub",
            "[package]\nname='sub'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=false\n",
        )?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['dep/*']\nexclude = ['dep/*']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "exclude = dep/* matches no member: dep/sub keeps its harness authority"
        );

        // The same holds for a literal member entry under the wildcard
        // exclusion: a literal prefix `dep/*` does not cover `dep/sub`.
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['dep/sub']\nexclude = ['dep/*']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the literal member is retained under the wildcard exclusion"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// #3634 (round-eight review): cargo expands character-class member
    /// globs — `members = ['crates/[ab]']` admits both packages, so the
    /// harness-declaring member's declaration claims the shared target.
    /// The pre-#3634 emulation treated the pattern as a literal
    /// directory, found no manifest there, and failed closed with
    /// ManifestUnavailable.
    #[test]
    fn character_class_member_glob_resolves() -> Result<(), String> {
        let dir = unique_workspace("charclass");
        write_member_package(
            &dir,
            "crates/a",
            "[package]\nname='a'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='mimic'\npath='../../shared/mimic.rs'\nharness=false\n",
        )?;
        write_member_package(
            &dir,
            "crates/b",
            "[package]\nname='b'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        make_dir(&dir, "crates/c")?;
        make_dir(&dir, "shared")?;
        write_file(
            &dir,
            "Cargo.toml",
            "[workspace]\nmembers = ['crates/[ab]']\n",
        )?;
        assert_eq!(
            cargo_test_target_harness_verdict(&dir, Path::new("shared/mimic.rs")),
            CargoHarnessVerdict::HarnessDisabled,
            "the character-class glob expands: member a's declaration claims the target"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
