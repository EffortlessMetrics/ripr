//! Python project auto-detection for configuration defaults.
//!
//! This module also owns the canonical Python discovery predicates
//! (#3672): the excluded-directory authority and the generated-name
//! authority. Project detection, diff workspace collection, repo
//! discovery, and repo role classification all consume these helpers, so
//! the four surfaces cannot drift apart.

use std::path::Path;

pub(crate) const PYTHON_PROJECT_MARKERS: &[&str] = &[
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "pytest.ini",
    "tox.ini",
    "noxfile.py",
];
pub(crate) const PYTHON_SOURCE_DIR_MARKERS: &[&str] = &["src", "tests"];

/// Directory components excluded from Python discovery, by family (#3672):
///
/// - repository and tooling state: `.git`, `target`, `node_modules`,
///   `.ripr`, `.direnv`;
/// - environment: `.venv`, `venv`, `env`, `site-packages`;
/// - cache: `__pycache__`, `.pytest_cache`, `.mypy_cache`;
/// - build output and test tooling: `.tox`, `.nox`, `dist`, `build`.
///
/// The `vendor` family ([`PYTHON_VENDOR_DIR`]) is intentionally not part of
/// this table: repo discovery must keep walking vendor trees, so the
/// vendor exclusion lives in [`is_python_excluded_dir_everywhere`] instead.
pub(crate) const PYTHON_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".ripr",
    ".direnv",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    ".tox",
    ".nox",
    "site-packages",
    ".pytest_cache",
    ".mypy_cache",
    "dist",
    "build",
];

/// The vendored third-party dependency directory family (#3672). Vendored
/// Python is not project or production source.
pub(crate) const PYTHON_VENDOR_DIR: &str = "vendor";

/// Whether a directory component is excluded from every Python discovery
/// surface (#3672): project detection, diff workspace collection, and repo
/// role classification.
///
/// This includes the [`PYTHON_VENDOR_DIR`] family: vendored Python is not
/// project or production source, so no surface treats it as either.
pub(crate) fn is_python_excluded_dir_everywhere(name: &str) -> bool {
    PYTHON_EXCLUDED_DIRS.contains(&name) || name == PYTHON_VENDOR_DIR
}

/// Whether a directory component is pruned from the Python repo discovery
/// walk (#3672).
///
/// Derived from the same [`PYTHON_EXCLUDED_DIRS`] authority as
/// [`is_python_excluded_dir_everywhere`], but deliberately does NOT prune
/// `vendor`: repo discovery keeps walking vendor trees so their files are
/// discovered and counted as typed excluded-role inputs
/// (`ExcludedEnvironment`) instead of silently vanishing from the run
/// accounting. Environment, cache, and build subtrees are pruned at
/// directory granularity and contribute no file count.
pub(crate) fn is_python_dir_pruned_from_repo_discovery(name: &str) -> bool {
    PYTHON_EXCLUDED_DIRS.contains(&name)
}

/// Path-level form of [`is_python_excluded_dir_everywhere`] (#3672): true
/// when ANY component of `path` is a member of [`PYTHON_EXCLUDED_DIRS`] or
/// equals [`PYTHON_VENDOR_DIR`].
///
/// Diff-mode production inputs exclude these subtrees entirely, consistent
/// with the workspace-collection exclusion: the diff workspace walk prunes
/// them at directory granularity, so a changed file under one of them can
/// never be backed by workspace facts. Diff analysis consults this authority
/// before counting a changed subject, the same way it consults the
/// generated-name authority — otherwise the report denominator counts a
/// file the adapter cannot inspect.
///
/// As with the generated-name path authority, a component that cannot be
/// read as UTF-8 is never compared against the UTF-8 table literals and is
/// therefore not excluded by this predicate.
pub(crate) fn is_detectable_excluded_python_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(is_python_excluded_dir_everywhere)
    })
}

pub(crate) fn detect_python_project(root: &Path) -> bool {
    PYTHON_PROJECT_MARKERS
        .iter()
        .any(|marker| root.join(marker).is_file())
        || PYTHON_SOURCE_DIR_MARKERS
            .iter()
            .any(|marker| dir_contains_python_source(&root.join(marker)))
}

/// Canonical root project-marker file name for a candidate name. Marker
/// files are resolved through the filesystem by the detector, so on Windows
/// the comparison is case-insensitive. `None` when the name is not a marker.
pub(crate) fn python_project_marker_name(name: &str) -> Option<&'static str> {
    canonical_marker_name(PYTHON_PROJECT_MARKERS, name)
}

