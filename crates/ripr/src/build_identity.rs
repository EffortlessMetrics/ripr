//! The commit this binary was built from, as recorded by `build.rs`.
//!
//! `ripr --version` prints it so a candidate binary can be bound to source
//! without hashing it, and `ripr doctor` reports it next to the binary's path.

const COMMIT: &str = env!("RIPR_BUILD_COMMIT");
const COMMIT_DIRTY: &str = env!("RIPR_BUILD_COMMIT_DIRTY");

/// The full commit id, or `None` when the build had no commit record (for
/// example a source archive built outside Git).
pub(crate) fn commit() -> Option<&'static str> {
    (!COMMIT.is_empty()).then_some(COMMIT)
}

/// Whether the crate sources differed from [`commit`] when it was built.
pub(crate) fn commit_dirty() -> bool {
    COMMIT_DIRTY == "true"
}

/// The `ripr --version` line: `ripr <version>`, followed by
/// `(<commit>)` or `(<commit>-dirty)` when the commit is known.
pub(crate) fn version_line() -> String {
    render_version_line(env!("CARGO_PKG_VERSION"), commit(), commit_dirty())
}

fn render_version_line(version: &str, commit: Option<&str>, dirty: bool) -> String {
    match commit {
        None => format!("ripr {version}"),
        Some(commit) if dirty => format!("ripr {version} ({commit}-dirty)"),
        Some(commit) => format!("ripr {version} ({commit})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "e4da2d4a0c1b2d3e4f5a6b7c8d9e0f1a2b3c4d5e";

    #[test]
    fn version_line_names_the_commit_and_dirty_state() {
        assert_eq!(render_version_line("0.11.0", None, false), "ripr 0.11.0");
        assert_eq!(render_version_line("0.11.0", None, true), "ripr 0.11.0");
        assert_eq!(
            render_version_line("0.11.0", Some(SHA), false),
            format!("ripr 0.11.0 ({SHA})")
        );
        assert_eq!(
            render_version_line("0.11.0", Some(SHA), true),
            format!("ripr 0.11.0 ({SHA}-dirty)")
        );
    }

    #[test]
    fn recorded_commit_is_empty_or_a_full_commit_id() {
        if let Some(commit) = commit() {
            assert!(
                crate::build_commit_record::is_full_commit_id(commit),
                "build.rs recorded a malformed commit: {commit:?}"
            );
        }
        assert!(matches!(COMMIT_DIRTY, "true" | "false"));
    }
}
