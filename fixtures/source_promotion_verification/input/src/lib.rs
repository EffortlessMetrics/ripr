//! Minimal source-side input for the source-promotion verifier fixture.

/// The source-promotion command is intentionally represented as a stable,
/// fallible boundary in this analyzer fixture.
pub fn verify_join(join: &str) -> Result<(), &'static str> {
    if join.len() == 40 && join.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("join must be an exact hexadecimal object id")
    }
}
