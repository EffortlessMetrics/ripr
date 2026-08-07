use crate::analysis::seam_cache::{
    CACHE_DIR_ENV, CacheStatus, cache_base_dir_from_env, inspect_cache_dir,
};
use crate::cli::suggest::unknown_argument;
use serde_json::json;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

const CACHE_STATUS_SCHEMA_VERSION: &str = "0.1";

const CACHE_USAGE: &str =
    "Usage:\n  ripr cache status [--json]\n  ripr cache clear [--dry-run] [--force]";
const STATUS_USAGE: &str = "Usage: ripr cache status [--json]";
const CLEAR_USAGE: &str = r#"Usage: ripr cache clear [--dry-run] [--force]

Removes the resolved cache directory (RIPR_CACHE_DIR when set, otherwise
target/ripr/cache under the Cargo workspace root.

  --dry-run   Report what would be removed and remove nothing.
  --force     Required to remove a cache directory that holds entries."#;

/// Directory names `ripr` itself creates under the cache base directory.
///
/// `clear` deletes files, so it only removes a directory it can recognize as
/// one of its own. This list mirrors the cache layers in
/// `analysis::seam_cache`; a layer added there but missed here only makes
/// `clear` more conservative for a relocated `RIPR_CACHE_DIR`, never less.
const CACHE_ROOT_MARKERS: &[&str] = &[
    "repo-seam-facts",
    "repo-seam-facts-sharded",
    "repo-compact-classified-seams",
    "repo-compact-classified-seams-sharded",
    "repo-corpus-fingerprint",
    "repo-file-facts",
    "repo-seam-counts",
];

/// Trailing path components of the default cache root
/// (`{workspace_root}/target/ripr/cache`).
const DEFAULT_CACHE_ROOT_SUFFIX: &[&str] = &["target", "ripr", "cache"];

/// Whether the resolved cache root exists on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CacheRoot {
    Missing,
    Present,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ClearOptions {
    dry_run: bool,
    force: bool,
}

fn parse_status_args(args: &[String]) -> Result<bool, String> {
    let mut is_json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => is_json = true,
            other => return Err(unknown_argument("cache status", other)),
        }
    }
    Ok(is_json)
}

fn cache_dir_for_root(
    workspace_root: &Path,
    env_value: Result<String, std::env::VarError>,
) -> PathBuf {
    cache_base_dir_from_env(workspace_root, env_value)
}

fn cache_dir_for_current_dir(
    current_dir: &Path,
    env_value: Result<String, std::env::VarError>,
) -> Result<PathBuf, String> {
    let explicit_cache_dir = matches!(&env_value, Ok(value) if !value.trim().is_empty());
    let workspace_root = if explicit_cache_dir {
        current_dir.to_path_buf()
    } else {
        let current_dir = std::fs::canonicalize(current_dir).map_err(|error| {
            format!(
                "resolve cache workspace root from {} failed: {error}",
                current_dir.display()
            )
        })?;
        super::check::resolve_workspace_root(&current_dir)?.unwrap_or(current_dir)
    };
    Ok(cache_dir_for_root(&workspace_root, env_value))
}

fn render_status(cache_dir: &Path, status: &CacheStatus, is_json: bool) -> Result<String, String> {
    let cache_dir_str = cache_dir.display().to_string();
    if is_json {
        serde_json::to_string_pretty(&json!({
            "schema_version": CACHE_STATUS_SCHEMA_VERSION,
            "cache_dir": cache_dir_str,
            "status": status.state,
            "total_size_bytes": status.total_size_bytes,
            "entry_count": status.entry_count
        }))
        .map_err(|error| error.to_string())
    } else {
        Ok(format!(
            "Cache dir: {cache_dir_str}\nStatus: {}\nTotal size: {} bytes\nEntries: {}",
            status.state, status.total_size_bytes, status.entry_count
        ))
    }
}

fn parse_clear_args(args: &[String]) -> Result<ClearOptions, String> {
    let mut options = ClearOptions::default();
    for arg in args {
        match arg.as_str() {
            "--dry-run" => options.dry_run = true,
            "--force" => options.force = true,
            other => return Err(unknown_argument("cache clear", other)),
        }
    }
    Ok(options)
}

fn normal_components(path: &Path) -> Vec<&OsStr> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .collect()
}

