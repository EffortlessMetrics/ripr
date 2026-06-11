# Security Scan Report

**Generated:** 2026-06-01
**Scan Type:** Weekly Scheduled
**Repository:** EffortlessMetrics/ripr
**Severity Threshold:** medium

## Executive Summary

| Severity | Count | Auto-fixed | Manual Required |
|----------|-------|------------|-----------------|
| CRITICAL | 0 | 0 | 0 |
| HIGH | 2 | 0 | 2 |
| MEDIUM | 3 | 0 | 3 |
| LOW | 0 | 0 | 0 |

**Total Findings:** 5
**Auto-fixed:** 0
**Manual Review Required:** 5

## High Findings

### VULN-001: Secret Written to Plaintext File on Runner

| Attribute | Value |
|-----------|-------|
| **Severity** | HIGH |
| **STRIDE Category** | Information Disclosure |
| **CWE** | CWE-312 (Cleartext Storage of Sensitive Information) |
| **File** | .github/workflows/droid-security-scan.yml:29-43 |
| **Status** | Manual fix required |

**Description:**
The `MINIMAX_API_KEY` secret is written in plaintext to `~/.factory/settings.local.json` on the GitHub Actions runner disk. Any process running on the same runner with access to the filesystem can read this secret. After the workflow completes, artifacts from the runner may persist temporarily.

**Evidence:**
```yaml
- name: Configure MiniMax BYOK for Factory Droid
  shell: bash
  run: |
    mkdir -p "$HOME/.factory"
    cat > "$HOME/.factory/settings.local.json" <<'JSON'
    {
      "customModels": [
        {
          "displayName": "MiniMax-M2.7",
          "model": "MiniMax-M2.7",
          "baseUrl": "https://api.minimax.io/anthropic",
          "apiKey": "${MINIMAX_API_KEY}",
          ...
        }
      ]
    }
    JSON
```

**Recommended Fix:**
Use environment variables instead of writing secrets to disk. GitHub Actions provides `GITHUB_ENV` to set environment variables for subsequent steps. The secret should be passed directly to the action via input parameters or environment variables rather than written to a file.

Example remediation:
```yaml
- name: Configure MiniMax BYOK for Factory Droid
  env:
    MINIMAX_API_KEY: ${{ secrets.MINIMAX_API_KEY }}
  run: |
    mkdir -p "$HOME/.factory"
    cat > "$HOME/.factory/settings.local.json" <<'JSON'
    {
      "customModels": [
        {
          "displayName": "MiniMax-M2.7",
          "model": "MiniMax-M2.7",
          "baseUrl": "https://api.minimax.io/anthropic",
          "apiKey": "$MINIMAX_API_KEY",
          ...
        }
      ]
    }
    JSON
```

---

### VULN-002: Excessive Permissions - OIDC Token Generation

| Attribute | Value |
|-----------|-------|
| **Severity** | HIGH |
| **STRIDE Category** | Elevation of Privilege |
| **CWE** | CWE-284 (Improper Access Control) |
| **File** | .github/workflows/droid-security-scan.yml:18-23 |
| **Status** | Manual fix required |

**Description:**
The workflow requests `id-token: write` permission which allows the workflow to obtain OIDC tokens for cloud authentication. This is a high-privilege permission that can potentially be used for privilege escalation if the workflow or action is compromised.

**Evidence:**
```yaml
permissions:
  contents: write
  pull-requests: write
  issues: write
  id-token: write  # <-- Excessive privilege
  actions: read
```

**Recommended Fix:**
Remove `id-token: write` permission unless explicitly required for cloud authentication to a specific provider. If OIDC tokens are required, scope the permission to a specific environment rather than granting it broadly:
```yaml
permissions:
  contents: write
  pull-requests: write
  issues: write
  actions: read
  # Remove id-token: write or scope it if actually needed
```

---

## Medium Findings

### VULN-003: Third-Party Action Not Pinned to Tagged Release

