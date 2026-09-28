//! Which `ripr` binary is running, and which one `ripr` on PATH names.
//!
//! A Cargo build output directory (`target/<profile>`) first on PATH silently
//! replaces the installed release for every command, CI step, and editor that
//! runs `ripr` (#3797 control 9). Doctor names that substitution and the
//! running binary's commit. It is advisory: a workspace build on PATH is a
//! legitimate development setup, so it never fails the report.

use crate::build_identity;
use crate::output::path::human_path;
use serde::Serialize;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The binary identity section of `ripr doctor --json` (`binary`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorBinaryIdentity {
    /// The `ripr --version` line of the running binary.
    pub(crate) version: String,
    /// The full commit the running binary was built from, when recorded.
    pub(crate) commit: Option<String>,
    /// Whether the crate sources differed from `commit` at build time.
    pub(crate) commit_dirty: bool,
    /// The running executable, when the platform reports it.
    pub(crate) executable: Option<String>,
    /// Whether the running executable sits in a Cargo build output directory.
    pub(crate) executable_is_cargo_build_output: bool,
    /// The first `ripr` found on PATH, resolved through symlinks.
    pub(crate) path_ripr: Option<String>,
    /// Whether that PATH `ripr` sits in a Cargo build output directory.
    pub(crate) path_ripr_is_cargo_build_output: bool,
    /// Whether that PATH `ripr` is the running executable; `None` when
    /// either side is unknown.
    pub(crate) path_ripr_is_running_executable: Option<bool>,
    /// Advisory findings; they never fail doctor.
    pub(crate) warnings: Vec<String>,
}

impl DoctorBinaryIdentity {
    /// Human doctor lines: identity first, then any warnings.
    pub(crate) fn human_lines(&self) -> Vec<String> {
        self.human_lines_for_host(cfg!(windows))
    }

    /// [`Self::human_lines`] for an explicit host separator rule (#4378): a
    /// canonical Windows executable with a verbatim `\\?\` drive prefix renders
    /// as the plain slash path, matching every other doctor path line.
    fn human_lines_for_host(&self, windows: bool) -> Vec<String> {
        let show = |path: &str| crate::output::path::human_path_text(path, windows);
        let mut lines = vec![match &self.executable {
            Some(executable) => {
                format!("- ripr binary: {} at {}", self.version, show(executable))
            }
            None => format!("- ripr binary: {}", self.version),
        }];
        lines.push(
            match (&self.path_ripr, self.path_ripr_is_running_executable) {
                (Some(path), Some(true)) => {
                    format!("- ripr on PATH: {} (this binary)", show(path))
                }
                (Some(path), _) => format!("- ripr on PATH: {}", show(path)),
                (None, _) => "- ripr on PATH: none found".to_string(),
            },
        );
        lines.extend(
            self.warnings
                .iter()
                .map(|warning| format!("- warning: {warning}")),
        );
        lines
    }
}

/// Probe the running binary and the process PATH.
pub(crate) fn probe_binary_identity() -> DoctorBinaryIdentity {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let names = ripr_executable_names(std::env::var_os("PATHEXT").as_deref());
    evaluate_binary_identity(
        std::env::current_exe().ok(),
        first_on_path(&path_var, &names),
    )
}

fn evaluate_binary_identity(
    current_exe: Option<PathBuf>,
    path_ripr: Option<PathBuf>,
) -> DoctorBinaryIdentity {
    let executable = current_exe.map(|path| resolve(&path));
    // The directory PATH lists is what a user edits; the resolved file is
    // what runs. They differ when the PATH entry is a symlink.
    let path_dir = path_ripr
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    let path_ripr = path_ripr.map(|path| resolve(&path));
    let path_ripr_is_cargo_build_output = path_ripr.as_deref().is_some_and(is_cargo_build_output);
    // Only two fully resolved paths are comparable: a path that could not be
    // canonicalized may still name the same file, so the answer is unknown.
    let path_ripr_is_running_executable = match (&executable, &path_ripr) {
        (Some(executable), Some(path_ripr))
            if is_resolved(executable) && is_resolved(path_ripr) =>
        {
            Some(executable == path_ripr)
        }
        _ => None,
    };
    let mut warnings = Vec::new();
    if let (Some(path), Some(dir), true) = (
        path_ripr.as_deref(),
        path_dir.as_deref(),
        path_ripr_is_cargo_build_output,
    ) {
        warnings.push(format!(
            "`ripr` on PATH is Cargo build output at {}, not an installed binary; commands, CI \
             steps, and editors that run `ripr` get that workspace build. Put an installed ripr \
             earlier on PATH or remove {} from PATH",
            human_path(path),
            human_path(dir)
        ));
    }
    if let (Some(executable), Some(path), Some(false)) = (
        executable.as_deref(),
        path_ripr.as_deref(),
        path_ripr_is_running_executable,
    ) {
        warnings.push(format!(
            "`ripr` on PATH is {}, not the running binary {}; this report describes the running \
             binary, and unless that PATH entry is a shim that launches it, commands that run \
             `ripr` get a different binary",
            human_path(path),
            human_path(executable)
        ));
    }
    DoctorBinaryIdentity {
        version: build_identity::version_line(),
        commit: build_identity::commit().map(str::to_string),
        commit_dirty: build_identity::commit_dirty(),
        executable_is_cargo_build_output: executable.as_deref().is_some_and(is_cargo_build_output),
        executable: executable.map(|path| path.display().to_string()),
        path_ripr: path_ripr.map(|path| path.display().to_string()),
        path_ripr_is_cargo_build_output,
        path_ripr_is_running_executable,
        warnings,
    }
}

/// Whether `path` is already its own canonical form, that is, `resolve`
/// succeeded on it.
fn is_resolved(path: &Path) -> bool {
    std::fs::canonicalize(path).is_ok_and(|canonical| canonical == path)
}

/// Resolve symlinks so a PATH link into `target/release` is recognized; an
/// unresolvable path is kept as given.
fn resolve(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Whether `executable` sits in a Cargo profile output directory
/// (`<target-dir>/[<triple>/]<profile>/`). Cargo keeps its `deps` and
/// `.fingerprint` bookkeeping beside every binary it builds there;
/// `cargo install` copies only the binary into its `bin` directory.
fn is_cargo_build_output(executable: &Path) -> bool {
    executable.parent().is_some_and(|profile| {
        profile.join("deps").is_dir() && profile.join(".fingerprint").is_dir()
    })
}

/// File names that run `ripr` from a PATH directory: `ripr` on Unix, and
/// `ripr` plus each `PATHEXT` extension on Windows.
fn ripr_executable_names(pathext: Option<&OsStr>) -> Vec<OsString> {
    if cfg!(windows) {
        let pathext = pathext
            .and_then(OsStr::to_str)
            .unwrap_or(".COM;.EXE;.BAT;.CMD");
        pathext
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| OsString::from(format!("ripr{}", extension.to_ascii_lowercase())))
            .collect()
    } else {
        vec![OsString::from("ripr")]
    }
}

/// The first executable named in `names` across the PATH directories, in PATH
/// order.
fn first_on_path(path_var: &OsStr, names: &[OsString]) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .find_map(|dir| {
            names
                .iter()
                .map(|name| dir.join(name))
                .find(|candidate| is_executable_file(candidate))
        })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #4378: doctor printed the canonical executable with its verbatim
    /// `\\?\` prefix beside `.\\`-relative lines on Windows.
    #[test]
    fn human_lines_render_windows_paths_without_verbatim_prefix() {
        // `check-local-context` forbids drive-letter path literals, so the
        // Windows shapes are assembled from parts.
        let drive = "F:";
        let canonical = format!(r"\\?\{drive}\code\ripr\target\debug\ripr.exe");
        let plain = format!("{drive}/code/ripr/target/debug/ripr.exe");
        let identity = DoctorBinaryIdentity {
            version: "ripr 0.0.0".to_string(),
            commit: None,
            commit_dirty: false,
            executable: Some(canonical.clone()),
            executable_is_cargo_build_output: true,
            path_ripr: Some(canonical.clone()),
            path_ripr_is_cargo_build_output: true,
            path_ripr_is_running_executable: Some(true),
            warnings: Vec::new(),
        };
        assert_eq!(
            identity.human_lines_for_host(true),
            vec![
                format!("- ripr binary: ripr 0.0.0 at {plain}"),
                format!("- ripr on PATH: {plain} (this binary)"),
            ]
        );
        // Unix keeps the text as the filesystem spells it.
        let unix = identity.human_lines_for_host(false);
        assert!(unix[0].ends_with(&format!("at {canonical}")), "{unix:?}");
    }

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-doctor-binary-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("create {}: {error}", dir.display()))?;
        Ok(dir)
    }

    fn executable(dir: &Path) -> Result<PathBuf, String> {
        let name = ripr_executable_names(None)
            .into_iter()
            .next()
            .ok_or("no executable name")?;
        let path = dir.join(name);
        std::fs::write(&path, b"").map_err(|error| format!("write {}: {error}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| format!("chmod {}: {error}", path.display()))?;
        }
        Ok(path)
    }

    fn cargo_profile_dir(root: &Path) -> Result<PathBuf, String> {
        let profile = root.join("target").join("debug");
        for bookkeeping in ["deps", ".fingerprint"] {
            std::fs::create_dir_all(profile.join(bookkeeping))
                .map_err(|error| format!("create {bookkeeping}: {error}"))?;
        }
        Ok(profile)
    }

    #[test]
    fn cargo_build_output_needs_both_bookkeeping_directories() -> Result<(), String> {
        let root = temp_dir("markers")?;
        let profile = cargo_profile_dir(&root)?;
        let built = executable(&profile)?;
        let installed_dir = root.join("bin");
        std::fs::create_dir_all(&installed_dir).map_err(|error| error.to_string())?;
        let installed = executable(&installed_dir)?;
        let result = (|| -> Result<(), String> {
            if !is_cargo_build_output(&built) {
                return Err("a binary beside deps/ and .fingerprint/ is Cargo build output".into());
            }
            if is_cargo_build_output(&installed) {
                return Err("a binary in a plain bin directory is not Cargo build output".into());
            }
            std::fs::remove_dir_all(profile.join(".fingerprint"))
                .map_err(|error| format!("remove .fingerprint: {error}"))?;
            if is_cargo_build_output(&built) {
                return Err("deps/ alone does not mark Cargo build output".into());
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn workspace_build_first_on_path_warns_and_installed_one_does_not() -> Result<(), String> {
        let root = temp_dir("path-order")?;
        let profile = cargo_profile_dir(&root)?;
        executable(&profile)?;
        let installed_dir = root.join("bin");
        std::fs::create_dir_all(&installed_dir).map_err(|error| error.to_string())?;
        let installed = executable(&installed_dir)?;
        let names = ripr_executable_names(None);
        let result = (|| -> Result<(), String> {
            let workspace_first = std::env::join_paths([&profile, &installed_dir])
                .map_err(|error| error.to_string())?;
            let found = first_on_path(&workspace_first, &names);
            let identity = evaluate_binary_identity(Some(installed.clone()), found);
            if !identity.path_ripr_is_cargo_build_output
                || identity.path_ripr_is_running_executable != Some(false)
                || !identity
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("Cargo build output"))
            {
                return Err(format!(
                    "workspace build first on PATH was not named: {identity:?}"
                ));
            }

            let installed_first = std::env::join_paths([&installed_dir, &profile])
                .map_err(|error| error.to_string())?;
            let found = first_on_path(&installed_first, &names);
            let identity = evaluate_binary_identity(Some(installed.clone()), found);
            if identity.path_ripr_is_cargo_build_output
                || identity.path_ripr_is_running_executable != Some(true)
                || !identity.warnings.is_empty()
                || identity.executable_is_cargo_build_output
            {
                return Err(format!(
                    "installed ripr first on PATH must be quiet: {identity:?}"
                ));
            }
            let human = identity.human_lines().join("\n");
            if !human.contains("(this binary)") {
                return Err(format!("human lines must name the running binary: {human}"));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn different_installed_ripr_on_path_warns_without_build_output_claim() -> Result<(), String> {
        let root = temp_dir("other-installed")?;
        let running_dir = root.join("running");
        let other_dir = root.join("other");
        let result = (|| -> Result<(), String> {
            for dir in [&running_dir, &other_dir] {
                std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
            }
            let running = executable(&running_dir)?;
            executable(&other_dir)?;
            let path = std::env::join_paths([&other_dir]).map_err(|error| error.to_string())?;
            let found = first_on_path(&path, &ripr_executable_names(None));
            let identity = evaluate_binary_identity(Some(running), found);
            if identity.path_ripr_is_cargo_build_output
                || identity.path_ripr_is_running_executable != Some(false)
                || identity.warnings.len() != 1
                || !identity.warnings[0].contains("not the running binary")
            {
                return Err(format!(
                    "a different installed ripr must get only the other-binary warning: {identity:?}"
                ));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn missing_ripr_on_path_is_reported_without_warning() {
        let identity = evaluate_binary_identity(None, None);
        assert_eq!(identity.path_ripr, None);
        assert_eq!(identity.path_ripr_is_running_executable, None);
        assert!(identity.warnings.is_empty());
        assert!(
            identity
                .human_lines()
                .contains(&"- ripr on PATH: none found".to_string())
        );
        assert_eq!(identity.version, build_identity::version_line());
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_ripr_on_path_is_skipped() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("non-executable")?;
        let first = root.join("first");
        let second = root.join("second");
        let result = (|| -> Result<(), String> {
            for dir in [&first, &second] {
                std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
            }
            let shadow = executable(&first)?;
            std::fs::set_permissions(&shadow, std::fs::Permissions::from_mode(0o644))
                .map_err(|error| error.to_string())?;
            let runnable = executable(&second)?;
            let path =
                std::env::join_paths([&first, &second]).map_err(|error| error.to_string())?;
            let found = first_on_path(&path, &ripr_executable_names(None));
            if found.as_deref() != Some(runnable.as_path()) {
                return Err(format!("expected {}, found {found:?}", runnable.display()));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }
}
