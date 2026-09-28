//! Explicit opt-in gate for fixture re-blessing.
//!
//! Several fixture-pin tests rewrite their expected files when
//! `RIPR_UPDATE_FIXTURES` is set. Bare presence of that variable (leaked
//! from an unrelated shell) must never convert asserts into re-blesses:
//! only the documented explicit value `RIPR_UPDATE_FIXTURES=1` opts in
//! (#3742 class (e)).

/// The documented opt-in value that authorizes fixture rewrites.
pub(crate) const FIXTURE_REBLESS_OPT_IN: &str = "1";

/// Pure predicate over an observed variable value. Kept separate from the
/// environment read so the contract is unit-testable without mutating
/// process state (the crate forbids `unsafe_code`, and `set_var` is
/// `unsafe` in edition 2024).
pub(crate) fn rebless_enabled_for(value: Option<&str>) -> bool {
    value.is_some_and(|value| value == FIXTURE_REBLESS_OPT_IN)
}

/// Returns true only when the environment carries the explicit re-bless
/// opt-in value. A present-but-empty or unrecognized value behaves exactly
/// like an unset variable: assert, never rewrite.
pub(crate) fn fixture_rebless_enabled() -> bool {
    rebless_enabled_for(std::env::var("RIPR_UPDATE_FIXTURES").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebless_requires_the_explicit_opt_in_value() {
        assert!(!rebless_enabled_for(None));
        assert!(!rebless_enabled_for(Some("")));
        assert!(!rebless_enabled_for(Some("yes")));
        assert!(rebless_enabled_for(Some("1")));
    }
}
