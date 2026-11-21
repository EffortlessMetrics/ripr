//! Mechanical dependency-direction checks for the convergence hexagon.

const REQUIRED_SURFACES: &[&str] = &[
    "xtask/src/convergence/mod.rs",
    "xtask/src/convergence/types/mod.rs",
    "xtask/src/convergence/ports.rs",
    "xtask/src/convergence/domain/mod.rs",
    "xtask/src/convergence/domain/product_proof.rs",
    "xtask/src/convergence/domain/semantic_registry.rs",
    "xtask/src/convergence/domain/projection.rs",
    "xtask/src/convergence/domain/transaction.rs",
    "xtask/src/convergence/domain/admission.rs",
    "xtask/src/convergence/domain/landing.rs",
    "xtask/src/convergence/domain/health.rs",
    "xtask/src/convergence/adapters/mod.rs",
    "xtask/src/convergence/adapters/git.rs",
    "xtask/src/convergence/adapters/github.rs",
    "xtask/src/convergence/adapters/filesystem.rs",
    "xtask/src/convergence/adapters/executor.rs",
    "xtask/src/convergence/adapters/clock.rs",
    "xtask/src/convergence/commands/mod.rs",
    "docs/specs/RIPR-SPEC-0167-convergence-architecture.md",
    "fixtures/convergence/README.md",
];

const DOMAIN_FORBIDDEN: &[(&str, &str)] = &[
    ("std::fs", "filesystem access"),
    ("std::process", "process execution"),
    ("std::time", "wall-clock observation"),
    ("crate::convergence::adapters", "concrete adapter access"),
    ("crate::convergence::commands", "command-layer access"),
    ("serde_json", "serialized adapter representation"),
];

const COMMAND_FORBIDDEN: &[(&str, &str)] = &[
    ("std::fs", "filesystem bypass"),
    ("std::process", "executor bypass"),
    ("crate::convergence::adapters", "concrete adapter bypass"),
];

const PORT_FORBIDDEN: &[(&str, &str)] = &[
    (
        "trait GitHubPort",
        "broad observation and mutation capability",
    ),
    ("fn update_settings", "repository administration"),
    ("fn set_protection", "repository protection administration"),
    ("fn publish_release", "release publication"),
    ("fn publish_registry", "registry publication"),
    ("fn publish_marketplace", "marketplace publication"),
];

const WORKFLOW_SEMANTIC_TOKENS: &[&str] = &[
    "manual_decision_required",
    "semantic_resolution",
    "projection_disposition",
    "auto_merge_eligibility",
];

pub fn is_source_candidate(path: &str) -> bool {
    path == "xtask/src/main.rs"
        || path.starts_with("xtask/src/convergence/")
        || path.starts_with(".github/workflows/")
}

pub fn source_violations(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    if path.starts_with("xtask/src/convergence/domain/")
        || path.starts_with("xtask/src/convergence/types/")
    {
        push_forbidden(path, text, DOMAIN_FORBIDDEN, &mut violations);
    }
    if path.starts_with("xtask/src/convergence/commands/") {
        push_forbidden(path, text, COMMAND_FORBIDDEN, &mut violations);
    }
    if path == "xtask/src/convergence/ports.rs" {
        push_forbidden(path, text, PORT_FORBIDDEN, &mut violations);
    }
    if path.starts_with(".github/workflows/") {
        for token in WORKFLOW_SEMANTIC_TOKENS {
            if text.contains(token) {
                violations.push(format!(
                    "{path} contains convergence semantic token `{token}`\n  reason: workflow transport must call the Rust convergence authority instead of owning semantic decisions"
                ));
            }
        }
    }
    if path == "xtask/src/main.rs" {
        for token in owned_definition_tokens() {
            if text.contains(&token) {
                violations.push(format!(
                    "{path} contains convergence implementation token `{token}`\n  reason: convergence implementation belongs in xtask/src/convergence, not the xtask mega-module"
                ));
            }
        }
    }
    violations
}

pub fn required_surface_violations(files: &[String]) -> Vec<String> {
    REQUIRED_SURFACES
        .iter()
        .filter(|required| !files.iter().any(|file| file == *required))
        .map(|required| {
            format!(
                "missing required convergence architecture surface `{required}`\n  reason: RIPR-SPEC-0167 assigns one canonical owner to code, spec, and fixtures"
            )
        })
        .collect()
}

fn push_forbidden(path: &str, text: &str, rules: &[(&str, &str)], violations: &mut Vec<String>) {
    for (token, capability) in rules {
        if contains_dependency(text, token) {
            violations.push(format!(
                "{path} contains forbidden convergence dependency `{token}`\n  reason: {capability} must enter through a bounded port"
            ));
        }
    }
}

