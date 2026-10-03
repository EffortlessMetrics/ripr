use crate::analysis::seam_cache::{
    CACHE_DIR_ENV, CACHE_LAYER_NAMES, CacheStatus, cache_base_dir_from_env, inspect_cache_dir,
};
use crate::cli::suggest::unknown_argument;
use serde_json::json;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

const CACHE_STATUS_SCHEMA_VERSION: &str = "0.1";

const CACHE_USAGE: &str = "Inspect or clear the disposable analysis cache.\n\nUsage:\n  ripr cache status [--json]\n  ripr cache clear [--dry-run] [--force]";
/// Help body for `ripr cache status`. Also the flag source for unknown-argument
/// suggestions, so `--json` has to appear as an option-list line, not only
/// inside the usage brackets.
pub(in crate::cli) const CACHE_STATUS_HELP: &str = r#"Usage: ripr cache status [--json]

Report the resolved analysis cache directory (RIPR_CACHE_DIR when set,
otherwise target/ripr/cache under the Cargo workspace root).

  --json    Print machine-readable status JSON. Its `schema_version` is the
            version of this status report, not of the on-disk cache layers
            (those carry their own version in their directory names).

The cache is disposable: deleting it only costs a slower next run. Use
`ripr cache clear` to remove it.
"#;
/// Help body for `ripr cache clear`. Also the flag source for unknown-argument
/// suggestions; keep accepted flags on option-list lines.
pub(in crate::cli) const CACHE_CLEAR_HELP: &str = r#"Usage: ripr cache clear [--dry-run] [--force]

Removes only recognized ripr cache layers under the resolved cache directory
(RIPR_CACHE_DIR when set, otherwise target/ripr/cache under the Cargo workspace
root). The cache root and unrelated sibling files or directories are preserved.

  --dry-run   Report what would be removed and remove nothing.
  --force     Required to remove cache layers that hold entries.
"#;

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheLayerPlan {
    name: &'static str,
    path: PathBuf,
    total_size_bytes: u64,
    entry_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ClearPlan {
    layers: Vec<CacheLayerPlan>,
    total_size_bytes: u64,
    entry_count: usize,
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
        super::check::resolve_project_root(&current_dir)
            .map_err(|error| {
                format!("{error}; set RIPR_CACHE_DIR to select the cache directory explicitly")
            })?
            .map_or(current_dir, |(root, _reason)| root)
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

fn reject_symlinked_ancestor(cache_dir: &Path) -> Result<(), String> {
    for ancestor in cache_dir.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing to clear {}: path component {} is a symlink; no files were removed",
                    cache_dir.display(),
                    ancestor.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect cache path component {}: {error}; no files were removed",
                    ancestor.display()
                ));
            }
        }
    }
    Ok(())
}

/// Reject any path `clear` must not delete before touching cache contents.
///
/// Empty, relative, traversing, near-root, non-directory, leaf-symlink, and
/// ancestor-symlink paths fail closed. `--force` never widens this boundary.
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
    if cache_dir
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
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
        Ok(metadata) if metadata.is_dir() => {
            reject_symlinked_ancestor(cache_dir)?;
            Ok(CacheRoot::Present)
        }
        Ok(_) => Err(format!(
            "refusing to clear {display}: it is not a directory"
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CacheRoot::Missing),
        Err(error) => Err(format!("failed to inspect {display}: {error}")),
    }
}

fn layer_from_status(
    cache_dir: &Path,
    name: &'static str,
    path: PathBuf,
    status: CacheStatus,
) -> Result<CacheLayerPlan, String> {
    if status.state != "ok" {
        return Err(format!(
            "refusing to clear {}: cache layer `{name}` inspection is {}; no files were removed",
            cache_dir.display(),
            status.state
        ));
    }
    Ok(CacheLayerPlan {
        name,
        path,
        total_size_bytes: status.total_size_bytes,
        entry_count: status.entry_count,
    })
}

