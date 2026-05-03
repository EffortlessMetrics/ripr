use weak_error_oracle_assert_matches_fixture::{AuthError, authenticate};

macro_rules! assert_matches {
    ($expr:expr, $pat:pat $(,)?) => {
        assert!(matches!($expr, $pat));
    };
}

#[test]
fn empty_token_rejects_with_revoked_token() {
    assert_matches!(authenticate(""), Err(AuthError::RevokedToken));
}