fn contains_dependency(text: &str, dependency: &str) -> bool {
    if text.contains(dependency) {
        return true;
    }

    let compact = text
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    for (separator, _) in dependency.match_indices("::") {
        let root = &dependency[..separator];
        let member = &dependency[separator + 2..];
        let group_prefix = format!("{root}::{{");
        let mut remainder = compact.as_str();
        while let Some(start) = remainder.find(&group_prefix) {
            let group = &remainder[start + group_prefix.len()..];
            let Some(end) = group.find("};") else {
                break;
            };
            if group[..end].split(',').any(|item| {
                item == member
                    || item.starts_with(&format!("{member}::"))
                    || item.starts_with(&format!("{member}as"))
            }) {
                return true;
            }
            remainder = &group[end + 2..];
        }
    }
    false
}

fn owned_definition_tokens() -> Vec<String> {
    [include_str!("ports.rs"), include_str!("types/mod.rs")]
        .into_iter()
        .flat_map(str::lines)
        .filter_map(|line| {
            let line = line.trim_start();
            ["pub struct ", "pub enum ", "pub trait "]
                .into_iter()
                .find_map(|prefix| line.strip_prefix(prefix).map(|tail| (prefix, tail)))
        })
        .filter_map(|(prefix, tail)| {
            let name = tail
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .next()?;
            (!name.is_empty()).then(|| format!("{}{name}", prefix.trim_start_matches("pub ")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        REQUIRED_SURFACES, is_source_candidate, required_surface_violations, source_violations,
    };

    #[test]
    fn source_candidate_filter_is_bounded() {
        assert!(is_source_candidate("xtask/src/convergence/ports.rs"));
        assert!(is_source_candidate(".github/workflows/converge.yml"));
        assert!(!is_source_candidate("fixtures/large/output.json"));
    }

    #[test]
    fn architecture_gate_invokes_convergence_checks() {
        let main = include_str!("../main.rs");
        assert!(main.contains("convergence::architecture::source_violations"));
        assert!(main.contains("convergence::architecture::required_surface_violations"));
    }

    #[test]
    fn convergence_fixture_root_uses_manifest_only_contract() -> Result<(), String> {
        let path = std::path::Path::new("fixtures/convergence");
        assert!(crate::reports::is_manifest_only_fixture_dir(path));
        assert_eq!(
            crate::reports::fixture_contract_violations(path)?,
            Vec::<String>::new()
        );
        Ok(())
    }

    #[test]
    fn domain_process_access_is_rejected() {
        let violations = source_violations(
            "xtask/src/convergence/domain/projection.rs",
            concat!("use std::", "process::Command;"),
        );
        assert!(violations.iter().any(|item| item.contains("process")));
    }

    #[test]
    fn grouped_domain_imports_are_rejected() {
        let violations = source_violations(
            "xtask/src/convergence/domain/projection.rs",
            "use std::{fs, process::Command, time::SystemTime};",
        );
        assert_eq!(violations.len(), 3);
    }

    #[test]
    fn command_adapter_bypass_is_rejected() {
        let violations = source_violations(
            "xtask/src/convergence/commands/project.rs",
            "use crate::convergence::adapters::git::GitAdapter;",
        );
        assert!(violations.iter().any(|item| item.contains("adapter")));

        let grouped = source_violations(
            "xtask/src/convergence/commands/project.rs",
            "use crate::convergence::{adapters::git::GitAdapter, ports::GitObservation};",
        );
        assert!(grouped.iter().any(|item| item.contains("adapter")));
    }

    #[test]
    fn workflow_owned_semantic_resolution_is_rejected() {
        let violations = source_violations(
            ".github/workflows/converge.yml",
            "projection_disposition: select_source",
        );
        assert!(
            violations
                .iter()
                .any(|item| item.contains("Rust convergence authority"))
        );
    }

    #[test]
    fn broad_credential_bearing_port_is_rejected() {
        let violations = source_violations(
            "xtask/src/convergence/ports.rs",
            "pub trait GitHubPort { fn update_settings(&mut self); }",
        );
        assert_eq!(violations.len(), 2);
        assert!(
            violations
                .iter()
                .any(|item| item.contains("administration"))
        );
    }

    #[test]
    fn canonical_convergence_definitions_are_rejected_in_main() {
        let violations = source_violations(
            "xtask/src/main.rs",
            "struct RepositoryPair; trait CandidateTransport {}",
        );
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn unrelated_workflow_transport_is_allowed() {
        assert!(
            source_violations(
                ".github/workflows/converge.yml",
                "cargo xtask converge plan"
            )
            .is_empty()
        );
    }

    #[test]
    fn missing_canonical_surface_fails_closed() {
        let files = REQUIRED_SURFACES
            .iter()
            .skip(1)
            .map(|path| (*path).to_string())
            .collect::<Vec<_>>();
        let violations = required_surface_violations(&files);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains(REQUIRED_SURFACES[0]));
    }
}