fn build_clear_plan(cache_dir: &Path) -> Result<ClearPlan, String> {
    let mut plan = ClearPlan::default();
    for &marker in CACHE_LAYER_NAMES {
        let layer_path = cache_dir.join(marker);
        match std::fs::symlink_metadata(&layer_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing to clear {}: recognized cache layer `{marker}` is a symlink; no files were removed",
                    cache_dir.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "refusing to clear {}: recognized cache layer `{marker}` is not a directory; no files were removed",
                    cache_dir.display()
                ));
            }
            Ok(_) => {
                reject_symlinked_ancestor(&layer_path)?;
                let status = inspect_cache_dir(&layer_path);
                let layer = layer_from_status(cache_dir, marker, layer_path, status)?;
                plan.total_size_bytes =
                    plan.total_size_bytes.saturating_add(layer.total_size_bytes);
                plan.entry_count = plan.entry_count.saturating_add(layer.entry_count);
                plan.layers.push(layer);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect cache layer {}: {error}; no files were removed",
                    layer_path.display()
                ));
            }
        }
    }
    Ok(plan)
}

fn root_has_entries(cache_dir: &Path) -> Result<bool, String> {
    let mut entries = std::fs::read_dir(cache_dir).map_err(|error| {
        format!(
            "failed to inspect cache root {}: {error}; no files were removed",
            cache_dir.display()
        )
    })?;
    entries
        .next()
        .transpose()
        .map(|entry| entry.is_some())
        .map_err(|error| {
            format!(
                "failed to inspect cache root {}: {error}; no files were removed",
                cache_dir.display()
            )
        })
}

fn describe_entries(entry_count: usize, total_size_bytes: u64) -> String {
    format!("{entry_count} entries ({total_size_bytes} bytes)")
}

fn describe_layers(plan: &ClearPlan) -> String {
    plan.layers
        .iter()
        .map(|layer| format!("`{}`", layer.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn layer_count_label(count: usize) -> String {
    if count == 1 {
        "1 ripr cache layer".to_string()
    } else {
        format!("{count} ripr cache layers")
    }
}

fn verify_layer_before_delete(cache_dir: &Path, layer: &CacheLayerPlan) -> Result<(), String> {
    match std::fs::symlink_metadata(&layer.path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to clear {}: cache layer `{}` became a symlink; no further files were removed",
            cache_dir.display(),
            layer.name
        )),
        Ok(metadata) if metadata.is_dir() => reject_symlinked_ancestor(&layer.path),
        Ok(_) => Err(format!(
            "refusing to clear {}: cache layer `{}` is no longer a directory; no further files were removed",
            cache_dir.display(),
            layer.name
        )),
        Err(error) => Err(format!(
            "refusing to clear {}: cache layer `{}` changed after inspection ({error}); no further files were removed",
            cache_dir.display(),
            layer.name
        )),
    }
}

fn remove_owned_layers(cache_dir: &Path, plan: &ClearPlan) -> Result<(), String> {
    for layer in &plan.layers {
        verify_layer_before_delete(cache_dir, layer)?;
    }
    for layer in &plan.layers {
        std::fs::remove_dir_all(&layer.path).map_err(|error| {
            format!(
                "failed to remove ripr cache layer {}: {error}; the cache root and unrelated siblings were preserved",
                layer.path.display()
            )
        })?;
    }
    Ok(())
}

/// Remove only independently recognized cache-layer children of `cache_dir`.
/// The configured parent is never recursively deleted, even when it ends in
/// `target/ripr/cache` or holds one valid layer.
fn clear_cache_dir(cache_dir: &Path, options: ClearOptions) -> Result<String, String> {
    let display = cache_dir.display();
    if classify_cache_root(cache_dir)? == CacheRoot::Missing {
        return Ok(format!("No cache directory at {display}; removed nothing."));
    }

    let plan = build_clear_plan(cache_dir)?;
    if plan.layers.is_empty() {
        if root_has_entries(cache_dir)? {
            return Err(format!(
                "refusing to clear {display}: it contains no independently recognized ripr cache layers; \
                 the cache root and all files were preserved. Check {CACHE_DIR_ENV}."
            ));
        }
        return Ok(format!(
            "Cache at {display} holds no entries; removed nothing."
        ));
    }
    if plan.entry_count == 0 {
        return Ok(format!(
            "Recognized cache layers at {display} hold no entries; removed nothing."
        ));
    }

    let entries = describe_entries(plan.entry_count, plan.total_size_bytes);
    let layers = describe_layers(&plan);
    let layer_count = layer_count_label(plan.layers.len());
    if options.dry_run {
        return Ok(format!(
            "Dry run: would remove {entries} from {layer_count} under {display} ({layers}); \
             the cache root and unrelated siblings would remain; removed nothing."
        ));
    }
    if !options.force {
        return Err(format!(
            "refusing to clear {display}: {layer_count} ({layers}) hold {entries}. \
             Re-run with `--force` to remove only those layers, or `--dry-run` to preview."
        ));
    }

    remove_owned_layers(cache_dir, &plan)?;
    Ok(format!(
        "Removed {entries} from {layer_count} under {display} ({layers}). \
         Preserved the cache root and unrelated siblings."
    ))
}

fn run_status(args: &[String]) -> Result<(), String> {
    // Help is positional-free like every sibling command family (#5024):
    // `ripr cache status --json --help` prints this help and exits 0.
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{CACHE_STATUS_HELP}");
        return Ok(());
    }

    let is_json = parse_status_args(args)?;
    let current_dir =
        std::env::current_dir().map_err(|error| format!("failed to get current dir: {error}"))?;
    let cache_dir = cache_dir_for_current_dir(&current_dir, std::env::var(CACHE_DIR_ENV))?;
    let status = inspect_cache_dir(&cache_dir);
    println!("{}", render_status(&cache_dir, &status, is_json)?);
    if !is_json {
        eprintln!("{CLEANUP_HINT}");
    }

    Ok(())
}

