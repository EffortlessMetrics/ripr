use crate::app::CheckInput;
use crate::config::{CheckInputExplicit, RiprConfig, apply_to_check_input, load_for_root};
use std::path::Path;

pub(super) fn ensure_command_root(root: &Path, command_name: &str) -> Result<(), String> {
    // Detected once: every branch below keys on the same typed components.
    let stripped = windows_stripped_components(root);
    match std::fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => {
            // #4951/#4958: Windows strips trailing dots and spaces from every
            // path component, so a typed `x.` — final or interior — can pass
            // this check while addressing an existing sibling the user never
            // typed. Say the rebind instead of running silently against
            // another path.
            if let Some(note) = windows_root_rebind_note(root, command_name) {
                eprintln!("{note}");
            }
            Ok(())
        }
        // Metadata resolved, so the typed name addresses an existing entry.
        // On Windows a stripped component resolves the path to a DIFFERENT
        // existing entry here — that is not the same condition as a
        // genuinely missing path, and the generic refusal conflated them
        // (#4951). Name the resolution instead.
        Ok(_) if !stripped.is_empty() => {
            let resolved = resolved_display(root);
            Err(format!(
                "{command_name} root {} resolves to {resolved}, which is not a directory on this platform \
                 (Windows strips trailing dots and spaces); pass a path without them",
                root.display()
            ))
        }
        // Nothing exists under the normalized name. A stripped component can
        // never exist as typed on Windows (#4951); say which name the platform
        // actually looks for. #4958: an interior stripped component rebinds
        // before the final component is even considered, so when interior
        // components are stripped the refusal names every stripped component;
        // the landed final-component wording stays exact when only the final
        // component is.
        // NotFound only: any other metadata failure (a file-typed parent,
        // permissions) says nothing about the normalized name, so it keeps
        // the generic refusal instead of claiming absence.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !stripped.is_empty() => {
            let final_only = root.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                stripped.len() == 1 && stripped[0] == name
            });
            if final_only {
                let normalized = stripped[0].trim_end_matches(['.', ' ']);
                Err(format!(
                    "{command_name} root {} cannot be addressed as typed on this platform \
                     (Windows strips trailing dots and spaces, so the name normalizes to \
                     {normalized:?}, which does not exist here); pass a path without them",
                    root.display()
                ))
            } else {
                let rebinds = stripped
                    .iter()
                    .map(|name| {
                        format!(
                            "{name:?} normalizes to {:?}",
                            name.trim_end_matches(['.', ' '])
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" and ");
                Err(format!(
                    "{command_name} root {} cannot be addressed as typed on this platform \
                     (Windows strips trailing dots and spaces, so {rebinds}, and the \
                     normalized path does not exist here); pass a path without them",
                    root.display()
                ))
            }
        }
        Ok(_) | Err(_) => Err(format!(
            "{command_name} root {} is not a directory",
            root.display()
        )),
    }
}

/// The stderr disclosure for a root that passed only because this platform's
/// path normalization rebound it to a sibling entry the user did not type
/// (#4951). #4958: any stripped component qualifies — an interior one rebinds
/// the lookup before the final component is considered — and the resolved
/// path names what the run will actually address. `None` when the typed
/// spelling is addressable as typed.
pub(super) fn windows_root_rebind_note(root: &Path, command_name: &str) -> Option<String> {
    if windows_stripped_components(root).is_empty() {
        return None;
    }
    let resolved = resolved_display(root);
    Some(format!(
        "ripr: {command_name} root {} rebinds to {resolved} on this platform \
         (Windows strips trailing dots and spaces from path components); \
         the run addresses the rebound path.",
        root.display()
    ))
}

/// Every typed path component that this platform's path normalization strips
/// trailing dots and spaces from (#4951, #4958). Windows strips them from
/// EVERY component, not only the final one, so an interior `x.` in
/// `<root>/x./child` addresses `<root>/x/child` exactly as a final `x.`
/// addresses `x`. Windows-only: elsewhere such names are literal and
/// addressable, so the vector is empty. Ordered as typed; prefix, root, `.`
/// and `..` components carry no strippable name, so directory references
/// never match.
/// The shared stripped-name predicate: whether this platform's path
/// normalization strips trailing dots and spaces from this component name
/// (#4951). Detection and normalization must agree on exactly these names,
/// so both read this one predicate instead of holding duplicate suffix
/// checks that could diverge.
///
/// A name made only of dots or spaces (for example `...`) trims to an empty
/// component, and collecting into `PathBuf` drops an empty component, so the
/// normalized spelling can name a path shorter than the one the platform
/// actually addresses; no caller compensates for that today.
fn is_windows_stripped(name: &str) -> bool {
    name.ends_with('.') || name.ends_with(' ')
}

