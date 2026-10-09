//! Repository-selected Rust toolchain paths.
//!
//! rustup's `cargo` and `rustc` proxies pick a toolchain from the nearest
//! `rust-toolchain.toml` or `rust-toolchain` at or above the working
//! directory. A `[toolchain] path = "..."` entry makes the proxy execute
//! `<path>/bin/cargo`, and `/proc/self/cwd/...` turns that into a path inside
//! the checkout. Running `cargo --version` or `cargo metadata` in an untrusted
//! repository would then run the repository's own program, so ripr refuses to
//! spawn the Rust toolchain there unless the caller pinned one.

use std::path::{Path, PathBuf};

const TOOLCHAIN_FILE_NAMES: [&str; 2] = ["rust-toolchain.toml", "rust-toolchain"];

/// rustup ignores toolchain files when this is set.
const RUSTUP_TOOLCHAIN_ENV: &str = "RUSTUP_TOOLCHAIN";

/// The toolchain file whose `path` rustup would execute for a process started
/// in `root`, or `None` when the Rust toolchain is safe to spawn there.
pub(crate) fn repository_toolchain_path_pin(root: &Path) -> Option<PathBuf> {
    if std::env::var_os(RUSTUP_TOOLCHAIN_ENV).is_some_and(|value| !value.is_empty()) {
        return None;
    }
    nearest_toolchain_path_pin(root)
}

/// [`repository_toolchain_path_pin`] without the environment override. Cargo
/// exports `RUSTUP_TOOLCHAIN` to the tests it runs, so tests call this.
fn nearest_toolchain_path_pin(root: &Path) -> Option<PathBuf> {
    let start = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    for directory in start.ancestors() {
        let files: Vec<PathBuf> = TOOLCHAIN_FILE_NAMES
            .iter()
            .map(|name| directory.join(name))
            .filter(|file| file.is_file())
            .collect();
        if files.is_empty() {
            continue;
        }
        // rustup stops at the nearest directory with a toolchain file. When
        // both names exist there it picks one of them, so either pin counts.
        return files.into_iter().find(|file| names_toolchain_path(file));
    }
    None
}

/// Fail closed: an unreadable file, or a legacy one-line file that looks like
/// a path rather than a channel name, is treated as a path pin.
fn names_toolchain_path(file: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(file) else {
        return true;
    };
    match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value
            .get("toolchain")
            .and_then(|toolchain| toolchain.get("path"))
            .is_some(),
        Err(_) => text.contains(['/', '\\']),
    }
}

/// The refusal ripr reports instead of running the toolchain.
pub(crate) fn toolchain_path_pin_refusal(file: &Path) -> String {
    format!(
        "not run: {} selects a toolchain by `path`, which would execute a program the \
         repository chooses; set {RUSTUP_TOOLCHAIN_ENV} to use your own toolchain here",
        file.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::io::Result<PathBuf> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ripr-toolchain-file-{name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("member"))?;
        Ok(dir)
    }

    #[test]
    fn toolchain_path_pins_are_found_at_the_nearest_toolchain_file() -> std::io::Result<()> {
        let dir = fixture("pin")?;
        let member = dir.join("member");
        let result = (|| {
            std::fs::write(
                dir.join("rust-toolchain.toml"),
                "[toolchain]\npath = \"/proc/self/cwd/tc\"\n",
            )?;
            let pinned_from_member = nearest_toolchain_path_pin(&member);
            std::fs::write(
                member.join("rust-toolchain.toml"),
                "[toolchain]\nchannel = \"stable\"\n",
            )?;
            // The nearer channel file shadows the ancestor path pin.
            let shadowed = nearest_toolchain_path_pin(&member);
            std::fs::remove_file(member.join("rust-toolchain.toml"))?;
            std::fs::write(member.join("rust-toolchain"), "stable\n")?;
            let legacy_channel = nearest_toolchain_path_pin(&member);
            std::fs::write(member.join("rust-toolchain"), "/opt/evil\n")?;
            let legacy_path = nearest_toolchain_path_pin(&member);
            // A channel `.toml` beside a path-pinned legacy file still pins.
            std::fs::write(
                member.join("rust-toolchain.toml"),
                "[toolchain]\nchannel = \"stable\"\n",
            )?;
            let both_files = nearest_toolchain_path_pin(&member);
            Ok::<_, std::io::Error>((
                pinned_from_member,
                shadowed,
                legacy_channel,
                legacy_path,
                both_files,
            ))
        })();
        let _ = std::fs::remove_dir_all(&dir);
        let (pinned, shadowed, legacy_channel, legacy_path, both_files) = result?;
        assert!(
            pinned.is_some_and(|file| file.ends_with("rust-toolchain.toml")),
            "a path pin above the root must be found"
        );
        assert!(
            legacy_path.is_some(),
            "a legacy path-shaped file fails closed"
        );
        assert!(
            both_files.is_some_and(|file| file.ends_with("rust-toolchain")),
            "a path pin in either same-directory file counts"
        );
        assert!(shadowed.is_none(), "the nearest file decides");
        assert!(
            legacy_channel.is_none(),
            "a legacy channel name is not a pin"
        );
        Ok(())
    }
}
