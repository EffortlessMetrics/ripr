use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;

const DEFAULT_MAX_SIZE_GB: u64 = 20;
const DEFAULT_TTL_DAYS: u64 = 14;
const BYTES_PER_GB: u64 = 1_000_000_000;
const SECONDS_PER_DAY: u64 = 86_400;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(cache_usage());
    };
    let root = std::env::current_dir().map_err(|err| format!("locate current directory: {err}"))?;
    match subcommand.as_str() {
        "report" => cache_report(&root),
        "gc" => cache_gc(&root, rest),
        "help" | "--help" | "-h" => {
            println!("{}", cache_usage());
            Ok(())
        }
        other => Err(format!(
            "unknown cache subcommand `{other}`\n\n{}",
            cache_usage()
        )),
    }
}

fn cache_report(root: &Path) -> Result<(), String> {
    let report = build_cache_report(root)?;
    let markdown = cache_report_markdown(&report);
    write_report("cache-report.md", &markdown)?;
    write_report("cache-report.json", &cache_report_json(&report)?)?;
    print!("{markdown}");
    Ok(())
}

fn cache_gc(root: &Path, args: &[String]) -> Result<(), String> {
    let options = parse_gc_options(args)?;
    let started_at = SystemTime::now();
    let plan = build_gc_plan(root, &options, started_at)?;
    if !options.dry_run {
        for deletion in &plan.deletions {
            let path = root.join(&deletion.relative_path);
            fs::remove_file(&path)
                .map_err(|err| format!("failed to delete {}: {err}", path.display()))?;
        }
    }

    let markdown = cache_gc_markdown(&plan, &options);
    write_report("cache-gc.md", &markdown)?;
    write_report("cache-gc.json", &cache_gc_json(&plan, &options)?)?;
    print!("{markdown}");
    Ok(())
}

fn cache_usage() -> String {
    [
        "Usage:",
        "  cargo xtask cache report",
        "  cargo xtask cache gc [--dry-run] [--max-size-gb <n>] [--ttl-days <n>]",
        "",
        "Scope:",
        "  Only target/ripr/cache is scanned or deleted.",
        "  Reports, receipts, PR/review artifacts, workflow artifacts, build output, and source files are ignored.",
        "",
        "Defaults:",
        "  --max-size-gb 20",
        "  --ttl-days 14",
    ]
    .join("\n")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheReport {
    cache_root: PathBuf,
    total_files: usize,
    total_bytes: u64,
    families: Vec<CacheFamily>,
    largest_files: Vec<CacheFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheFamily {
    name: String,
    files: usize,
    bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheFile {
    path: PathBuf,
    relative_path: PathBuf,
    family: String,
    size_bytes: u64,
    modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GcOptions {
    dry_run: bool,
    max_size_bytes: Option<u64>,
    ttl_days: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GcPlan {
    cache_root: PathBuf,
    total_files_before: usize,
    total_bytes_before: u64,
    selected_files: usize,
    selected_bytes: u64,
    projected_bytes_after: u64,
    deletions: Vec<GcDeletion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GcDeletion {
    relative_path: PathBuf,
    family: String,
    size_bytes: u64,
    reasons: Vec<String>,
}

fn parse_gc_options(args: &[String]) -> Result<GcOptions, String> {
    let mut options = GcOptions {
        dry_run: false,
        max_size_bytes: Some(DEFAULT_MAX_SIZE_GB.saturating_mul(BYTES_PER_GB)),
        ttl_days: Some(DEFAULT_TTL_DAYS),
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                options.dry_run = true;
                index += 1;
            }
            "--max-size-gb" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--max-size-gb requires a value".to_string());
                };
                options.max_size_bytes = Some(parse_gb(value)?);
                index += 2;
            }
            "--ttl-days" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--ttl-days requires a value".to_string());
                };
                options.ttl_days = Some(parse_days(value)?);
                index += 2;
            }
            flag if flag.starts_with("--max-size-gb=") => {
                let value = flag.trim_start_matches("--max-size-gb=");
                options.max_size_bytes = Some(parse_gb(value)?);
                index += 1;
            }
            flag if flag.starts_with("--ttl-days=") => {
                let value = flag.trim_start_matches("--ttl-days=");
                options.ttl_days = Some(parse_days(value)?);
                index += 1;
            }
            "--no-size-limit" => {
                options.max_size_bytes = None;
                index += 1;
            }
            "--no-ttl" => {
                options.ttl_days = None;
                index += 1;
            }
            "--help" | "-h" => return Err(cache_usage()),
            other => {
                return Err(format!(
                    "unknown cache gc option `{other}`\n\n{}",
                    cache_usage()
                ));
            }
        }
    }
    Ok(options)
}

