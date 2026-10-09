//! Typed repair-attempt store identity and the one resolver that owns it.
//!
//! Repository, root, store, and attempt identities stay separate. Before,
//! status, and after consume this resolver; they do not search parent
//! directories, sibling worktrees, or a newest-mtime folder, and they never
//! fall back from an explicit store to the default.

use crate::agent::loop_commands::lexically_clean;
use crate::output::path::display_path;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use super::REPAIR_ATTEMPT_DIRECTORY;

pub(crate) const REPAIR_ATTEMPT_STORE_SCHEMA_VERSION: &str = "0.1";

/// How a caller names the store. The default is the repository-local
/// `target/ripr/repair-attempts` directory. An explicit locator is accepted
/// only as typed input and is resolved against the selected repository root,
/// never against process CWD independently of that root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepairAttemptStoreLocator<'a> {
    Default,
    Explicit(&'a Path),
}

/// Whether the caller is preparing a new attempt (the store may be created)
/// or opening an existing store (missing explicit stores fail closed).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepairAttemptStoreAccess {
    Prepare,
    Open,
}

/// Portable store identity retained on a manifest. Absolute checkout spelling
/// is diagnostic on the runtime ref, not this identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepairAttemptStoreIdentity {
    pub(crate) schema_version: String,
    pub(crate) location_class: RepairAttemptStoreLocationClass,
    pub(crate) locator: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepairAttemptStoreLocationClass {
    DefaultRepository,
    ExplicitRepository,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepairAttemptStoreCurrentness {
    Present,
    Missing,
}

/// Runtime store authority: repository root, portable locator, concrete path,
/// class, filesystem identity, and currentness. One resolved value is shared
/// by before, status, and after.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepairAttemptStoreRef {
    canonical_root: PathBuf,
    locator: String,
    resolved_path: PathBuf,
    location_class: RepairAttemptStoreLocationClass,
    filesystem_identity: Option<String>,
    currentness: RepairAttemptStoreCurrentness,
    limitations: Vec<String>,
}

impl RepairAttemptStoreIdentity {
    pub(crate) fn default_repository() -> Self {
        Self {
            schema_version: REPAIR_ATTEMPT_STORE_SCHEMA_VERSION.to_string(),
            location_class: RepairAttemptStoreLocationClass::DefaultRepository,
            locator: REPAIR_ATTEMPT_DIRECTORY.to_string(),
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "identity helper for store-class equality; tests pin default vs explicit"
        )
    )]
    pub(crate) fn is_default(&self) -> bool {
        self.location_class == RepairAttemptStoreLocationClass::DefaultRepository
            && self.locator == REPAIR_ATTEMPT_DIRECTORY
    }
}

impl RepairAttemptStoreRef {
    pub(crate) fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    pub(crate) fn locator(&self) -> &str {
        &self.locator
    }

    pub(crate) fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }

    pub(crate) fn location_class(&self) -> RepairAttemptStoreLocationClass {
        self.location_class
    }

    pub(crate) fn is_default(&self) -> bool {
        self.location_class == RepairAttemptStoreLocationClass::DefaultRepository
    }

    pub(crate) fn currentness(&self) -> RepairAttemptStoreCurrentness {
        self.currentness
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "filesystem identity is diagnostic on the runtime ref; tests pin present stores"
        )
    )]
    pub(crate) fn filesystem_identity(&self) -> Option<&str> {
        self.filesystem_identity.as_deref()
    }

    pub(crate) fn limitations(&self) -> &[String] {
        &self.limitations
    }

    pub(crate) fn identity(&self) -> RepairAttemptStoreIdentity {
        RepairAttemptStoreIdentity {
            schema_version: REPAIR_ATTEMPT_STORE_SCHEMA_VERSION.to_string(),
            location_class: self.location_class,
            locator: self.locator.clone(),
        }
    }

    /// Identity retained on the manifest. Default-store manifests omit the
    /// field so ordinary before → after bytes stay compatible.
    pub(crate) fn manifest_identity(&self) -> Option<RepairAttemptStoreIdentity> {
        if self.is_default() {
            None
        } else {
            Some(self.identity())
        }
    }

    pub(crate) fn attempt_directory(&self, attempt_id: &super::RepairAttemptId) -> PathBuf {
        self.resolved_path.join(attempt_id.as_str())
    }

    pub(crate) fn matches_manifest(
        &self,
        declared: Option<&RepairAttemptStoreIdentity>,
    ) -> Result<(), String> {
        let expected = declared
            .cloned()
            .unwrap_or_else(RepairAttemptStoreIdentity::default_repository);
        if expected.schema_version != REPAIR_ATTEMPT_STORE_SCHEMA_VERSION {
            return Err(format!(
                "repair attempt store schema must be {REPAIR_ATTEMPT_STORE_SCHEMA_VERSION}, got {}",
                expected.schema_version
            ));
        }
        let actual = self.identity();
        if expected.location_class != actual.location_class || expected.locator != actual.locator {
            return Err(format!(
                "repair attempt belongs to store `{}` ({:?}), not `{}` ({:?}); an attempt ID from one store cannot resolve through another",
                expected.locator, expected.location_class, actual.locator, actual.location_class
            ));
        }
        Ok(())
    }

    pub(crate) fn quoted_store_flag(&self) -> String {
        if self.is_default() {
            String::new()
        } else {
            quoted_store_flag(Some(Path::new(&self.locator)))
        }
    }
}