/// Reject any path `clear` must not delete before touching the filesystem.
///
/// The shape checks run first and unconditionally: an empty, relative, or
/// near-root path is refused without so much as a `stat`, so a misconfigured
/// `RIPR_CACHE_DIR` can never turn `clear` into a recursive delete of a home
/// directory or a filesystem root.
///
/// `..` and `.` components are rejected explicitly and before the
/// `normal_components`-based depth check. `normal_components` silently strips
/// `Component::ParentDir`, so without this guard a cache path suffixed with
/// `../../..` would pass both the depth rule and the suffix recognition while
/// `remove_dir_all` operates on the un-normalized path — deleting far outside
/// the cache (#2865 review P1).
fn classify_cache_root(cache_dir: &Path) -> Result<CacheRoot, String> {
    let display = cache_dir.display();
    if cache_dir.as_os_str().is_empty() {
        return Err(format!(
            "refusing to clear an empty cache path; set {CACHE_DIR_ENV} to the cache directory or unset it"
        ));
    }
    if !cache_dir.is_absolute() {
        return Err(format!(
            "refusing to clear relative cache path {display}; set {CACHE_DIR_ENV} to an absolute path"
        ));
    }
    // Reject `..` / `.` anywhere in the path. This runs before the depth rule
    // because `normal_components` strips these components and would hide a
    // traversal that escapes the intended cache root.
    if cache_dir
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(format!(
            "refusing to clear {display}: it contains a `..` or `.` component; \
             set {CACHE_DIR_ENV} to an absolute cache path without parent-directory traversal"
        ));
    }
    if normal_components(cache_dir).len() < 2 {
        return Err(format!(
            "refusing to clear {display}: a cache root must sit at least two directories below the filesystem root"
        ));
    }
    match std::fs::symlink_metadata(cache_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to clear {display}: it is a symlink, not a cache directory"
        )),
        Ok(metadata) if metadata.is_dir() => Ok(CacheRoot::Present),
        Ok(_) => Err(format!(
            "refusing to clear {display}: it is not a directory"
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CacheRoot::Missing),
        Err(error) => Err(format!("failed to inspect {display}: {error}")),
    }
}

/// Whether the directory is one `ripr` would have written: either the default
/// `target/ripr/cache` layout, or a relocated root that already holds a known
/// cache layer. Anything else is somebody's data, not a cache.
fn is_recognized_cache_root(cache_dir: &Path) -> bool {
    let components = normal_components(cache_dir);
    let default_layout = components.len() >= DEFAULT_CACHE_ROOT_SUFFIX.len()
        && components[components.len() - DEFAULT_CACHE_ROOT_SUFFIX.len()..]
            .iter()
            .zip(DEFAULT_CACHE_ROOT_SUFFIX)
            .all(|(actual, expected)| *actual == OsStr::new(expected));
    default_layout
        || CACHE_ROOT_MARKERS
            .iter()
            .any(|marker| cache_dir.join(marker).is_dir())
}

/// Describe what was counted, never presenting a partial traversal as a
/// complete one — `inspect_cache_dir` reports `partial` when it could not read
/// every entry, and those counts understate what removal will delete.
fn describe_entries(status: &CacheStatus) -> String {
    let counts = format!(
        "{} entries ({} bytes)",
        status.entry_count, status.total_size_bytes
    );
    if status.state == "partial" {
        format!("{counts}, counted partially")
    } else {
        counts
    }
}

/// Remove `cache_dir` when it is safe to do so, returning the line to report.
///
/// Takes the directory rather than resolving it so tests can exercise every
/// refusal path against a temporary directory.
fn clear_cache_dir(cache_dir: &Path, options: ClearOptions) -> Result<String, String> {
    let display = cache_dir.display();
    if classify_cache_root(cache_dir)? == CacheRoot::Missing {
        return Ok(format!("No cache directory at {display}; removed nothing."));
    }

    let status = inspect_cache_dir(cache_dir);
    clear_cache_status(cache_dir, &status, options)
}

