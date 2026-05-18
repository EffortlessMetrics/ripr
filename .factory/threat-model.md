# Threat Model for ripr

**Generated:** 2026-05-18
**Repository:** EffortlessMetrics/ripr
**Methodology:** STRIDE

## Overview

ripr is a static RIPR (Reach-Infect-Propagate-Observe-Discriminate) exposure analyzer for Rust/Cargo workspaces. It analyzes code diffs to determine if existing tests would catch behavior changes.

## System Architecture

### Components

1. **CLI Binary (`ripr`)** - Main command-line interface
2. **Library (`ripr` crate)** - Core analysis engine
3. **xtask** - Repository automation tool
4. **VS Code Extension** - LSP-based editor integration
5. **GitHub Actions** - CI/CD workflows

## STRIDE Threat Analysis

### Spoofing

| Threat | Affected Component | Risk | Mitigation |
|--------|-------------------|------|------------|
| Fake CI artifacts | GitHub Actions | Low | Artifacts use SHA references |
| Impersonating ripr binary | Distribution | Low | Verified first-run download |

### Tampering

| Threat | Affected Component | Risk | Mitigation |
|--------|-------------------|------|------------|
| Malicious diff files | Input processing | Low | Static analysis only, no execution |
| Modified fixture files | Tests | Low | Golden output validation |
| Workflow injection | GitHub Actions | Low | Minimal permissions, no untrusted input |

### Repudiation

| Threat | Affected Component | Risk | Mitigation |
|--------|-------------------|------|------------|
| No audit trail | Analysis results | Medium | GitHub Actions provides audit trail |
| Missing evidence | PR review | Low | Structured output format |

### Information Disclosure

| Threat | Affected Component | Risk | Mitigation |
|--------|-------------------|------|------------|
| Hardcoded secrets | Source code | Critical | Forbidden by policy, checked by CI |
| API key exposure | GitHub Actions | High | Written to config, not logged |
| Path traversal | File reading | Low | PathBuf usage, no user-controlled paths |

### Denial of Service

| Threat | Affected Component | Risk | Mitigation |
|--------|-------------------|------|------------|
| Large diff files | Parser | Low | Reasonable limits enforced |
| Infinite loops | Analysis | Low | Timeout on long operations |

### Elevation of Privilege

| Threat | Affected Component | Risk | Mitigation |
|--------|-------------------|------|------------|
| Arbitrary code execution | xtask | Low | No shell interpolation, safe argument passing |
| Workflow permission creep | GitHub Actions | Low | Minimal required permissions |

## Security Controls

1. **Unsafe Code Policy**: `#![forbid(unsafe_code)]` enforced workspace-wide
2. **Process Execution**: Uses `Command::new()` with separate arguments (no shell)
3. **Secret Handling**: API keys written to local config files, not exposed in logs
4. **GitHub Actions**: 
   - Pinned SHA references for actions
   - Minimal permissions (`contents: read` where possible)
   - No `pull_request_target` without proper guards
5. **Input Validation**: Environment variables validated before use

## Key Security Boundaries

1. **Diff Input → Analysis Engine**: No execution of diff content
2. **GitHub Actions → Secrets**: Separated via environment, not direct exposure
3. **xtask → System**: Limited to allowed commands, no arbitrary shell execution
4. **LSP → Editor**: Read-only analysis, no code modification

## Residual Risks

1. **User-provided diffs**: Could contain extremely large files causing resource exhaustion
2. **Custom probe shapes**: Could theoretically probe for sensitive patterns (mitigated by static-only analysis)
3. **Public PR artifacts**: Could expose findings to unauthorized users (acceptable risk for open-source tool)

## References

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [STRIDE Methodology](https://docs.microsoft.com/en-us/azure/security/develop/threat-modeling-tool-threats)
- [CWE Database](https://cwe.mitre.org/)