/// CLI `--store PATH` fragment for recovery and status follow-up commands.
/// Default invocations omit the flag so ordinary bytes stay compatible.
pub(crate) fn quoted_store_flag(store: Option<&Path>) -> String {
    match store {
        Some(path) if !path.as_os_str().is_empty() => format!(
            " --store {}",
            crate::agent::loop_commands::shell_arg(&path.to_string_lossy())
        ),
        _ => String::new(),
    }
}

pub(crate) fn quoted_store_flag_from_identity(
    identity: Option<&RepairAttemptStoreIdentity>,
) -> String {
    match identity {
        Some(identity)
            if identity.location_class != RepairAttemptStoreLocationClass::DefaultRepository =>
        {
            quoted_store_flag(Some(Path::new(&identity.locator)))
        }
        _ => String::new(),
    }
}

pub(crate) fn resolve_store(
    root: &Path,
    store: Option<&Path>,
    access: RepairAttemptStoreAccess,
) -> Result<RepairAttemptStoreRef, String> {
    resolve_repair_attempt_store(
        root,
        match store {
            None => RepairAttemptStoreLocator::Default,
            Some(path) => RepairAttemptStoreLocator::Explicit(path),
        },
        access,
    )
}

pub(crate) fn resolve_repair_attempt_store(
    root: &Path,
    locator: RepairAttemptStoreLocator<'_>,
    access: RepairAttemptStoreAccess,
) -> Result<RepairAttemptStoreRef, String> {
    let canonical_root = root.canonicalize().map_err(|error| {
        format!(
            "canonicalize repair attempt repository root {} failed: {error}",
            root.display()
        )
    })?;
    let requested = match locator {
        RepairAttemptStoreLocator::Default => PathBuf::from(REPAIR_ATTEMPT_DIRECTORY),
        RepairAttemptStoreLocator::Explicit(path) => {
            validate_explicit_locator(path)?;
            path.to_path_buf()
        }
    };
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "repair attempt store {} is not contained in repository root {}; traversal components are refused",
            display_path(&requested),
            display_path(&canonical_root)
        ));
    }
    let joined = if requested.is_absolute() {
        requested.clone()
    } else {
        canonical_root.join(&requested)
    };
    let cleaned = lexically_clean(&joined);
    if path_escapes_root(&canonical_root, &cleaned) {
        return Err(format!(
            "repair attempt store {} is not contained in repository root {}",
            display_path(&requested),
            display_path(&canonical_root)
        ));
    }
    let relative = relative_locator(&canonical_root, &cleaned)?;
    if relative.is_empty() {
        return Err(
            "repair attempt store cannot be the repository root; name a directory inside it"
                .to_string(),
        );
    }

    let exists = cleaned.exists();
    if exists {
        if !cleaned.is_dir() {
            return Err(format!(
                "repair attempt store {} is not a directory",
                display_path(&cleaned)
            ));
        }
        let canonical_store = canonicalize_no_escape(&cleaned)?;
        if path_escapes_root(&canonical_root, &canonical_store) {
            return Err(format!(
                "repair attempt store {} escapes repository root {} through a symlink or junction",
                display_path(&requested),
                display_path(&canonical_root)
            ));
        }
        let canonical_relative = relative_locator(&canonical_root, &canonical_store)?;
        if canonical_relative != relative
            && !equivalent_windows_drive_spelling(&relative, &canonical_relative)
        {
            return Err(format!(
                "repair attempt store {} is a symlink, junction, or case-fold alias of `{}`; store identity uses the concrete contained locator, and aliases are rejected",
                display_path(&requested),
                canonical_relative
            ));
        }
        return Ok(store_ref(
            canonical_root,
            canonical_relative,
            canonical_store,
            RepairAttemptStoreCurrentness::Present,
        ));
    }

    match (locator, access) {
        (RepairAttemptStoreLocator::Explicit(_), RepairAttemptStoreAccess::Open) => Err(format!(
            "explicit repair attempt store `{}` is missing or unreadable under {}; it does not fall back to the default store `{}`",
            relative,
            display_path(&canonical_root),
            REPAIR_ATTEMPT_DIRECTORY
        )),
        (RepairAttemptStoreLocator::Default, RepairAttemptStoreAccess::Open) => Ok(store_ref(
            canonical_root,
            relative,
            cleaned,
            RepairAttemptStoreCurrentness::Missing,
        )),
        (_, RepairAttemptStoreAccess::Prepare) => {
            validate_prepare_ancestors(&canonical_root, &cleaned)?;
            std::fs::create_dir_all(&cleaned).map_err(|error| {
                format!(
                    "create repair attempt store {} failed: {error}",
                    display_path(&cleaned)
                )
            })?;
            let canonical_store = canonicalize_no_escape(&cleaned)?;
            if path_escapes_root(&canonical_root, &canonical_store) {
                let _ = std::fs::remove_dir_all(&cleaned);
                return Err(format!(
                    "repair attempt store {} escaped repository root {} while being created",
                    display_path(&requested),
                    display_path(&canonical_root)
                ));
            }
            let canonical_relative = relative_locator(&canonical_root, &canonical_store)?;
            if canonical_relative != relative
                && !equivalent_windows_drive_spelling(&relative, &canonical_relative)
            {
                let _ = std::fs::remove_dir_all(&cleaned);
                return Err(format!(
                    "repair attempt store {} is a symlink, junction, or case-fold alias of `{}`; store identity uses the concrete contained locator, and aliases are rejected",
                    display_path(&requested),
                    canonical_relative
                ));
            }
            Ok(store_ref(
                canonical_root,
                canonical_relative,
                canonical_store,
                RepairAttemptStoreCurrentness::Present,
            ))
        }
    }
}