fn clear_cache_status(
    cache_dir: &Path,
    status: &CacheStatus,
    options: ClearOptions,
) -> Result<String, String> {
    let display = cache_dir.display();
    if status.state != "ok" {
        return Err(format!(
            "refusing to clear {display}: cache inspection is {}; no files were removed",
            status.state
        ));
    }
    if status.entry_count == 0 {
        return Ok(format!(
            "Cache at {display} holds no entries; removed nothing."
        ));
    }
    if !is_recognized_cache_root(cache_dir) {
        return Err(format!(
            "refusing to clear {display}: it holds files but does not look like a ripr cache root \
             (expected a path ending in target/ripr/cache, or a ripr cache directory such as \
             `repo-seam-facts` inside it). Check {CACHE_DIR_ENV}."
        ));
    }

    let entries = describe_entries(status);
    if options.dry_run {
        return Ok(format!(
            "Dry run: would remove {entries} from {display}; removed nothing."
        ));
    }
    if !options.force {
        return Err(format!(
            "refusing to clear {display}: it holds {entries}. Re-run with `--force` to remove them, \
             or `--dry-run` to preview."
        ));
    }
    std::fs::remove_dir_all(cache_dir)
        .map_err(|error| format!("failed to remove {display}: {error}"))?;
    Ok(format!("Removed {entries} from {display}."))
}

fn run_status(args: &[String]) -> Result<(), String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("{STATUS_USAGE}");
        return Ok(());
    }

    let is_json = parse_status_args(args)?;
    let current_dir =
        std::env::current_dir().map_err(|e| format!("failed to get current dir: {}", e))?;
    let cache_dir = cache_dir_for_current_dir(&current_dir, std::env::var(CACHE_DIR_ENV))?;
    let status = inspect_cache_dir(&cache_dir);
    println!("{}", render_status(&cache_dir, &status, is_json)?);
    if !is_json {
        eprintln!(
            "To clean up the cache, use: cargo xtask cache gc [--dry-run] [--max-size-gb N] [--ttl-days N]"
        );
    }

    Ok(())
}

