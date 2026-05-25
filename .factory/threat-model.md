# Threat Model for ripr

**Generated:** 2026-05-25
**Tool:** Static RIPR (Reach-Infect-Propagate-Observe-Discriminate) exposure analyzer

## Overview

ripr is a static analysis tool that examines code diffs and evaluates whether tests contain discriminators that would catch behavioral changes. It does NOT execute code, run mutations, or modify systems.

## STRIDE Analysis

### Spoofing

**Risk:** Low

ripr is a read-only static analyzer. It does not authenticate users or represent other identities.

Mitigations:
- No user authentication in the tool itself
- GitHub Actions workflows use explicit permissions and tokens
- No secrets embedded in code

### Tampering

**Risk:** Low

ripr only reads code and produces reports. It does not modify source code, configuration, or artifacts.

Mitigations:
- All file operations use standard Rust filesystem APIs
- No code generation that could inject malicious content
- Output is written to configured report directories only

### Repudiation

**Risk:** Very Low

ripr's outputs are advisory and clearly labeled as static evidence.

Mitigations:
- All git operations capture errors properly
- Timestamps and process IDs in output for traceability
- Clear "limits" section in all reports stating no runtime proof

### Information Disclosure

**Risk:** Low

ripr only processes code diffs and produces static evidence reports.

Mitigations:
- No secrets in codebase (verified via grep)
- Environment variables properly validated with error propagation
- No sensitive data in output artifacts

### Denial of Service

**Risk:** Low

ripr enforces timeouts on all subprocess calls (git, cargo, etc.).

Mitigations:
- `capture_output_with_timeout` and `capture_stdout_to_file_with_timeout` in xtask
- Default timeouts configured via environment variables
- Progress reporting for long operations

### Elevation of Privilege

**Risk:** Very Low

ripr runs with the same privileges as the user invoking it.

Mitigations:
- No setuid binaries or privilege escalation mechanisms
- GitHub Actions use minimal required permissions
- No capability to bypass security controls

## Security Controls

| Control | Implementation |
|---------|----------------|
| Unsafe code | `unsafe_code = "forbid"` workspace-wide |
| Secrets | No hardcoded secrets; env vars validated |
| Dependencies | `cargo-deny` + `dependency-review-action` |
| Workflow permissions | Explicit, minimal scope |
| Fork handling | `pull_request` trigger with same-repo guards |
| Timeouts | All subprocess calls have timeouts |
| Input validation | Proper error propagation throughout |

## Data Flow

```
User Input (diff/file path)
    ↓
Git Operations (read-only)
    ↓
Code Parsing (syntactic analysis)
    ↓
Probe Generation (static analysis)
    ↓
Classification (exposure evidence)
    ↓
Report Output (JSON/Markdown)
```

## Attack Surface

- **Input:** Diff files or git refs (user-controlled content)
- **Processing:** Rust code parsing (memory-safe language)
- **Output:** Report files (controlled directory)

## Conclusion

ripr maintains strong security posture due to:
1. Rust's memory safety guarantees (no unsafe code allowed)
2. Read-only analysis (no side effects)
3. Explicit timeout enforcement
4. No secrets in codebase
5. Minimal CI permissions