fn store_ref(
    canonical_root: PathBuf,
    locator: String,
    resolved_path: PathBuf,
    currentness: RepairAttemptStoreCurrentness,
) -> RepairAttemptStoreRef {
    let location_class = if locator == REPAIR_ATTEMPT_DIRECTORY {
        RepairAttemptStoreLocationClass::DefaultRepository
    } else {
        RepairAttemptStoreLocationClass::ExplicitRepository
    };
    let filesystem_identity = (currentness == RepairAttemptStoreCurrentness::Present)
        .then(|| display_path(&resolved_path));
    RepairAttemptStoreRef {
        canonical_root,
        locator,
        resolved_path,
        location_class,
        filesystem_identity,
        currentness,
        limitations: vec![
            "concrete path spelling is for access and diagnostics; store identity is the repository-relative locator".to_string(),
            "this store identity does not prove an attempt is current, correct, or useful".to_string(),
        ],
    }
}

fn validate_explicit_locator(path: &Path) -> Result<(), String> {
    let raw = path.as_os_str();
    if raw.is_empty() {
        return Err("repair attempt --store requires a non-empty path".to_string());
    }
    let text = path.to_string_lossy();
    if text.trim().is_empty() {
        return Err("repair attempt --store requires a non-empty path".to_string());
    }
    if is_unsupported_unc(&text) {
        return Err(format!(
            "repair attempt store {} uses unsupported UNC or share spelling; name a repository-contained path",
            display_path(path)
        ));
    }
    if is_drive_relative(&text) {
        return Err(format!(
            "repair attempt store {} is a drive-relative path; use a root-contained relative path or an absolute path inside the selected repository",
            display_path(path)
        ));
    }
    Ok(())
}

fn is_unsupported_unc(text: &str) -> bool {
    let trimmed = text.trim();
    let bytes = trimmed.as_bytes();
    if bytes.starts_with(br"\\") {
        return true;
    }
    if cfg!(windows) && bytes.starts_with(b"//") {
        return true;
    }
    false
}

