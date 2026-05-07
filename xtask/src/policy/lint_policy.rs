//! Cross-check `policy/clippy-lints.toml` against `[workspace.lints.*]` in
//! `Cargo.toml` and the workspace MSRV. See `docs/CLIPPY_POLICY.md`.

pub(crate) fn check_lint_policy() -> Result<(), String> {
    crate::check_lint_policy_impl()
}
