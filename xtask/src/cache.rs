use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const DEFAULT_MAX_SIZE_GB: u64 = 20;
const DEFAULT_TTL_DAYS: u64 = 14;
const BYTES_PER_GB: u64 = 1_000_000_000;
const SECONDS_PER_DAY: u64 = 86_400;
const SHARDED_CACHE_FAMILY_SUFFIX: &str = "-sharded";
const SHARD_MANIFEST_FILE: &str = "manifest.json";
const SHARD_FILE_PREFIX: &str = "shard-";
const MAX_REPORT_ROWS: usize = 20;

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
    sharded_cache: CacheShardSummary,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CacheShardSummary {
    shard_sets: usize,
    complete_sets: usize,
    orphan_sets: usize,
    incomplete_sets: usize,
    manifest_files: usize,
    shard_files: usize,
    bytes: u64,
    largest_sets: Vec<CacheShardSet>,
    problem_sets: Vec<CacheShardSet>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheShardSet {
    family: String,
    relative_path: PathBuf,
    status: ShardSetStatus,
    bytes: u64,
    manifest_path: Option<PathBuf>,
    manifest_bytes: u64,
    shard_files: usize,
    shard_bytes: u64,
    manifest_declared_shards: Option<usize>,
    manifest_declared_seams: Option<usize>,
    missing_shards: Vec<String>,
    extra_shards: Vec<String>,
    manifest_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ShardSetStatus {
    Complete,
    OrphanShards,
    Incomplete,
    ManifestUnreadable,
}

impl ShardSetStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::OrphanShards => "orphan_shards",
            Self::Incomplete => "incomplete",
            Self::ManifestUnreadable => "manifest_unreadable",
        }
    }
}

#[derive(Default)]
struct CacheShardSetBuilder {
    family: String,
    relative_path: PathBuf,
    manifest: Option<CacheFile>,
    shards: Vec<CacheFile>,
}

struct ShardManifestInfo {
    declared_shards: Option<usize>,
    declared_seams: Option<usize>,
    shard_files: Vec<String>,
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

    let sharded_cache = build_shard_summary(&files);

    Ok(CacheReport {
        cache_root,
        total_files: files.len(),
        total_bytes,
        families,
        largest_files,
        sharded_cache,
    })
}

