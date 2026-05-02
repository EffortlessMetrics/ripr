#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    EmptyToken,
    RevokedToken,
}
pub fn authenticate(token: &str) -> Result<&'static str, AuthError> {
    if token.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok("accepted")
}
