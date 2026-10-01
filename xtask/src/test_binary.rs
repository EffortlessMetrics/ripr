//! Resolve-only lookup for the built `ripr` analyzer binary in xtask unit tests.
//!
//! Several test harnesses execute the real analyzer binary (never a stub) and
//! bind its explicit identity (path, `--version`, sha256) into their records.
//! `CARGO_BIN_EXE_ripr` is only set for integration tests inside the `ripr`
//! package itself, so these cross-package unit tests must locate the binary on
//! the filesystem. Resolution is strictly resolve-only: a test never spawns a
//! nested `cargo build`, it reports the probed locations and asks for
//! `cargo build -p ripr` first.
//!
//! Candidate order: the `RIPR_TEST_BINARY` explicit override first, then the
//! active `CARGO_TARGET_DIR` (routed CI points it at scratch), then
//! `CARGO_LLVM_COV_TARGET_DIR` (the Coverage workflow builds the instrumented
//! analyzer under llvm-cov's separate target), then the workspace `target/`
//! directory. The profile follows the test build itself (`cargo test
//! --release` looks under `release/`).
//!
//! [`resolve_built_ripr_binary`] is the pure core and is unit-tested below
//! against staged synthetic target layouts. The environment wrapper is covered
//! end to end by the adopting harnesses, which execute the resolved binary.

use std::path::{Path, PathBuf};

/// Explicit inputs to [`resolve_built_ripr_binary`]; every field is a plain
/// path so unit tests can stage synthetic target layouts without touching
/// process-wide environment variables.
pub(crate) struct BinarySearch {
    /// `RIPR_TEST_BINARY` override: used verbatim when it names a file.
    pub(crate) override_binary: Option<PathBuf>,
    /// Active cargo target directory (`CARGO_TARGET_DIR`).
    pub(crate) cargo_target_dir: Option<PathBuf>,
    /// llvm-cov target directory (`CARGO_LLVM_COV_TARGET_DIR`).
    pub(crate) llvm_cov_target_dir: Option<PathBuf>,
    /// Workspace root anchoring the default `target/` fallback.
    pub(crate) workspace_root: PathBuf,
    /// Build profile subdirectory (`debug` or `release`).
    pub(crate) profile: &'static str,
}

/// First `is_file()` hit wins; the hit is returned as an absolute display
/// path. When nothing matches, the error lists every probed location so a
/// miss under an unfamiliar target layout is diagnosable instead of silent.
pub(crate) fn resolve_built_ripr_binary(search: &BinarySearch) -> Result<String, String> {
    let candidates = built_ripr_binary_candidates(search);
    for candidate in &candidates {
        if candidate.is_file() {
            return std::path::absolute(candidate)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| format!("resolve built ripr binary: {error}"));
        }
    }
    Err(format!(
        "no built ripr binary found (looked at {}); run `cargo build -p ripr` first (tests resolve the binary and never spawn a nested build)",
        candidates
            .iter()
            .map(|candidate| format!("`{}`", candidate.display()))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Thin environment wrapper over [`resolve_built_ripr_binary`]: the override,
/// active target dir, and llvm-cov target dir come from the environment, the
/// workspace root from the xtask manifest location, and the profile from the
/// test build itself.
pub(crate) fn resolve_built_ripr_binary_from_env() -> Result<String, String> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or("xtask manifest has no repository parent")?;
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    resolve_built_ripr_binary(&BinarySearch {
        override_binary: std::env::var_os("RIPR_TEST_BINARY").map(PathBuf::from),
        cargo_target_dir: std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        llvm_cov_target_dir: std::env::var_os("CARGO_LLVM_COV_TARGET_DIR").map(PathBuf::from),
        workspace_root,
        profile,
    })
}

fn built_ripr_binary_candidates(search: &BinarySearch) -> Vec<PathBuf> {
    let file_name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = Vec::new();
    if let Some(override_binary) = &search.override_binary {
        candidates.push(override_binary.clone());
    }
    if let Some(target_dir) = &search.cargo_target_dir {
        candidates.push(target_dir.join(search.profile).join(&file_name));
    }
    if let Some(target_dir) = &search.llvm_cov_target_dir {
        candidates.push(target_dir.join(search.profile).join(&file_name));
    }
    candidates.push(
        search
            .workspace_root
            .join("target")
            .join(search.profile)
            .join(&file_name),
    );
    candidates
}

struct TempRoot {
    root: PathBuf,
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl TempRoot {
    fn new(name: &str) -> Result<Self, String> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-test-binary-{name}-{}-{:?}-{unique}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(Self { root })
    }

    fn touch(&self, relative: &str) -> Result<PathBuf, String> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, []).map_err(|error| error.to_string())?;
        Ok(path)
    }

    fn mkdir(&self, relative: &str) -> Result<PathBuf, String> {
        let path = self.root.join(relative);
        std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("test-binary test failed: {message}"))
    }
}

fn binary_file_name() -> String {
    format!("ripr{}", std::env::consts::EXE_SUFFIX)
}

/// Normalizes staged paths exactly the way resolution does, so expectations
/// compare against the same absolute form on every platform.
fn absolute_string(path: &Path) -> Result<String, String> {
    std::path::absolute(path)
        .map(|absolute| absolute.to_string_lossy().into_owned())
        .map_err(|error| error.to_string())
}