fn is_drive_relative(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes
            .get(2)
            .is_none_or(|next| *next != b'\\' && *next != b'/')
}

fn path_escapes_root(root: &Path, candidate: &Path) -> bool {
    let mut leftover = candidate.components();
    for expected in root.components() {
        match leftover.next() {
            Some(actual) if components_match(expected, actual) => {}
            _ => return true,
        }
    }
    false
}

fn components_match(left: Component<'_>, right: Component<'_>) -> bool {
    if left == right {
        return true;
    }
    if cfg!(windows)
        && let (Component::Prefix(left_prefix), Component::Prefix(right_prefix)) = (left, right)
    {
        return left_prefix
            .as_os_str()
            .eq_ignore_ascii_case(right_prefix.as_os_str());
    }
    false
}

fn relative_locator(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|_prefix| {
        format!(
            "repair attempt store {} is not contained in repository root {}",
            display_path(path),
            display_path(root)
        )
    })?;
    let mut locator = String::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                if !locator.is_empty() {
                    locator.push('/');
                }
                locator.push_str(&part.to_string_lossy());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(format!(
                    "repair attempt store {} is not a portable repository-relative locator",
                    display_path(path)
                ));
            }
        }
    }
    Ok(locator)
}

fn equivalent_windows_drive_spelling(left: &str, right: &str) -> bool {
    cfg!(windows) && left.eq_ignore_ascii_case(right)
}

fn canonicalize_no_escape(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize().map_err(|error| {
        format!(
            "canonicalize repair attempt store {} failed: {error}",
            display_path(path)
        )
    })
}

