//! Crate-internal typed error authority (#4859 / ERR1 under #2688).
//!
//! Git invocation timeout is the first migrated matchable family. Later
//! PR-sized slices add cancellation (#4860) and oversized-diff (#4861)
//! variants. Unmigrated failures travel as [`CoreError::Message`] until
//! those migrations; they are not a semantic family.
//!
//! Public CLI, JSON, and LSP text is rendered at [`Display`] boundaries.
//! Semantic consumers match variants (including through
//! [`CoreError::with_context`]), not rendered prefixes. #2811 remains the
//! public/LSP projection owner: a typed timeout maps to the existing
//! `git_invocation_timeout` kind string.

use std::error::Error;
use std::fmt;

/// Public/LSP projection token for git invocation timeout (#2303 / #2811).
pub(crate) const GIT_INVOCATION_TIMEOUT_KIND: &str = "git_invocation_timeout";

pub(crate) const GIT_TIMEOUT_REPAIR_GUIDANCE: &str = " Repair route: raise or disable the git deadline (0 disables it) — \
     --git-timeout SECS or RIPR_GIT_TIMEOUT=<seconds> for CLI runs, the \
     gitTimeoutMs initialization option for editor sessions — then re-run.";

/// Crate-internal error used for semantic control flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoreError {
    /// A git invocation that was given a zero deadline, or that exceeded its
    /// cooperative deadline and was terminated.
    GitInvocationTimeout {
        operation: String,
        timeout_ms: u128,
        spawned: bool,
    },
    /// Unmigrated remainder. Display-only until a later family is typed.
    Message(String),
    /// Structured wrap that preserves the source kind.
    Context {
        context: String,
        source: Box<CoreError>,
    },
}

impl CoreError {
    pub(crate) fn git_invocation_timeout(
        operation: impl Into<String>,
        timeout_ms: u128,
        spawned: bool,
    ) -> Self {
        Self::GitInvocationTimeout {
            operation: operation.into(),
            timeout_ms,
            spawned,
        }
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    /// Attach human context without dropping a typed timeout kind.
    pub(crate) fn with_context(self, context: impl fmt::Display) -> Self {
        Self::Context {
            context: context.to_string(),
            source: Box::new(self),
        }
    }

    pub(crate) fn is_git_invocation_timeout(&self) -> bool {
        match self {
            Self::GitInvocationTimeout { .. } => true,
            Self::Context { source, .. } => source.is_git_invocation_timeout(),
            Self::Message(_) => false,
        }
    }

    /// The #2811 / component-outcome kind when this error is a git timeout.
    pub(crate) fn git_invocation_timeout_kind(&self) -> Option<&'static str> {
        self.is_git_invocation_timeout()
            .then_some(GIT_INVOCATION_TIMEOUT_KIND)
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitInvocationTimeout {
                operation,
                spawned: false,
                ..
            } => write!(
                formatter,
                "{GIT_INVOCATION_TIMEOUT_KIND}: {operation} was given a zero deadline (not spawned)"
            ),
            Self::GitInvocationTimeout {
                operation,
                timeout_ms,
                spawned: true,
            } => write!(
                formatter,
                "{GIT_INVOCATION_TIMEOUT_KIND}: {operation} exceeded the {timeout_ms}ms deadline (process terminated).{}",
                GIT_TIMEOUT_REPAIR_GUIDANCE
            ),
            Self::Message(message) => formatter.write_str(message),
            Self::Context { context, source } => write!(formatter, "{context}: {source}"),
        }
    }
}

impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Context { source, .. } => Some(source.as_ref()),
            Self::GitInvocationTimeout { .. } | Self::Message(_) => None,
        }
    }
}