#[test]
fn override_binary_wins_over_every_target_dir() -> Result<(), String> {
    let temp = TempRoot::new("override-wins")?;
    let file_name = binary_file_name();
    let expected = temp.touch("override/ripr-override")?;
    temp.touch(&format!("cargo-target/debug/{file_name}"))?;
    temp.touch(&format!("llvm-cov-target/debug/{file_name}"))?;
    temp.touch(&format!("workspace/target/debug/{file_name}"))?;
    let search = BinarySearch {
        override_binary: Some(expected.clone()),
        cargo_target_dir: Some(temp.root.join("cargo-target")),
        llvm_cov_target_dir: Some(temp.root.join("llvm-cov-target")),
        workspace_root: temp.root.join("workspace"),
        profile: "debug",
    };
    let resolved = resolve_built_ripr_binary(&search)?;
    ensure(
        Path::new(&resolved).is_absolute(),
        "the resolved binary path must be absolute",
    )?;
    ensure(
        resolved == absolute_string(&expected)?,
        "the explicit override must win over every target dir",
    )?;
    Ok(())
}

#[test]
fn cargo_target_dir_wins_over_llvm_cov_and_default() -> Result<(), String> {
    let temp = TempRoot::new("cargo-target-wins")?;
    let file_name = binary_file_name();
    let expected = temp.touch(&format!("cargo-target/debug/{file_name}"))?;
    temp.touch(&format!("llvm-cov-target/debug/{file_name}"))?;
    temp.touch(&format!("workspace/target/debug/{file_name}"))?;
    let search = BinarySearch {
        override_binary: None,
        cargo_target_dir: Some(temp.root.join("cargo-target")),
        llvm_cov_target_dir: Some(temp.root.join("llvm-cov-target")),
        workspace_root: temp.root.join("workspace"),
        profile: "debug",
    };
    ensure(
        resolve_built_ripr_binary(&search)? == absolute_string(&expected)?,
        "the active cargo target dir must win over llvm-cov and default",
    )?;
    Ok(())
}

#[test]
fn llvm_cov_target_dir_wins_over_default() -> Result<(), String> {
    let temp = TempRoot::new("llvm-cov-wins")?;
    let file_name = binary_file_name();
    let expected = temp.touch(&format!("llvm-cov-target/debug/{file_name}"))?;
    temp.touch(&format!("workspace/target/debug/{file_name}"))?;
    let search = BinarySearch {
        override_binary: None,
        cargo_target_dir: None,
        llvm_cov_target_dir: Some(temp.root.join("llvm-cov-target")),
        workspace_root: temp.root.join("workspace"),
        profile: "debug",
    };
    ensure(
        resolve_built_ripr_binary(&search)? == absolute_string(&expected)?,
        "the llvm-cov target dir must win over the default target dir",
    )?;
    Ok(())
}

#[test]
fn workspace_target_is_the_final_fallback() -> Result<(), String> {
    let temp = TempRoot::new("default-fallback")?;
    let file_name = binary_file_name();
    let expected = temp.touch(&format!("workspace/target/debug/{file_name}"))?;
    let search = BinarySearch {
        override_binary: None,
        cargo_target_dir: None,
        llvm_cov_target_dir: None,
        workspace_root: temp.root.join("workspace"),
        profile: "debug",
    };
    ensure(
        resolve_built_ripr_binary(&search)? == absolute_string(&expected)?,
        "the workspace target dir must resolve when nothing else is set",
    )?;
    Ok(())
}

#[test]
fn profile_selects_the_profile_subdir() -> Result<(), String> {
    let temp = TempRoot::new("profile-subdir")?;
    let file_name = binary_file_name();
    let expected = temp.touch(&format!("workspace/target/release/{file_name}"))?;
    temp.touch(&format!("workspace/target/debug/{file_name}"))?;
    let search = BinarySearch {
        override_binary: None,
        cargo_target_dir: None,
        llvm_cov_target_dir: None,
        workspace_root: temp.root.join("workspace"),
        profile: "release",
    };
    ensure(
        resolve_built_ripr_binary(&search)? == absolute_string(&expected)?,
        "a release test build must resolve under release/, not debug/",
    )?;
    Ok(())
}

#[test]
fn directories_and_missing_files_are_skipped() -> Result<(), String> {
    let temp = TempRoot::new("skip-non-files")?;
    let file_name = binary_file_name();
    // A directory at the cargo-target candidate path must not match: only
    // `is_file()` hits resolve.
    temp.mkdir(&format!("cargo-target/debug/{file_name}"))?;
    let expected = temp.touch(&format!("workspace/target/debug/{file_name}"))?;
    let search = BinarySearch {
        // A set-but-missing override is skipped, like every other candidate.
        override_binary: Some(temp.root.join("no-such-override")),
        cargo_target_dir: Some(temp.root.join("cargo-target")),
        llvm_cov_target_dir: Some(temp.root.join("no-such-llvm-cov-target")),
        workspace_root: temp.root.join("workspace"),
        profile: "debug",
    };
    ensure(
        resolve_built_ripr_binary(&search)? == absolute_string(&expected)?,
        "directories and missing files must be skipped for the next candidate",
    )?;
    Ok(())
}

#[test]
fn missing_everywhere_names_each_probed_location() -> Result<(), String> {
    let temp = TempRoot::new("missing-error")?;
    let search = BinarySearch {
        override_binary: Some(temp.root.join("no-such-override")),
        cargo_target_dir: Some(temp.root.join("no-such-cargo-target")),
        llvm_cov_target_dir: Some(temp.root.join("no-such-llvm-cov-target")),
        workspace_root: temp.root.join("no-such-workspace"),
        profile: "debug",
    };
    let error = resolve_built_ripr_binary(&search)
        .err()
        .ok_or("test-binary test failed: a total miss must be an error")?;
    for expected in [
        "no-such-override",
        "no-such-cargo-target",
        "no-such-llvm-cov-target",
        "no-such-workspace",
        "cargo build -p ripr",
        "never spawn a nested build",
    ] {
        ensure(
            error.contains(expected),
            &format!("the miss error must name `{expected}`, got: {error}"),
        )?;
    }
    Ok(())
}
