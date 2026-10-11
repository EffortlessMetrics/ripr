//! Repository configuration loader for `ripr.toml`.
//!
//! The loader is intentionally small and repository-scoped. It walks from the
//! selected root toward the repository boundary for `ripr.toml`; it does not
//! read user-global config, environment variables, or hidden alternate config
//! paths. Command adapters decide precedence by applying explicit flags or LSP
//! initialization options after this file is loaded.

use crate::app::{CheckInput, Mode};
use crate::domain::{LanguageId, OracleStrength};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

mod diagnostic;
mod model;
mod python;
mod toolchain_file;
#[cfg(feature = "lang-typescript")]
mod typescript;

pub(crate) use diagnostic::ConfigDiagnostic;
#[cfg(test)]
use diagnostic::ConfigLocationStatus;
use model::{BunUbProfileConfig, FindingSeverityConfig, ProfilesConfig, SeamSeverityConfig};
pub use model::{
    CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION, CheckInputExplicit, ConfigIdentityRole, ConfigSeverity,
    LspDiagnosticProfile, OraclePolicy, PerlConfig, RiprConfig, SeverityConfig, TestHarnessAdapter,
    TestHarnessKind, TestHarnessRegistration, TypescriptConfig,
};
pub(crate) use model::{PERL_EXECUTABLE_OPT_IN_ENV, RustLanguageConfig};
#[cfg(test)]
pub(crate) use python::{PYTHON_EXCLUDED_DIRS, PYTHON_VENDOR_DIR};
pub(crate) use python::{
    PYTHON_PROJECT_MARKERS, PYTHON_SOURCE_DIR_MARKERS, detect_python_project,
    is_detectable_excluded_python_path, is_detectable_generated_python_path,
    is_detectable_python_source_name, is_python_dir_pruned_from_repo_discovery,
    is_python_excluded_dir_everywhere, python_project_marker_name, python_source_dir_marker_name,
    source_dir_contains_detectable_python, source_dir_contains_detectable_python_cancellable,
};
pub(crate) use toolchain_file::{repository_toolchain_path_pin, toolchain_path_pin_refusal};
#[cfg(feature = "lang-typescript")]
pub(crate) use typescript::{
    is_detectable_excluded_typescript_path, is_detectable_generated_typescript_path,
    is_typescript_dir_pruned_from_discovery,
};

pub(crate) const CONFIG_FILE_NAME: &str = "ripr.toml";
pub(crate) const DEFAULT_CONTEXT_RELATED_TESTS: usize = 5;
pub(crate) const DEFAULT_LSP_SEAM_DIAGNOSTICS: bool = true;
const DEFAULT_SUPPRESSIONS_PATH: &str = ".ripr/suppressions.toml";
const INIT_CONFIG_TEXT: &str = r#"[analysis]
# Default analysis mode when CLI flags or LSP initialization options do not
# set one explicitly. Valid: instant, draft, fast, deep, ready.
mode = "draft"
include_unchanged_tests = true

[oracles]
# Probe-relative defaults for oracle shapes that are repo-policy-sensitive.
# Valid strengths: strong, medium, weak, smoke, none, unknown.
snapshot_strength = "medium"
mock_expectation_strength = "medium"
broad_error_strength = "weak"

[severity.findings]
# Valid severities: info, warning, note.
exposed = "info"
weakly_exposed = "warning"
reachable_unrevealed = "warning"
no_static_path = "warning"
infection_unknown = "warning"
propagation_unknown = "note"
static_unknown = "note"

[severity.seams]
# Valid severities: off, info, warning, note.
strongly_gripped = "off"
weakly_gripped = "warning"
ungripped = "warning"
reachable_unrevealed = "warning"
activation_unknown = "info"
propagation_unknown = "info"
observation_unknown = "info"
discrimination_unknown = "info"
opaque = "info"
intentional = "off"
suppressed = "off"

[lsp]
# Built-in defaults enable bounded saved-workspace seam diagnostics. LSP
# initializationOptions.seamDiagnostics still wins explicitly, and repo policy
# may disable this with seam_diagnostics = false.
seam_diagnostics = true

[reports]
# Default for context packets and editor collect-context commands when no
# explicit --max-related-tests argument is supplied.
max_related_tests = 5

[suppressions]
# Repo-relative, slash-separated path. Badge renderers load this path.
path = ".ripr/suppressions.toml"

[languages]
# Per RIPR-SPEC-0026, only `rust` is enabled by default. Add `typescript` or
# `python` to opt into preview adapters when the ripr binary was built with
# the matching Cargo feature (`lang-typescript` or `lang-python`). When this
# file is absent, Python project markers can enable Python preview analysis
# automatically for the detected repository root; this explicit list remains
# authoritative when present.
# Valid values: rust, typescript, python, perl.
# (`perl` consumes externally-produced `ripr-perl-facts-v1` packets and does
# not parse Perl source directly. Pass --perl-facts <path> for an explicit
# packet, or configure a managed [perl].producer and make its exporter
# available. Without a packet or available exporter, Perl analysis is
# unavailable. See Campaign 31 #1379 + Support Tiers.)
enabled = ["rust"]
# Optional additive Rust generated-source globs. Built-in generated names and
# directories remain excluded unless declared in handwritten_files. A pattern without `/` matches any filename;
# patterns with `/` match the repository-relative path.
#
# [languages.rust]
# generated_file_patterns = ["*.gen.rs", "src/generated/**/*.rs"]
# handwritten_files = ["tests/generated_workflow.rs"]
# Exact paths exempt naming conventions only; patterns, headers and vendor markers win.

# Optional Bun stable-byte UB advisory profile. Leave this commented unless the
# repository wants TypeScript-family preview evidence for Bun Rust/FFI seams.
# JavaScript test files are covered by the `typescript` adapter.
#
# [profiles.bun_ub]
# test_roots = [
#   "test/js/**/*.test.ts",
#   "test/js/**/*.test.js",
# ]
# bridge_hints = "ripr.bun.bridge.toml"

# Optional TypeScript adapter settings (preview). Requires
# enabled = ["typescript"] in [languages] and the `lang-typescript` Cargo feature.
#
# [typescript]
# resolve_tsconfig_paths = false

# Optional Perl adapter settings (fact-packet consumer, preview — see Campaign 31 #1379).
#
# [perl]
# producer = "perl-ripr-facts"  # canonical managed exporter; "perllsp"/"perl-lsp" are compatibility wrappers
# executable = "perl-ripr-facts"  # Exporter path; honored only when RIPR_ALLOW_REPO_PERL_EXECUTABLE=1
# timeout_ms = 30000       # Per-invocation timeout
# cache_dir = "target/ripr/perl-facts"  # Fact cache location
"#;

/// Whether a `ripr.toml` entry exists at `path`, without following links.
/// `Path::exists` and `Path::is_file` follow them, so a dangling or
/// self-referencing symlink read as "no config" and consumers silently used
/// built-in defaults. The entry is present; reading it reports the real failure.
pub(crate) fn config_entry_present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Whether the analyzed root has a `ripr.toml` directory entry. Presence uses
/// [`config_entry_present`]: a dangling link is present, not absent.
pub(crate) fn config_present_at_root(root: &Path) -> bool {
    config_entry_present(&root.join(CONFIG_FILE_NAME))
}

trait ConfigurationIo: python::DiscoveryControl {
    fn config_entry(&mut self, path: &Path) -> Result<bool, String>;
    fn canonical_root(&mut self, path: &Path) -> Result<Option<PathBuf>, String>;
    fn canonical_config(&mut self, path: PathBuf) -> Result<PathBuf, String>;
    fn git_boundary(&mut self, path: &Path) -> Result<bool, String>;
    fn cargo_text(&mut self, path: &Path) -> Result<Option<String>, String>;
    fn config_text(&mut self, path: &Path) -> Result<String, String>;
}

struct OrdinaryConfigurationIo;

impl python::DiscoveryControl for OrdinaryConfigurationIo {
    fn checkpoint(&mut self) -> Result<(), crate::core_error::CoreError> {
        Ok(())
    }
    fn strict(&self) -> bool {
        false
    }
    fn joined_path(
        &mut self,
        parent: &Path,
        name: &std::ffi::OsStr,
    ) -> Result<PathBuf, crate::core_error::CoreError> {
        Ok(parent.join(name))
    }
    fn entry_path(
        &mut self,
        _parent: &Path,
        entry: &crate::analysis::committed_source::frozen::fs::FrozenDirEntry,
    ) -> Result<PathBuf, crate::core_error::CoreError> {
        Ok(entry.path())
    }
    fn before_entry(&mut self) -> Result<(), crate::core_error::CoreError> {
        Ok(())
    }
}

impl ConfigurationIo for OrdinaryConfigurationIo {
    fn config_entry(&mut self, path: &Path) -> Result<bool, String> {
        Ok(config_entry_present(path))
    }
    fn canonical_root(&mut self, path: &Path) -> Result<Option<PathBuf>, String> {
        Ok(std::fs::canonicalize(path).ok())
    }
    fn canonical_config(&mut self, path: PathBuf) -> Result<PathBuf, String> {
        Ok(std::fs::canonicalize(&path).unwrap_or(path))
    }
    fn git_boundary(&mut self, path: &Path) -> Result<bool, String> {
        Ok(path.exists())
    }
    fn cargo_text(&mut self, path: &Path) -> Result<Option<String>, String> {
        Ok(std::fs::read_to_string(path).ok())
    }
    fn config_text(&mut self, path: &Path) -> Result<String, String> {
        crate::bounded_input::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }
}

fn discover_config_path(
    root: &Path,
    io: &mut impl ConfigurationIo,
) -> Result<Option<PathBuf>, String> {
    let direct = io
        .joined_path(root, std::ffi::OsStr::new(CONFIG_FILE_NAME))
        .map_err(|error| error.to_string())?;
    if io.config_entry(&direct)? {
        // Ordinary unresolved links retain the original diagnostic path.
        return io.canonical_config(direct).map(Some);
    }

    let Some(search_root) = io.canonical_root(root)? else {
        return Ok(None);
    };
    for ancestor in search_root.ancestors() {
        let path = io
            .joined_path(ancestor, std::ffi::OsStr::new(CONFIG_FILE_NAME))
            .map_err(|error| error.to_string())?;
        if io.config_entry(&path)? {
            return Ok(Some(path));
        }
        if is_repository_boundary(ancestor, io)? {
            break;
        }
    }
    Ok(None)
}

