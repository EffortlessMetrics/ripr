# Security Rules

Review these areas with elevated attention:

- Secrets, tokens, credentials, API keys, and auth headers.
- GitHub Actions permissions and trigger behavior.
- Artifact and log exposure.
- Unsafe process execution.
- Path traversal and filesystem writes.
- Dependency and release workflows.
- Untrusted input crossing into shell, filesystem, network, or config surfaces.

A security finding should explain:
- attacker or failure scenario;
- affected surface;
- why the current code permits it;
- fix direction;
- validation or regression check.
