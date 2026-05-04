# Rust Rules

## Error handling

- Prefer `Result` propagation over panics in production paths.
- Avoid `unwrap` and `expect` unless the invariant is local, obvious, and documented.
- Preserve error context where it helps diagnose user-facing failures.

## Filesystem and process behavior

- Treat filesystem paths as cross-platform unless the code is explicitly platform-specific.
- Use `std::path::Path` and `PathBuf` for cross-platform path manipulation.
- Avoid manual path string concatenation and platform-specific separators unless the code is explicitly platform-scoped.
- Be careful with path traversal, symlinks, environment variables, and working directory assumptions.
- Avoid unsafe command construction or shell interpolation.

## Tests

- Test changed behavior and edge cases.
- Do not add tests that only assert implementation details unless the behavior requires it.