fn is_repository_boundary(directory: &Path, io: &mut impl ConfigurationIo) -> Result<bool, String> {
    let git = io
        .joined_path(directory, std::ffi::OsStr::new(".git"))
        .map_err(|error| error.to_string())?;
    if io.git_boundary(&git)? {
        return Ok(true);
    }

    let manifest = io
        .joined_path(directory, std::ffi::OsStr::new("Cargo.toml"))
        .map_err(|error| error.to_string())?;
    let Some(contents) = io.cargo_text(&manifest)? else {
        return Ok(false);
    };
    if !contents.contains("workspace") {
        return Ok(false);
    }
    let Ok(document) = toml::from_str::<toml::Value>(&contents) else {
        return Ok(false);
    };
    Ok(document.get("workspace").is_some_and(toml::Value::is_table))
}

pub fn load_for_root(root: &Path) -> Result<RiprConfig, String> {
    load_for_root_with_io(root, &mut OrdinaryConfigurationIo)
}

fn load_for_root_with_io(root: &Path, io: &mut impl ConfigurationIo) -> Result<RiprConfig, String> {
    let Some(path) = discover_config_path(root, io)? else {
        return default_config_with_control(root, io);
    };
    let text = io.config_text(&path)?;
    let mut config = parse_config(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    config.source_path = Some(path);
    config.source_text = Some(text);
    Ok(config)
}

fn default_config_for_root(root: &Path) -> Result<RiprConfig, String> {
    default_config_with_control(root, &mut OrdinaryConfigurationIo)
}

fn default_config_with_control(
    root: &Path,
    control: &mut impl python::DiscoveryControl,
) -> Result<RiprConfig, String> {
    let mut config = RiprConfig::default();
    if python::detect_python_project_with_control(root, control)
        .map_err(|error| error.to_string())?
    {
        if !LanguageId::Python.is_available() {
            return Err(
                "Python project markers were detected, but this ripr binary was built without Cargo feature `lang-python`; use a Python-enabled ripr binary or add `ripr.toml` with `[languages] enabled = [\"rust\"]` to keep Python preview disabled"
                    .to_string(),
            );
        }
        if !config.languages.enabled.contains(&LanguageId::Python) {
            config.languages.enabled.push(LanguageId::Python);
        }
    }
    Ok(config)
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
#[derive(Clone, Copy)]
pub(crate) struct LiveConfigurationLimits {
    pub(crate) max_configuration_bytes: u64,
    pub(crate) max_boundary_bytes: u64,
    pub(crate) max_read_bytes: u64,
    pub(crate) max_entries: u64,
    pub(crate) max_path_bytes: u64,
    pub(crate) max_buffered_bytes: u64,
}

/// Private observation DATA, never an execution or saved-generation authority.
/// Syscall checkpoints are cooperative; the genuine worker cohort owns timeout.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct LiveConfigurationBudget {
    deadline: std::time::Instant,
    limits: LiveConfigurationLimits,
    retained_bytes: u64,
    entries: u64,
    paths: u64,
    read_bytes: u64,
    claimed: bool,
    failed: Option<String>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl LiveConfigurationBudget {
    const SCRATCH_BYTES: u64 = 64 * 1024;

    pub(crate) fn new(
        deadline: std::time::Instant,
        limits: LiveConfigurationLimits,
        retained_bytes: u64,
    ) -> Result<Self, String> {
        if !cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
            return Err("strict live configuration reads unsupported on this Linux target".into());
        }
        if [
            limits.max_configuration_bytes,
            limits.max_boundary_bytes,
            limits.max_read_bytes,
            limits.max_entries,
            limits.max_path_bytes,
            limits.max_buffered_bytes,
        ]
        .contains(&0)
            || limits.max_configuration_bytes > crate::bounded_input::MAX_CLI_INPUT_BYTES
            || limits.max_boundary_bytes > crate::bounded_input::MAX_CLI_INPUT_BYTES
        {
            return Err("live configuration invalid existing scalar bounds".into());
        }
        let budget = Self {
            deadline,
            limits,
            retained_bytes,
            entries: 0,
            paths: 0,
            read_bytes: 0,
            claimed: false,
            failed: None,
        };
        budget.checkpoint_live()?;
        budget.admit(0)?;
        Ok(budget)
    }

    fn checkpoint_live(&self) -> Result<(), String> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if std::time::Instant::now() >= self.deadline {
            return Err("live configuration original deadline expired".into());
        }
        crate::analysis::cancellation::checkpoint_typed().map_err(|error| error.to_string())
    }

    fn admit(&self, extra: u64) -> Result<(), String> {
        let total = self
            .retained_bytes
            .checked_add(self.paths)
            .and_then(|n| n.checked_add(Self::SCRATCH_BYTES))
            .and_then(|n| n.checked_add(extra))
            .ok_or("live configuration retained-byte overflow")?;
        if total > self.limits.max_buffered_bytes {
            return Err("live configuration retained-byte bound exceeded".into());
        }
        Ok(())
    }

    fn admit_entry(&mut self) -> Result<(), String> {
        self.checkpoint_live()?;
        self.entries = self
            .entries
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_entries)
            .ok_or("live configuration entry bound exceeded")?;
        Ok(())
    }

    fn admit_path(&mut self, bytes: u64, temporary_name_bytes: u64) -> Result<(), String> {
        self.checkpoint_live()?;
        let next = self
            .paths
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_path_bytes)
            .ok_or("live configuration path bound exceeded")?;
        // Cumulative admission is conservative: ancestor/recursive parent
        // frames and selected provenance remain charged until this pass drops.
        let old = self.paths;
        self.paths = next;
        let result = self.admit(temporary_name_bytes);
        if result.is_err() {
            self.paths = old;
        }
        result
    }

    fn join_live(&mut self, parent: &Path, name: &std::ffi::OsStr) -> Result<PathBuf, String> {
        let name_bytes = name.as_encoded_bytes().len() as u64;
        let bytes = (parent.as_os_str().as_encoded_bytes().len() as u64)
            .checked_add(1)
            .and_then(|n| n.checked_add(name_bytes))
            .ok_or("live configuration joined-path overflow")?;
        self.admit_path(bytes, name_bytes)?;
        Ok(parent.join(name))
    }

    fn inspect(
        &mut self,
        path: &Path,
        no_follow: bool,
    ) -> Result<Option<std::fs::Metadata>, String> {
        self.admit_entry()?;
        let result = if no_follow {
            std::fs::symlink_metadata(path)
        } else {
            std::fs::metadata(path)
        };
        self.checkpoint_live()?;
        match result {
            Ok(metadata) => Ok(Some(metadata)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!(
                "observe live configuration {}: {error}",
                path.display()
            )),
        }
    }

    fn canonical_live(&mut self, path: &Path) -> Result<PathBuf, String> {
        self.admit_entry()?;
        // Resolver internals are short-lived native-AS computation. Measure
        // and admit its returned path before retaining it in discovery state.
        let canonical = std::fs::canonicalize(path)
            .map_err(|error| format!("resolve live configuration {}: {error}", path.display()))?;
        self.checkpoint_live()?;
        self.admit_path(canonical.as_os_str().as_encoded_bytes().len() as u64, 0)?;
        Ok(canonical)
    }

    fn regular_text(&mut self, path: &Path, cap: u64) -> Result<String, String> {
        use std::io::Read as _;
        use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
        self.admit_entry()?;
        // Linux ARM64 has a distinct O_NOFOLLOW ABI. Unsupported Linux
        // targets cannot use this private reader. Ordinary nonregular inputs
        // retain their existing loader behavior.
        let no_follow = if cfg!(target_arch = "x86_64") {
            0x0002_0000
        } else if cfg!(target_arch = "aarch64") {
            0x0000_8000
        } else {
            return Err("strict live configuration reads unsupported on this Linux target".into());
        };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(no_follow | 0x0000_0800)
            .open(path)
            .map_err(|error| format!("open live configuration {}: {error}", path.display()))?;
        self.checkpoint_live()?;
        let before = file
            .metadata()
            .map_err(|error| format!("stat live configuration {}: {error}", path.display()))?;
        if !before.file_type().is_file() {
            return Err(format!(
                "live configuration {} is not a regular file",
                path.display()
            ));
        }
        if before.len() > cap {
            return Err(format!(
                "live configuration {} exceeds byte cap {cap}",
                path.display()
            ));
        }
        let identity = |metadata: &std::fs::Metadata| {
            (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        };
        let original = identity(&before);
        let mut reader = file;
        let mut bytes = Vec::new();
        let mut scratch = [0_u8; 64 * 1024];
        loop {
            self.checkpoint_live()?;
            // Preserve the existing one-byte EOF/overflow sentinel. A failed
            // probe can read one excess byte, never an uncharged full chunk.
            let remaining_file = cap
                .checked_sub(bytes.len() as u64)
                .ok_or("live configuration file-read counter exceeds cap")?;
            let remaining_total = self
                .limits
                .max_read_bytes
                .checked_sub(self.read_bytes)
                .ok_or("live configuration aggregate-read counter exceeds cap")?;
            let requested = remaining_file
                .min(remaining_total)
                .saturating_add(1)
                .min(scratch.len() as u64) as usize;
            self.admit(bytes.len() as u64)?;
            let read = match reader.read(&mut scratch[..requested]) {
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(format!(
                        "read live configuration {}: {error}",
                        path.display()
                    ));
                }
            };
            // Charge every byte actually returned immediately, including a
            // failed probe or a read that crossed cancellation/deadline.
            self.read_bytes = self
                .read_bytes
                .checked_add(read as u64)
                .ok_or("live configuration aggregate read counter overflow")?;
            if self.read_bytes > self.limits.max_read_bytes {
                return Err("live configuration aggregate read bound exceeded".into());
            }
            self.checkpoint_live()?;
            if read == 0 {
                break;
            }
            let next = (bytes.len() as u64)
                .checked_add(read as u64)
                .filter(|n| *n <= cap)
                .ok_or("live configuration byte cap exceeded during read")?;
            self.admit(next)?;
            // Payload growth is checked before allocation. Allocator rounding
            // and old/new allocation overlap remain under genuine native AS.
            let next = usize::try_from(next)
                .map_err(|error| format!("live configuration buffer size: {error}"))?;
            if bytes.capacity() < next {
                bytes
                    .try_reserve_exact(next - bytes.len())
                    .map_err(|error| format!("live configuration buffer allocation: {error}"))?;
            }
            bytes.extend_from_slice(&scratch[..read]);
        }
        let after = reader
            .metadata()
            .map_err(|error| format!("restat live configuration {}: {error}", path.display()))?;
        let named = std::fs::symlink_metadata(path)
            .map_err(|error| format!("reobserve live configuration {}: {error}", path.display()))?;
        self.checkpoint_live()?;
        if identity(&after) != original
            || identity(&named) != original
            || bytes.len() as u64 != before.len()
        {
            return Err("live configuration regular file changed during observation".into());
        }
        String::from_utf8(bytes).map_err(|error| format!("read {} failed: {error}", path.display()))
    }

    pub(crate) fn reobserve_root(
        &mut self,
        requested_root: &Path,
        expected_root: &Path,
    ) -> Result<(), String> {
        self.admit_path(
            requested_root.as_os_str().as_encoded_bytes().len() as u64,
            0,
        )?;
        let observed = self.canonical_live(requested_root)?;
        if observed.as_os_str() != expected_root.as_os_str() {
            return Err("live configuration bound root changed".into());
        }
        Ok(())
    }

    pub(crate) fn admit_full_copy(&self, text_bytes: u64, copy_bytes: u64) -> Result<(), String> {
        self.checkpoint_live()?;
        self.admit(
            text_bytes
                .checked_add(copy_bytes)
                .ok_or("live configuration comparison-byte overflow")?,
        )
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl python::DiscoveryControl for LiveConfigurationBudget {
    fn checkpoint(&mut self) -> Result<(), crate::core_error::CoreError> {
        self.checkpoint_live().map_err(Into::into)
    }
    fn strict(&self) -> bool {
        true
    }
    fn before_directory(&mut self, dir: &Path) -> Result<(), crate::core_error::CoreError> {
        self.admit_entry()?;
        // A directory iterator retains its root through recursive descent.
        self.admit_path(dir.as_os_str().as_encoded_bytes().len() as u64, 0)
            .map_err(Into::into)
    }
    fn after_io(&mut self) -> Result<(), crate::core_error::CoreError> {
        self.checkpoint_live().map_err(Into::into)
    }
    fn absent_directory(&self, error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
        )
    }
    fn joined_path(
        &mut self,
        parent: &Path,
        name: &std::ffi::OsStr,
    ) -> Result<PathBuf, crate::core_error::CoreError> {
        self.join_live(parent, name).map_err(Into::into)
    }
    fn entry_path(
        &mut self,
        parent: &Path,
        entry: &crate::analysis::committed_source::frozen::fs::FrozenDirEntry,
    ) -> Result<PathBuf, crate::core_error::CoreError> {
        // This transient OS name is native-AS-contained; charge coexistence
        // with the new joined path before growth, then drop it before descent.
        let name = entry.file_name();
        // The entry's original name remains alive while a child is visited.
        // The temporary copy coexists until the measured join completes.
        let name_bytes = name.as_encoded_bytes().len() as u64;
        self.admit_path(name_bytes, name_bytes)?;
        self.join_live(parent, &name).map_err(Into::into)
    }
    fn before_entry(&mut self) -> Result<(), crate::core_error::CoreError> {
        self.admit_entry().map_err(Into::into)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl ConfigurationIo for LiveConfigurationBudget {
    fn config_entry(&mut self, path: &Path) -> Result<bool, String> {
        self.inspect(path, true).map(|value| value.is_some())
    }
    fn canonical_root(&mut self, path: &Path) -> Result<Option<PathBuf>, String> {
        self.canonical_live(path).map(Some)
    }
    fn canonical_config(&mut self, path: PathBuf) -> Result<PathBuf, String> {
        self.canonical_live(&path)
    }
    fn git_boundary(&mut self, path: &Path) -> Result<bool, String> {
        self.inspect(path, false).map(|value| value.is_some())
    }
    fn cargo_text(&mut self, path: &Path) -> Result<Option<String>, String> {
        if self.inspect(path, true)?.is_none() {
            return Ok(None);
        }
        self.regular_text(path, self.limits.max_boundary_bytes)
            .map(Some)
    }
    fn config_text(&mut self, path: &Path) -> Result<String, String> {
        self.regular_text(path, self.limits.max_configuration_bytes)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) fn load_for_root_bounded(
    root: &Path,
    budget: &mut LiveConfigurationBudget,
) -> Result<RiprConfig, String> {
    if let Some(error) = &budget.failed {
        return Err(error.clone());
    }
    if budget.claimed {
        let error = "live configuration observation already claimed".to_string();
        budget.failed = Some(error.clone());
        return Err(error);
    }
    // Claim before cancellation/deadline checks as well as discovery. Restoring
    // a cancellation context cannot turn this failed attempt into fresh DATA.
    budget.claimed = true;
    let result = budget.checkpoint_live().and_then(|()| {
        if crate::analysis::committed_source::frozen::current().is_some() {
            Err("live configuration observation still has frozen source context".into())
        } else {
            budget
                .admit_path(root.as_os_str().as_encoded_bytes().len() as u64, 0)
                .and_then(|()| load_for_root_with_io(root, budget))
        }
    });
    // Parsing/default construction must also finish under the ORIGINAL clock.
    // Existing failures keep their first diagnostic and cannot be retried.
    let result = result.and_then(|config| {
        budget.checkpoint_live()?;
        Ok(config)
    });
    if let Err(error) = &result {
        budget.failed = Some(error.clone());
    }
    result
}

/// Construct configuration from the snapshot owner's captured configuration.
/// The owner authenticates the capture and retains the snapshot lifetime.
/// `snapshot_root` is physical during preparation, or the admitted logical root
/// while frozen filesystem authority is installed. The source path is provenance
/// and diagnostic data; it is never read.
pub(crate) fn config_for_captured_snapshot(
    snapshot_root: &Path,
    logical_source_path: &Path,
    captured: &crate::analysis::git_candidate_execution::CapturedConfiguration,
) -> Result<RiprConfig, String> {
    use crate::analysis::git_candidate_execution::CapturedConfiguration;

    match captured {
        CapturedConfiguration::NotRequested => {
            Err("snapshot configuration capture was not requested".to_string())
        }
        CapturedConfiguration::Absent => default_config_for_root(snapshot_root),
        CapturedConfiguration::Present { text, .. } => {
            let mut config = parse_config(text)
                .map_err(|error| format!("{}: {error}", logical_source_path.display()))?;
            config.source_path = Some(logical_source_path.to_path_buf());
            config.source_text = Some(text.clone());
            Ok(config)
        }
    }
}

pub(crate) fn generated_init_config() -> &'static str {
    INIT_CONFIG_TEXT
}

pub(crate) fn config_fingerprint(source_text: &str) -> String {
    bytes_fingerprint(source_text.as_bytes())
}

/// [`config_fingerprint`] over raw bytes, for inputs that need not be UTF-8.
pub(crate) fn bytes_fingerprint(bytes: &[u8]) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Canonical analysis-config identity for the check-artifact identity gate
/// (RIPR-SPEC-0140): the finding-affecting allowlist fields, canonically
/// serialized (field name, normalized value, defaults materialized), sorted,
/// and hashed. Render-only knobs are excluded by the allowlist and are
/// honored fresh at render time by the consuming command.
pub(crate) fn check_artifact_config_identity_hash(config: &RiprConfig) -> String {
    let mut pairs = config
        .check_artifact_identity_fields()
        .into_iter()
        .filter(|field| field.role == ConfigIdentityRole::FindingAffecting)
        .map(|field| format!("{}={}", field.name, field.value.clone().unwrap_or_default()))
        .collect::<Vec<_>>();
    pairs.sort();
    config_fingerprint(&pairs.join("\n"))
}

/// The config identity published in the diff-check outcome identity block
/// (#5988): the fingerprint of the exact `ripr.toml` text loaded for the run,
/// so any change to the loaded config — finding-affecting allowlist fields,
/// but also the mode / unchanged-test / enabled-language settings the
/// check-artifact identity records in separate fields — moves the identity
/// block agents compare (#6777 review: the finding-affecting allowlist alone
/// let finding-changing settings share one block). `Some` exactly when a
/// `ripr.toml` was actually loaded ([`RiprConfig::source_text`] is present);
/// a defaults-only run — no config file, or a bound Git-candidate subject
/// that must ignore the worktree config — keeps `null`.
pub(crate) fn loaded_config_identity(config: &RiprConfig) -> Option<String> {
    config.source_text().map(config_fingerprint)
}

/// The exact `ripr.toml` fields the repo-exposure producer (the seam
/// inventory in `crates/ripr/src/analysis/seam_inventory.rs`) consumes
/// semantically. Verified against the producer: the seam walker is Rust-only
/// and reads the oracle-strength policy (via `rust_index::apply_oracle_policy`),
/// production-like / harness opt-ins, and `[languages.rust]
/// generated_file_patterns` (#4788). It does not read `languages.enabled`
/// or any typescript/perl field, so those SPEC-0140 finding-affecting
/// fields must NOT move the repo-exposure input identity (#2823 — two runs
/// differing only in an unconsumed setting stay comparable). Closed set:
/// when the producer starts consuming another config field, add it here in
/// the same PR; do not widen the filter to whole sections.
pub(crate) const REPO_EXPOSURE_CONSUMED_CONFIG_FIELDS: [&str; 7] = [
    "oracles.broad_error_strength",
    "oracles.mock_expectation_strength",
    "oracles.snapshot_strength",
    // The production-like opt-in changes which files are production
    // subjects in the repo seam inventory (#3283).
    "analysis.production_like_targets",
    // Harness registrations change which files are evidence subjects and
    // which functions are executable tests in the repo seam inventory
    // (#3532).
    "analysis.test_harnesses",
    // Generated-file patterns change which Rust files become seams and
    // which paths appear in `generated_rust_source_skipped` (#4788).
    "languages.rust.generated_file_patterns",
    "languages.rust.handwritten_files",
];

/// Canonical config identity for the repo-exposure artifact input identity
/// (#2823): exactly the producer-consumed fields
/// ([`REPO_EXPOSURE_CONSUMED_CONFIG_FIELDS`]), canonically serialized through
/// the same SPEC-0140 field enumerator (field name, normalized value,
/// defaults materialized), sorted, and hashed. This is deliberately narrower
/// than [`check_artifact_config_identity_hash`], which serves the diff-check
/// pipeline and legitimately includes typescript/perl inputs the seam
/// inventory never reads.
pub fn repo_exposure_config_identity_hash(config: &RiprConfig) -> String {
    // This producer is Rust-only regardless of the diff adapter selection.
    // Reuse the canonical field authority with its actual consumed language.
    let mut consumed = config.clone();
    consumed.languages.enabled = vec![LanguageId::Rust];
    let mut pairs = consumed
        .check_artifact_identity_fields()
        .into_iter()
        .filter(|field| {
            field.role == ConfigIdentityRole::FindingAffecting
                && REPO_EXPOSURE_CONSUMED_CONFIG_FIELDS.contains(&field.name)
        })
        .map(|field| format!("{}={}", field.name, field.value.clone().unwrap_or_default()))
        .collect::<Vec<_>>();
    pairs.sort();
    config_fingerprint(&pairs.join("\n"))
}

pub(crate) fn apply_to_check_input(
    input: &mut CheckInput,
    config: &RiprConfig,
    explicit: CheckInputExplicit,
) {
    if !explicit.mode
        && let Some(mode) = config.analysis.mode()
    {
        input.mode = mode.clone();
    }
    if !explicit.include_unchanged_tests
        && let Some(include) = config.analysis.include_unchanged_tests()
    {
        input.include_unchanged_tests = include;
    }
}

fn parse_config(text: &str) -> Result<RiprConfig, String> {
    parse_config_diagnostic(text).map_err(|diagnostic| diagnostic.message)
}

/// Preserve semantic source locations without changing the typed configuration
/// authority. Consumers can project this diagnostic into their own transports.
/// The diagnostic is boxed: it is deliberately rich (spans, expected values),
/// which would otherwise trip `clippy::result_large_err` on every parser hop.
pub(crate) fn parse_config_diagnostic(text: &str) -> Result<RiprConfig, Box<ConfigDiagnostic>> {
    let raw: RawConfig = toml::from_str(text).map_err(|err| {
        // A valid key in the wrong table names its table (#4534). The hint is
        // built from the fixed KEY_HOME_TABLES allowlist, never from file text.
        let err = err.to_string();
        let hint = misplaced_key_hint(&err).unwrap_or_default();
        Box::new(ConfigDiagnostic::structural(format!(
            "invalid ripr.toml: {err}{hint}"
        )))
    })?;
    RiprConfig::from_raw(raw, text)
}

/// Keys that belong to exactly one table, so a key found anywhere else was
/// put in the wrong table rather than misspelled (#4534). Keys shared by two
/// tables (the `[severity.findings]`/`[severity.seams]` classes) stay out:
/// naming one table would be a guess.
const KEY_HOME_TABLES: &[(&str, &str)] = &[
    ("mode", "analysis"),
    ("include_unchanged_tests", "analysis"),
    ("production_like_targets", "analysis"),
    ("test_harnesses", "analysis"),
    ("snapshot_strength", "oracles"),
    ("mock_expectation_strength", "oracles"),
    ("broad_error_strength", "oracles"),
    ("seam_diagnostics", "lsp"),
    ("diagnostic_profile", "lsp"),
    ("max_related_tests", "reports"),
    ("enabled", "languages"),
    ("generated_file_patterns", "languages.rust"),
    ("resolve_tsconfig_paths", "typescript"),
    ("bun_ub", "profiles"),
    ("findings", "severity"),
    ("seams", "severity"),
];

/// The table an unknown-field key belongs in, when serde rejected a real key
/// that sits in the wrong table (for example a top-level `mode`).
fn misplaced_key_hint(error: &str) -> Option<String> {
    let (_, rest) = error.split_once("unknown field `")?;
    let (key, _) = rest.split_once('`')?;
    let (key, table) = KEY_HOME_TABLES.iter().find(|(known, _)| *known == key)?;
    Some(format!("\n{}", misplaced_key_hint_line(key, table)))
}

fn misplaced_key_hint_line(key: &str, table: &str) -> String {
    format!("`{key}` is a valid key, but it belongs under [{table}]")
}

/// A source-free summary of a config load error for surfaces that must not
/// carry the TOML parser's source excerpt (doctor JSON, editor
/// notifications; RIPR-SPEC-0007): the first line (path, parse location),
/// plus the misplaced-key hint (#4534) when present. A hint line is kept
/// only when it equals one built from the fixed key/table allowlist, so no
/// file content passes through.
pub(crate) fn config_error_summary(error: &str) -> String {
    let first = error.lines().next().unwrap_or(error).trim();
    let hint = error.lines().skip(1).map(str::trim).find(|line| {
        KEY_HOME_TABLES
            .iter()
            .any(|(key, table)| *line == misplaced_key_hint_line(key, table))
    });
    match hint {
        Some(hint) => format!("{first}; {hint}"),
        None => first.to_string(),
    }
}

#[cfg(test)]
pub(crate) fn tests_only_parse(text: &str) -> Result<RiprConfig, String> {
    parse_config(text)
}

impl RiprConfig {
    fn from_raw(raw: RawConfig, text: &str) -> Result<Self, Box<ConfigDiagnostic>> {
        let mut config = RiprConfig::default();
        if let Some(analysis) = raw.analysis {
            if let Some(mode) = analysis.mode {
                config.analysis.mode =
                    Some(parse_mode_value(mode.get_ref()).map_err(|message| {
                        ConfigDiagnostic::at_value(message, "analysis.mode", mode.span(), text)
                    })?);
            }
            config.analysis.include_unchanged_tests = analysis.include_unchanged_tests;
            if let Some(targets) = analysis.production_like_targets {
                let mut parsed = std::collections::BTreeSet::new();
                for target in targets {
                    parsed.insert(parse_relative_path(
                        "analysis.production_like_targets",
                        &target,
                    )?);
                }
                config.analysis.production_like_targets = parsed;
            }
            if let Some(registrations) = analysis.test_harnesses {
                config.analysis.test_harnesses = parse_test_harness_registrations(registrations)?;
            }
        }
        if let Some(oracles) = raw.oracles {
            if let Some(strength) = oracles.snapshot_strength {
                // The key-named message (#4534) rides in `message`; the
                // diagnostic additionally carries the span and expected values.
                config.oracles.snapshot_strength =
                    parse_oracle_strength("oracles.snapshot_strength", strength.get_ref())
                        .map_err(|message| {
                            ConfigDiagnostic::at_value(
                                message,
                                "oracles.snapshot_strength",
                                strength.span(),
                                text,
                            )
                        })?;
            }
            if let Some(strength) = oracles.mock_expectation_strength {
                config.oracles.mock_expectation_strength =
                    parse_oracle_strength("oracles.mock_expectation_strength", strength.get_ref())
                        .map_err(|message| {
                            ConfigDiagnostic::at_value(
                                message,
                                "oracles.mock_expectation_strength",
                                strength.span(),
                                text,
                            )
                        })?;
            }
            if let Some(strength) = oracles.broad_error_strength {
                config.oracles.broad_error_strength =
                    parse_oracle_strength("oracles.broad_error_strength", strength.get_ref())
                        .map_err(|message| {
                            ConfigDiagnostic::at_value(
                                message,
                                "oracles.broad_error_strength",
                                strength.span(),
                                text,
                            )
                        })?;
            }
        }
        if let Some(severity) = raw.severity {
            config.severity = merge_severity(config.severity, severity, text)?;
        }
        if let Some(lsp) = raw.lsp {
            if let Some(seam_diagnostics) = lsp.seam_diagnostics {
                config.lsp.seam_diagnostics = Some(seam_diagnostics);
            }
            if let Some(profile) = lsp.diagnostic_profile {
                config.lsp.diagnostic_profile = Some(
                    LspDiagnosticProfile::parse(profile.get_ref()).map_err(|err| {
                        ConfigDiagnostic::at_value(
                            format!("{err} in [lsp]"),
                            "lsp.diagnostic_profile",
                            profile.span(),
                            text,
                        )
                    })?,
                );
            }
        }
        if let Some(reports) = raw.reports
            && let Some(max) = reports.max_related_tests
        {
            config.reports.max_related_tests = max;
        }
        if let Some(suppressions) = raw.suppressions
            && let Some(path) = suppressions.path
        {
            config.suppressions.path = parse_relative_path("suppressions.path", &path)?;
        }
        if let Some(languages) = raw.languages {
            if let Some(enabled) = languages.enabled {
                config.languages.enabled = parse_languages_enabled(&enabled)?;
            }
            if let Some(rust) = languages.rust {
                if let Some(patterns) = rust.generated_file_patterns {
                    config.languages.rust.generated_file_patterns =
                        parse_generated_file_patterns(&patterns)?;
                }
                if let Some(paths) = rust.handwritten_files {
                    config.languages.rust.handwritten_files = parse_handwritten_files(&paths)?;
                }
            }
        }
        if let Some(profiles) = raw.profiles {
            config.profiles = parse_profiles(profiles)?;
        }
        if let Some(ts) = raw.typescript {
            config.typescript = TypescriptConfig {
                resolve_tsconfig_paths: ts.resolve_tsconfig_paths.unwrap_or(false),
            };
        }
        if let Some(perl) = raw.perl {
            config.perl = PerlConfig {
                producer: perl.producer,
                executable: perl.executable.map(PathBuf::from),
                timeout_ms: perl.timeout_ms.unwrap_or(30_000),
                // Repository config: ripr creates, writes and renames files
                // here, so it must not name a directory outside the checkout.
                cache_dir: perl
                    .cache_dir
                    .map(|path| parse_relative_path("perl.cache_dir", &path))
                    .transpose()?,
            };
        }
        Ok(config)
    }
}

fn parse_languages_enabled(values: &[String]) -> Result<Vec<LanguageId>, String> {
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let language = match value.as_str() {
            "rust" => LanguageId::Rust,
            "typescript" => LanguageId::TypeScript,
            "python" => LanguageId::Python,
            "perl" => LanguageId::Perl,
            other => {
                return Err(format!(
                    "languages.enabled lists unknown language `{other}`; valid values are rust, typescript, python, perl (Perl consumes externally-produced fact packets; use --perl-facts <path> or a configured managed [perl].producer)"
                ));
            }
        };
        if parsed.contains(&language) {
            return Err(format!(
                "languages.enabled lists `{value}` more than once; remove the duplicate"
            ));
        }
        if !language.is_available() {
            return Err(format!(
                "languages.enabled lists `{value}`, but this ripr binary was built without Cargo feature `{}`; {}",
                language.required_feature(),
                language.unavailable_adapter_recovery()
            ));
        }
        parsed.push(language);
    }
    Ok(parsed)
}

fn parse_generated_file_patterns(values: &[String]) -> Result<Vec<String>, String> {
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        if value.chars().any(char::is_control) {
            return Err(
                "languages.rust.generated_file_patterns must not contain control characters"
                    .to_string(),
            );
        }
        let trimmed = value.trim();
        parse_relative_path("languages.rust.generated_file_patterns", trimmed)?;
        if trimmed == "." {
            return Err(
                "languages.rust.generated_file_patterns must identify a file pattern, not `.`"
                    .to_string(),
            );
        }
        if parsed.iter().any(|existing| existing == trimmed) {
            return Err(format!(
                "languages.rust.generated_file_patterns lists `{trimmed}` more than once; remove the duplicate"
            ));
        }
        parsed.push(trimmed.to_string());
    }
    Ok(parsed)
}

