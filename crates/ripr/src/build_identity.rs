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

const SOURCE_DIGEST: &str = env!("RIPR_BUILD_SOURCE_DIGEST");

/// The analysis-code identity that persisted caches key on.
///
/// The package version alone is not enough: every build between two
/// releases carries the same version, so a cache written by one build would
/// be served to another whose extraction or classification differs. A clean
/// build is identified by its commit. A dirty or commit-less build has no
/// commit that describes its code, so it is identified by the digest
/// `build.rs` takes of its sources. Without that digest the identity is
/// unique to this process and nothing persisted is reused.
pub(crate) fn cache_identity() -> &'static str {
    static IDENTITY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(|| {
        render_cache_identity(
            env!("CARGO_PKG_VERSION"),
            commit(),
            commit_dirty(),
            (!SOURCE_DIGEST.is_empty()).then_some(SOURCE_DIGEST),
        )
    })
}

fn render_cache_identity(
    version: &str,
    commit: Option<&str>,
    dirty: bool,
    source_digest: Option<&str>,
) -> String {
    let source = match commit {
        Some(commit) if !dirty => return format!("{version}+{commit}"),
        Some(commit) => format!("{commit}-dirty"),
        None => "unknown".to_string(),
    };
    match source_digest {
        Some(digest) => format!("{version}+{source}+src:{digest}"),
        None => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            format!("{version}+{source}+process:{}:{nanos}", std::process::id())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "e4da2d4a0c1b2d3e4f5a6b7c8d9e0f1a2b3c4d5e";
    const OTHER_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

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
    #[test]
    fn cache_identity_binds_the_build_not_only_the_version() {
        assert_eq!(
            render_cache_identity("0.11.0", Some(SHA), false, Some("ignored")),
            format!("0.11.0+{SHA}")
        );
        assert_ne!(
            render_cache_identity("0.11.0", Some(SHA), false, None),
            render_cache_identity("0.11.0", Some(OTHER_SHA), false, None),
            "two clean builds of one version must not share cache entries"
        );
        assert_eq!(
            render_cache_identity("0.11.0", Some(SHA), true, Some("00000000000000aa")),
            format!("0.11.0+{SHA}-dirty+src:00000000000000aa")
        );
        assert_eq!(
            render_cache_identity("0.11.0", None, false, Some("00000000000000aa")),
            "0.11.0+unknown+src:00000000000000aa"
        );
        // Two dirty builds of one commit share entries only when their
        // sources match; an executable's size or timestamp plays no part.
        assert_ne!(
            render_cache_identity("0.11.0", Some(SHA), true, Some("00000000000000aa")),
            render_cache_identity("0.11.0", Some(SHA), true, Some("00000000000000bb")),
            "a dirty build with other sources must not reuse the previous build's entries"
        );
        let first = render_cache_identity("0.11.0", None, false, None);
        assert!(
            first.starts_with(&format!("0.11.0+unknown+process:{}:", std::process::id())),
            "{first}"
        );
        assert_ne!(
            first, "0.11.0",
            "the bare version must never be a cache identity"
        );
    }

    #[test]
    fn live_cache_identity_is_stable_and_names_the_version() {
        let identity = cache_identity();
        assert!(identity.starts_with(concat!(env!("CARGO_PKG_VERSION"), "+")));
        assert_eq!(identity, cache_identity());
        // build.rs records a digest exactly when no clean commit describes
        // this build.
        let described_by_commit = commit().is_some() && !commit_dirty();
        assert_eq!(SOURCE_DIGEST.is_empty(), described_by_commit);
        if !described_by_commit {
            assert!(
                SOURCE_DIGEST.len() == 16
                    && SOURCE_DIGEST.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "malformed source digest {SOURCE_DIGEST:?}"
            );
        }
    }
}
