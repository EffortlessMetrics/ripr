//! tsconfig.json / jsconfig.json alias-map loader (RIPR-SPEC-0099).
//!
//! This module is SINGLE-HOP and FAIL-CLOSED:
//!
//! - Only `compilerOptions.baseUrl` and `compilerOptions.paths` are read.
//! - `extends` and `references` are NOT followed.
//! - Resolution succeeds ONLY when a specifier matches a SINGLE existing
//!   workspace file (.ts/.tsx/.js/.jsx).  Zero or >1 matches → `None`.
//! - Multi-entry value arrays (more than one candidate template) → `None`.
//! - Multi-`*` glob patterns → `None`.
//! - Any parse error or missing field → `None`.
//!
//! The alias map is built once per analysis run and reused for every
//! `normalized_relative_import_module` call that encounters a non-relative
//! specifier.  Building it is opt-in: `AnalysisOptions::resolve_tsconfig_paths`
//! must be `true`; otherwise `TsAliasMap::empty()` (no-op) is returned.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ── Wire types for parsing ────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTsConfig {
    compiler_options: Option<RawCompilerOptions>,
    // If `extends` or `references` are present we MUST NOT follow them;
    // we detect their presence and bail out (fail-closed).
    extends: Option<serde_json::Value>,
    references: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCompilerOptions {
    base_url: Option<String>,
    paths: Option<HashMap<String, Vec<String>>>,
}

// ── Public types ──────────────────────────────────────────────────────────────

/// Compiled alias map derived from `tsconfig.json` / `jsconfig.json`.
///
/// Contains only the resolvable entries: literal keys or single-`*` glob keys
/// whose value array has length exactly 1.  All other entries are silently
/// excluded during construction (fail-closed per RIPR-SPEC-0099).
#[derive(Debug, Default, Clone)]
pub(crate) struct TsAliasMap {
    /// Workspace root so that `resolve` can check file existence.
    root: PathBuf,
    /// `baseUrl`, relative to `root` (often `.`).
    base_url: String,
    /// Literal entries: key → single template string.
    literal_entries: HashMap<String, String>,
    /// Glob entries: (prefix, suffix) → single template string.
    ///
    /// The template may itself contain a `*`; the captured group from the
    /// specifier replaces that `*` in the template.
    glob_entries: Vec<GlobEntry>,
}

#[derive(Debug, Clone)]
struct GlobEntry {
    /// Part of the pattern key before the `*` (may be empty).
    prefix: String,
    /// Part of the pattern key after the `*` (may be empty).
    suffix: String,
    /// The single value template (exactly one `*` or none).
    template: String,
}

impl TsAliasMap {
    /// `true` when this map has no entries (opt-out / parse-failure path).
    pub(crate) fn is_empty(&self) -> bool {
        self.literal_entries.is_empty() && self.glob_entries.is_empty()
    }

    /// Resolve a non-relative specifier to a canonical workspace-relative path.
    ///
    /// Returns `None` (fail-closed) unless ALL of the following hold:
    /// 1. `specifier` is non-relative (does not start with `./` or `../`).
    /// 2. A literal or single-`*` paths key matches.
    /// 3. The matched value array has exactly one entry.
    /// 4. The value template has at most one `*`.
    /// 5. After substituting the captured `*`, the candidate path resolves to
    ///    EXACTLY ONE existing workspace file (.ts/.tsx/.js/.jsx).
    pub(crate) fn resolve(&self, specifier: &str) -> Option<PathBuf> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            return None; // relative paths are handled by the normal resolver
        }
        if self.is_empty() {
            return None;
        }

        // 1. Try literal match first.
        if let Some(template) = self.literal_entries.get(specifier) {
            let candidate_str = strip_ts_ext(template);
            return self.unique_file_for(&candidate_str);
        }

        // 2. Try glob entries (prefix*, prefix*suffix).
        for entry in &self.glob_entries {
            let Some(captured) = match_glob(specifier, &entry.prefix, &entry.suffix) else {
                continue;
            };
            let expanded = entry.template.replace('*', &captured);
            let candidate_str = strip_ts_ext(&expanded);
            return self.unique_file_for(&candidate_str);
        }

        None
    }

    /// Given a base candidate string (extension-stripped, slash-separated),
    /// try each TS extension and collect the unique matching file.
    ///
    /// Returns `None` if zero files or more than one file match.
    fn unique_file_for(&self, candidate_base: &str) -> Option<PathBuf> {
        let base_dir = self.root.join(self.base_url.trim_matches('/'));
        let extensions = [".ts", ".tsx", ".js", ".jsx"];
        let mut found: Vec<PathBuf> = Vec::new();
        for ext in &extensions {
            let candidate = base_dir.join(format!("{candidate_base}{ext}"));
            if candidate.is_file() {
                // Normalize to forward-slash workspace-relative path.
                if let Ok(rel) = candidate.strip_prefix(&self.root) {
                    found.push(rel.to_path_buf());
                }
            }
        }
        if found.len() == 1 {
            Some(found.remove(0))
        } else {
            None // zero or ambiguous — fail-closed
        }
    }
}