impl From<String> for CoreError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CoreError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl From<CoreError> for String {
    fn from(error: CoreError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{CoreError, GIT_INVOCATION_TIMEOUT_KIND};
    use std::error::Error;

    fn timeout() -> CoreError {
        CoreError::git_invocation_timeout("git -C /workspace [\"diff\"]", 30000, true)
    }

    #[test]
    fn display_parity_matches_the_public_timeout_wording() {
        let spawned = timeout();
        assert_eq!(
            spawned.to_string(),
            concat!(
                "git_invocation_timeout: git -C /workspace [\"diff\"] exceeded the 30000ms deadline (process terminated).",
                " Repair route: raise or disable the git deadline (0 disables it) — ",
                "--git-timeout SECS or RIPR_GIT_TIMEOUT=<seconds> for CLI runs, the ",
                "gitTimeoutMs initialization option for editor sessions — then re-run."
            )
        );
        let zero = CoreError::git_invocation_timeout("git -C /x [\"status\"]", 0, false);
        assert_eq!(
            zero.to_string(),
            "git_invocation_timeout: git -C /x [\"status\"] was given a zero deadline (not spawned)"
        );
    }

    #[test]
    fn lookalike_message_is_not_a_timeout() {
        // Control 3: an unrelated error whose Display begins with the legacy
        // prefix cannot trigger timeout control flow.
        let lookalike = CoreError::message("git_invocation_timeout: forged prefix must not match");
        assert!(
            lookalike
                .to_string()
                .starts_with(GIT_INVOCATION_TIMEOUT_KIND),
            "the lookalike Display must start with the legacy prefix so a restored prefix matcher would misfire"
        );
        assert!(
            !lookalike.is_git_invocation_timeout(),
            "Message must not be classified as a typed timeout"
        );
        assert_eq!(lookalike.git_invocation_timeout_kind(), None);
    }

    #[test]
    fn wrapped_timeout_keeps_its_kind_without_a_prefix_match() {
        // Controls 4 and 8: wrapping a real timeout must not lose the kind,
        // and restoring prefix matching on Display would fail this case.
        let wrapped = timeout().with_context("workspace analysis failed");
        assert!(wrapped.is_git_invocation_timeout());
        assert_eq!(
            wrapped.git_invocation_timeout_kind(),
            Some(GIT_INVOCATION_TIMEOUT_KIND)
        );
        assert!(
            !wrapped.to_string().starts_with(GIT_INVOCATION_TIMEOUT_KIND),
            "wrapped Display must not start with the legacy prefix; prefix matching would miss it"
        );
        assert!(
            wrapped.to_string().contains(GIT_INVOCATION_TIMEOUT_KIND),
            "inner public wording remains in the Display chain"
        );
    }

    #[test]
    fn source_chain_is_inspectable_without_command_output() {
        // Control 5: source() yields the typed timeout, not unbounded
        // stdout/stderr or extra private paths.
        let wrapped = timeout().with_context("failed to run git diff");
        let source = wrapped
            .source()
            .and_then(|source| source.downcast_ref::<CoreError>());
        assert!(
            source.is_some_and(CoreError::is_git_invocation_timeout),
            "context should expose the typed timeout as source"
        );
        if let Some(CoreError::GitInvocationTimeout {
            operation,
            timeout_ms,
            spawned,
        }) = source
        {
            assert_eq!(operation, "git -C /workspace [\"diff\"]");
            assert_eq!(*timeout_ms, 30000);
            assert!(*spawned);
            assert!(
                !operation.contains('\n'),
                "timeout source must not carry command output"
            );
        }
    }

    #[test]
    fn ordinary_message_with_timeout_words_is_not_typed() {
        // A Message remains ordinary even
        // when stderr mentions similar words.
        let failure = CoreError::message(
            "git -C /workspace [\"diff\"] failed\nstdout: \nstderr: git_invocation_timeout is not a git command",
        );
        assert!(!failure.is_git_invocation_timeout());
        assert!(failure.to_string().contains("git_invocation_timeout"));
    }

    #[test]
    fn prefix_matching_display_is_not_the_semantic_oracle() {
        // Control 8: if production restored `starts_with` on Display, the
        // wrapped timeout would be missed and the lookalike would fire.
        let wrapped = timeout().with_context("failed to run git diff");
        let lookalike = CoreError::message("git_invocation_timeout: forged");
        assert_ne!(
            wrapped.is_git_invocation_timeout(),
            wrapped.to_string().starts_with(GIT_INVOCATION_TIMEOUT_KIND)
        );
        assert_ne!(
            lookalike.is_git_invocation_timeout(),
            lookalike
                .to_string()
                .starts_with(GIT_INVOCATION_TIMEOUT_KIND)
        );
    }
}