| Attribute | Value |
|-----------|-------|
| **Severity** | MEDIUM |
| **STRIDE Category** | Supply Chain |
| **CWE** | CWE-829 (Inclusion of Functionality from Untrusted Control Sphere) |
| **File** | .github/workflows/droid-security-scan.yml:46 |
| **Status** | Manual fix required |

**Description:**
The workflow uses a third-party GitHub Action (`EffortlessMetrics/droid-action-safe`) pinned to a raw commit hash rather than a tagged release. While commit pinning is better than using branches, tagged releases provide better maintainability and make it easier to track changes.

**Evidence:**
```yaml
uses: EffortlessMetrics/droid-action-safe@7c1377ccbacddc95560d1570547a5baa51de01ec
```

**Recommended Fix:**
If a tagged release exists (e.g., `v5.x.x`), pin to that instead:
```yaml
uses: EffortlessMetrics/droid-action-safe@v5
```
Monitor the repository for updates and test new versions before upgrading.

---

### VULN-004: Artifact Download Without Checksum Verification

| Attribute | Value |
|-----------|-------|
| **STRIDE Category** | Tampering |
| **CWE** | CWE-494 (Download of Code Without Integrity Check) |
| **Files** | 
  - .github/workflows/publish-extension.yml (lines 60, 88, 124)
  - .github/workflows/release-server-binaries.yml (lines 53, 68)
  - .github/workflows/ci.yml (implied via upload-artifact)
| **Status** | Manual fix required |

**Description:**
Artifacts are downloaded from previous jobs without cryptographic verification. If a previous job is compromised, malicious artifacts could be used in downstream jobs.

**Evidence:**
```yaml
# From publish-extension.yml
- uses: actions/download-artifact@v8
  with:
    name: ripr-vsix
    path: dist
```

**Recommended Fix:**
Add SHA256 checksum verification after downloading artifacts:
```yaml
- uses: actions/download-artifact@v8
  with:
    name: ripr-vsix
    path: dist

- name: Verify artifact checksum
  run: |
    echo "${{ hashFiles('editors/vscode/dist/*.vsix') }}" > expected.sha256
    # In real implementation, compare against a known-good checksum
    # from a trusted source or a previous job's output
```

---

### VULN-005: Secrets in Environment Variables

| Attribute | Value |
|-----------|-------|
| **Severity** | MEDIUM |
| **STRIDE Category** | Information Disclosure |
| **CWE** | CWE-214 (Configuration Error) |
| **File** | .github/workflows/publish-extension.yml |
| **Status** | Manual fix required |

**Description:**
Personal Access Tokens (VSCE_PAT, OVSX_PAT) are passed as environment variables. While GitHub Actions masks these in logs by default, misconfiguration or debugging output could expose them. The `write` permission on the workflow also allows posting artifacts that might contain logged secrets.

**Evidence:**
```yaml
- name: Publish to VS Marketplace
  if: ${{ env.VSCE_PAT != '' }}
  run: |
    version="${RAW_VERSION#v}"
    npx -y @vscode/vsce publish --packagePath "$vsix_file" --pat "$VSCE_PAT"
```

**Recommended Fix:**
1. Ensure debug mode is disabled in production runs
2. Use `set -x` only when explicitly debugging and remove before committing
3. Consider using GitHub's environment variable masking, which is already default behavior
4. Validate that no debug output can leak secrets

---

## Appendix

### Threat Model
- Version: Newly generated (2026-06-01)
- Location: .factory/threat-model.md
- Note: Threat model was not previously present; generated during this scan

### Scan Metadata
- Commits Scanned: 1 (8ba88c7)
- Files Changed: 1627 (documentation, CI configs, workflow files, source code)
- Scan Duration: ~5 minutes
- Skills Used: workflow security analysis, static code review

### References
- [CWE Database](https://cwe.mitre.org/)
- [STRIDE Threat Model](https://docs.microsoft.com/en-us/azure/security/develop/threat-modeling-tool-threats)
- [GitHub Actions Security Best Practices](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions)
- [OIDC Token Security](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions#understanding-the-risk-of-script-injections)
