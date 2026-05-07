# Threat Model for ripr

**Generated:** 2026-05-07
**Repository:** EffortlessMetrics/ripr
**Tool:** STRIDE-based threat analysis

## Overview

ripr is a static mutation-exposure analyzer for Rust/Cargo workspaces. It takes code diffs as input, performs static analysis, and produces exposure reports indicating whether tests would catch behavior changes.

## Components Analyzed

### 1. Diff Input Processing (`crates/ripr/src/analysis/diff/`)
- Loads and parses diff files
- Handles file path references in diffs
- **STRIDE Analysis:**
  - **Spoofing (MEDIUM):** Malicious diff files with crafted paths could reference sensitive system files
  - **Tampering (LOW):** Diff content could be modified if written to writable paths
  - **Information Disclosure (HIGH):** Path traversal in diff loading (`../` in paths) could read arbitrary files
  - **Denial of Service (MEDIUM):** Malformed diffs with extreme line counts could cause resource exhaustion

### 2. CLI Layer (`crates/ripr/src/cli/`)
- Command parsing and execution
- File path argument handling
- **STRIDE Analysis:**
  - **Spoofing (MEDIUM):** Arguments could specify paths to malicious symlinks
  - **Tampering (LOW):** Output file paths could be overwritten via path traversal
  - **Elevation of Privilege (MEDIUM):** If running with elevated privileges, path traversal could write to sensitive locations

### 3. Analysis Engine (`crates/ripr/src/analysis/`)
- Classifier, seam inventory, rust_index, probes
- Parses Rust source code using rust-analyzer
- **STRIDE Analysis:**
  - **Information Disclosure (LOW):** Analysis results might include sensitive code context
  - **Denial of Service (MEDIUM):** Complex code structures could cause deep recursion or large memory allocation

### 4. Configuration (`crates/ripr/src/config.rs`)
- Loads `ripr.toml` configuration
- **STRIDE Analysis:**
  - **Tampering (MEDIUM):** Malicious config files could modify analysis behavior
  - **Information Disclosure (LOW):** Config might expose project structure

### 5. LSP Server (`crates/ripr/src/lsp/`)
- tower-lsp-server backend for editor integration
- **STRIDE Analysis:**
  - **Spoofing (MEDIUM):** LSP clients could send malicious requests
  - **Denial of Service (MEDIUM):** Large requests could cause resource exhaustion

### 6. Output Renderers (`crates/ripr/src/output/`)
- human, JSON, GitHub, SARIF, badge outputs
- **STRIDE Analysis:**
  - **Tampering (HIGH):** Path traversal in output file paths could overwrite arbitrary files
  - **Information Disclosure (MEDIUM):** Output might expose system paths or internal structure

### 7. xtask Automation (`xtask/src/`)
- Repo automation and policy checking
- **STRIDE Analysis:**
  - **Tampering (MEDIUM):** Policy files could be modified to bypass checks
  - **Elevation of Privilege (MEDIUM):** xtask commands might execute arbitrary code

### 8. VS Code Extension (`editors/vscode/`)
- TypeScript LSP client
- Downloads and executes ripr binary
- **STRIDE Analysis:**
  - **Spoofing (MEDIUM):** Updates could be delivered via compromised channels
  - **Tampering (HIGH):** Downloaded binaries could be replaced with malicious ones
  - **Elevation of Privilege (MEDIUM):** Extension runs with VS Code privileges

## Key Threat Summary

| Category | Severity | Key Threats |
|----------|----------|------------|
| **Spoofing** | MEDIUM | Malicious symlinks, LSP client impersonation |
| **Tampering** | HIGH | Path traversal in output file paths, config manipulation |
| **Repudiation** | MEDIUM | No cryptographic audit trail for analysis results |
| **Information Disclosure** | HIGH | Path traversal in diff loading could read arbitrary files |
| **Denial of Service** | MEDIUM | Malformed diffs, complex code structures, large LSP requests |
| **Elevation of Privilege** | MEDIUM | Suppression file bypass, xtask with elevated privileges |

## Mitigations in Place

1. **unsafe_code = "forbid"** - Workspace-wide prohibition of unsafe code
2. **No panics** - Enforced via xtask check
3. **Strict input validation** - Diff parsing with error handling
4. **Read-only analysis** - Static analysis without execution

## Recommendations

1. Add path canonicalization to prevent path traversal
2. Implement input size limits for diff files
3. Add audit logging for analysis operations
4. Consider signing for binary downloads