fn build_shard_summary(files: &[CacheFile]) -> CacheShardSummary {
    let mut builders = BTreeMap::<PathBuf, CacheShardSetBuilder>::new();
    for file in files {
        if !file.family.ends_with(SHARDED_CACHE_FAMILY_SUFFIX) {
            continue;
        }
        let Some(file_name) = file
            .relative_path
            .file_name()
            .map(|name| name.to_string_lossy())
        else {
            continue;
        };
        let is_manifest = file_name == SHARD_MANIFEST_FILE;
        let is_shard = file_name.starts_with(SHARD_FILE_PREFIX) && file_name.ends_with(".json");
        if !is_manifest && !is_shard {
            continue;
        }
        let Some(set_path) = file.relative_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let builder = builders
            .entry(set_path.clone())
            .or_insert_with(|| CacheShardSetBuilder {
                family: file.family.clone(),
                relative_path: set_path,
                manifest: None,
                shards: Vec::new(),
            });
        if is_manifest {
            builder.manifest = Some(file.clone());
        } else {
            builder.shards.push(file.clone());
        }
    }

    let mut sets = builders
        .into_values()
        .map(cache_shard_set_from_builder)
        .collect::<Vec<_>>();
    sets.sort_by(|left, right| {
        left.family
            .cmp(&right.family)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });

    let mut summary = CacheShardSummary {
        shard_sets: sets.len(),
        complete_sets: sets
            .iter()
            .filter(|set| set.status == ShardSetStatus::Complete)
            .count(),
        orphan_sets: sets
            .iter()
            .filter(|set| set.status == ShardSetStatus::OrphanShards)
            .count(),
        incomplete_sets: sets
            .iter()
            .filter(|set| {
                matches!(
                    set.status,
                    ShardSetStatus::Incomplete | ShardSetStatus::ManifestUnreadable
                )
            })
            .count(),
        manifest_files: sets
            .iter()
            .filter(|set| set.manifest_path.is_some())
            .count(),
        shard_files: sets.iter().map(|set| set.shard_files).sum(),
        bytes: sets
            .iter()
            .fold(0u64, |sum, set| sum.saturating_add(set.bytes)),
        largest_sets: sets.clone(),
        problem_sets: sets
            .iter()
            .filter(|set| set.status != ShardSetStatus::Complete)
            .cloned()
            .collect(),
    };
    summary.largest_sets.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    summary.largest_sets.truncate(MAX_REPORT_ROWS);
    summary.problem_sets.sort_by(|left, right| {
        left.status
            .as_str()
            .cmp(right.status.as_str())
            .then_with(|| right.bytes.cmp(&left.bytes))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    summary
}

fn cache_shard_set_from_builder(mut builder: CacheShardSetBuilder) -> CacheShardSet {
    builder.shards.sort_by(|left, right| {
        left.relative_path
            .file_name()
            .cmp(&right.relative_path.file_name())
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let manifest_bytes = builder
        .manifest
        .as_ref()
        .map_or(0, |manifest| manifest.size_bytes);
    let shard_bytes = builder
        .shards
        .iter()
        .fold(0u64, |sum, shard| sum.saturating_add(shard.size_bytes));
    let actual_shards = builder
        .shards
        .iter()
        .filter_map(|shard| shard.relative_path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();

    let mut manifest_declared_shards = None;
    let mut manifest_declared_seams = None;
    let mut missing_shards = Vec::new();
    let mut extra_shards = Vec::new();
    let mut manifest_error = None;

    let status = match &builder.manifest {
        None => {
            extra_shards = actual_shards.iter().cloned().collect();
            ShardSetStatus::OrphanShards
        }
        Some(manifest_file) => match read_shard_manifest(&manifest_file.path) {
            Err(reason) => {
                manifest_error = Some(reason);
                ShardSetStatus::ManifestUnreadable
            }
            Ok(manifest) => {
                manifest_declared_shards = manifest.declared_shards;
                manifest_declared_seams = manifest.declared_seams;
                let expected = manifest.shard_files.into_iter().collect::<BTreeSet<_>>();
                missing_shards = expected.difference(&actual_shards).cloned().collect();
                extra_shards = actual_shards.difference(&expected).cloned().collect();
                let declared_count_mismatch = manifest_declared_shards.is_some_and(|declared| {
                    declared != expected.len() || declared != actual_shards.len()
                });
                if missing_shards.is_empty() && extra_shards.is_empty() && !declared_count_mismatch
                {
                    ShardSetStatus::Complete
                } else {
                    ShardSetStatus::Incomplete
                }
            }
        },
    };

    CacheShardSet {
        family: builder.family,
        relative_path: builder.relative_path,
        status,
        bytes: manifest_bytes.saturating_add(shard_bytes),
        manifest_path: builder.manifest.map(|manifest| manifest.relative_path),
        manifest_bytes,
        shard_files: builder.shards.len(),
        shard_bytes,
        manifest_declared_shards,
        manifest_declared_seams,
        missing_shards,
        extra_shards,
        manifest_error,
    }
}

fn read_shard_manifest(path: &Path) -> Result<ShardManifestInfo, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read shard manifest {}: {err}", path.display()))?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|err| format!("parse shard manifest {}: {err}", path.display()))?;
    let declared_shards = value
        .get("shard_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let declared_seams = value
        .get("total_seams")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let shard_files = value
        .get("shards")
        .and_then(Value::as_array)
        .map(|shards| {
            shards
                .iter()
                .filter_map(|shard| shard.get("file").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(ShardManifestInfo {
        declared_shards,
        declared_seams,
        shard_files,
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
        markdown.push_str("No cache files found.\n\n");
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
        markdown.push('\n');
    }

    markdown.push_str("\n## Sharded cache sets\n\n");
    let shard_summary = &report.sharded_cache;
    markdown.push_str(&format!(
        "- shard sets: {} (complete {}, orphan {}, incomplete {})\n- manifests: {}\n- shard files: {}\n- sharded size: {} ({})\n\n",
        shard_summary.shard_sets,
        shard_summary.complete_sets,
        shard_summary.orphan_sets,
        shard_summary.incomplete_sets,
        shard_summary.manifest_files,
        shard_summary.shard_files,
        human_bytes(shard_summary.bytes),
        shard_summary.bytes
    ));
    markdown.push_str("### Largest shard sets\n\n");
    if shard_summary.largest_sets.is_empty() {
        markdown.push_str("No sharded cache sets found.\n\n");
    } else {
        markdown.push_str(
            "| path | family | status | manifest shards | manifest seams | shard files | size |\n|---|---|---|---:|---:|---:|---:|\n",
        );
        for set in &shard_summary.largest_sets {
            markdown.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} | {} | {} ({}) |\n",
                set.relative_path.display(),
                set.family,
                set.status.as_str(),
                optional_usize(set.manifest_declared_shards),
                optional_usize(set.manifest_declared_seams),
                set.shard_files,
                human_bytes(set.bytes),
                set.bytes
            ));
        }
        markdown.push('\n');
    }
    markdown.push_str("### Orphan or incomplete shard sets\n\n");
    if shard_summary.problem_sets.is_empty() {
        markdown.push_str("No orphan or incomplete shard sets found.\n");
    } else {
        markdown.push_str("| path | family | status | issue |\n|---|---|---|---|\n");
        for set in &shard_summary.problem_sets {
            markdown.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} |\n",
                set.relative_path.display(),
                set.family,
                set.status.as_str(),
                shard_set_issue(set)
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
        "schema_version": "0.2",
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
        "sharded_cache": {
            "shard_sets": report.sharded_cache.shard_sets,
            "complete_sets": report.sharded_cache.complete_sets,
            "orphan_sets": report.sharded_cache.orphan_sets,
            "incomplete_sets": report.sharded_cache.incomplete_sets,
            "manifest_files": report.sharded_cache.manifest_files,
            "shard_files": report.sharded_cache.shard_files,
            "bytes": report.sharded_cache.bytes,
            "largest_sets": report.sharded_cache.largest_sets.iter().map(cache_shard_set_json).collect::<Vec<_>>(),
            "problem_sets": report.sharded_cache.problem_sets.iter().map(cache_shard_set_json).collect::<Vec<_>>(),
        },
    }))
    .map_err(|err| format!("serialize cache report: {err}"))
}

fn cache_shard_set_json(set: &CacheShardSet) -> Value {
    json!({
        "path": &set.relative_path,
        "family": &set.family,
        "status": set.status.as_str(),
        "bytes": set.bytes,
        "manifest_path": &set.manifest_path,
        "manifest_bytes": set.manifest_bytes,
        "shard_files": set.shard_files,
        "shard_bytes": set.shard_bytes,
        "manifest_declared_shards": set.manifest_declared_shards,
        "manifest_declared_seams": set.manifest_declared_seams,
        "missing_shards": &set.missing_shards,
        "extra_shards": &set.extra_shards,
        "manifest_error": &set.manifest_error,
    })
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

fn optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn shard_set_issue(set: &CacheShardSet) -> String {
    let mut parts = Vec::new();
    if let Some(error) = &set.manifest_error {
        parts.push(format!("manifest_error: `{}`", markdown_escape_cell(error)));
    }
    if !set.missing_shards.is_empty() {
        parts.push(format!(
            "missing_shards: `{}`",
            markdown_escape_cell(&set.missing_shards.join("`, `"))
        ));
    }
    if !set.extra_shards.is_empty() {
        parts.push(format!(
            "extra_shards: `{}`",
            markdown_escape_cell(&set.extra_shards.join("`, `"))
        ));
    }
    if parts.is_empty() {
        parts.push(set.status.as_str().to_string());
    }
    parts.join("; ")
}

fn markdown_escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
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
        GcOptions, ShardSetStatus, build_cache_report, build_gc_plan, cache_gc_markdown,
        cache_report_json, cache_report_markdown, parse_gc_options,
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
    fn cache_report_summarizes_sharded_sets_and_problem_sets() -> Result<(), String> {
        let root = temp_root("sharded-report")?;
        let complete_dir =
            root.join("target/ripr/cache/repo-seam-facts-sharded/0.2/0.1/complete-cache-key");
        write_text(
            &complete_dir.join("manifest.json"),
            r#"{
  "total_seams": 3,
  "shard_count": 2,
  "shards": [
    { "index": 0, "file": "shard-00000.json", "seams": 2 },
    { "index": 1, "file": "shard-00001.json", "seams": 1 }
  ]
}"#,
        )?;
        write_bytes(&complete_dir.join("shard-00000.json"), 11)?;
        write_bytes(&complete_dir.join("shard-00001.json"), 13)?;

        let orphan_dir =
            root.join("target/ripr/cache/repo-seam-facts-sharded/0.2/0.1/orphan-cache-key");
        write_bytes(&orphan_dir.join("shard-00000.json"), 17)?;

        let incomplete_dir =
            root.join("target/ripr/cache/repo-seam-facts-sharded/0.2/0.1/incomplete-cache-key");
        write_text(
            &incomplete_dir.join("manifest.json"),
            r#"{
  "total_seams": 4,
  "shard_count": 2,
  "shards": [
    { "index": 0, "file": "shard-00000.json", "seams": 2 },
    { "index": 1, "file": "shard-00001.json", "seams": 2 }
  ]
}"#,
        )?;
        write_bytes(&incomplete_dir.join("shard-00000.json"), 19)?;
        write_bytes(
            &root.join("target/ripr/reports/not-cache-shard-00000.json"),
            23,
        )?;

        let report = build_cache_report(&root)?;
        assert_eq!(report.sharded_cache.shard_sets, 3);
        assert_eq!(report.sharded_cache.complete_sets, 1);
        assert_eq!(report.sharded_cache.orphan_sets, 1);
        assert_eq!(report.sharded_cache.incomplete_sets, 1);
        assert_eq!(report.sharded_cache.manifest_files, 2);
        assert_eq!(report.sharded_cache.shard_files, 4);
        let complete = report
            .sharded_cache
            .largest_sets
            .iter()
            .find(|set| set.relative_path.ends_with("complete-cache-key"))
            .ok_or_else(|| "complete shard set should be reported".to_string())?;
        assert_eq!(complete.status, ShardSetStatus::Complete);
        assert_eq!(complete.manifest_declared_shards, Some(2));
        assert_eq!(complete.manifest_declared_seams, Some(3));
        assert_eq!(complete.shard_files, 2);

        let orphan = report
            .sharded_cache
            .problem_sets
            .iter()
            .find(|set| set.relative_path.ends_with("orphan-cache-key"))
            .ok_or_else(|| "orphan shard set should be reported".to_string())?;
        assert_eq!(orphan.status, ShardSetStatus::OrphanShards);
        assert_eq!(orphan.extra_shards, vec!["shard-00000.json".to_string()]);
        let incomplete = report
            .sharded_cache
            .problem_sets
            .iter()
            .find(|set| set.relative_path.ends_with("incomplete-cache-key"))
            .ok_or_else(|| "incomplete shard set should be reported".to_string())?;
        assert_eq!(incomplete.status, ShardSetStatus::Incomplete);
        assert_eq!(
            incomplete.missing_shards,
            vec!["shard-00001.json".to_string()]
        );

        let markdown = cache_report_markdown(&report).replace('\\', "/");
        assert!(markdown.contains("## Sharded cache sets"));
        assert!(
            markdown
                .contains("target/ripr/cache/repo-seam-facts-sharded/0.2/0.1/complete-cache-key")
        );
        assert!(markdown.contains("missing_shards: `shard-00001.json`"));
        assert!(markdown.contains("extra_shards: `shard-00000.json`"));
        assert!(!markdown.contains("not-cache-shard-00000.json"));

        let json = cache_report_json(&report)?;
        let parsed = serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|err| format!("parse cache report json: {err}"))?;
        assert_eq!(parsed["schema_version"], "0.2");
        assert_eq!(parsed["sharded_cache"]["shard_sets"], 3);
        assert_eq!(parsed["sharded_cache"]["manifest_files"], 2);
        assert_eq!(
            parsed["sharded_cache"]["problem_sets"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );

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

    fn write_text(path: &std::path::Path, text: &str) -> Result<(), String> {
        let Some(parent) = path.parent() else {
            return Err(format!("path has no parent: {}", path.display()));
        };
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn rel(parts: &[&str]) -> PathBuf {
        parts.iter().collect()
    }

    fn cleanup(root: PathBuf) -> Result<(), String> {
        fs::remove_dir_all(&root).map_err(|err| format!("remove {}: {err}", root.display()))
    }
}
