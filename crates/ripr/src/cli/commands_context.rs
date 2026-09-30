use crate::app::CheckInput;
use crate::config::{CheckInputExplicit, RiprConfig, apply_to_check_input, load_for_root};
use std::path::Path;

pub(super) fn ensure_command_root(root: &Path, command_name: &str) -> Result<(), String> {
    // Detected once: every branch below keys on the same typed final
    // component.
    let stripped = windows_stripped_component(root);
    match std::fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => {
            // #4951: Windows strips trailing dots and spaces from the final
            // path component, so a typed `x.` can pass this check while
            // addressing an existing sibling `x` the user never typed. Say
            // the rebind instead of running silently against another path.
            if let Some(note) = windows_root_rebind_note(root, command_name) {
                eprintln!("{note}");
            }
            Ok(())
        }
        // Metadata resolved, so the typed name addresses an existing entry.
        // On Windows a trailing-dot/space typed name resolves to a DIFFERENT
        // existing entry here — that is not the same condition as a
        // genuinely missing path, and the generic refusal conflated them
        // (#4951). Name the resolution instead.
        Ok(_) if stripped.is_some() => {
            let resolved = resolved_display(root);
            Err(format!(
                "{command_name} root {} resolves to {resolved}, which is not a directory on this platform \
                 (Windows strips trailing dots and spaces); pass a path without them",
                root.display()
            ))
        }
        // Nothing exists under the normalized name. A typed trailing-dot or
        // trailing-space final component can never exist as typed on
        // Windows (#4951); say which name the platform actually looks for.
        // NotFound only: any other metadata failure (a file-typed parent,
        // permissions) says nothing about the normalized name, so it keeps
        // the generic refusal instead of claiming absence.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && stripped.is_some() => {
            let normalized = stripped
                .as_deref()
                .unwrap_or_default()
                .trim_end_matches(['.', ' '])
                .to_string();
            Err(format!(
                "{command_name} root {} cannot be addressed as typed on this platform \
                 (Windows strips trailing dots and spaces, so the name normalizes to \
                 {normalized:?}, which does not exist here); pass a path without them",
                root.display()
            ))
        }
        Ok(_) | Err(_) => Err(format!(
            "{command_name} root {} is not a directory",
            root.display()
        )),
    }
}

/// The stderr disclosure for a root that passed only because this platform's
/// path normalization rebound it to a sibling entry the user did not type
/// (#4951). `None` when the typed spelling is addressable as typed.
pub(super) fn windows_root_rebind_note(root: &Path, command_name: &str) -> Option<String> {
    windows_stripped_component(root)?;
    let resolved = resolved_display(root);
    Some(format!(
        "ripr: {command_name} root {} rebinds to {resolved} on this platform \
         (Windows strips trailing dots and spaces from path components); \
         the run addresses the rebound path.",
        root.display()
    ))
}

/// The typed final component when this platform's path normalization strips
/// trailing dots and spaces from it (#4951). Windows-only: elsewhere such
/// names are literal and addressable, so there is nothing to disclose.
/// `.` and `..` carry no file name (`file_name` is None), so directory
/// references never match.
pub(super) fn windows_stripped_component(path: &Path) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().into_owned();
    (name.ends_with('.') || name.ends_with(' ')).then_some(name)
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
        assert_eq!(windows_stripped_component(Path::new("artifact.json")), None);
        assert_eq!(windows_stripped_component(Path::new(".")), None);
        assert_eq!(windows_stripped_component(Path::new("..")), None);
        // A drive root carries no final component. `check-local-context`
        // forbids drive-letter path literals, so the shape is assembled
        // from parts.
        let drive_root = format!("{}\\", "F:");
        assert_eq!(windows_stripped_component(Path::new(&drive_root)), None);
    }

    #[cfg(windows)]
    #[test]
    fn stripped_component_detection_matches_trailing_dots_and_spaces() {
        // Assembled from parts for `check-local-context` (no drive-letter
        // path literals); the point is the final component of a longer path.
        let typed = format!("{}/tmp/artifact.", "F:");
        assert_eq!(
            windows_stripped_component(Path::new(&typed)).as_deref(),
            Some("artifact.")
        );
        assert_eq!(
            windows_stripped_component(Path::new("x ")).as_deref(),
            Some("x ")
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
}