fn parse_handwritten_files(values: &[String]) -> Result<Vec<String>, String> {
    let field = "languages.rust.handwritten_files";
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        if value.chars().any(char::is_control) {
            return Err(format!("{field} must not contain control characters"));
        }
        if value.contains(['*', '?', '[', ']']) {
            return Err(format!(
                "{field} must contain exact file paths, not glob patterns"
            ));
        }
        let path = parse_relative_path(field, value)?;
        if path.extension().is_none_or(|extension| extension != "rs") {
            return Err(format!("{field} must identify a Rust `.rs` file"));
        }
        let normalized = path.to_string_lossy().replace('\\', "/");
        if parsed.contains(&normalized) {
            return Err(format!(
                "{field} lists `{normalized}` more than once; remove the duplicate"
            ));
        }
        parsed.push(normalized);
    }
    parsed.sort_unstable();
    Ok(parsed)
}

fn parse_profiles(raw: RawProfilesConfig) -> Result<ProfilesConfig, String> {
    Ok(ProfilesConfig {
        bun_ub: raw.bun_ub.map(parse_bun_ub_profile).transpose()?,
    })
}

fn parse_bun_ub_profile(raw: RawBunUbProfileConfig) -> Result<BunUbProfileConfig, String> {
    let test_roots = raw
        .test_roots
        .ok_or_else(|| "profiles.bun_ub.test_roots is required".to_string())?;
    if test_roots.is_empty() {
        return Err("profiles.bun_ub.test_roots must list at least one test root".to_string());
    }
    let mut parsed_roots = Vec::with_capacity(test_roots.len());
    for root in test_roots {
        let trimmed = root.trim();
        parse_relative_path("profiles.bun_ub.test_roots", trimmed)?;
        if parsed_roots.iter().any(|existing| existing == trimmed) {
            return Err(format!(
                "profiles.bun_ub.test_roots lists `{trimmed}` more than once; remove the duplicate"
            ));
        }
        parsed_roots.push(trimmed.to_string());
    }
    let bridge_hints = raw
        .bridge_hints
        .ok_or_else(|| "profiles.bun_ub.bridge_hints is required".to_string())
        .and_then(|path| parse_relative_path("profiles.bun_ub.bridge_hints", &path))?;
    Ok(BunUbProfileConfig {
        test_roots: parsed_roots,
        bridge_hints,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    analysis: Option<RawAnalysisConfig>,
    oracles: Option<RawOraclePolicy>,
    severity: Option<RawSeverityConfig>,
    lsp: Option<RawLspConfig>,
    reports: Option<RawReportsConfig>,
    suppressions: Option<RawSuppressionsConfig>,
    languages: Option<RawLanguagesConfig>,
    profiles: Option<RawProfilesConfig>,
    typescript: Option<RawTypescriptConfig>,
    perl: Option<RawPerlConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawTypescriptConfig {
    resolve_tsconfig_paths: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawLanguagesConfig {
    enabled: Option<Vec<String>>,
    rust: Option<RawRustLanguageConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawRustLanguageConfig {
    generated_file_patterns: Option<Vec<String>>,
    handwritten_files: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawPerlConfig {
    producer: Option<String>,
    executable: Option<String>,
    timeout_ms: Option<u64>,
    cache_dir: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawProfilesConfig {
    bun_ub: Option<RawBunUbProfileConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawBunUbProfileConfig {
    test_roots: Option<Vec<String>>,
    bridge_hints: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawAnalysisConfig {
    mode: Option<toml::Spanned<String>>,
    include_unchanged_tests: Option<bool>,
    production_like_targets: Option<Vec<String>>,
    test_harnesses: Option<Vec<RawTestHarnessRegistration>>,
}

/// Raw `[analysis.test_harnesses]` entry (#3532). Every field is exact:
/// unknown values, root escapes, and kind/adapter mismatches fail closed
/// at parse time with named errors.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawTestHarnessRegistration {
    registration_id: Option<String>,
    target: Option<String>,
    kind: Option<String>,
    adapter: Option<String>,
    marker: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawOraclePolicy {
    snapshot_strength: Option<toml::Spanned<String>>,
    mock_expectation_strength: Option<toml::Spanned<String>>,
    broad_error_strength: Option<toml::Spanned<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawLspConfig {
    seam_diagnostics: Option<bool>,
    diagnostic_profile: Option<toml::Spanned<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawReportsConfig {
    max_related_tests: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawSuppressionsConfig {
    path: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawSeverityConfig {
    findings: Option<RawFindingSeverityConfig>,
    seams: Option<RawSeamSeverityConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawFindingSeverityConfig {
    exposed: Option<toml::Spanned<String>>,
    weakly_exposed: Option<toml::Spanned<String>>,
    reachable_unrevealed: Option<toml::Spanned<String>>,
    no_static_path: Option<toml::Spanned<String>>,
    infection_unknown: Option<toml::Spanned<String>>,
    propagation_unknown: Option<toml::Spanned<String>>,
    static_unknown: Option<toml::Spanned<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawSeamSeverityConfig {
    strongly_gripped: Option<toml::Spanned<String>>,
    weakly_gripped: Option<toml::Spanned<String>>,
    ungripped: Option<toml::Spanned<String>>,
    reachable_unrevealed: Option<toml::Spanned<String>>,
    activation_unknown: Option<toml::Spanned<String>>,
    propagation_unknown: Option<toml::Spanned<String>>,
    observation_unknown: Option<toml::Spanned<String>>,
    discrimination_unknown: Option<toml::Spanned<String>>,
    opaque: Option<toml::Spanned<String>>,
    intentional: Option<toml::Spanned<String>>,
    suppressed: Option<toml::Spanned<String>>,
}

fn merge_severity(
    mut current: SeverityConfig,
    raw: RawSeverityConfig,
    text: &str,
) -> Result<SeverityConfig, Box<ConfigDiagnostic>> {
    if let Some(findings) = raw.findings {
        merge_finding_severity(&mut current.findings, findings, text)?;
    }
    if let Some(seams) = raw.seams {
        merge_seam_severity(&mut current.seams, seams, text)?;
    }
    Ok(current)
}

fn merge_finding_severity(
    current: &mut FindingSeverityConfig,
    raw: RawFindingSeverityConfig,
    text: &str,
) -> Result<(), Box<ConfigDiagnostic>> {
    assign_severity(
        &mut current.exposed,
        raw.exposed,
        "severity.findings.exposed",
        false,
        text,
    )?;
    assign_severity(
        &mut current.weakly_exposed,
        raw.weakly_exposed,
        "severity.findings.weakly_exposed",
        false,
        text,
    )?;
    assign_severity(
        &mut current.reachable_unrevealed,
        raw.reachable_unrevealed,
        "severity.findings.reachable_unrevealed",
        false,
        text,
    )?;
    assign_severity(
        &mut current.no_static_path,
        raw.no_static_path,
        "severity.findings.no_static_path",
        false,
        text,
    )?;
    assign_severity(
        &mut current.infection_unknown,
        raw.infection_unknown,
        "severity.findings.infection_unknown",
        false,
        text,
    )?;
    assign_severity(
        &mut current.propagation_unknown,
        raw.propagation_unknown,
        "severity.findings.propagation_unknown",
        false,
        text,
    )?;
    assign_severity(
        &mut current.static_unknown,
        raw.static_unknown,
        "severity.findings.static_unknown",
        false,
        text,
    )?;
    Ok(())
}

fn merge_seam_severity(
    current: &mut SeamSeverityConfig,
    raw: RawSeamSeverityConfig,
    text: &str,
) -> Result<(), Box<ConfigDiagnostic>> {
    assign_severity(
        &mut current.strongly_gripped,
        raw.strongly_gripped,
        "severity.seams.strongly_gripped",
        true,
        text,
    )?;
    assign_severity(
        &mut current.weakly_gripped,
        raw.weakly_gripped,
        "severity.seams.weakly_gripped",
        true,
        text,
    )?;
    assign_severity(
        &mut current.ungripped,
        raw.ungripped,
        "severity.seams.ungripped",
        true,
        text,
    )?;
    assign_severity(
        &mut current.reachable_unrevealed,
        raw.reachable_unrevealed,
        "severity.seams.reachable_unrevealed",
        true,
        text,
    )?;
    assign_severity(
        &mut current.activation_unknown,
        raw.activation_unknown,
        "severity.seams.activation_unknown",
        true,
        text,
    )?;
    assign_severity(
        &mut current.propagation_unknown,
        raw.propagation_unknown,
        "severity.seams.propagation_unknown",
        true,
        text,
    )?;
    assign_severity(
        &mut current.observation_unknown,
        raw.observation_unknown,
        "severity.seams.observation_unknown",
        true,
        text,
    )?;
    assign_severity(
        &mut current.discrimination_unknown,
        raw.discrimination_unknown,
        "severity.seams.discrimination_unknown",
        true,
        text,
    )?;
    assign_severity(
        &mut current.opaque,
        raw.opaque,
        "severity.seams.opaque",
        true,
        text,
    )?;
    assign_severity(
        &mut current.intentional,
        raw.intentional,
        "severity.seams.intentional",
        true,
        text,
    )?;
    assign_severity(
        &mut current.suppressed,
        raw.suppressed,
        "severity.seams.suppressed",
        true,
        text,
    )?;
    Ok(())
}

fn assign_severity(
    target: &mut ConfigSeverity,
    raw: Option<toml::Spanned<String>>,
    field: &str,
    allow_off: bool,
    text: &str,
) -> Result<(), Box<ConfigDiagnostic>> {
    if let Some(value) = raw {
        *target = parse_severity(field, value.get_ref(), allow_off)
            .map_err(|message| ConfigDiagnostic::at_value(message, field, value.span(), text))?;
    }
    Ok(())
}

fn parse_mode_value(value: &str) -> Result<Mode, String> {
    match value {
        "instant" => Ok(Mode::Instant),
        "draft" => Ok(Mode::Draft),
        "fast" => Ok(Mode::Fast),
        "deep" => Ok(Mode::Deep),
        "ready" => Ok(Mode::Ready),
        _ => Err(format!(
            "analysis.mode `{value}` is not supported; expected instant, draft, fast, deep, or ready"
        )),
    }
}

fn parse_oracle_strength(field: &str, value: &str) -> Result<OracleStrength, String> {
    match value {
        "strong" => Ok(OracleStrength::Strong),
        "medium" => Ok(OracleStrength::Medium),
        "weak" => Ok(OracleStrength::Weak),
        "smoke" => Ok(OracleStrength::Smoke),
        "none" => Ok(OracleStrength::None),
        "unknown" => Ok(OracleStrength::Unknown),
        _ => Err(format!(
            "{field} `{value}` is not supported; expected strong, medium, weak, smoke, none, or unknown"
        )),
    }
}

fn parse_severity(field: &str, value: &str, allow_off: bool) -> Result<ConfigSeverity, String> {
    match value {
        "info" => Ok(ConfigSeverity::Info),
        "warning" => Ok(ConfigSeverity::Warning),
        "note" => Ok(ConfigSeverity::Note),
        "off" if allow_off => Ok(ConfigSeverity::Off),
        "off" => Err(format!(
            "{field} cannot be `off`; use suppressions for accepted debt"
        )),
        _ => Err(format!(
            "{field} `{value}` is not supported; expected info, warning, or note{}",
            if allow_off { ", or off" } else { "" }
        )),
    }
}

fn parse_relative_path(field: &str, value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if trimmed.contains('\\') {
        return Err(format!(
            "{field} `{value}` uses backslashes; use `/` separators"
        ));
    }
    if trimmed.contains(':') {
        return Err(format!(
            "{field} `{value}` uses a drive or scheme prefix; use a repository-relative path"
        ));
    }
    // `.` segments are normalized away rather than rejected: shared path
    // settings legitimately carry `./` prefixes, and canonical target
    // identity needs `./tests/a.rs` and `tests/a.rs` to be one path.
    // Normalization stays on the `/`-separated string form so the stored
    // path keeps its workspace-relative display on every host.
    if trimmed.starts_with('/') {
        return Err(format!(
            "{field} `{value}` uses a leading `/`; use a repository-relative path"
        ));
    }
    let normalized = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() {
        return Err(format!("{field} `{value}` must name a path"));
    }
    let path = PathBuf::from(&normalized);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!("{field} `{value}` must stay within the repository"));
    }
    Ok(path)
}

/// Parse and validate `[analysis.test_harnesses]` entries (#3532).
///
/// Fail-closed rules, each with a named error:
/// - every field is required (missing fields fail, not default);
/// - `target` must be a repository-relative path ([`parse_relative_path`]);
/// - `kind` and `adapter` must be known values (unknown versions fail
///   closed) and must match each other;
/// - `marker` must be an exact identifier path — no wildcards, globs,
///   prefixes, or attribute arguments;
/// - duplicate `registration_id`s or two registrations claiming the same
///   `target` are non-clean and named.
fn parse_test_harness_registrations(
    registrations: Vec<RawTestHarnessRegistration>,
) -> Result<Vec<TestHarnessRegistration>, String> {
    let mut parsed = Vec::with_capacity(registrations.len());
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut seen_targets = std::collections::BTreeSet::new();
    for (index, raw) in registrations.into_iter().enumerate() {
        let position = format!("analysis.test_harnesses[{index}]");
        let registration_id = required_text(&position, "registration_id", raw.registration_id)?;
        if !seen_ids.insert(registration_id.clone()) {
            return Err(format!(
                "{position}: registration_id `{registration_id}` is registered twice; conflicting registrations fail closed"
            ));
        }
        let target = parse_relative_path(
            &format!("{position}.target"),
            &required_text(&position, "target", raw.target)?,
        )?;
        if !seen_targets.insert(normalize_identity_path(&target)) {
            return Err(format!(
                "{position}: target `{}` is claimed by two registrations; conflicting registrations fail closed",
                target.to_string_lossy().replace('\\', "/")
            ));
        }
        let kind = TestHarnessKind::parse(&required_text(&position, "kind", raw.kind)?)?;
        let adapter =
            TestHarnessAdapter::parse(&required_text(&position, "adapter", raw.adapter)?)?;
        if !adapter.supports_kind(kind) {
            return Err(format!(
                "{position}: adapter `{}` does not support kind `{}`; the mismatch fails closed",
                adapter.as_str(),
                kind.as_str()
            ));
        }
        let marker = required_text(&position, "marker", raw.marker)?;
        if !marker_path_is_exact(&marker) {
            return Err(format!(
                "{position}: marker `{marker}` must be an exact identifier path (e.g. `libtest_mimic`, `myco::contract_test`) without wildcards, prefixes, or arguments"
            ));
        }
        parsed.push(TestHarnessRegistration {
            registration_id,
            target,
            kind,
            adapter,
            marker,
        });
    }
    Ok(parsed)
}

fn required_text(position: &str, field: &str, value: Option<String>) -> Result<String, String> {
    match value {
        Some(text) if !text.trim().is_empty() => Ok(text.trim().to_string()),
        Some(_) => Err(format!("{position}.{field} must not be empty")),
        None => Err(format!("{position}.{field} is required")),
    }
}

fn normalize_identity_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Exact marker paths only: `::`-separated Rust identifiers, no
/// wildcards, globs, whitespace, or attribute arguments. A lookalike
/// marker (`myco::contract_test*`, `myco::contract_test(`) never parses.
fn marker_path_is_exact(marker: &str) -> bool {
    !marker.is_empty()
        && marker.split("::").all(|segment| {
            let mut characters = segment.chars();
            matches!(characters.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
                && characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_')
        })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Tests assert an expected config diagnostic via `.expect_err(\"why\")`; the closure-style helper makes the expected failure mode part of the assertion message."
)]
mod tests;

/// The config a bound immutable-subject run uses (#3279 R4): the
/// candidate tree's own `ripr.toml` when the tree carries one, else the
/// default config. The worktree file (already loaded as `worktree`)
/// contributes nothing — `source_path`/`source_text` are cleared so the
/// recorded identity cannot claim the worktree file as its source.
pub(crate) fn config_for_candidate(
    subject: &crate::domain::GitCandidateSubject,
    worktree: &RiprConfig,
) -> Result<RiprConfig, String> {
    let bytes = crate::analysis::git_candidate_execution::candidate_config_bytes(
        subject,
        Some(std::time::Duration::from_secs(30)),
    )
    .map_err(|error| error.to_string())?;
    let Some(text) = bytes else {
        // Pure default: no worktree fact may enter a subject run
        // (#3279 review B1 — the worktree's enabled-languages list is
        // mutable state, and toggling it flipped subject completeness).
        // Binary capability is already inside the default config.
        let _ = worktree;
        return Ok(RiprConfig::default());
    };
    let mut config =
        parse_config(&text).map_err(|err| format!("candidate tree ripr.toml: {err}"))?;
    config.source_path = None;
    config.source_text = Some(text);
    Ok(config)
}

#[cfg(test)]
mod captured_snapshot_tests {
    use super::*;
    use crate::analysis::git_candidate_execution::CapturedConfiguration;
    use crate::domain::GitObjectId;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn with_snapshot_fixture(
        name: &str,
        run: impl FnOnce(&Path, &Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("fixture clock: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-captured-config-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&root).map_err(|error| format!("create fixture root: {error}"))?;
        let result = (|| {
            fs::create_dir(root.join(".git"))
                .map_err(|error| format!("create fixture boundary: {error}"))?;
            let snapshot = root.join("snapshot");
            fs::create_dir(&snapshot).map_err(|error| format!("create snapshot root: {error}"))?;
            run(&root, &snapshot)
        })();
        let cleanup = fs::remove_dir_all(&root)
            .map_err(|error| format!("remove fixture {}: {error}", root.display()));
        match (result, cleanup) {
            (Err(run_error), Err(cleanup_error)) => Err(format!("{run_error}; {cleanup_error}")),
            (result, cleanup) => result.and(cleanup),
        }
    }

    fn write_fixture(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        fs::write(path, text).map_err(|error| format!("write {}: {error}", path.display()))
    }

    fn present(text: &str) -> Result<CapturedConfiguration, String> {
        // This fixture exercises construction from a capture. Object authentication
        // belongs to prepare_named_tree and is not asserted by a synthetic object ID.
        let blob_oid = GitObjectId::parse("0123456789abcdef0123456789abcdef01234567")
            .map_err(|error| error.to_string())?;
        Ok(CapturedConfiguration::Present {
            blob_oid,
            text: text.to_string(),
        })
    }

    #[test]
    fn captured_snapshot_present_ignores_live_and_ancestor_configuration() -> Result<(), String> {
        with_snapshot_fixture("present-decoys", |root, snapshot| {
            write_fixture(&root.join(CONFIG_FILE_NAME), "not valid TOML")?;
            let live_text = "[analysis]\nmode = \"fast\"\n";
            write_fixture(&snapshot.join(CONFIG_FILE_NAME), live_text)?;
            assert_eq!(
                load_for_root(snapshot)?.analysis.mode,
                Some(Mode::Fast),
                "the live decoy must affect the ordinary loader"
            );
            let text =
                "# captured Δ\n[analysis]\nmode = \"deep\"\ninclude_unchanged_tests = false\n";
            let logical = Path::new("logical/repository/ripr.toml");
            let captured = present(text)?;
            let config = config_for_captured_snapshot(snapshot, logical, &captured)?;
            let mut expected = parse_config(text)?;
            expected.source_path = Some(logical.to_path_buf());
            expected.source_text = Some(text.to_string());
            assert_eq!(config, expected);
            assert_eq!(
                loaded_config_identity(&config),
                Some(config_fingerprint(text))
            );
            assert_eq!(
                fs::read_to_string(snapshot.join(CONFIG_FILE_NAME))
                    .map_err(|error| error.to_string())?,
                live_text
            );

            fs::remove_file(snapshot.join(CONFIG_FILE_NAME))
                .map_err(|error| format!("remove live decoy: {error}"))?;
            assert!(
                load_for_root(snapshot).is_err(),
                "the ancestor decoy must be invalid"
            );
            assert_eq!(
                config_for_captured_snapshot(snapshot, logical, &captured)?,
                expected
            );
            Ok(())
        })
    }

    #[test]
    fn captured_snapshot_absent_ignores_ancestor_policy_and_python_markers() -> Result<(), String> {
        with_snapshot_fixture("absent-ancestor", |root, snapshot| {
            write_fixture(
                &root.join(CONFIG_FILE_NAME),
                "[analysis]\nmode = \"ready\"\n",
            )?;
            write_fixture(
                &root.join("pyproject.toml"),
                "[project]\nname = \"ambient\"\n",
            )?;
            assert!(detect_python_project(root));
            assert!(!detect_python_project(snapshot));
            assert_eq!(
                load_for_root(snapshot)?.analysis.mode,
                Some(Mode::Ready),
                "the ordinary loader must see the ancestor policy"
            );
            let logical = root.join(CONFIG_FILE_NAME);
            let config =
                config_for_captured_snapshot(snapshot, &logical, &CapturedConfiguration::Absent)?;
            assert_eq!(config, RiprConfig::default());
            assert_eq!(loaded_config_identity(&config), None);
            write_fixture(&logical, "not valid TOML")?;
            load_for_root(snapshot)
                .err()
                .ok_or("the ordinary loader must reject the malformed ancestor policy")?;
            assert_eq!(
                config_for_captured_snapshot(snapshot, &logical, &CapturedConfiguration::Absent)?,
                config
            );
            Ok(())
        })
    }

    #[test]
    fn captured_snapshot_absent_preserves_snapshot_python_detection() -> Result<(), String> {
        for marker in ["pyproject.toml", "src/owned.py"] {
            with_snapshot_fixture("snapshot-python", |root, snapshot| {
                write_fixture(&root.join(CONFIG_FILE_NAME), "not valid TOML")?;
                write_fixture(&snapshot.join(marker), "owned = 1\n")?;
                assert!(
                    detect_python_project(snapshot),
                    "snapshot marker must be detected"
                );
                let result = config_for_captured_snapshot(
                    snapshot,
                    &root.join(CONFIG_FILE_NAME),
                    &CapturedConfiguration::Absent,
                );
                if LanguageId::Python.is_available() {
                    let mut expected = RiprConfig::default();
                    expected.languages.enabled.push(LanguageId::Python);
                    assert_eq!(result?, expected);
                } else {
                    let error = result.err().ok_or("unavailable Python was admitted")?;
                    assert!(error.starts_with("Python project markers were detected,"));
                    assert!(error.contains("without Cargo feature `lang-python`"));
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    #[test]
    fn captured_snapshot_empty_is_explicit_without_python_detection() -> Result<(), String> {
        with_snapshot_fixture("present-empty", |root, snapshot| {
            write_fixture(
                &snapshot.join("pyproject.toml"),
                "[project]\nname = \"snapshot\"\n",
            )?;
            write_fixture(&root.join(CONFIG_FILE_NAME), "not valid TOML")?;
            assert!(detect_python_project(snapshot));
            let logical = Path::new("logical/empty/ripr.toml");
            let config = config_for_captured_snapshot(snapshot, logical, &present("")?)?;
            let expected = RiprConfig {
                source_path: Some(logical.to_path_buf()),
                source_text: Some(String::new()),
                ..RiprConfig::default()
            };
            assert_eq!(config, expected);
            assert_eq!(config.source_text(), Some(""));
            assert_eq!(
                loaded_config_identity(&config),
                Some(config_fingerprint(""))
            );
            assert!(!config.languages.enabled.contains(&LanguageId::Python));
            Ok(())
        })
    }

    #[test]
    fn captured_snapshot_not_requested_refuses_live_config_and_defaults() -> Result<(), String> {
        with_snapshot_fixture("not-requested", |root, snapshot| {
            write_fixture(
                &snapshot.join("pyproject.toml"),
                "[project]\nname = \"snapshot\"\n",
            )?;
            write_fixture(
                &snapshot.join(CONFIG_FILE_NAME),
                "[analysis]\nmode = \"fast\"\n",
            )?;
            assert!(detect_python_project(snapshot));
            load_for_root(snapshot)?;
            let error = config_for_captured_snapshot(
                snapshot,
                &root.join(CONFIG_FILE_NAME),
                &CapturedConfiguration::NotRequested,
            )
            .err()
            .ok_or("unrequested capture was admitted")?;
            assert_eq!(error, "snapshot configuration capture was not requested");
            Ok(())
        })
    }

    #[test]
    fn captured_snapshot_malformed_text_preserves_logical_diagnostic() -> Result<(), String> {
        with_snapshot_fixture("malformed-capture", |root, snapshot| {
            write_fixture(
                &snapshot.join(CONFIG_FILE_NAME),
                "[analysis]\nmode = \"fast\"\n",
            )?;
            assert_eq!(load_for_root(snapshot)?.analysis.mode, Some(Mode::Fast));
            let text = "[analysis]\nmode = \"unknown-captured-mode\"\n";
            let logical = root.join("logical").join(CONFIG_FILE_NAME);
            let parse_error = parse_config(text)
                .err()
                .ok_or("malformed control was valid")?;
            let error = config_for_captured_snapshot(snapshot, &logical, &present(text)?)
                .err()
                .ok_or("malformed captured configuration was admitted")?;
            assert_eq!(error, format!("{}: {parse_error}", logical.display()));
            Ok(())
        })
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
mod live_configuration_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn limits() -> LiveConfigurationLimits {
        LiveConfigurationLimits {
            max_configuration_bytes: 64 * 1024,
            max_boundary_bytes: 64 * 1024,
            max_read_bytes: 256 * 1024,
            max_entries: 1024,
            max_path_bytes: 256 * 1024,
            max_buffered_bytes: 1024 * 1024,
        }
    }

    fn budget_with(limits: LiveConfigurationLimits) -> Result<LiveConfigurationBudget, String> {
        LiveConfigurationBudget::new(Instant::now() + Duration::from_secs(10), limits, 0)
    }

    fn bounded(root: &Path) -> Result<RiprConfig, String> {
        load_for_root_bounded(root, &mut budget_with(limits())?)
    }

    fn with_fixture(
        label: &str,
        run: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        struct FixtureCleanup<'a> {
            root: &'a Path,
            armed: bool,
        }

        impl Drop for FixtureCleanup<'_> {
            fn drop(&mut self) {
                if self.armed {
                    let _ = std::fs::remove_dir_all(self.root);
                }
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-live-config-{label}-{}-{stamp}",
            std::process::id(),
        ));
        std::fs::create_dir(&root).map_err(|error| error.to_string())?;
        let mut fixture_cleanup = FixtureCleanup {
            root: &root,
            armed: true,
        };
        let result = (|| {
            std::fs::create_dir(root.join(".git")).map_err(|error| error.to_string())?;
            run(&root)
        })();
        fixture_cleanup.armed = false;
        let cleanup = std::fs::remove_dir_all(&root).map_err(|error| error.to_string());
        match (result, cleanup) {
            (Err(first), Err(last)) => Err(format!("{first}; {last}")),
            (result, cleanup) => result.and(cleanup),
        }
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        std::fs::write(path, text).map_err(|error| error.to_string())
    }

    fn refuses<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
        match result {
            Err(error) if error.contains(expected) => Ok(()),
            Err(error) => Err(format!("expected {expected}, got {error}")),
            Ok(_) => Err(format!("expected refusal {expected}")),
        }
    }

    #[test]
    fn bounded_loader_preserves_direct_absent_empty_and_dot_root_configuration()
    -> Result<(), String> {
        with_fixture("direct", |root| {
            assert_eq!(bounded(root)?, load_for_root(root)?);
            let config_path = root.join(CONFIG_FILE_NAME);
            write(&config_path, "")?;
            assert_eq!(bounded(root)?, load_for_root(root)?);
            let text = "[analysis]\nmode=\"fast\"\n[reports]\nmax_related_tests=9\n";
            write(&config_path, text)?;
            let dot = root.join(".");
            let config = bounded(&dot)?;
            assert_eq!(config, load_for_root(&dot)?);
            assert_eq!(config.analysis.mode, Some(Mode::Fast));
            assert_eq!(config.reports.max_related_tests, 9);
            assert_eq!(
                config.source_path,
                Some(config_path.canonicalize().map_err(|e| e.to_string())?)
            );
            assert_eq!(config.source_text.as_deref(), Some(text));
            Ok(())
        })
    }

    #[test]
    fn bounded_loader_preserves_ancestor_and_workspace_boundary_order() -> Result<(), String> {
        with_fixture("ancestor", |root| {
            write(&root.join(CONFIG_FILE_NAME), "[analysis]\nmode=\"ready\"\n")?;
            let child = root.join("child");
            std::fs::create_dir(&child).map_err(|e| e.to_string())?;
            assert_eq!(bounded(&child)?, load_for_root(&child)?);
            assert_eq!(bounded(&child)?.analysis.mode, Some(Mode::Ready));
            write(&child.join("Cargo.toml"), "[workspace]\nmembers=[]\n")?;
            assert_eq!(bounded(&child)?, load_for_root(&child)?);
            assert_eq!(bounded(&child)?.analysis.mode, None);
            // Malformed TOML syntax is the same available non-workspace result.
            write(&child.join("Cargo.toml"), "workspace = [\n")?;
            assert_eq!(bounded(&child)?, load_for_root(&child)?);
            assert_eq!(bounded(&child)?.analysis.mode, Some(Mode::Ready));
            // A direct config precedes an existing repository boundary.
            std::fs::create_dir(child.join(".git")).map_err(|e| e.to_string())?;
            write(&child.join(CONFIG_FILE_NAME), "[analysis]\nmode=\"deep\"\n")?;
            assert_eq!(bounded(&child)?, load_for_root(&child)?);
            assert_eq!(bounded(&child)?.analysis.mode, Some(Mode::Deep));
            Ok(())
        })
    }

    #[test]
    fn bounded_loader_observes_finding_render_and_python_default_edits() -> Result<(), String> {
        with_fixture("drift", |root| {
            let config_path = root.join(CONFIG_FILE_NAME);
            write(
                &config_path,
                "[analysis]\nmode=\"draft\"\n[reports]\nmax_related_tests=5\n",
            )?;
            let initial = bounded(root)?;
            write(
                &config_path,
                "[analysis]\nmode=\"fast\"\n[reports]\nmax_related_tests=5\n",
            )?;
            assert_ne!(bounded(root)?, initial);
            write(
                &config_path,
                "[analysis]\nmode=\"draft\"\n[reports]\nmax_related_tests=17\n",
            )?;
            let rendered = bounded(root)?;
            assert_ne!(rendered, initial);
            assert_eq!(rendered, load_for_root(root)?);
            std::fs::remove_file(&config_path).map_err(|e| e.to_string())?;
            let defaults = bounded(root)?;
            let source = root.join("src");
            std::fs::create_dir(&source).map_err(|e| e.to_string())?;
            write(&source.join("generated_client.py"), "")?;
            assert_eq!(bounded(root)?, defaults);
            write(&source.join("real.py"), "def real():\n    return 1\n")?;
            if LanguageId::Python.is_available() {
                let python = bounded(root)?;
                assert_ne!(python, defaults);
                assert!(python.languages.enabled.contains(&LanguageId::Python));
                assert_eq!(python, load_for_root(root)?);
            } else {
                refuses(bounded(root), "lang-python")?;
                assert_eq!(bounded(root).err(), load_for_root(root).err());
            }
            std::fs::remove_file(source.join("real.py")).map_err(|e| e.to_string())?;
            assert_eq!(bounded(root)?, defaults);
            Ok(())
        })
    }

    #[test]
    fn bounded_loader_fails_inaccessible_present_entries_and_keeps_alias_provenance()
    -> Result<(), String> {
        use std::os::unix::fs::symlink;
        with_fixture("links", |root| {
            let config_path = root.join(CONFIG_FILE_NAME);
            let text = "[reports]\nmax_related_tests=7\n";
            write(&config_path, text)?;
            let ordinary_source = bounded(root)?;
            std::fs::remove_file(&config_path).map_err(|e| e.to_string())?;
            symlink(CONFIG_FILE_NAME, &config_path).map_err(|e| e.to_string())?;
            refuses(bounded(root), "resolve live configuration")?;
            refuses(load_for_root(root), "read ")?;
            std::fs::remove_file(&config_path).map_err(|e| e.to_string())?;
            let alias_target = root.join("same-text.toml");
            write(&alias_target, text)?;
            symlink(&alias_target, &config_path).map_err(|e| e.to_string())?;
            let alias = bounded(root)?;
            assert_eq!(alias, load_for_root(root)?);
            assert_eq!(alias.source_text, ordinary_source.source_text);
            assert_ne!(alias.source_path, ordinary_source.source_path);
            std::fs::remove_file(&config_path).map_err(|e| e.to_string())?;
            write(&config_path, text)?;
            assert_eq!(bounded(root)?, ordinary_source);
            Ok(())
        })
    }

    #[test]
    fn bounded_loader_refuses_caps_work_paths_expired_clock_and_reuse_then_recovers()
    -> Result<(), String> {
        with_fixture("refusal", |root| {
            let text = "[reports]\nmax_related_tests=7\n";
            write(&root.join(CONFIG_FILE_NAME), text)?;
            let expected = load_for_root(root)?;
            let mut small = limits();
            small.max_configuration_bytes = text.len() as u64;
            assert_eq!(
                load_for_root_bounded(root, &mut budget_with(small)?)?,
                expected
            );
            small.max_configuration_bytes -= 1;
            let mut refused = budget_with(small)?;
            refuses(load_for_root_bounded(root, &mut refused), "byte cap")?;
            refuses(load_for_root_bounded(root, &mut refused), "byte cap")?;
            let mut one_entry = limits();
            one_entry.max_entries = 1;
            refuses(
                load_for_root_bounded(root, &mut budget_with(one_entry)?),
                "entry bound",
            )?;
            let mut no_path = limits();
            no_path.max_path_bytes = 1;
            refuses(
                load_for_root_bounded(root, &mut budget_with(no_path)?),
                "path bound",
            )?;
            let mut no_read = limits();
            no_read.max_read_bytes = 1;
            let mut exhausted = budget_with(no_read)?;
            refuses(
                load_for_root_bounded(root, &mut exhausted),
                "aggregate read",
            )?;
            // Exactly one admitted byte plus the failed one-byte probe was
            // read and charged; a 64KiB overflow chunk is not permitted.
            assert_eq!(exhausted.read_bytes, 2);
            let original_deadline = Instant::now() + Duration::from_millis(20);
            let mut expired = LiveConfigurationBudget::new(original_deadline, limits(), 0)?;
            std::thread::sleep(Duration::from_millis(25));
            refuses(
                load_for_root_bounded(root, &mut expired),
                "original deadline",
            )?;
            let mut used = budget_with(limits())?;
            assert_eq!(load_for_root_bounded(root, &mut used)?, expected);
            refuses(load_for_root_bounded(root, &mut used), "already claimed")?;
            assert_eq!(bounded(root)?, expected);
            Ok(())
        })
    }
    #[test]
    fn strict_reader_refuses_final_symlink_before_read_and_nonregular_or_non_utf8_data()
    -> Result<(), String> {
        use std::os::unix::fs::symlink;
        with_fixture("strict-reader", |root| {
            let regular = root.join("regular.toml");
            let link = root.join("link.toml");
            write(&regular, "[reports]\nmax_related_tests=7\n")?;
            symlink(&regular, &link).map_err(|e| e.to_string())?;
            // Removing NOFOLLOW would read the target and reach the later
            // named-file identity refusal; this requires the earlier open error.
            refuses(
                budget_with(limits())?.regular_text(&link, 64 * 1024),
                "open live configuration",
            )?;
            refuses(
                budget_with(limits())?.regular_text(root, 64 * 1024),
                "not a regular file",
            )?;
            let binary = root.join("binary.toml");
            std::fs::write(&binary, [0xff_u8, 0xfe]).map_err(|e| e.to_string())?;
            refuses(
                budget_with(limits())?.regular_text(&binary, 64 * 1024),
                "failed",
            )?;
            assert_eq!(
                budget_with(limits())?.regular_text(&regular, 64 * 1024)?,
                "[reports]\nmax_related_tests=7\n"
            );
            Ok(())
        })
    }

    #[test]
    fn bounded_loader_claims_cancelled_attempt_and_preserves_first_fault_after_token_restoration()
    -> Result<(), String> {
        use crate::analysis::cancellation::{
            AnalysisAbortKind, AnalysisCancellationToken, with_token,
        };
        with_fixture("cancelled", |root| {
            write(
                &root.join(CONFIG_FILE_NAME),
                "[reports]\nmax_related_tests=11\n",
            )?;
            let token = AnalysisCancellationToken::new();
            token.cancel(AnalysisAbortKind::Cancelled);
            let mut attempt = budget_with(limits())?;
            let first = with_token(&token, || load_for_root_bounded(root, &mut attempt))
                .err()
                .ok_or("cancelled observation must refuse")?;
            assert!(first.contains("analysis cancelled"));
            let restored = load_for_root_bounded(root, &mut attempt)
                .err()
                .ok_or("restored token must not refresh a failed observation")?;
            assert_eq!(restored, first);
            assert_eq!(bounded(root)?, load_for_root(root)?);
            Ok(())
        })
    }
}