/// Canonical root source-directory marker (`src` or `tests`) for a path
/// component, with the same platform rule as the marker files. The detector
/// resolves these directories through the filesystem, so detection state can
/// change when detectable Python source appears anywhere below them.
pub(crate) fn python_source_dir_marker_name(name: &str) -> Option<&'static str> {
    canonical_marker_name(PYTHON_SOURCE_DIR_MARKERS, name)
}

/// Whether a file name is a detectable Python source file name: a `.py`
/// extension that is not a generated-file name, exactly as the recursive
/// detector compares names. Presence classification only — content is never
/// read.
pub(crate) fn is_detectable_python_source_name(name: &str) -> bool {
    is_python_source_name(name) && !is_detectable_generated_python_name(name)
}

/// Whether a root source-directory marker contains detectable Python source,
/// exactly as `detect_python_project` consumes it. Presence-only: the bound
/// value is this boolean, never a per-file enumeration.
pub(crate) fn source_dir_contains_detectable_python(root: &Path, marker: &str) -> bool {
    dir_contains_python_source(&root.join(marker))
}

fn canonical_marker_name<'a>(candidates: &[&'a str], name: &str) -> Option<&'a str> {
    candidates.iter().copied().find(|candidate| {
        if cfg!(windows) {
            candidate.eq_ignore_ascii_case(name)
        } else {
            *candidate == name
        }
    })
}

fn dir_contains_python_source(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if is_python_excluded_dir_everywhere(name) {
                continue;
            }
            if dir_contains_python_source(&path) {
                return true;
            }
        } else if file_type.is_file() && is_detectable_python_source_path(&path, name) {
            return true;
        }
    }
    false
}

/// The `.py` check runs on the entry path's extension — `Path::extension` is
/// byte-based, so a non-UTF-8 stem still detects exactly as the pre-refactor
/// detector did — while the generated-name exclusion applies only to the
/// UTF-8 name (a non-UTF-8 name can never match a generated suffix).
fn is_detectable_python_source_path(path: &Path, utf8_name: &str) -> bool {
    let is_python_source = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "py");
    is_python_source && !is_detectable_generated_python_name(utf8_name)
}

fn is_python_source_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "py")
}

/// Whether a UTF-8 file name belongs to a generated Python family (#3672).
///
/// The families are matched exactly, in both directions:
///
/// - suffix `_pb2.py` (protobuf) and `_pb2_grpc.py` (gRPC);
/// - suffix `.generated.py` and `_generated.py`;
/// - the historical `generated_` PREFIX, so `generated_client.py` matches
///   regardless of what follows the prefix, while `generated.py`,
///   `regenerated_client.py`, and `pb2.py` do not match.
///
/// One implementation serves project detection, diff analysis, and repo role
/// classification; there is no second table.
pub(crate) fn is_detectable_generated_python_name(name: &str) -> bool {
    name.ends_with("_pb2.py")
        || name.ends_with("_pb2_grpc.py")
        || name.ends_with(".generated.py")
        || name.ends_with("_generated.py")
        || name.starts_with("generated_")
}

