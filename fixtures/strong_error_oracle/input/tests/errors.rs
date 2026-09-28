use strong_error_oracle_fixture::{AuthError, authenticate};

#[test]
fn empty_token_rejects_with_revoked_token() {
    assert!(matches!(authenticate(""), Err(AuthError::RevokedToken)));
}