fn parse_gb(value: &str) -> Result<u64, String> {
    let gb = value
        .parse::<u64>()
        .map_err(|err| format!("invalid --max-size-gb `{value}`: {err}"))?;
    gb.checked_mul(BYTES_PER_GB)
        .ok_or_else(|| format!("--max-size-gb `{value}` is too large"))
}

fn parse_days(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid --ttl-days `{value}`: {err}"))
}

fn build_cache_report(root: &Path) -> Result<CacheReport, String> {
    let cache_root = cache_root(root);
    let files = collect_cache_files(root)?;
    let mut families = BTreeMap::<String, CacheFamily>::new();
    let mut total_bytes = 0u64;
    for file in &files {
        total_bytes = total_bytes.saturating_add(file.size_bytes);
        let entry = families
            .entry(file.family.clone())
            .or_insert_with(|| CacheFamily {
                name: file.family.clone(),
                files: 0,
                bytes: 0,
            });
        entry.files += 1;
        entry.bytes = entry.bytes.saturating_add(file.size_bytes);
    }
    let mut families = families.into_values().collect::<Vec<_>>();
    families.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut largest_files = files.clone();
    largest_files.sort_by(|left, right| {
        right
            .size_bytes
            .cmp(&left.size_bytes)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    largest_files.truncate(20);

    Ok(CacheReport {
        cache_root,
        total_files: files.len(),
        total_bytes,
        families,
        largest_files,
    })
}

fn build_gc_plan(
    root: &Path,
    options: &GcOptions,
    started_at: SystemTime,
) -> Result<GcPlan, String> {
    let cache_root = cache_root(root);
    let files = collect_cache_files(root)?;
    let total_bytes = files
        .iter()
        .fold(0u64, |sum, file| sum.saturating_add(file.size_bytes));
    let mut selected = BTreeMap::<usize, BTreeSet<String>>::new();

    if let Some(ttl_days) = options.ttl_days {
        let ttl = Duration::from_secs(ttl_days.saturating_mul(SECONDS_PER_DAY));
        if let Some(cutoff) = SystemTime::now().checked_sub(ttl) {
            for (index, file) in files.iter().enumerate() {
                if is_current_run_file(file, started_at) {
                    continue;
                }
                if file.modified.is_some_and(|modified| modified < cutoff) {
                    selected
                        .entry(index)
                        .or_default()
                        .insert(format!("ttl_days>{ttl_days}"));
                }
            }
        }
    }

    let selected_bytes = selected.keys().fold(0u64, |sum, index| {
        sum.saturating_add(files[*index].size_bytes)
    });
    let mut projected_bytes = total_bytes.saturating_sub(selected_bytes);
    if let Some(max_size_bytes) = options
        .max_size_bytes
        .filter(|max_size_bytes| projected_bytes > *max_size_bytes)
    {
        let mut candidates = files
            .iter()
            .enumerate()
            .filter(|(index, file)| {
                !selected.contains_key(index) && !is_current_run_file(file, started_at)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|(left_index, left), (right_index, right)| {
            modified_sort_key(left)
                .cmp(&modified_sort_key(right))
                .then_with(|| right.size_bytes.cmp(&left.size_bytes))
                .then_with(|| left_index.cmp(right_index))
        });
        for (index, file) in candidates {
            if projected_bytes <= max_size_bytes {
                break;
            }
            selected
                .entry(index)
                .or_default()
                .insert(format!("max_size_gb>{}", max_size_bytes / BYTES_PER_GB));
            projected_bytes = projected_bytes.saturating_sub(file.size_bytes);
        }
    }

    let mut deletions = selected
        .into_iter()
        .map(|(index, reasons)| GcDeletion {
            relative_path: files[index].relative_path.clone(),
            family: files[index].family.clone(),
            size_bytes: files[index].size_bytes,
            reasons: reasons.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    deletions.sort_by(|left, right| {
        left.family
            .cmp(&right.family)
            .then_with(|| right.size_bytes.cmp(&left.size_bytes))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let selected_bytes = deletions.iter().fold(0u64, |sum, deletion| {
        sum.saturating_add(deletion.size_bytes)
    });

    Ok(GcPlan {
        cache_root,
        total_files_before: files.len(),
        total_bytes_before: total_bytes,
        selected_files: deletions.len(),
        selected_bytes,
        projected_bytes_after: total_bytes.saturating_sub(selected_bytes),
        deletions,
    })
}

fn is_current_run_file(file: &CacheFile, started_at: SystemTime) -> bool {
    file.modified.is_some_and(|modified| modified >= started_at)
}

fn modified_sort_key(file: &CacheFile) -> (u64, u32) {
    file.modified
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or((0, 0), |duration| {
            (duration.as_secs(), duration.subsec_nanos())
        })
}

fn collect_cache_files(root: &Path) -> Result<Vec<CacheFile>, String> {
    let cache_root = cache_root(root);
    if !cache_root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(&cache_root)
        .map_err(|err| format!("failed to inspect {}: {err}", cache_root.display()))?;
    if !metadata.is_dir() {
        return Err(format!("{} is not a directory", cache_root.display()));
    }

    let mut files = Vec::new();
    let mut stack = vec![cache_root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?
        {
            let entry = entry.map_err(|err| format!("failed to read cache entry: {err}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            files.push(CacheFile {
                family: cache_family(&cache_root, &path),
                path,
                relative_path,
                size_bytes: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
    }
    Ok(files)
}

fn cache_root(root: &Path) -> PathBuf {
    root.join("target").join("ripr").join("cache")
}

fn cache_family(cache_root: &Path, path: &Path) -> String {
    path.strip_prefix(cache_root)
        .ok()
        .and_then(|relative| relative.components().next())
        .and_then(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "cache-root".to_string())
}

fn cache_report_markdown(report: &CacheReport) -> String {
    let mut markdown = String::new();
    markdown.push_str("# ripr cache report\n\n");
    markdown.push_str("Status: pass\n\n");
    markdown.push_str(&format!(
        "Scope: `{}` only\n\n",
        report.cache_root.display()
    ));
    markdown.push_str(&format!(
        "- total files: {}\n- total size: {} ({})\n\n",
        report.total_files,
        human_bytes(report.total_bytes),
        report.total_bytes
    ));
    markdown.push_str("## Largest cache families\n\n");
    if report.families.is_empty() {
        markdown.push_str("No cache files found.\n\n");
    } else {
        markdown.push_str("| family | files | size |\n|---|---:|---:|\n");
        for family in &report.families {
            markdown.push_str(&format!(
                "| `{}` | {} | {} ({}) |\n",
                family.name,
                family.files,
                human_bytes(family.bytes),
                family.bytes
            ));
        }
        markdown.push('\n');
    }
    markdown.push_str("## Largest cache files\n\n");
    if report.largest_files.is_empty() {
        markdown.push_str("No cache files found.\n");
    } else {
        markdown.push_str("| path | family | size |\n|---|---|---:|\n");
        for file in &report.largest_files {
            markdown.push_str(&format!(
                "| `{}` | `{}` | {} ({}) |\n",
                file.relative_path.display(),
                file.family,
                human_bytes(file.size_bytes),
                file.size_bytes
            ));
        }
    }
    markdown
}

fn cache_gc_markdown(plan: &GcPlan, options: &GcOptions) -> String {
    let mut markdown = String::new();
    markdown.push_str("# ripr cache gc\n\n");
    markdown.push_str("Status: pass\n\n");
    markdown.push_str(&format!(
        "Mode: {}\n\n",
        if options.dry_run { "dry-run" } else { "delete" }
    ));
    markdown.push_str(&format!("Scope: `{}` only\n\n", plan.cache_root.display()));
    markdown.push_str(&format!(
        "- max size: {}\n- ttl days: {}\n- total before: {} ({})\n- selected: {} files, {} ({})\n- projected after: {} ({})\n\n",
        options
            .max_size_bytes
            .map(|bytes| format!("{} ({bytes})", human_bytes(bytes)))
            .unwrap_or_else(|| "none".to_string()),
        options
            .ttl_days
            .map(|days| days.to_string())
            .unwrap_or_else(|| "none".to_string()),
        human_bytes(plan.total_bytes_before),
        plan.total_bytes_before,
        plan.selected_files,
        human_bytes(plan.selected_bytes),
        plan.selected_bytes,
        human_bytes(plan.projected_bytes_after),
        plan.projected_bytes_after
    ));
    markdown.push_str("## Deletions\n\n");
    if plan.deletions.is_empty() {
        markdown.push_str("No deletions selected.\n");
    } else {
        markdown.push_str("| path | family | size | reason |\n|---|---|---:|---|\n");
        for deletion in &plan.deletions {
            markdown.push_str(&format!(
                "| `{}` | `{}` | {} ({}) | `{}` |\n",
                deletion.relative_path.display(),
                deletion.family,
                human_bytes(deletion.size_bytes),
                deletion.size_bytes,
                deletion.reasons.join("`, `")
            ));
        }
    }
    markdown
}

fn cache_report_json(report: &CacheReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": "0.1",
        "status": "pass",
        "scope": "target/ripr/cache",
        "cache_root": report.cache_root,
        "total_files": report.total_files,
        "total_bytes": report.total_bytes,
        "families": report.families.iter().map(|family| json!({
            "name": family.name,
            "files": family.files,
            "bytes": family.bytes,
        })).collect::<Vec<_>>(),
        "largest_files": report.largest_files.iter().map(|file| json!({
            "path": file.relative_path,
            "family": file.family,
            "bytes": file.size_bytes,
            "modified_unix_seconds": unix_seconds(file.modified),
        })).collect::<Vec<_>>(),
    }))
    .map_err(|err| format!("serialize cache report: {err}"))
}

fn cache_gc_json(plan: &GcPlan, options: &GcOptions) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": "0.1",
        "status": "pass",
        "mode": if options.dry_run { "dry_run" } else { "delete" },
        "scope": "target/ripr/cache",
        "cache_root": plan.cache_root,
        "max_size_bytes": options.max_size_bytes,
        "ttl_days": options.ttl_days,
        "total_files_before": plan.total_files_before,
        "total_bytes_before": plan.total_bytes_before,
        "selected_files": plan.selected_files,
        "selected_bytes": plan.selected_bytes,
        "projected_bytes_after": plan.projected_bytes_after,
        "deletions": plan.deletions.iter().map(|deletion| json!({
            "path": deletion.relative_path,
            "family": deletion.family,
            "bytes": deletion.size_bytes,
            "reasons": deletion.reasons,
        })).collect::<Vec<_>>(),
    }))
    .map_err(|err| format!("serialize cache gc report: {err}"))
}

fn unix_seconds(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn write_report(name: &str, contents: &str) -> Result<(), String> {
    let reports_dir = Path::new("target").join("ripr").join("reports");
    fs::create_dir_all(&reports_dir)
        .map_err(|err| format!("failed to create {}: {err}", reports_dir.display()))?;
    let path = reports_dir.join(name);
    fs::write(&path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn human_bytes(bytes: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.2} MB", value / MB)
    } else if value >= KB {
        format!("{:.2} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GcOptions, build_cache_report, build_gc_plan, cache_gc_markdown, cache_report_markdown,
        parse_gc_options,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn cache_report_lists_families_and_largest_files() -> Result<(), String> {
        let root = temp_root("report")?;
        write_bytes(&root.join("target/ripr/cache/repo-seam-facts/v1/a.json"), 7)?;
        write_bytes(&root.join("target/ripr/cache/file-facts/b.json"), 3)?;
        write_bytes(&root.join("target/ripr/reports/not-cache.json"), 100)?;

        let report = build_cache_report(&root)?;
        assert_eq!(report.total_files, 2);
        assert_eq!(report.total_bytes, 10);
        assert_eq!(report.families[0].name, "repo-seam-facts");
        assert_eq!(report.families[0].bytes, 7);
        assert_eq!(report.families[1].name, "file-facts");
        assert_eq!(
            report.largest_files[0].relative_path,
            rel(&["target", "ripr", "cache", "repo-seam-facts", "v1", "a.json"])
        );

        let markdown = cache_report_markdown(&report);
        let markdown = markdown.replace('\\', "/");
        assert!(markdown.contains("target/ripr/cache/repo-seam-facts/v1/a.json"));
        assert!(!markdown.contains("not-cache.json"));

        cleanup(root)?;
        Ok(())
    }

    #[test]
    fn cache_gc_dry_run_selects_only_cache_files() -> Result<(), String> {
        let root = temp_root("gc-dry-run")?;
        let cache_file = root.join("target/ripr/cache/repo-seam-facts/v1/a.json");
        let report_file = root.join("target/ripr/reports/current-run.json");
        let build_file = root.join("target/debug/build/output.bin");
        write_bytes(&cache_file, 11)?;
        write_bytes(&report_file, 13)?;
        write_bytes(&build_file, 17)?;

        let plan = build_gc_plan(
            &root,
            &GcOptions {
                dry_run: true,
                max_size_bytes: Some(0),
                ttl_days: None,
            },
            SystemTime::now() + Duration::from_secs(1),
        )?;

        assert_eq!(plan.selected_files, 1);
        assert_eq!(
            plan.deletions[0].relative_path,
            rel(&["target", "ripr", "cache", "repo-seam-facts", "v1", "a.json"])
        );
        assert!(cache_file.exists());
        assert!(report_file.exists());
        assert!(build_file.exists());

        let markdown = cache_gc_markdown(
            &plan,
            &GcOptions {
                dry_run: true,
                max_size_bytes: Some(0),
                ttl_days: None,
            },
        );
        let markdown = markdown.replace('\\', "/");
        assert!(markdown.contains("Mode: dry-run"));
        assert!(markdown.contains("target/ripr/cache/repo-seam-facts/v1/a.json"));
        assert!(!markdown.contains("current-run.json"));

        cleanup(root)?;
        Ok(())
    }

    #[test]
    fn cache_gc_skips_files_modified_during_current_run() -> Result<(), String> {
        let root = temp_root("gc-current-run")?;
        write_bytes(
            &root.join("target/ripr/cache/repo-seam-facts/v1/a.json"),
            11,
        )?;

        let plan = build_gc_plan(
            &root,
            &GcOptions {
                dry_run: true,
                max_size_bytes: Some(0),
                ttl_days: None,
            },
            UNIX_EPOCH,
        )?;

        assert_eq!(plan.selected_files, 0);

        cleanup(root)?;
        Ok(())
    }

    #[test]
    fn cache_gc_options_default_to_bounded_policy() -> Result<(), String> {
        let options = parse_gc_options(&["--dry-run".to_string()])?;
        assert!(options.dry_run);
        assert_eq!(options.max_size_bytes, Some(20_000_000_000));
        assert_eq!(options.ttl_days, Some(14));

        let explicit = parse_gc_options(&[
            "--max-size-gb".to_string(),
            "1".to_string(),
            "--ttl-days=2".to_string(),
        ])?;
        assert_eq!(explicit.max_size_bytes, Some(1_000_000_000));
        assert_eq!(explicit.ttl_days, Some(2));
        Ok(())
    }

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-xtask-cache-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
        Ok(root)
    }

    fn write_bytes(path: &std::path::Path, len: usize) -> Result<(), String> {
        let Some(parent) = path.parent() else {
            return Err(format!("path has no parent: {}", path.display()));
        };
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(path, vec![0u8; len]).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn rel(parts: &[&str]) -> PathBuf {
        parts.iter().collect()
    }

    fn cleanup(root: PathBuf) -> Result<(), String> {
        fs::remove_dir_all(&root).map_err(|err| format!("remove {}: {err}", root.display()))
    }
}
