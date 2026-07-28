use crate::analysis::seam_cache::{
    CACHE_DIR_ENV, CacheStatus, cache_base_dir_from_env, inspect_cache_dir,
};
use crate::cli::suggest::unknown_argument;
use serde_json::json;
use std::path::{Path, PathBuf};

const CACHE_STATUS_SCHEMA_VERSION: &str = "0.1";

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

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("Usage: ripr cache status [--json]");
        return Ok(());
    }

    let Some((subcommand, rest)) = args.split_first() else {
        return Err("cache requires subcommand `status`".to_string());
    };

    if subcommand != "status" {
        return Err(format!(
            "unknown cache subcommand {subcommand:?}; expected `status`"
        ));
    }

    if matches!(rest, [arg] if arg == "--help" || arg == "-h") {
        println!("Usage: ripr cache status [--json]");
        return Ok(());
    }

    let is_json = parse_status_args(rest)?;
    let current_dir =
        std::env::current_dir().map_err(|e| format!("failed to get current dir: {}", e))?;
    let cache_dir = cache_dir_for_root(&current_dir, std::env::var(CACHE_DIR_ENV));
    let status = inspect_cache_dir(&cache_dir);
    println!("{}", render_status(&cache_dir, &status, is_json)?);

    Ok(())
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

    #[test]
    fn run_rejects_missing_and_unknown_subcommands() -> Result<(), String> {
        let missing =
            run(&[]).map_err(|error| format!("missing subcommand unexpectedly passed: {error}"));
        if missing.is_ok() {
            return Err("missing cache subcommand unexpectedly passed".to_string());
        }
        let unknown = run(&["show".to_string()]);
        if unknown.is_ok() {
            return Err("unknown cache subcommand unexpectedly passed".to_string());
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