/// Canonicalize the deepest existing ancestor before `create_dir_all` so a
/// missing child under an escaping or aliasing parent is refused without
/// creating directories outside the selected root.
fn validate_prepare_ancestors(canonical_root: &Path, cleaned: &Path) -> Result<(), String> {
    let mut current = cleaned.parent().ok_or_else(|| {
        format!(
            "repair attempt store {} is missing a parent directory",
            display_path(cleaned)
        )
    })?;
    loop {
        match current.symlink_metadata() {
            Ok(_) => {
                let canonical = current.canonicalize().map_err(|error| {
                    format!(
                        "repair attempt store parent {} is a dangling symlink, junction, or unreadable alias: {error}",
                        display_path(current)
                    )
                })?;
                if path_escapes_root(canonical_root, &canonical) {
                    return Err(format!(
                        "repair attempt store {} escapes repository root {} through a symlink or junction",
                        display_path(cleaned),
                        display_path(canonical_root)
                    ));
                }
                let lexical = lexically_clean(current);
                let lexical_rel = relative_locator(canonical_root, &lexical).unwrap_or_default();
                let canonical_rel =
                    relative_locator(canonical_root, &canonical).unwrap_or_default();
                if lexical_rel != canonical_rel
                    && !equivalent_windows_drive_spelling(&lexical_rel, &canonical_rel)
                {
                    return Err(format!(
                        "repair attempt store {} is a symlink, junction, or case-fold alias of `{canonical_rel}`; store identity uses the concrete contained locator, and aliases are rejected",
                        display_path(cleaned)
                    ));
                }
                return Ok(());
            }
            Err(_) => {
                current = current.parent().ok_or_else(|| {
                    format!(
                        "repair attempt store {} has no existing ancestor under {}",
                        display_path(cleaned),
                        display_path(canonical_root)
                    )
                })?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("test clock failed: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-repair-store-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create {} failed: {error}", root.display()))?;
        root.canonicalize()
            .map_err(|error| format!("canonicalize {} failed: {error}", root.display()))
    }

    fn cleanup(root: &Path) {
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn default_store_is_the_repository_local_directory() -> Result<(), String> {
        let root = test_root("default")?;
        let result = (|| {
            let store = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Default,
                RepairAttemptStoreAccess::Open,
            )?;
            if !store.is_default()
                || store.locator() != REPAIR_ATTEMPT_DIRECTORY
                || store.currentness() != RepairAttemptStoreCurrentness::Missing
                || store.manifest_identity().is_some()
            {
                return Err(format!(
                    "default store was not the compatible locator: {store:?}"
                ));
            }
            if store.resolved_path() != root.join(REPAIR_ATTEMPT_DIRECTORY) {
                return Err(format!(
                    "default store resolved to {}",
                    store.resolved_path().display()
                ));
            }
            if store.limitations().is_empty() {
                return Err("default store omitted its non-claim limitations".to_string());
            }
            if store.filesystem_identity().is_some() {
                return Err("a missing default store invented a filesystem identity".to_string());
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn equivalent_default_spellings_share_one_identity() -> Result<(), String> {
        let root = test_root("equiv")?;
        let result = (|| {
            let prepared = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Default,
                RepairAttemptStoreAccess::Prepare,
            )?;
            let dotted = root.join("./target/ripr/repair-attempts");
            let absolute = prepared.resolved_path().to_path_buf();
            for locator in [
                RepairAttemptStoreLocator::Explicit(Path::new(REPAIR_ATTEMPT_DIRECTORY)),
                RepairAttemptStoreLocator::Explicit(Path::new("./target/ripr/repair-attempts")),
                RepairAttemptStoreLocator::Explicit(&dotted),
                RepairAttemptStoreLocator::Explicit(&absolute),
            ] {
                let store =
                    resolve_repair_attempt_store(&root, locator, RepairAttemptStoreAccess::Open)?;
                if !store.is_default() || store.identity() != prepared.identity() {
                    return Err(format!(
                        "equivalent spelling {:?} resolved as {:?}",
                        locator,
                        store.identity()
                    ));
                }
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn explicit_store_prepares_and_reopens_without_falling_back() -> Result<(), String> {
        let root = test_root("explicit")?;
        let result = (|| {
            let locator = Path::new("target/ripr/alt-attempts");
            let prepared = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(locator),
                RepairAttemptStoreAccess::Prepare,
            )?;
            if prepared.is_default()
                || prepared.locator() != "target/ripr/alt-attempts"
                || prepared.location_class() != RepairAttemptStoreLocationClass::ExplicitRepository
            {
                return Err(format!("explicit store lost its class: {prepared:?}"));
            }
            let reopened = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(locator),
                RepairAttemptStoreAccess::Open,
            )?;
            if reopened.identity() != prepared.identity() {
                return Err("reopened explicit store did not keep its identity".to_string());
            }
            if prepared.filesystem_identity().is_none() || reopened.limitations().is_empty() {
                return Err(
                    "present explicit store omitted access identity or limitations".to_string(),
                );
            }
            if prepared.identity().is_default() {
                return Err("explicit store identity claimed to be the default".to_string());
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn missing_explicit_store_does_not_fall_back_to_default() -> Result<(), String> {
        let root = test_root("missing-explicit")?;
        let result = (|| {
            let default = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Default,
                RepairAttemptStoreAccess::Prepare,
            )?;
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/missing-store")),
                RepairAttemptStoreAccess::Open,
            ) {
                Ok(store) => {
                    return Err(format!(
                        "missing explicit store opened as {}",
                        store.locator()
                    ));
                }
                Err(error) => error,
            };
            if !error.contains("does not fall back") || !error.contains("missing-store") {
                return Err(format!("missing explicit store error was opaque: {error}"));
            }
            if default.locator() != REPAIR_ATTEMPT_DIRECTORY {
                return Err(
                    "default store identity changed while refusing the explicit miss".to_string(),
                );
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn two_explicit_stores_stay_isolated() -> Result<(), String> {
        let root = test_root("two-stores")?;
        let result = (|| {
            let first = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/store-a")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            let second = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/store-b")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            if first.identity() == second.identity() {
                return Err("two explicit stores collapsed to one identity".to_string());
            }
            first
                .matches_manifest(second.manifest_identity().as_ref())
                .err()
                .ok_or_else(|| "store-b identity was accepted as store-a".to_string())?;
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn two_repositories_cannot_cross_resolve_the_same_locator() -> Result<(), String> {
        let first = test_root("repo-a")?;
        let second = test_root("repo-b")?;
        let result = (|| {
            let store_a = resolve_repair_attempt_store(
                &first,
                RepairAttemptStoreLocator::Default,
                RepairAttemptStoreAccess::Prepare,
            )?;
            let store_b = resolve_repair_attempt_store(
                &second,
                RepairAttemptStoreLocator::Default,
                RepairAttemptStoreAccess::Prepare,
            )?;
            if store_a.canonical_root() == store_b.canonical_root() {
                return Err("two repositories shared a canonical root".to_string());
            }
            if store_a.locator() != store_b.locator() {
                return Err("default locators diverged across repositories".to_string());
            }
            let foreign = resolve_repair_attempt_store(
                &first,
                RepairAttemptStoreLocator::Explicit(store_b.resolved_path()),
                RepairAttemptStoreAccess::Open,
            );
            match foreign {
                Err(error) if error.contains("not contained") => Ok(()),
                other => Err(format!(
                    "foreign repository store was not rejected: {other:?}"
                )),
            }
        })();
        cleanup(&first);
        cleanup(&second);
        result
    }

    #[test]
    fn foreign_cwd_still_resolves_against_the_selected_root() -> Result<(), String> {
        let root = test_root("foreign-cwd")?;
        let elsewhere = test_root("elsewhere")?;
        let previous = std::env::current_dir().map_err(|error| error.to_string())?;
        let result = (|| {
            std::env::set_current_dir(&elsewhere).map_err(|error| error.to_string())?;
            let store = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/cwd-store")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            if !store.resolved_path().starts_with(&root)
                || store.resolved_path().starts_with(&elsewhere)
            {
                return Err(format!(
                    "store resolved against CWD instead of root: {}",
                    store.resolved_path().display()
                ));
            }
            Ok(())
        })();
        let _ = std::env::set_current_dir(&previous);
        cleanup(&root);
        cleanup(&elsewhere);
        result
    }

    #[test]
    fn traversal_and_absolute_outside_root_reject() -> Result<(), String> {
        let root = test_root("escape")?;
        let result = (|| {
            for locator in [
                PathBuf::from("../escape"),
                PathBuf::from("target/ripr/../../escape"),
                std::env::temp_dir().join("ripr-outside-store"),
            ] {
                let error = match resolve_repair_attempt_store(
                    &root,
                    RepairAttemptStoreLocator::Explicit(&locator),
                    RepairAttemptStoreAccess::Prepare,
                ) {
                    Ok(store) => {
                        return Err(format!(
                            "escape locator {} opened as {}",
                            locator.display(),
                            store.locator()
                        ));
                    }
                    Err(error) => error,
                };
                if !error.contains("not contained")
                    && !error.contains("outside")
                    && !error.contains("traversal")
                {
                    return Err(format!(
                        "escape locator {} was not a containment failure: {error}",
                        locator.display()
                    ));
                }
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn spaces_and_non_ascii_round_trip() -> Result<(), String> {
        let root = test_root("unicode")?;
        let result = (|| {
            let locator = Path::new("target/ripr/store with spaces/попытка");
            let prepared = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(locator),
                RepairAttemptStoreAccess::Prepare,
            )?;
            if !prepared.locator().contains("store with spaces")
                || !prepared.locator().contains("попытка")
            {
                return Err(format!(
                    "unicode locator was rewritten: {}",
                    prepared.locator()
                ));
            }
            let reopened = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(locator),
                RepairAttemptStoreAccess::Open,
            )?;
            if reopened.identity() != prepared.identity() {
                return Err("unicode store identity did not round-trip".to_string());
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn windows_backslash_locator_is_one_identity() -> Result<(), String> {
        if !cfg!(windows) {
            let text = r"target\ripr\alt-attempts";
            // On Unix a backslash is a filename character, so this locator is
            // a different directory, not a separator alias.
            assert!(
                Path::new(text).to_string_lossy().contains('\\'),
                "unix backslash locator lost its filename character"
            );
            return Ok(());
        }
        let root = test_root("backslash")?;
        let result = (|| {
            let slash = Path::new("target/ripr/alt-attempts");
            let prepared = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(slash),
                RepairAttemptStoreAccess::Prepare,
            )?;
            let backslash = PathBuf::from(r"target\ripr\alt-attempts");
            let reopened = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(&backslash),
                RepairAttemptStoreAccess::Open,
            )?;
            if reopened.identity() != prepared.identity() {
                return Err("backslash spelling was a different store".to_string());
            }
            if reopened.locator().contains('\\') {
                return Err(format!(
                    "portable locator retained a backslash: {}",
                    reopened.locator()
                ));
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn unc_and_drive_relative_locators_fail_closed() -> Result<(), String> {
        assert!(is_unsupported_unc(r"\\server\share\attempts"));
        // Drive-letter prefixes are assembled at runtime: check-local-context
        // forbids a contiguous drive-letter path literal in tracked source.
        let drive_relative = format!("{}:attempts", 'C');
        let drive_absolute = format!("{}:\\attempts", 'C');
        assert!(is_drive_relative(&drive_relative));
        assert!(!is_drive_relative(&drive_absolute));
        assert!(!is_drive_relative("target/ripr/repair-attempts"));
        let error = match validate_explicit_locator(Path::new(r"\\server\share")) {
            Err(error) => error,
            Ok(()) => return Err("unc locator was accepted".to_string()),
        };
        if !error.contains("UNC") {
            return Err(format!("unc error was opaque: {error}"));
        }
        let drive = format!("{}:relative", 'D');
        let error = match validate_explicit_locator(Path::new(&drive)) {
            Err(error) => error,
            Ok(()) => return Err("drive-relative locator was accepted".to_string()),
        };
        if !error.contains("drive-relative") {
            return Err(format!("drive-relative error was opaque: {error}"));
        }
        Ok(())
    }

    #[test]
    fn empty_explicit_locator_is_rejected() -> Result<(), String> {
        let error = match validate_explicit_locator(Path::new("")) {
            Err(error) => error,
            Ok(()) => return Err("empty locator was accepted".to_string()),
        };
        if !error.contains("non-empty") {
            return Err(format!("empty locator error was opaque: {error}"));
        }
        let error = match validate_explicit_locator(Path::new("   ")) {
            Err(error) => error,
            Ok(()) => return Err("blank locator was accepted".to_string()),
        };
        if !error.contains("non-empty") {
            return Err(format!("blank locator error was opaque: {error}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_rejects() -> Result<(), String> {
        let root = test_root("symlink")?;
        let outside = test_root("symlink-outside")?;
        let result = (|| {
            let link = root.join("target/ripr/link-store");
            std::fs::create_dir_all(
                link.parent()
                    .ok_or_else(|| "missing symlink parent".to_string())?,
            )
            .map_err(|error| format!("create link parent failed: {error}"))?;
            std::os::unix::fs::symlink(&outside, &link)
                .map_err(|error| format!("create symlink failed: {error}"))?;
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/link-store")),
                RepairAttemptStoreAccess::Open,
            ) {
                Ok(store) => {
                    return Err(format!(
                        "symlink escape opened as {}",
                        store.resolved_path().display()
                    ));
                }
                Err(error) => error,
            };
            if !error.contains("symlink") && !error.contains("escapes") && !error.contains("alias")
            {
                return Err(format!("symlink escape error was opaque: {error}"));
            }
            Ok(())
        })();
        cleanup(&root);
        cleanup(&outside);
        result
    }

    #[test]
    fn default_manifest_identity_is_omitted() -> Result<(), String> {
        let root = test_root("omit")?;
        let result = (|| {
            let store = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Default,
                RepairAttemptStoreAccess::Prepare,
            )?;
            store.matches_manifest(None)?;
            if store.manifest_identity().is_some() {
                return Err("default store leaked a manifest store object".to_string());
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn a_file_is_not_a_store_directory() -> Result<(), String> {
        let root = test_root("file-store")?;
        let result = (|| {
            let locator = Path::new("target/ripr/not-a-dir");
            let path = root.join(locator);
            std::fs::create_dir_all(path.parent().ok_or_else(|| "missing parent".to_string())?)
                .map_err(|error| format!("create parent failed: {error}"))?;
            std::fs::write(&path, b"not a directory")
                .map_err(|error| format!("write {} failed: {error}", path.display()))?;
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(locator),
                RepairAttemptStoreAccess::Open,
            ) {
                Ok(store) => {
                    return Err(format!(
                        "file store opened as {}",
                        store.resolved_path().display()
                    ));
                }
                Err(error) => error,
            };
            if !error.contains("not a directory") {
                return Err(format!("file store error was opaque: {error}"));
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn the_repository_root_is_not_a_store() -> Result<(), String> {
        let root = test_root("root-store")?;
        let result = (|| {
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new(".")),
                RepairAttemptStoreAccess::Prepare,
            ) {
                Ok(store) => {
                    return Err(format!("repository root opened as {}", store.locator()));
                }
                Err(error) => error,
            };
            if !error.contains("cannot be the repository root") {
                return Err(format!("root store error was opaque: {error}"));
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn case_fold_spelling_does_not_silently_share_identity() -> Result<(), String> {
        let root = test_root("case-fold")?;
        let result = (|| {
            let prepared = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/Store")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            let folded = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/store")),
                RepairAttemptStoreAccess::Open,
            );
            match folded {
                Ok(store) if store.identity() == prepared.identity() => Err(
                    "case-fold spelling silently shared the prepared store identity".to_string(),
                ),
                Ok(store) => Err(format!(
                    "case-fold spelling opened a different present store {}",
                    store.locator()
                )),
                Err(error)
                    if error.contains("alias")
                        || error.contains("does not fall back")
                        || error.contains("missing") =>
                {
                    Ok(())
                }
                Err(error) => Err(format!("case-fold spelling error was opaque: {error}")),
            }
        })();
        cleanup(&root);
        result
    }

    #[cfg(unix)]
    #[test]
    fn in_tree_symlink_alias_rejects() -> Result<(), String> {
        let root = test_root("in-tree-alias")?;
        let result = (|| {
            let real = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/real-store")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            let link = root.join("target/ripr/alias-store");
            std::os::unix::fs::symlink(real.resolved_path(), &link)
                .map_err(|error| format!("create in-tree symlink failed: {error}"))?;
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/alias-store")),
                RepairAttemptStoreAccess::Open,
            ) {
                Ok(store) => {
                    return Err(format!(
                        "in-tree symlink alias opened as {}",
                        store.locator()
                    ));
                }
                Err(error) => error,
            };
            if !error.contains("symlink") && !error.contains("alias") && !error.contains("junction")
            {
                return Err(format!("in-tree alias error was opaque: {error}"));
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[cfg(unix)]
    #[test]
    fn missing_child_under_escaping_symlink_parent_is_refused_without_external_mutation()
    -> Result<(), String> {
        let root = test_root("escape-parent")?;
        let outside = test_root("escape-parent-outside")?;
        let result = (|| {
            let link = root.join("escape-link");
            std::os::unix::fs::symlink(&outside, &link)
                .map_err(|error| format!("create escaping parent symlink failed: {error}"))?;
            let outside_child = outside.join("child-store");
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("escape-link/child-store")),
                RepairAttemptStoreAccess::Prepare,
            ) {
                Ok(store) => {
                    return Err(format!(
                        "escaping parent symlink prepared as {}",
                        store.locator()
                    ));
                }
                Err(error) => error,
            };
            if !error.contains("symlink")
                && !error.contains("escape")
                && !error.contains("junction")
            {
                return Err(format!("escaping parent error was opaque: {error}"));
            }
            if outside_child.exists() {
                return Err("prepare mutated the escaped parent before refusing".to_string());
            }
            Ok(())
        })();
        cleanup(&root);
        cleanup(&outside);
        result
    }

    #[cfg(unix)]
    #[test]
    fn missing_child_under_in_tree_symlink_parent_is_refused() -> Result<(), String> {
        let root = test_root("alias-parent")?;
        let result = (|| {
            let real = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/real-store")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            let link = root.join("target/ripr/alias-parent");
            std::os::unix::fs::symlink(real.resolved_path(), &link)
                .map_err(|error| format!("create in-tree parent symlink failed: {error}"))?;
            let error = match resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new(
                    "target/ripr/alias-parent/child-store",
                )),
                RepairAttemptStoreAccess::Prepare,
            ) {
                Ok(store) => {
                    return Err(format!(
                        "in-tree parent alias prepared as {}",
                        store.locator()
                    ));
                }
                Err(error) => error,
            };
            if !error.contains("symlink") && !error.contains("alias") && !error.contains("junction")
            {
                return Err(format!("in-tree parent alias error was opaque: {error}"));
            }
            if real.resolved_path().join("child-store").exists() {
                return Err("prepare created a child through an in-tree alias".to_string());
            }
            Ok(())
        })();
        cleanup(&root);
        result
    }

    #[test]
    fn nested_store_schema_version_mismatch_is_refused() -> Result<(), String> {
        let root = test_root("store-schema")?;
        let result = (|| {
            let store = resolve_repair_attempt_store(
                &root,
                RepairAttemptStoreLocator::Explicit(Path::new("target/ripr/alt-attempts")),
                RepairAttemptStoreAccess::Prepare,
            )?;
            let mut declared = store.identity();
            declared.schema_version = "9.9".to_string();
            let error = match store.matches_manifest(Some(&declared)) {
                Ok(()) => return Err("nested store schema 9.9 was accepted".to_string()),
                Err(error) => error,
            };
            if !error.contains("0.1") || !error.contains("9.9") {
                return Err(format!("store schema error was opaque: {error}"));
            }
            store.matches_manifest(Some(&store.identity()))?;
            Ok(())
        })();
        cleanup(&root);
        result
    }
}