fn run_clear(args: &[String]) -> Result<(), String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("{CLEAR_USAGE}");
        return Ok(());
    }

    let options = parse_clear_args(args)?;
    let current_dir =
        std::env::current_dir().map_err(|e| format!("failed to get current dir: {}", e))?;
    let cache_dir = cache_dir_for_current_dir(&current_dir, std::env::var(CACHE_DIR_ENV))?;
    // Name the directory at risk before anything is removed, so an operator who
    // resolved the wrong root sees which path was chosen either way.
    println!("Cache dir: {}", cache_dir.display());
    println!("{}", clear_cache_dir(&cache_dir, options)?);

    Ok(())
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("{CACHE_USAGE}");
        return Ok(());
    }

    // This wording still names only `status`, and it is known to be incomplete:
    // `cli::execute` pins the exact string in
    // `execute_dispatches_subcommand_args_without_reparsing_argv`, which this
    // change is not scoped to touch. `ripr cache --help` and `ripr help --all`
    // both list `clear`. Widen both together in a follow-up.
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("cache requires subcommand `status` or `clear`".to_string());
    };

    match subcommand.as_str() {
        "status" => run_status(rest),
        "clear" => run_clear(rest),
        other => Err(format!(
            "unknown cache subcommand {other:?}; expected `status` or `clear`"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ripr-cache-{label}-{nonce}"))
    }

    #[test]
    fn missing_cache_is_reported_without_fabricated_counts() -> Result<(), String> {
        let path = temp_dir("missing");
        let status = inspect_cache_dir(&path);
        let expected = CacheStatus {
            state: "not_found",
            total_size_bytes: 0,
            entry_count: 0,
        };
        if status != expected {
            return Err(format!("expected {expected:?}, got {status:?}"));
        }
        Ok(())
    }

    #[test]
    fn nested_cache_files_are_counted_and_marked_ok() -> Result<(), String> {
        let root = temp_dir("nested");
        fs::create_dir_all(root.join("nested")).map_err(|error| error.to_string())?;
        fs::write(root.join("one"), b"123").map_err(|error| error.to_string())?;
        fs::write(root.join("nested").join("two"), b"4567").map_err(|error| error.to_string())?;

        let status = inspect_cache_dir(&root);
        let cleanup = fs::remove_dir_all(&root).map_err(|error| error.to_string());
        cleanup?;
        if status.state != "ok" {
            return Err(format!("expected ok status, got {:?}", status.state));
        }
        if status.total_size_bytes != 7 {
            return Err(format!("expected 7 bytes, got {}", status.total_size_bytes));
        }
        if status.entry_count != 2 {
            return Err(format!("expected 2 entries, got {}", status.entry_count));
        }
        Ok(())
    }

    #[test]
    fn cache_file_is_not_treated_as_a_directory() -> Result<(), String> {
        let path = temp_dir("file");
        fs::write(&path, b"not a cache directory").map_err(|error| error.to_string())?;
        let status = inspect_cache_dir(&path);
        let cleanup = fs::remove_file(&path).map_err(|error| error.to_string());
        cleanup?;
        if status.state != "unavailable" {
            return Err(format!(
                "expected unavailable status, got {:?}",
                status.state
            ));
        }
        Ok(())
    }

    #[test]
    fn unknown_status_arguments_fail_closed() -> Result<(), String> {
        let args = vec!["--jsoon".to_string()];
        if parse_status_args(&args).is_ok() {
            return Err("unknown cache status argument was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn relocated_cache_root_is_used_for_status() -> Result<(), String> {
        let workspace = temp_dir("workspace");
        let relocated = temp_dir("relocated");
        let resolved = cache_dir_for_root(&workspace, Ok(relocated.display().to_string()));
        if resolved != relocated {
            return Err(format!(
                "expected relocated cache root {relocated:?}, got {resolved:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn default_cache_root_uses_the_workspace_ancestor() -> Result<(), String> {
        let workspace = temp_dir("workspace-root");
        let nested = workspace.join("crates").join("member");
        fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        fs::write(
            workspace.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/member\"]\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            nested.join("Cargo.toml"),
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|error| error.to_string())?;

        let resolved = cache_dir_for_current_dir(&nested, Err(std::env::VarError::NotPresent))?;
        let expected = std::fs::canonicalize(&workspace)
            .map_err(|error| error.to_string())?
            .join("target")
            .join("ripr")
            .join("cache");
        remove_base(&workspace)?;

        if resolved != expected {
            return Err(format!(
                "nested workspace cache resolved to {resolved:?}, expected {expected:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn run_handles_help_without_workspace_io() -> Result<(), String> {
        run(&["--help".to_string()])
    }

    #[test]
    fn run_handles_short_help_without_workspace_io() -> Result<(), String> {
        run(&["-h".to_string()])
    }

    #[test]
    fn run_status_modes_are_successful() -> Result<(), String> {
        run(&["status".to_string()])?;
        run(&["status".to_string(), "--json".to_string()])
    }

    /// Build a default-layout cache root (`.../target/ripr/cache`) holding one
    /// entry, inside a temporary directory that no test may escape.
    fn populated_cache_root(label: &str) -> Result<(PathBuf, PathBuf), String> {
        let base = temp_dir(label);
        let cache_dir = base.join("target").join("ripr").join("cache");
        fs::create_dir_all(cache_dir.join("repo-seam-facts")).map_err(|error| error.to_string())?;
        fs::write(cache_dir.join("repo-seam-facts").join("entry.json"), b"{}")
            .map_err(|error| error.to_string())?;
        Ok((base, cache_dir))
    }

    fn remove_base(base: &Path) -> Result<(), String> {
        fs::remove_dir_all(base).map_err(|error| error.to_string())
    }

    #[test]
    fn clear_arguments_are_parsed_and_unknown_flags_fail_closed() -> Result<(), String> {
        let parsed = parse_clear_args(&["--dry-run".to_string(), "--force".to_string()])?;
        if parsed
            != (ClearOptions {
                dry_run: true,
                force: true,
            })
        {
            return Err(format!("unexpected clear options: {parsed:?}"));
        }
        if parse_clear_args(&["--forced".to_string()]).is_ok() {
            return Err("unknown cache clear argument was accepted".to_string());
        }
        Ok(())
    }

    /// The dominant risk of a delete command is deleting the wrong thing, so
    /// the shape refusals are checked before any filesystem state exists.
    #[test]
    fn clear_refuses_empty_relative_and_near_root_paths() -> Result<(), String> {
        for candidate in ["", "target/ripr/cache", "/", "/onlyone"] {
            if clear_cache_dir(Path::new(candidate), ClearOptions::default()).is_ok() {
                return Err(format!("clear accepted unsafe path {candidate:?}"));
            }
        }
        // An absolute path one level below the platform's own root, so the
        // depth rule is exercised on Windows too (where `/onlyone` is refused
        // as relative instead). `--force` is deliberately absent: a regression
        // in the depth rule must not be able to delete this.
        let mut shallow = PathBuf::new();
        for component in std::env::temp_dir().components() {
            let part = component.as_os_str();
            shallow.push(part);
            if matches!(component, Component::Normal(_)) {
                break;
            }
        }
        if normal_components(&shallow).len() == 1
            && clear_cache_dir(&shallow, ClearOptions::default()).is_ok()
        {
            return Err(format!("clear accepted near-root path {shallow:?}"));
        }
        Ok(())
    }

    /// A `RIPR_CACHE_DIR` containing `..` must be rejected before any filesystem
    /// access — `normal_components` strips `..`, so without the explicit guard
    /// a path like `<tmp>/target/ripr/cache/../../..` passes the depth rule and
    /// the suffix check while `remove_dir_all` deletes outside the cache (#2865).
    #[test]
    fn clear_refuses_parent_dir_traversal_in_cache_path() -> Result<(), String> {
        let base = temp_dir("traversal-bait");
        fs::create_dir_all(&base).map_err(|error| error.to_string())?;
        // Place a sentinel OUTSIDE the apparent cache root to prove nothing is
        // deleted even if `--force` is supplied.
        let bait = base.join("do-not-delete.txt");
        fs::write(&bait, b"sentinel").map_err(|error| error.to_string())?;
        let traversing = base
            .join("target")
            .join("ripr")
            .join("cache")
            .join("..")
            .join("..")
            .join("..");

        let refused = clear_cache_dir(
            &traversing,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let sentinel_survived = bait.is_file();
        let _ = fs::remove_dir_all(&base);

        match refused {
            Ok(message) => Err(format!("clear accepted a path with `..`: {message}")),
            Err(error) if !error.contains("`..`") => {
                Err(format!("refusal did not name the traversal: {error}"))
            }
            Err(_) if !sentinel_survived => {
                Err("clear deleted a file outside the cache root".to_string())
            }
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn clear_refuses_a_directory_that_is_not_a_cache_root() -> Result<(), String> {
        let base = temp_dir("not-a-cache");
        fs::create_dir_all(&base).map_err(|error| error.to_string())?;
        fs::write(base.join("notes.txt"), b"user data").map_err(|error| error.to_string())?;

        let refused = clear_cache_dir(
            &base,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let still_present = base.join("notes.txt").is_file();
        remove_base(&base)?;

        match refused {
            Ok(message) => Err(format!("clear removed a non-cache directory: {message}")),
            Err(error) if !error.contains("does not look like a ripr cache root") => {
                Err(format!("unexpected refusal message: {error}"))
            }
            Err(_) if !still_present => Err("clear deleted a non-cache file".to_string()),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn clear_refuses_a_non_empty_cache_root_without_force() -> Result<(), String> {
        let (base, cache_dir) = populated_cache_root("needs-force")?;
        let refused = clear_cache_dir(&cache_dir, ClearOptions::default());
        let still_present = cache_dir.is_dir();
        remove_base(&base)?;

        match refused {
            Ok(message) => Err(format!(
                "clear removed the cache without --force: {message}"
            )),
            Err(error) if !error.contains("--force") => {
                Err(format!("refusal did not name --force: {error}"))
            }
            Err(error) if !error.contains("target") => {
                Err(format!("refusal did not name the resolved path: {error}"))
            }
            Err(_) if !still_present => Err("clear deleted the cache anyway".to_string()),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn clear_dry_run_previews_without_removing() -> Result<(), String> {
        let (base, cache_dir) = populated_cache_root("dry-run")?;
        let reported = clear_cache_dir(
            &cache_dir,
            ClearOptions {
                dry_run: true,
                force: true,
            },
        );
        let still_present = cache_dir.is_dir();
        remove_base(&base)?;

        let message = reported?;
        if !still_present {
            return Err("dry run removed the cache".to_string());
        }
        if !message.contains("1 entries") || !message.contains("removed nothing") {
            return Err(format!("dry run did not preview the removal: {message}"));
        }
        Ok(())
    }

    #[test]
    fn clear_with_force_removes_the_resolved_cache_root() -> Result<(), String> {
        let (base, cache_dir) = populated_cache_root("forced")?;
        let reported = clear_cache_dir(
            &cache_dir,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let removed = !cache_dir.exists();
        remove_base(&base)?;

        let message = reported?;
        if !removed {
            return Err(format!(
                "cache root remained after a forced clear: {message}"
            ));
        }
        if !message.contains("Removed 1 entries") {
            return Err(format!("clear did not report what it removed: {message}"));
        }
        Ok(())
    }

    /// A relocated `RIPR_CACHE_DIR` is recognized by the cache layers it holds,
    /// not by its name.
    #[test]
    fn clear_removes_a_relocated_root_recognized_by_its_cache_layers() -> Result<(), String> {
        let base = temp_dir("relocated-clear");
        let cache_dir = base.join("elsewhere");
        fs::create_dir_all(cache_dir.join("repo-file-facts")).map_err(|error| error.to_string())?;
        fs::write(cache_dir.join("repo-file-facts").join("f.json"), b"{}")
            .map_err(|error| error.to_string())?;

        let reported = clear_cache_dir(
            &cache_dir,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let removed = !cache_dir.exists();
        remove_base(&base)?;

        reported?;
        if !removed {
            return Err("relocated cache root remained after a forced clear".to_string());
        }
        Ok(())
    }

    #[test]
    fn clear_reports_missing_and_empty_cache_roots_without_removing() -> Result<(), String> {
        let missing = temp_dir("missing-clear");
        let missing_message = clear_cache_dir(&missing, ClearOptions::default())?;
        if !missing_message.contains("removed nothing") {
            return Err(format!(
                "unexpected missing-cache report: {missing_message}"
            ));
        }

        let empty = temp_dir("empty-clear");
        fs::create_dir_all(&empty).map_err(|error| error.to_string())?;
        let empty_message = clear_cache_dir(&empty, ClearOptions::default());
        let still_present = empty.is_dir();
        remove_base(&empty)?;

        let empty_message = empty_message?;
        if !empty_message.contains("holds no entries") {
            return Err(format!("unexpected empty-cache report: {empty_message}"));
        }
        if !still_present {
            return Err("clear removed an empty directory it did not recognize".to_string());
        }
        Ok(())
    }

    #[test]
    fn clear_refuses_unavailable_or_partial_inspection() -> Result<(), String> {
        let cache_dir = temp_dir("unavailable-clear");
        for state in ["unavailable", "partial"] {
            let status = CacheStatus {
                state,
                total_size_bytes: 0,
                entry_count: 0,
            };
            let result = clear_cache_status(
                &cache_dir,
                &status,
                ClearOptions {
                    dry_run: false,
                    force: true,
                },
            );
            let error = result
                .err()
                .ok_or_else(|| format!("{state} inspection was reported as clearable"))?;
            if !error.contains(state) || !error.contains("no files were removed") {
                return Err(format!(
                    "{state} inspection error was not fail-closed: {error}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn run_rejects_missing_and_unknown_subcommands() -> Result<(), String> {
        let missing =
            run(&[]).map_err(|error| format!("missing subcommand unexpectedly passed: {error}"));
        let missing = missing
            .err()
            .ok_or_else(|| "missing cache subcommand unexpectedly passed".to_string())?;
        if !missing.contains("status` or `clear") {
            return Err(format!(
                "missing cache error omitted accepted subcommands: {missing}"
            ));
        }
        let unknown = run(&["show".to_string()]);
        if unknown.is_ok() {
            return Err("unknown cache subcommand unexpectedly passed".to_string());
        }
        // `clear` is in the accepted set now; `--help` proves the routing
        // without letting a unit test resolve and delete a real cache root.
        run(&["clear".to_string(), "--help".to_string()])?;
        if run(&["clear".to_string(), "--forced".to_string()]).is_ok() {
            return Err("unknown cache clear argument unexpectedly passed".to_string());
        }
        if run(&[
            "status".to_string(),
            "--jsoon".to_string(),
            "--help".to_string(),
        ])
        .is_ok()
        {
            return Err("unknown status argument was hidden by help".to_string());
        }
        Ok(())
    }

    #[test]
    fn render_status_has_stable_human_and_json_shapes() -> Result<(), String> {
        let cache_dir = Path::new("target/ripr/cache");
        let status = CacheStatus {
            state: "partial",
            total_size_bytes: 12,
            entry_count: 2,
        };
        let human = render_status(cache_dir, &status, false)?;
        for expected in ["Status: partial", "Total size: 12 bytes", "Entries: 2"] {
            if !human.contains(expected) {
                return Err(format!("human output omitted `{expected}`: {human}"));
            }
        }
        let json = render_status(cache_dir, &status, true)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if value.get("status").and_then(serde_json::Value::as_str) != Some("partial") {
            return Err(format!("JSON output omitted status: {json}"));
        }
        if value.get("entry_count").and_then(serde_json::Value::as_u64) != Some(2) {
            return Err(format!("JSON output omitted entry count: {json}"));
        }
        if value
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            != Some(CACHE_STATUS_SCHEMA_VERSION)
        {
            return Err(format!("JSON output omitted schema version: {json}"));
        }
        Ok(())
    }
}