/// Names the published cleanup command (#4383). `ripr cache clear` resolves
/// the same directory from the same working directory and `RIPR_CACHE_DIR`,
/// so the hint needs no path to retype; `cargo xtask cache gc` is repository
/// automation that users of the published crate do not have.
const CLEANUP_HINT: &str = "The cache is disposable; ripr rebuilds it on the next run. To clear it, run `ripr cache clear --dry-run` to preview, then `ripr cache clear --force`.";

fn run_clear(args: &[String]) -> Result<(), String> {
    // Help is positional-free like every sibling command family (#5024):
    // `ripr cache clear --dry-run --help` prints this help and exits 0.
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{CACHE_CLEAR_HELP}");
        return Ok(());
    }

    let options = parse_clear_args(args)?;
    let current_dir =
        std::env::current_dir().map_err(|error| format!("failed to get current dir: {error}"))?;
    let cache_dir = cache_dir_for_current_dir(&current_dir, std::env::var(CACHE_DIR_ENV))?;
    println!("Cache dir: {}", cache_dir.display());
    println!("{}", clear_cache_dir(&cache_dir, options)?);

    Ok(())
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    // `ripr cache --help <anything>` prints the family usage (help wins,
    // matching siblings); subcommand-level argv keeps its own help handling
    // in `run_status`/`run_clear` (#5024).
    if matches!(args.first(), Some(arg) if arg == "--help" || arg == "-h") {
        println!("{CACHE_USAGE}");
        return Ok(());
    }

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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ripr-cache-{label}-{}-{nonce}-{sequence}",
            std::process::id()
        ))
    }

    fn remove_base(base: &Path) -> Result<(), String> {
        if base.exists() {
            fs::remove_dir_all(base).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn populated_cache_root(label: &str) -> Result<(PathBuf, PathBuf), String> {
        let base = temp_dir(label);
        let cache_dir = base.join("target").join("ripr").join("cache");
        fs::create_dir_all(cache_dir.join("repo-seam-facts")).map_err(|error| error.to_string())?;
        fs::write(cache_dir.join("repo-seam-facts").join("entry.json"), b"{}")
            .map_err(|error| error.to_string())?;
        Ok((base, cache_dir))
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
    fn status_and_argument_contracts_remain_stable() -> Result<(), String> {
        let status = CacheStatus {
            state: "partial",
            total_size_bytes: 12,
            entry_count: 2,
        };
        let cache_dir = Path::new("target/ripr/cache");
        let human = render_status(cache_dir, &status, false)?;
        for expected in ["Status: partial", "Total size: 12 bytes", "Entries: 2"] {
            if !human.contains(expected) {
                return Err(format!("human output omitted `{expected}`: {human}"));
            }
        }
        let json = render_status(cache_dir, &status, true)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if value.get("status").and_then(serde_json::Value::as_str) != Some("partial")
            || value.get("entry_count").and_then(serde_json::Value::as_u64) != Some(2)
        {
            return Err(format!("JSON status output drifted: {json}"));
        }
        if parse_status_args(&["--jsonn".to_string()])
            .err()
            .is_none_or(|error| !error.contains("Did you mean `--json`?"))
        {
            return Err("cache status typo did not fail with the expected suggestion".to_string());
        }
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

    #[test]
    fn relocated_and_default_cache_resolution_remain_supported() -> Result<(), String> {
        let workspace = temp_dir("workspace");
        let relocated = temp_dir("relocated");
        let resolved = cache_dir_for_root(&workspace, Ok(relocated.display().to_string()));
        if resolved != relocated {
            return Err(format!(
                "expected relocated cache root {relocated:?}, got {resolved:?}"
            ));
        }

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
        let default = cache_dir_for_current_dir(&nested, Err(std::env::VarError::NotPresent))?;
        let expected = fs::canonicalize(&workspace)
            .map_err(|error| error.to_string())?
            .join("target")
            .join("ripr")
            .join("cache");
        remove_base(&workspace)?;
        if default != expected {
            return Err(format!(
                "nested workspace cache resolved to {default:?}, expected {expected:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn clear_refuses_empty_relative_near_root_and_traversing_paths() -> Result<(), String> {
        for candidate in ["", "target/ripr/cache", "/", "/onlyone"] {
            if clear_cache_dir(Path::new(candidate), ClearOptions::default()).is_ok() {
                return Err(format!("clear accepted unsafe path {candidate:?}"));
            }
        }

        let base = temp_dir("traversal-bait");
        fs::create_dir_all(&base).map_err(|error| error.to_string())?;
        let sentinel = base.join("do-not-delete.txt");
        fs::write(&sentinel, b"sentinel").map_err(|error| error.to_string())?;
        let traversing = base
            .join("target")
            .join("ripr")
            .join("cache")
            .join("..")
            .join("..")
            .join("..");
        let result = clear_cache_dir(
            &traversing,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let persisted = sentinel.is_file();
        remove_base(&base)?;
        if result.is_ok() || !persisted {
            return Err("traversing cache path was accepted or deleted its sentinel".to_string());
        }
        Ok(())
    }

    #[test]
    fn clear_refuses_unknown_only_root() -> Result<(), String> {
        let root = temp_dir("not-a-cache");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let sentinel = root.join("notes.txt");
        fs::write(&sentinel, b"user data").map_err(|error| error.to_string())?;
        let result = clear_cache_dir(
            &root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let persisted = sentinel.is_file();
        remove_base(&root)?;
        match result {
            Ok(message) => Err(format!("clear accepted an unknown-only root: {message}")),
            Err(error) if !error.contains("no independently recognized") => {
                Err(format!("unexpected refusal message: {error}"))
            }
            Err(_) if !persisted => Err("clear deleted an unrelated file".to_string()),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn clear_requires_force_and_dry_run_matches_the_owned_plan() -> Result<(), String> {
        let (base, cache_dir) = populated_cache_root("confirmation")?;
        let refused = clear_cache_dir(&cache_dir, ClearOptions::default());
        if refused
            .err()
            .is_none_or(|error| !error.contains("--force") || !error.contains("repo-seam-facts"))
        {
            remove_base(&base)?;
            return Err("non-empty cache layer did not require --force".to_string());
        }
        let dry_run = clear_cache_dir(
            &cache_dir,
            ClearOptions {
                dry_run: true,
                force: true,
            },
        )?;
        let still_present = cache_dir.join("repo-seam-facts").is_dir();
        remove_base(&base)?;
        if !still_present
            || !dry_run.contains("1 entries")
            || !dry_run.contains("repo-seam-facts")
            || !dry_run.contains("unrelated siblings would remain")
        {
            return Err(format!(
                "dry-run plan was not exact and non-destructive: {dry_run}"
            ));
        }
        Ok(())
    }

    #[test]
    fn clear_removes_only_owned_layers_and_preserves_mixed_root() -> Result<(), String> {
        let root = temp_dir("mixed-root");
        let layer = root.join("repo-file-facts");
        fs::create_dir_all(&layer).map_err(|error| error.to_string())?;
        fs::write(layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
        let sentinel = root.join("KEEP.txt");
        fs::write(&sentinel, b"user data").map_err(|error| error.to_string())?;

        let result = clear_cache_dir(
            &root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        )?;
        let layer_removed = !layer.exists();
        let sentinel_survived = sentinel.is_file();
        let root_survived = root.is_dir();
        remove_base(&root)?;

        if !layer_removed || !sentinel_survived || !root_survived {
            return Err("clear did not confine deletion to the owned cache layer".to_string());
        }
        if !result.contains("Preserved the cache root and unrelated siblings") {
            return Err(format!(
                "clear did not disclose its bounded scope: {result}"
            ));
        }
        Ok(())
    }

    #[test]
    fn clear_derives_every_producer_layer_and_preserves_unrelated_data() -> Result<(), String> {
        let root = temp_dir("all-layers");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        for &layer in CACHE_LAYER_NAMES {
            let path = root.join(layer);
            fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            fs::write(path.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
        }
        let unrelated = root.join("KEEP.txt");
        fs::write(&unrelated, b"user data").map_err(|error| error.to_string())?;

        let planned = build_clear_plan(&root)?.layers.len();
        clear_cache_dir(
            &root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        )?;
        let intact = root.is_dir()
            && unrelated.is_file()
            && CACHE_LAYER_NAMES
                .iter()
                .all(|layer| !root.join(layer).exists());
        remove_base(&root)?;
        if planned != CACHE_LAYER_NAMES.len() {
            return Err(format!("cache plan omitted a producer layer: {planned}"));
        }
        if !intact {
            return Err("clear failed the producer-layer or unrelated-sibling boundary".into());
        }
        Ok(())
    }

    #[test]
    fn default_suffix_does_not_bless_unrelated_siblings() -> Result<(), String> {
        let base = temp_dir("default-mixed");
        let root = base.join("target").join("ripr").join("cache");
        let layer = root.join("repo-seam-counts");
        fs::create_dir_all(&layer).map_err(|error| error.to_string())?;
        fs::write(layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
        let sentinel = root.join("KEEP.txt");
        fs::write(&sentinel, b"user data").map_err(|error| error.to_string())?;
        clear_cache_dir(
            &root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        )?;
        let safe = !layer.exists() && sentinel.is_file() && root.is_dir();
        remove_base(&base)?;
        if !safe {
            return Err("default cache suffix authorized whole-parent deletion".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_root_with_cache_child_is_not_deleted() -> Result<(), String> {
        let root = temp_dir("workspace-root-mixed");
        let layer = root.join("repo-seam-facts");
        fs::create_dir_all(&layer).map_err(|error| error.to_string())?;
        fs::write(layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
        let manifest = root.join("Cargo.toml");
        fs::write(&manifest, "[workspace]\n").map_err(|error| error.to_string())?;
        clear_cache_dir(
            &root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        )?;
        let safe = manifest.is_file() && root.is_dir() && !layer.exists();
        remove_base(&root)?;
        if !safe {
            return Err("cache clear deleted or damaged an injected workspace root".to_string());
        }
        Ok(())
    }

    #[test]
    fn dedicated_default_and_relocated_roots_remain_clearable() -> Result<(), String> {
        let (base, default_root) = populated_cache_root("dedicated-default")?;
        clear_cache_dir(
            &default_root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        )?;
        let default_clear = default_root.is_dir() && !default_root.join("repo-seam-facts").exists();
        remove_base(&base)?;

        let relocated_base = temp_dir("dedicated-relocated");
        let relocated = relocated_base.join("cache-home");
        let layer = relocated.join("repo-corpus-fingerprint");
        fs::create_dir_all(&layer).map_err(|error| error.to_string())?;
        fs::write(layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
        clear_cache_dir(
            &relocated,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        )?;
        let relocated_clear = relocated.is_dir() && !layer.exists();
        remove_base(&relocated_base)?;
        if !default_clear || !relocated_clear {
            return Err("dedicated default or relocated cache was not clearable".to_string());
        }
        Ok(())
    }

    #[test]
    fn incomplete_layer_status_fails_closed() -> Result<(), String> {
        let cache_dir = temp_dir("status-failure");
        for state in ["partial", "unavailable"] {
            let result = layer_from_status(
                &cache_dir,
                "repo-file-facts",
                cache_dir.join("repo-file-facts"),
                CacheStatus {
                    state,
                    total_size_bytes: 0,
                    entry_count: 0,
                },
            );
            if result.err().is_none_or(|error| {
                !error.contains(state) || !error.contains("no files were removed")
            }) {
                return Err(format!("{state} layer inspection did not fail closed"));
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_layer_and_ancestor_fail_closed() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let base = temp_dir("symlink-boundary");
        let root = base.join("root");
        let target = base.join("target-layer");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        let sentinel = target.join("KEEP.txt");
        fs::write(&sentinel, b"user data").map_err(|error| error.to_string())?;
        symlink(&target, root.join("repo-file-facts")).map_err(|error| error.to_string())?;
        let layer_result = clear_cache_dir(
            &root,
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        if layer_result.is_ok() || !sentinel.is_file() {
            remove_base(&base)?;
            return Err("symlinked cache layer was accepted or followed".to_string());
        }

        let real_parent = base.join("real-parent");
        let real_root = real_parent.join("cache-root");
        let real_layer = real_root.join("repo-seam-facts");
        fs::create_dir_all(&real_layer).map_err(|error| error.to_string())?;
        fs::write(real_layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
        let real_sentinel = real_root.join("KEEP.txt");
        fs::write(&real_sentinel, b"user data").map_err(|error| error.to_string())?;
        let alias = base.join("alias-parent");
        symlink(&real_parent, &alias).map_err(|error| error.to_string())?;
        let ancestor_result = clear_cache_dir(
            &alias.join("cache-root"),
            ClearOptions {
                dry_run: false,
                force: true,
            },
        );
        let safe = ancestor_result.is_err()
            && real_sentinel.is_file()
            && real_layer.join("entry.json").is_file();
        remove_base(&base)?;
        if !safe {
            return Err("symlinked cache ancestor was accepted or followed".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_and_empty_roots_report_no_removal() -> Result<(), String> {
        let missing = temp_dir("missing-clear");
        let missing_message = clear_cache_dir(&missing, ClearOptions::default())?;
        if !missing_message.contains("removed nothing") {
            return Err(format!(
                "unexpected missing-cache report: {missing_message}"
            ));
        }

        let empty = temp_dir("empty-clear");
        fs::create_dir_all(&empty).map_err(|error| error.to_string())?;
        let empty_message = clear_cache_dir(&empty, ClearOptions::default())?;
        let persisted = empty.is_dir();
        remove_base(&empty)?;
        if !persisted || !empty_message.contains("holds no entries") {
            return Err(format!("unexpected empty-cache behavior: {empty_message}"));
        }
        Ok(())
    }

    #[test]
    fn run_help_and_subcommand_routing_are_non_destructive() -> Result<(), String> {
        run(&["--help".to_string()])?;
        run(&["-h".to_string()])?;
        run(&["clear".to_string(), "--help".to_string()])?;
        if run(&[])
            .err()
            .is_none_or(|error| !error.contains("status` or `clear"))
        {
            return Err("missing cache subcommand did not fail closed".to_string());
        }
        if run(&["show".to_string()]).is_ok() {
            return Err("unknown cache subcommand unexpectedly passed".to_string());
        }
        Ok(())
    }

    #[test]
    fn help_is_positional_free_for_status_and_clear() -> Result<(), String> {
        // #5024: help used to be recognized only as the sole argument, so
        // `ripr cache status --json --help` failed with a self-referential
        // "Run `ripr cache status --help`" error instead of printing help.
        run(&[
            "status".to_string(),
            "--json".to_string(),
            "--help".to_string(),
        ])?;
        run(&[
            "status".to_string(),
            "--help".to_string(),
            "--json".to_string(),
        ])?;
        run(&["status".to_string(), "-h".to_string(), "--json".to_string()])?;
        run(&[
            "clear".to_string(),
            "--dry-run".to_string(),
            "--help".to_string(),
        ])?;
        run(&[
            "clear".to_string(),
            "--help".to_string(),
            "--force".to_string(),
        ])?;
        run(&["clear".to_string(), "-h".to_string()])?;
        // `ripr cache --help <anything>`: help wins at the family level,
        // matching the sibling command families.
        run(&["--help".to_string(), "status".to_string()])?;
        run(&["-h".to_string(), "clear".to_string()])?;
        // Family help also wins over an unrecognized trailing token.
        run(&["--help".to_string(), "unknown".to_string()])?;
        run(&["-h".to_string(), "unknown".to_string()])?;
        Ok(())
    }
}