// ── Loader ────────────────────────────────────────────────────────────────────

/// Load and compile an alias map from `root/tsconfig.json` then
/// `root/jsconfig.json`.
///
/// Returns `None` (fail-closed) on any of:
/// - Neither file exists.
/// - JSON parse error.
/// - `compilerOptions` absent.
/// - `baseUrl` absent.
/// - `extends` or `references` present (single-hop only — do NOT follow).
pub(crate) fn load_alias_map(root: &Path) -> Option<TsAliasMap> {
    for filename in &["tsconfig.json", "jsconfig.json"] {
        let path = root.join(filename);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        return parse_alias_map(root, &text);
    }
    None
}

fn parse_alias_map(root: &Path, text: &str) -> Option<TsAliasMap> {
    let raw: RawTsConfig = serde_json::from_str(text).ok()?;

    // Fail-closed: if extends/references are present, do NOT follow them.
    if raw.extends.is_some() || raw.references.is_some() {
        return None;
    }

    let compiler_opts = raw.compiler_options?;
    let base_url = compiler_opts.base_url?;
    let paths = compiler_opts.paths.unwrap_or_default();

    let mut literal_entries: HashMap<String, String> = HashMap::new();
    let mut glob_entries: Vec<GlobEntry> = Vec::new();

    for (key, values) in &paths {
        // Condition (4): value array must have length exactly 1.
        if values.len() != 1 {
            continue;
        }
        let template = &values[0];

        // Condition (5): template may have at most one `*`.
        if template.chars().filter(|&c| c == '*').count() > 1 {
            continue;
        }

        // Determine if key is literal or single-`*` glob.
        let star_count = key.chars().filter(|&c| c == '*').count();
        if star_count == 0 {
            // Literal key.
            literal_entries.insert(key.clone(), template.clone());
        } else if star_count == 1 {
            // Single-`*` glob: split into prefix/suffix.
            if let Some(star_pos) = key.find('*') {
                glob_entries.push(GlobEntry {
                    prefix: key[..star_pos].to_string(),
                    suffix: key[star_pos + 1..].to_string(),
                    template: template.clone(),
                });
            }
        }
        // Multi-`*` keys → silently skipped (fail-closed).
    }

    Some(TsAliasMap {
        root: root.to_path_buf(),
        base_url,
        literal_entries,
        glob_entries,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Strip the file extension from a path string, preserving the rest.
fn strip_ts_ext(s: &str) -> String {
    for ext in &[".tsx", ".ts", ".jsx", ".js"] {
        if let Some(stripped) = s.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    s.to_string()
}

/// Try to match `specifier` against a single-`*` pattern given its
/// `prefix` and `suffix`.  Returns the captured group on success.
fn match_glob(specifier: &str, prefix: &str, suffix: &str) -> Option<String> {
    let after_prefix = specifier.strip_prefix(prefix)?;
    if suffix.is_empty() {
        return Some(after_prefix.to_string());
    }
    let captured = after_prefix.strip_suffix(suffix)?;
    Some(captured.to_string())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ripr-tsconfig-{label}-{stamp}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn write(dir: &Path, name: &str, content: &str) {
        if let Some(parent) = PathBuf::from(name).parent() {
            let _ = fs::create_dir_all(dir.join(parent));
        }
        let _ = fs::write(dir.join(name), content);
    }

    // ── load_alias_map ────────────────────────────────────────────────────

    #[test]
    fn returns_none_when_no_config_file_present() {
        let root = temp_dir("no-config");
        assert!(load_alias_map(&root).is_none());
    }

    #[test]
    fn returns_none_when_extends_present() {
        let root = temp_dir("extends");
        write(
            &root,
            "tsconfig.json",
            r#"{"extends":"./base","compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        assert!(load_alias_map(&root).is_none());
    }

    #[test]
    fn returns_none_when_references_present() {
        let root = temp_dir("refs");
        write(
            &root,
            "tsconfig.json",
            r#"{"references":[{"path":"./pkg"}],"compilerOptions":{"baseUrl":".","paths":{}}}"#,
        );
        assert!(load_alias_map(&root).is_none());
    }

    #[test]
    fn returns_none_when_base_url_absent() {
        let root = temp_dir("no-base-url");
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"paths":{"@/*":["src/*"]}}}"#,
        );
        assert!(load_alias_map(&root).is_none());
    }

    #[test]
    fn single_star_glob_resolves_to_unique_file() -> Result<(), String> {
        let root = temp_dir("single-star");
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        write(&root, "src/owner.ts", "export function owner() {}");

        let map = load_alias_map(&root).ok_or("should parse")?;
        let resolved = map.resolve("@/owner").ok_or("should resolve")?;
        // Normalize separators for assertion
        let resolved_str = resolved.to_string_lossy().replace('\\', "/");
        assert_eq!(resolved_str, "src/owner.ts");
        Ok(())
    }

    #[test]
    fn multi_entry_value_fails_closed() -> Result<(), String> {
        let root = temp_dir("multi-entry");
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*","lib/*"]}}}"#,
        );
        write(&root, "src/owner.ts", "export function owner() {}");
        // Even if one path would resolve, multi-entry value → None
        let map = load_alias_map(&root).ok_or("should parse")?;
        // The entry is excluded from the map entirely, so resolution returns None
        assert!(map.resolve("@/owner").is_none());
        Ok(())
    }

    #[test]
    fn ambiguous_two_files_fails_closed() -> Result<(), String> {
        let root = temp_dir("ambiguous");
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        write(&root, "src/owner.ts", "export function owner() {}");
        write(&root, "src/owner.tsx", "export function owner() {}");

        let map = load_alias_map(&root).ok_or("should parse")?;
        // Two matching extensions → fail-closed
        assert!(map.resolve("@/owner").is_none());
        Ok(())
    }

    #[test]
    fn no_matching_file_returns_none() -> Result<(), String> {
        let root = temp_dir("no-file");
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );

        let map = load_alias_map(&root).ok_or("should parse")?;
        assert!(map.resolve("@/nonexistent").is_none());
        Ok(())
    }

    #[test]
    fn relative_specifier_returns_none() -> Result<(), String> {
        let root = temp_dir("relative");
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{}}}"#,
        );
        let map = load_alias_map(&root).ok_or("should parse")?;
        assert!(map.resolve("./owner").is_none());
        assert!(map.resolve("../owner").is_none());
        Ok(())
    }

    #[test]
    fn empty_map_returns_none() {
        let map = TsAliasMap::default();
        assert!(map.resolve("@/owner").is_none());
        assert!(map.is_empty());
    }

    #[test]
    fn jsconfig_json_used_as_fallback() -> Result<(), String> {
        let root = temp_dir("jsconfig");
        write(
            &root,
            "jsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        write(&root, "src/owner.ts", "export function owner() {}");

        let map = load_alias_map(&root).ok_or("should parse jsconfig.json")?;
        assert!(map.resolve("@/owner").is_some());
        Ok(())
    }

    #[test]
    fn tsconfig_json_takes_priority_over_jsconfig_json() -> Result<(), String> {
        let root = temp_dir("priority");
        // tsconfig has empty paths; jsconfig has the alias
        write(
            &root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{}}}"#,
        );
        write(
            &root,
            "jsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        write(&root, "src/owner.ts", "export function owner() {}");

        let map = load_alias_map(&root).ok_or("should parse tsconfig.json")?;
        // tsconfig takes priority and has no @/* entry → None
        assert!(map.resolve("@/owner").is_none());
        Ok(())
    }
}