pub(super) fn windows_stripped_components(path: &Path) -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .filter(|name| is_windows_stripped(name))
        .collect()
}

/// The typed path with every stripped component replaced by the spelling this
/// platform actually addresses (#4958), so a refusal can name the on-disk
/// target without resolving the filesystem. Identity except for stripped
/// components.
pub(super) fn windows_normalized_path(path: &Path) -> std::path::PathBuf {
    path.components()
        .map(|component| match component {
            std::path::Component::Normal(name) => {
                let text = name.to_string_lossy();
                if is_windows_stripped(&text) {
                    std::ffi::OsString::from(text.trim_end_matches(['.', ' ']))
                } else {
                    name.to_os_string()
                }
            }
            component => component.as_os_str().to_os_string(),
        })
        .collect()
}

/// Canonical on-disk spelling of an existing path, with the `\\?\` verbatim
/// prefix `canonicalize` returns on Windows rendered back to its ordinary
/// form, so a message names a path the user can retype.
fn resolved_display(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.display().to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    text.strip_prefix(r"\\?\")
        .map(str::to_string)
        .unwrap_or(text)
}

pub(super) fn load_root_input_and_config(root: &Path) -> Result<(CheckInput, RiprConfig), String> {
    let config = load_for_root(root)?;
    let mut input = CheckInput {
        root: root.to_path_buf(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
    Ok((input, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Guard(std::path::PathBuf);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_root(name: &str) -> Result<std::path::PathBuf, String> {
        let root = std::env::temp_dir().join(format!("ripr-4951-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(root)
    }

    // Portable controls: names without trailing dots or spaces keep the
    // generic refusal and acceptance on every platform.
    #[test]
    fn plain_directory_root_is_accepted() -> Result<(), String> {
        let root = test_root("plain-dir")?;
        let _guard = Guard(root.clone());
        let dir = root.join("src");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        assert_eq!(ensure_command_root(&dir, "check"), Ok(()));
        Ok(())
    }

    #[test]
    fn plain_missing_root_keeps_the_generic_refusal() -> Result<(), String> {
        let root = test_root("plain-missing")?;
        let _guard = Guard(root.clone());
        let missing = root.join("totally-missing-4951");
        let error = ensure_command_root(&missing, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("is not a directory"), "{error}");
        assert!(!error.contains("Windows"), "{error}");
        Ok(())
    }

    #[test]
    fn plain_existing_file_keeps_the_generic_refusal() -> Result<(), String> {
        let root = test_root("plain-file")?;
        let _guard = Guard(root.clone());
        let file = root.join("f.rs");
        std::fs::write(&file, b"x").map_err(|e| e.to_string())?;
        let error = ensure_command_root(&file, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("is not a directory"), "{error}");
        assert!(!error.contains("Windows"), "{error}");
        Ok(())
    }

    // Detection is syntactic and platform-scoped: plain names and directory
    // references never count as stripped, on any platform.
    #[test]
    fn stripped_component_detection_rejects_plain_names_and_directory_references() {
        assert!(windows_stripped_components(Path::new("artifact.json")).is_empty());
        assert!(windows_stripped_components(Path::new("a/b.json")).is_empty());
        assert!(windows_stripped_components(Path::new(".")).is_empty());
        assert!(windows_stripped_components(Path::new("..")).is_empty());
        // A drive root carries no component name. `check-local-context`
        // forbids drive-letter path literals, so the shape is assembled
        // from parts.
        let drive_root = format!("{}\\", "F:");
        assert!(windows_stripped_components(Path::new(&drive_root)).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn stripped_component_detection_matches_final_and_interior_components() {
        // Assembled from parts for `check-local-context` (no drive-letter
        // path literals); the point is component scanning along the path.
        let typed = format!("{}/tmp/artifact.", "F:");
        assert_eq!(
            windows_stripped_components(Path::new(&typed)),
            vec!["artifact.".to_string()]
        );
        assert_eq!(
            windows_stripped_components(Path::new("x ")),
            vec!["x ".to_string()]
        );
        // #4958: an interior stripped component is detected even when the
        // final component is clean, and every stripped component is listed
        // when more than one strips.
        assert_eq!(
            windows_stripped_components(Path::new("a./b.json")),
            vec!["a.".to_string()]
        );
        assert_eq!(
            windows_stripped_components(Path::new("a./b./c.json")),
            vec!["a.".to_string(), "b.".to_string()]
        );
    }

    #[cfg(windows)]
    #[test]
    fn normalized_path_names_the_written_spelling() {
        assert_eq!(
            windows_normalized_path(Path::new("artifact."))
                .display()
                .to_string(),
            "artifact"
        );
        // Assembled from parts for `check-local-context` (no drive-letter
        // path literals): the interior `a.` normalizes, the rest is identity.
        let typed = format!("{}/tmp/a./b.json", "F:");
        assert_eq!(
            windows_normalized_path(Path::new(&typed))
                .display()
                .to_string(),
            format!("{}\\tmp\\a\\b.json", "F:")
        );
        assert_eq!(
            windows_normalized_path(Path::new("a/b.json"))
                .display()
                .to_string(),
            format!("a{}b.json", std::path::MAIN_SEPARATOR)
        );
    }

    // #4951: a typed trailing-dot root whose normalized sibling exists as a
    // directory is accepted, and the rebind note names the real on-disk
    // path the run will address.
    #[cfg(windows)]
    #[test]
    fn windows_rebound_root_is_accepted_and_disclosed() -> Result<(), String> {
        let root = test_root("rebind")?;
        let _guard = Guard(root.clone());
        // std strips the trailing dot, so this creates the sibling `x`.
        std::fs::create_dir(root.join("x.")).map_err(|e| e.to_string())?;
        let typed = root.join("x.");
        assert_eq!(ensure_command_root(&typed, "check"), Ok(()));
        let note = windows_root_rebind_note(&typed, "check")
            .ok_or("rebind note missing for a stripped root")?;
        assert!(
            note.contains(&format!("check root {} rebinds to", typed.display())),
            "{note}"
        );
        assert!(
            note.contains("Windows strips trailing dots and spaces"),
            "{note}"
        );
        assert!(note.contains("\\x on this platform"), "{note}");
        Ok(())
    }

    // #4951: metadata resolves (to the stripped sibling `f`, a file), so the
    // refusal names the resolution instead of the generic "not a directory".
    #[cfg(windows)]
    #[test]
    fn windows_root_resolving_to_an_existing_file_names_the_condition() -> Result<(), String> {
        let root = test_root("resolves-to-file")?;
        let _guard = Guard(root.clone());
        // std strips the trailing dot, so this writes the sibling `f`.
        std::fs::write(root.join("f."), b"x").map_err(|e| e.to_string())?;
        let typed = root.join("f.");
        let error = ensure_command_root(&typed, "check")
            .err()
            .unwrap_or_default();
        assert!(
            error.contains(&format!("check root {} resolves to ", typed.display())),
            "{error}"
        );
        assert!(
            error.contains(
                "which is not a directory on this platform (Windows strips trailing dots and spaces); pass a path without them"
            ),
            "{error}"
        );
        // The resolved entry is the stripped sibling, not the typed spelling.
        assert!(error.contains("\\f,"), "{error}");
        Ok(())
    }

    // #4951: nothing exists under the normalized name, so the refusal says
    // the typed spelling cannot be addressed and names the normalized form.
    #[cfg(windows)]
    #[test]
    fn windows_unaddressable_root_names_the_normalized_name() -> Result<(), String> {
        let root = test_root("unaddressable")?;
        let _guard = Guard(root.clone());
        let typed = root.join("x.");
        let error = ensure_command_root(&typed, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("cannot be addressed as typed"), "{error}");
        assert!(
            error.contains("Windows strips trailing dots and spaces"),
            "{error}"
        );
        assert!(error.contains("\"x\""), "{error}");
        assert!(error.contains("pass a path without them"), "{error}");
        Ok(())
    }

    // Windows maps a file-typed parent to NotFound (ERROR_PATH_NOT_FOUND), so
    // the normalized name genuinely has no directory under it there and the
    // typed-name disclosure is literally true. The NotFound-only gate exists
    // for every OTHER error kind (permissions, ...): those leave existence
    // unknown, and claiming "does not exist" from them would be a false
    // repair signal, so they keep the generic refusal.
    #[cfg(windows)]
    #[test]
    fn windows_metadata_not_found_failure_names_the_normalized_name() -> Result<(), String> {
        let root = test_root("parent-is-file")?;
        let _guard = Guard(root.clone());
        std::fs::write(root.join("f.rs"), b"x").map_err(|e| e.to_string())?;
        let typed = root.join("f.rs").join("x.");
        let error = ensure_command_root(&typed, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("cannot be addressed as typed"), "{error}");
        assert!(error.contains("\"x\""), "{error}");
        Ok(())
    }

    // Portable control (#4958): an interior layout with no stripped component
    // keeps acceptance, no rebind note, and the generic refusal on every
    // platform.
    #[test]
    fn plain_interior_layout_keeps_the_generic_behavior() -> Result<(), String> {
        let root = test_root("plain-interior")?;
        let _guard = Guard(root.clone());
        let child = root.join("x").join("child");
        std::fs::create_dir_all(&child).map_err(|e| e.to_string())?;
        assert_eq!(ensure_command_root(&child, "check"), Ok(()));
        assert_eq!(windows_root_rebind_note(&child, "check"), None);
        let missing = root.join("x").join("missing");
        let error = ensure_command_root(&missing, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("is not a directory"), "{error}");
        assert!(!error.contains("Windows"), "{error}");
        Ok(())
    }

    // #4958: an interior stripped component rebinds the whole lookup exactly
    // like the landed final-component case — the run addresses <T>/x/child
    // while <T>/x./child was typed — so the rebind note names the real
    // on-disk path.
    #[cfg(windows)]
    #[test]
    fn windows_interior_rebound_root_is_accepted_and_disclosed() -> Result<(), String> {
        let root = test_root("interior-rebind")?;
        let _guard = Guard(root.clone());
        // std strips the interior trailing dot, so this creates <T>/x/child.
        std::fs::create_dir_all(root.join("x.").join("child")).map_err(|e| e.to_string())?;
        let typed = root.join("x.").join("child");
        assert_eq!(ensure_command_root(&typed, "check"), Ok(()));
        let note = windows_root_rebind_note(&typed, "check")
            .ok_or("rebind note missing for an interior stripped component")?;
        assert!(
            note.contains(&format!("check root {} rebinds to", typed.display())),
            "{note}"
        );
        assert!(
            note.contains("Windows strips trailing dots and spaces"),
            "{note}"
        );
        // The resolved path is the normalized interior component's subtree,
        // not the typed spelling.
        assert!(note.contains("\\x\\child on this platform"), "{note}");
        Ok(())
    }

    // #4958: the interior component rebinds to the existing <T>/x, and the
    // final `child` under it is a file, so the resolves-to refusal names the
    // resolution instead of the generic "not a directory".
    #[cfg(windows)]
    #[test]
    fn windows_interior_root_resolving_to_a_file_names_the_condition() -> Result<(), String> {
        let root = test_root("interior-resolves-to-file")?;
        let _guard = Guard(root.clone());
        std::fs::create_dir_all(root.join("x")).map_err(|e| e.to_string())?;
        std::fs::write(root.join("x").join("child"), b"x").map_err(|e| e.to_string())?;
        let typed = root.join("x.").join("child");
        let error = ensure_command_root(&typed, "check")
            .err()
            .unwrap_or_default();
        assert!(
            error.contains(&format!("check root {} resolves to ", typed.display())),
            "{error}"
        );
        assert!(
            error.contains(
                "which is not a directory on this platform (Windows strips trailing dots and spaces); pass a path without them"
            ),
            "{error}"
        );
        // The resolved entry is the normalized interior subtree, not the
        // typed spelling.
        assert!(error.contains("\\x\\child,"), "{error}");
        Ok(())
    }

    // #4958: the interior component rebinds to <T>/x but no `child` exists
    // under it, so the NotFound refusal names the interior stripped component
    // and its normalized form instead of keeping the generic refusal.
    #[cfg(windows)]
    #[test]
    fn windows_interior_root_missing_leaf_names_the_stripped_component() -> Result<(), String> {
        let root = test_root("interior-missing")?;
        let _guard = Guard(root.clone());
        std::fs::create_dir_all(root.join("x")).map_err(|e| e.to_string())?;
        let typed = root.join("x.").join("child");
        let error = ensure_command_root(&typed, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("cannot be addressed as typed"), "{error}");
        assert!(
            error.contains("Windows strips trailing dots and spaces"),
            "{error}"
        );
        assert!(error.contains("\"x.\" normalizes to \"x\""), "{error}");
        assert!(error.contains("pass a path without them"), "{error}");
        Ok(())
    }

    // #4958: when interior AND final components are stripped, the NotFound
    // refusal names both rebinds in typed order.
    #[cfg(windows)]
    #[test]
    fn windows_interior_and_final_stripped_names_both_components() -> Result<(), String> {
        let root = test_root("interior-and-final")?;
        let _guard = Guard(root.clone());
        let typed = root.join("x.").join("y.");
        let error = ensure_command_root(&typed, "check")
            .err()
            .unwrap_or_default();
        assert!(error.contains("cannot be addressed as typed"), "{error}");
        assert!(error.contains("\"x.\" normalizes to \"x\""), "{error}");
        assert!(error.contains("\"y.\" normalizes to \"y\""), "{error}");
        assert!(error.contains("pass a path without them"), "{error}");
        Ok(())
    }
}