/// Path form of [`is_detectable_generated_python_name`].
///
/// A non-UTF-8 file name can never match a generated family and is not
/// generated, exactly as both historical implementations classified it: the
/// suffix and prefix families are UTF-8 literals, and a name that cannot be
/// read as UTF-8 is never compared against them by lossy substitution.
pub(crate) fn is_detectable_generated_python_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_detectable_generated_python_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_root(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-config-python-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn generated_name_families_and_near_misses_match_exactly() {
        for name in [
            "schema_pb2.py",
            "schema_pb2_grpc.py",
            "client.generated.py",
            "client_generated.py",
            "generated_client.py",
            // The historical prefix matches whatever follows it, even a
            // non-.py tail.
            "generated_client.txt",
        ] {
            assert!(
                is_detectable_generated_python_name(name),
                "{name} is a generated family"
            );
        }
        for name in [
            // No leading underscore on the protobuf suffix.
            "pb2.py",
            "pb2_grpc.py",
            // `.generated.py` needs the leading dot; `_generated.py` needs
            // the leading underscore.
            "generated.py",
            "mygenerated_thing.py",
            // The prefix is `generated_`, not a `generated` substring.
            "regenerated_client.py",
            // Suffixes must terminate the name.
            "client_generated.py.bak",
            "client.generated.py.orig",
        ] {
            assert!(
                !is_detectable_generated_python_name(name),
                "{name} is a near-miss, not a generated family"
            );
        }
    }

    #[test]
    fn path_authority_treats_non_utf8_names_as_not_generated() {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            // "gen" + unpaired surrogate + "d_pb2.py": not valid UTF-16
            // text, so `to_str` fails.
            let name = std::ffi::OsString::from_wide(&[
                0x0067, 0x0065, 0x006E, 0xD800, 0x0064, 0x005F, 0x0070, 0x0062, 0x0032, 0x002E,
                0x0070, 0x0079,
            ]);
            assert!(name.to_str().is_none(), "premise: the name is not UTF-8");
            assert!(!is_detectable_generated_python_path(Path::new(&name)));
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::OsStr::from_bytes(b"gen\xFFd_pb2.py");
            assert!(name.to_str().is_none(), "premise: the name is not UTF-8");
            assert!(!is_detectable_generated_python_path(Path::new(name)));
        }
    }

    #[test]
    fn excluded_dir_predicates_split_the_vendor_accounting() {
        for dir in PYTHON_EXCLUDED_DIRS {
            assert!(
                is_python_excluded_dir_everywhere(dir),
                "{dir} is excluded from every surface"
            );
            assert!(
                is_python_dir_pruned_from_repo_discovery(dir),
                "{dir} is pruned from the repo discovery walk"
            );
        }
        assert!(is_python_excluded_dir_everywhere(PYTHON_VENDOR_DIR));
        assert!(!is_python_dir_pruned_from_repo_discovery(PYTHON_VENDOR_DIR));
        // Ordinary project directories are neither.
        for dir in ["src", "tests", "pkg"] {
            assert!(!is_python_excluded_dir_everywhere(dir));
            assert!(!is_python_dir_pruned_from_repo_discovery(dir));
        }
    }

    #[test]
    fn excluded_path_authority_matches_any_path_component() {
        for path in [
            "vendor/dep.py",
            "src/vendor/dep.py",
            ".venv/x.py",
            "nested/.venv/x.py",
            "dist/pkg/mod.py",
            "build/obj/mod.py",
            "__pycache__/mod.py",
        ] {
            assert!(
                is_detectable_excluded_python_path(Path::new(path)),
                "{path} lies under an excluded component"
            );
        }
        for path in [
            // Ordinary project paths are not excluded.
            "src/mod.py",
            "tests/test_mod.py",
            // Near-misses are exact-component, not substring: `vendored`,
            // `environment`, and `envs` are not the `vendor` / `env`
            // families.
            "vendored/mod.py",
            "environment/mod.py",
            "src/envs/mod.py",
        ] {
            assert!(
                !is_detectable_excluded_python_path(Path::new(path)),
                "{path} has no excluded component"
            );
        }
    }

    #[test]
    fn vendor_python_does_not_enable_project_detection() -> Result<(), String> {
        let root = unique_test_root("vendor-detection");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let write = |rel: &str| -> Result<(), String> {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().ok_or("no parent")?)
                .map_err(|err| format!("create parent: {err}"))?;
            std::fs::write(&path, "VALUE = 1\n").map_err(|err| format!("write: {err}"))
        };
        write("vendor/dep.py")?;
        write("src/vendor/dep.py")?;
        assert!(
            !detect_python_project(&root),
            "vendored Python under src must not enable project detection"
        );
        write("src/app.py")?;
        assert!(
            detect_python_project(&root),
            "non-vendored source still enables detection (harness sanity)"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn excluded_directories_are_pruned_from_project_detection_traversal() -> Result<(), String> {
        let root = unique_test_root("detection-traversal");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let write = |rel: &str| -> Result<(), String> {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().ok_or("no parent")?)
                .map_err(|err| format!("create parent: {err}"))?;
            std::fs::write(&path, "VALUE = 1\n").map_err(|err| format!("write: {err}"))?;
            Ok(())
        };
        for dir in PYTHON_EXCLUDED_DIRS {
            write(&format!("src/{dir}/hidden.py"))?;
        }
        write("src/vendor/hidden.py")?;
        assert!(
            !detect_python_project(&root),
            "no excluded subtree may enable detection"
        );
        write("src/visible.py")?;
        assert!(detect_python_project(&root));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
