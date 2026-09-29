//! PATH and environment isolation oracles for the #4626 consumer journey.
//!
//! A clean consumer cannot see Cargo, rustc, a source checkout, or an ambient
//! `ripr`. A planted executable on PATH must not be selected while the wheel
//! payload is installed, and must not be deleted by uninstall.

use std::path::{Path, PathBuf};

const FORBIDDEN_TOOLS: [&str; 2] = ["cargo", "rustc"];

pub(crate) fn resolve_on_path(path_env: &str, name: &str) -> Option<PathBuf> {
    path_env.split(':').find_map(|entry| {
        if entry.is_empty() {
            return None;
        }
        let candidate = Path::new(entry).join(name);
        candidate.is_file().then_some(candidate)
    })
}

pub(crate) fn require_clean_consumer_path(
    path_env: &str,
    checkout: Option<&Path>,
) -> Result<(), String> {
    for tool in FORBIDDEN_TOOLS {
        if let Some(found) = resolve_on_path(path_env, tool) {
            return Err(format!(
                "clean consumer PATH resolved forbidden `{tool}` at {}; Rust toolchain must be unreachable",
                found.display()
            ));
        }
    }
    if let Some(found) = resolve_on_path(path_env, "ripr") {
        return Err(format!(
            "clean consumer PATH already resolves `ripr` at {}; ambient ripr must be unreachable before install",
            found.display()
        ));
    }
    if let Some(checkout) = checkout {
        let cargo_toml = checkout.join("Cargo.toml");
        if cargo_toml.is_file() {
            return Err(format!(
                "consumer still has a source checkout at {}; the installed journey must not analyze from the ripr tree",
                checkout.display()
            ));
        }
    }
    Ok(())
}

pub(crate) fn require_installed_beats_planted(
    path_env: &str,
    installed: &Path,
    planted: &Path,
    planted_sentinel: &Path,
) -> Result<(), String> {
    let resolved = resolve_on_path(path_env, "ripr").ok_or_else(|| {
        "installed consumer PATH does not resolve `ripr` after wheel install".to_string()
    })?;
    if resolved != installed {
        return Err(format!(
            "PATH resolved {} instead of installed payload {}",
            resolved.display(),
            installed.display()
        ));
    }
    if resolved == planted {
        return Err(format!(
            "planted PATH executable {} was selected over the installed wheel payload",
            planted.display()
        ));
    }
    if planted_sentinel.exists() {
        return Err(
            "planted PATH ripr was executed; the installed payload did not win PATH lookup"
                .to_string(),
        );
    }
    Ok(())
}

pub(crate) fn require_uninstall_leaves_planted_and_project(
    path_env: &str,
    installed: &Path,
    planted: &Path,
    project_marker: &Path,
) -> Result<(), String> {
    if installed.exists() {
        return Err(format!(
            "installed payload {} still exists after uninstall",
            installed.display()
        ));
    }
    if !planted.is_file() {
        return Err(format!(
            "uninstall deleted planted unrelated executable {}",
            planted.display()
        ));
    }
    if !project_marker.is_file() {
        return Err(format!(
            "uninstall deleted project contents at {}",
            project_marker.display()
        ));
    }
    let resolved = resolve_on_path(path_env, "ripr").ok_or_else(|| {
        "after uninstall PATH should fall through to the planted control executable".to_string()
    })?;
    if resolved != planted {
        return Err(format!(
            "after uninstall PATH resolved {} instead of planted {}",
            resolved.display(),
            planted.display()
        ));
    }
    Ok(())
}

pub(crate) fn require_project_python_idle(sentinel: &Path) -> Result<(), String> {
    if sentinel.exists() {
        return Err(
            "project virtualenv python was executed; the tool environment must stay distinct from the project's test environment"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::require_error;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ripr-pypi-isolation-{label}-{}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            stamp
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).map_err(|error| format!("create isolation fixture: {error}"))?;
        Ok(path)
    }

    fn touch(path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("create parent: {error}"))?;
        }
        fs::write(path, b"x").map_err(|error| format!("touch {}: {error}", path.display()))
    }

    #[test]
    fn clean_path_rejects_cargo_rustc_ambient_ripr_and_checkout() -> Result<(), String> {
        let root = temp_root("clean")?;
        let cargo = root.join("cargo");
        touch(&cargo)?;
        let path = format!("{}:/usr/bin", root.display());
        let error = require_error(
            require_clean_consumer_path(&path, None),
            "cargo on PATH must fail",
        )?;
        if !error.contains("forbidden `cargo`") {
            return Err(format!("unexpected cargo PATH error: {error}"));
        }

        let path = "/usr/bin:/bin";
        touch(&root.join("Cargo.toml"))?;
        let error = require_error(
            require_clean_consumer_path(path, Some(&root)),
            "source checkout must fail",
        )?;
        if !error.contains("source checkout") {
            return Err(format!("unexpected checkout error: {error}"));
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn planted_binary_loses_to_installed_payload_and_survives_uninstall() -> Result<(), String> {
        let root = temp_root("planted")?;
        let installed = root.join("venv/bin/ripr");
        let planted = root.join("planted/ripr");
        let sentinel = root.join("planted-executed");
        let project = root.join("project/src/pricing.py");
        touch(&installed)?;
        touch(&planted)?;
        touch(&project)?;
        let installed_parent = installed
            .parent()
            .ok_or_else(|| "venv bin missing parent".to_string())?;
        let planted_parent = planted
            .parent()
            .ok_or_else(|| "planted dir missing parent".to_string())?;
        let path = format!(
            "{}:{}:/usr/bin",
            installed_parent.display(),
            planted_parent.display()
        );
        require_installed_beats_planted(&path, &installed, &planted, &sentinel)?;

        fs::remove_file(&installed).map_err(|error| format!("uninstall payload: {error}"))?;
        require_uninstall_leaves_planted_and_project(&path, &installed, &planted, &project)?;
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn planted_sentinel_and_project_python_are_fail_closed() -> Result<(), String> {
        let root = temp_root("sentinels")?;
        let installed = root.join("venv/bin/ripr");
        let planted = root.join("planted/ripr");
        let sentinel = root.join("planted-executed");
        touch(&installed)?;
        touch(&planted)?;
        fs::write(&sentinel, b"ran").map_err(|error| format!("planted ran: {error}"))?;
        let installed_parent = installed
            .parent()
            .ok_or_else(|| "venv bin missing parent".to_string())?;
        let planted_parent = planted
            .parent()
            .ok_or_else(|| "planted dir missing parent".to_string())?;
        let path = format!(
            "{}:{}",
            installed_parent.display(),
            planted_parent.display()
        );
        let error = require_error(
            require_installed_beats_planted(&path, &installed, &planted, &sentinel),
            "executed planted binary must fail",
        )?;
        if !error.contains("planted PATH ripr was executed") {
            return Err(format!("unexpected planted error: {error}"));
        }

        let project_python = root.join("project-python-executed");
        fs::write(&project_python, b"ran")
            .map_err(|error| format!("project python ran: {error}"))?;
        let error = require_error(
            require_project_python_idle(&project_python),
            "project venv execution must fail",
        )?;
        if !error.contains("project virtualenv python was executed") {
            return Err(format!("unexpected project python error: {error}"));
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
