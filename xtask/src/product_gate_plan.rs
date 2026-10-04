//! Typed product-gate authority for ordinary PR qualification.
//!
//! This module deliberately does not execute gates or select runners. It owns
//! the proposition, applicability, and claim boundary so those meanings can
//! later be consumed by several routes without deriving product truth from
//! workflow YAML.

#![allow(
    dead_code,
    reason = "canonical authority is introduced before workflow consumers migrate"
)]

use std::collections::BTreeSet;

/// The selected #3825 runner contract. Nextest owns compiled lib, bin,
/// integration, and example test binaries; Cargo and rustdoc own doctests,
/// which nextest cannot execute. Both rows are required, both run with default
/// features, and neither stands in for the other. Non-default features are
/// owned outside the required lane (Test Analytics all-features telemetry, the
/// `lang-perl` job, and the Windows advisory feature matrix).
pub(crate) const TEST_RUNNER_CONTRACT: &str = "canonical_nextest_plus_cargo_doc";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductGateId {
    Formatting,
    WorkspaceCheck,
    Clippy,
    WorkspaceTests,
    WorkspaceDocTests,
    Precommit,
    EvidencePromotionHonesty,
    AgentSkills,
    Dependencies,
    ProcessPolicy,
    NetworkPolicy,
    Goldens,
    Fixtures,
}

impl ProductGateId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Formatting => "product.rust.formatting",
            Self::WorkspaceCheck => "product.rust.workspace_check",
            Self::Clippy => "product.rust.clippy",
            Self::WorkspaceTests => "product.rust.workspace_tests",
            Self::WorkspaceDocTests => "product.rust.workspace_doc_tests",
            Self::Precommit => "product.repository.precommit",
            Self::EvidencePromotionHonesty => "product.evidence.promotion_honesty",
            Self::AgentSkills => "product.repository.agent_skills",
            Self::Dependencies => "product.repository.dependencies",
            Self::ProcessPolicy => "product.repository.process_policy",
            Self::NetworkPolicy => "product.repository.network_policy",
            Self::Goldens => "product.evidence.goldens",
            Self::Fixtures => "product.evidence.fixtures",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductSurface {
    Rust,
    RepositoryPolicy,
    Evidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductGateRole {
    Required,
    Advisory,
    Scheduled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductGateSelection {
    Selected,
    NoOp,
    Quarantined,
    NotProven,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductGateTrustClass {
    Repository,
    ExternalTree,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductGateDefinition {
    pub(crate) id: ProductGateId,
    pub(crate) role: ProductGateRole,
    pub(crate) surfaces: BTreeSet<ProductSurface>,
    pub(crate) command: &'static str,
    pub(crate) claim: &'static str,
    pub(crate) non_claim: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductGateRow {
    pub(crate) definition: ProductGateDefinition,
    pub(crate) selection: ProductGateSelection,
    pub(crate) reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductGateSubject {
    pub(crate) changed_surfaces: BTreeSet<ProductSurface>,
    pub(crate) selectors_authoritative: bool,
    pub(crate) trust_class: ProductGateTrustClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductGatePlan {
    pub(crate) rows: Vec<ProductGateRow>,
    pub(crate) full_route_reason: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductGateParity {
    pub(crate) aligned: bool,
    pub(crate) plan_gate_ids: BTreeSet<&'static str>,
    pub(crate) producer_gate_ids: BTreeSet<&'static str>,
    pub(crate) missing_from_producer: BTreeSet<&'static str>,
    pub(crate) unrepresented_producer_gates: BTreeSet<&'static str>,
}

impl ProductGateParity {
    pub(crate) fn compare(producer_gate_ids: BTreeSet<ProductGateId>) -> Self {
        let plan_gate_ids: BTreeSet<_> = product_gate_definitions()
            .into_iter()
            .map(|gate| gate.id.as_str())
            .collect();
        let producer_gate_ids: BTreeSet<_> = producer_gate_ids
            .into_iter()
            .map(ProductGateId::as_str)
            .collect();
        let missing_from_producer: BTreeSet<&'static str> = plan_gate_ids
            .difference(&producer_gate_ids)
            .copied()
            .collect();
        let unrepresented_producer_gates: BTreeSet<&'static str> = producer_gate_ids
            .difference(&plan_gate_ids)
            .copied()
            .collect();

        Self {
            aligned: missing_from_producer.is_empty() && unrepresented_producer_gates.is_empty(),
            plan_gate_ids,
            producer_gate_ids,
            missing_from_producer,
            unrepresented_producer_gates,
        }
    }
}

impl ProductGatePlan {
    pub(crate) fn for_subject(subject: &ProductGateSubject) -> Self {
        let definitions = product_gate_definitions();
        let full_route_reason = if !subject.selectors_authoritative {
            Some("selector authority is missing; the complete product route is required")
        } else if subject.trust_class == ProductGateTrustClass::ExternalTree {
            Some(
                "external-tree applicability is not yet demonstrated; the complete product route is required",
            )
        } else {
            None
        };

        let rows = definitions
            .into_iter()
            .map(|definition| {
                let selection = if full_route_reason.is_some()
                    || definition
                        .surfaces
                        .iter()
                        .any(|surface| subject.changed_surfaces.contains(surface))
                {
                    ProductGateSelection::Selected
                } else {
                    ProductGateSelection::NoOp
                };
                ProductGateRow {
                    definition,
                    selection,
                    reason: if selection == ProductGateSelection::Selected {
                        full_route_reason.unwrap_or("changed surface is applicable")
                    } else {
                        "no changed surface is applicable"
                    },
                }
            })
            .collect();

        Self {
            rows,
            full_route_reason,
        }
    }
}

fn product_gate_definitions() -> Vec<ProductGateDefinition> {
    let mut rust = BTreeSet::new();
    rust.insert(ProductSurface::Rust);
    let mut policy = BTreeSet::new();
    policy.insert(ProductSurface::RepositoryPolicy);
    let mut evidence = BTreeSet::new();
    evidence.insert(ProductSurface::Evidence);

    vec![
        gate(
            ProductGateId::Formatting,
            ProductGateRole::Required,
            rust.clone(),
            "cargo fmt --check",
            "formatting is accepted as repository product input",
            "does not prove runtime behavior",
        ),
        gate(
            ProductGateId::WorkspaceCheck,
            ProductGateRole::Required,
            rust.clone(),
            "cargo check --workspace --all-targets",
            "the workspace type-checks",
            "does not prove tests observe changed behavior",
        ),
        gate(
            ProductGateId::Clippy,
            ProductGateRole::Required,
            rust.clone(),
            "cargo clippy --workspace --all-targets -- -D warnings",
            "the configured lint contract holds",
            "does not prove runtime behavior",
        ),
        gate(
            ProductGateId::WorkspaceTests,
            ProductGateRole::Required,
            rust.clone(),
            "cargo nextest run --workspace --profile ci",
            "the default-feature workspace test binaries selected by nextest's unfiltered `ci` profile pass, and a fresh JUnit report names at least one of them",
            "does not execute Rust doctests or non-default-feature tests such as `lang-perl`, and does not prove mutation resistance",
        ),
        gate(
            ProductGateId::WorkspaceDocTests,
            ProductGateRole::Required,
            rust.clone(),
            "cargo test --workspace --doc",
            "the default-feature workspace Rust doctests compile and pass under Cargo and rustdoc",
            "does not replace nextest coverage of compiled test binaries",
        ),
        gate(
            ProductGateId::Precommit,
            ProductGateRole::Required,
            policy.clone(),
            "cargo xtask precommit",
            "repository precommit invariants hold",
            "does not replace product tests",
        ),
        gate(
            ProductGateId::EvidencePromotionHonesty,
            ProductGateRole::Required,
            evidence.clone(),
            "cargo xtask check-evidence-promotion-honesty",
            "evidence promotion claims obey their contract",
            "does not establish the underlying evidence itself",
        ),
        gate(
            ProductGateId::AgentSkills,
            ProductGateRole::Required,
            policy.clone(),
            "cargo xtask check-agent-skills",
            "checked-in agent skills obey repository policy",
            "does not validate provider execution",
        ),
        gate(
            ProductGateId::Dependencies,
            ProductGateRole::Required,
            policy.clone(),
            "cargo xtask check-dependencies",
            "dependency policy holds",
            "does not prove dependency behavior",
        ),
        gate(
            ProductGateId::ProcessPolicy,
            ProductGateRole::Required,
            policy.clone(),
            "cargo xtask check-process-policy",
            "process policy holds",
            "does not authorize arbitrary processes",
        ),
        gate(
            ProductGateId::NetworkPolicy,
            ProductGateRole::Required,
            policy.clone(),
            "cargo xtask check-network-policy",
            "network policy holds",
            "does not prove network availability",
        ),
        gate(
            ProductGateId::Goldens,
            ProductGateRole::Required,
            evidence.clone(),
            "cargo xtask goldens check",
            "golden output contracts hold",
            "does not prove unrepresented behavior",
        ),
        gate(
            ProductGateId::Fixtures,
            ProductGateRole::Required,
            evidence,
            "cargo xtask fixtures",
            "fixture contracts hold",
            "does not prove live-repository behavior",
        ),
    ]
}

fn gate(
    id: ProductGateId,
    role: ProductGateRole,
    surfaces: BTreeSet<ProductSurface>,
    command: &'static str,
    claim: &'static str,
    non_claim: &'static str,
) -> ProductGateDefinition {
    ProductGateDefinition {
        id,
        role,
        surfaces,
        command,
        claim,
        non_claim,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(surface: ProductSurface) -> ProductGateSubject {
        ProductGateSubject {
            changed_surfaces: [surface].into_iter().collect(),
            selectors_authoritative: true,
            trust_class: ProductGateTrustClass::Repository,
        }
    }

    #[test]
    fn rust_changes_select_rust_gates_and_no_op_policy_gates() {
        let plan = ProductGatePlan::for_subject(&subject(ProductSurface::Rust));

        assert!(plan.full_route_reason.is_none());
        assert!(plan.rows.iter().any(|row| {
            row.definition.id == ProductGateId::WorkspaceTests
                && row.selection == ProductGateSelection::Selected
        }));
        assert!(plan.rows.iter().any(|row| {
            row.definition.id == ProductGateId::WorkspaceDocTests
                && row.selection == ProductGateSelection::Selected
        }));
        assert!(plan.rows.iter().any(|row| {
            row.definition.id == ProductGateId::Dependencies
                && row.selection == ProductGateSelection::NoOp
        }));
    }

    #[test]
    fn workspace_test_roles_keep_nextest_and_doctests_distinct() {
        let definitions = product_gate_definitions();

        assert_eq!(TEST_RUNNER_CONTRACT, "canonical_nextest_plus_cargo_doc");
        assert!(definitions.iter().any(|gate| {
            gate.id == ProductGateId::WorkspaceTests
                && gate.command == "cargo nextest run --workspace --profile ci"
                && gate.non_claim.contains("does not execute Rust doctests")
                && gate.non_claim.contains("non-default-feature tests")
        }));
        assert!(definitions.iter().any(|gate| {
            gate.id == ProductGateId::WorkspaceDocTests
                && gate.command == "cargo test --workspace --doc"
                && gate.non_claim.contains("does not replace nextest coverage")
        }));
    }

    #[test]
    fn missing_selector_authority_selects_every_gate() {
        let mut subject = subject(ProductSurface::Rust);
        subject.selectors_authoritative = false;
        let plan = ProductGatePlan::for_subject(&subject);

        assert!(plan.full_route_reason.is_some());
        assert!(
            plan.rows
                .iter()
                .all(|row| row.selection == ProductGateSelection::Selected)
        );
    }

    #[test]
    fn external_tree_is_not_silently_scoped() {
        let mut subject = subject(ProductSurface::Evidence);
        subject.trust_class = ProductGateTrustClass::ExternalTree;
        let plan = ProductGatePlan::for_subject(&subject);

        assert_eq!(
            plan.full_route_reason,
            Some(
                "external-tree applicability is not yet demonstrated; the complete product route is required",
            )
        );
        assert!(
            plan.rows
                .iter()
                .all(|row| row.selection == ProductGateSelection::Selected)
        );
    }

    #[test]
    fn definitions_have_unique_stable_ids_and_commands() {
        let definitions = product_gate_definitions();
        let ids: BTreeSet<_> = definitions.iter().map(|gate| gate.id.as_str()).collect();
        let commands: BTreeSet<_> = definitions.iter().map(|gate| gate.command).collect();

        assert_eq!(ids.len(), definitions.len());
        assert_eq!(commands.len(), definitions.len());
        assert!(
            definitions
                .iter()
                .all(|gate| gate.role == ProductGateRole::Required)
        );
    }

    #[test]
    fn current_required_gate_inventory_is_aligned() {
        let producer_ids: BTreeSet<_> = product_gate_definitions()
            .into_iter()
            .map(|gate| gate.id)
            .collect();

        assert!(ProductGateParity::compare(producer_ids).aligned);
    }

    #[test]
    fn parity_exposes_unrepresented_required_producer() {
        let producer_ids = [ProductGateId::Formatting]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let parity = ProductGateParity::compare(producer_ids);

        assert!(!parity.aligned);
        assert!(
            parity
                .missing_from_producer
                .contains(ProductGateId::WorkspaceTests.as_str())
        );
    }

    // #3825 runner contract: bind the typed rows to the producer bytes so a
    // workflow, nextest-config, or documentation edit cannot silently drop,
    // filter, or restate a required test proposition. The parity test above
    // compares the plan with itself; these read the real files.
    const REQUIRED_WORKFLOW: &str = include_str!("../../.github/workflows/rust-gates.yml");
    const NEXTEST_CONFIG: &str = include_str!("../../.config/nextest.toml");
    const GATE_PLAN_DOC: &str = include_str!("../../docs/ci/PRODUCT_GATE_PLAN.md");

    /// Shell command lines in a workflow, with a single-line `run:` unwrapped.
    fn workflow_command_lines(workflow: &str) -> Vec<&str> {
        workflow
            .lines()
            .map(str::trim)
            .map(|line| line.strip_prefix("run: ").unwrap_or(line))
            .filter(|line| line.starts_with("cargo "))
            .collect()
    }

    /// Whether a shell line invokes a Cargo test runner however it is
    /// spelled: behind a wrapper (`env`, `timeout`, `&&`), with flags or a
    /// `+toolchain` before the subcommand, or through the `t`/`r` aliases.
    /// Aliases a repository defines in `.cargo/config.toml` are not resolved.
    fn invokes_test_runner(line: &str) -> bool {
        let tokens: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')'))
            .filter(|token| !token.is_empty())
            .collect();
        tokens.iter().enumerate().any(|(index, token)| {
            let program = token.rsplit('/').next().unwrap_or(token);
            if program == "cargo-nextest" {
                return true;
            }
            if program != "cargo" {
                return false;
            }
            let mut rest = tokens[index + 1..].iter();
            let mut positional = Vec::new();
            while let Some(token) = rest.next() {
                if matches!(*token, "--config" | "-Z" | "-C" | "--color") {
                    rest.next();
                } else if !token.starts_with('-') && !token.starts_with('+') {
                    positional.push(*token);
                    if positional.len() == 2 {
                        break;
                    }
                }
            }
            matches!(
                positional.as_slice(),
                ["test" | "t", ..] | ["nextest", "run" | "r", ..]
            )
        })
    }

    /// The required job's steps, each as its YAML lines.
    fn workflow_steps(workflow: &str) -> Vec<Vec<&str>> {
        let mut steps: Vec<Vec<&str>> = Vec::new();
        let mut in_steps = false;
        for line in workflow.lines() {
            if line.trim_end() == "    steps:" {
                in_steps = true;
            } else if in_steps && line.starts_with("      - ") {
                steps.push(vec![line]);
            } else if let Some(step) = steps.last_mut() {
                step.push(line);
            }
        }
        steps
    }

    /// Whether a YAML line (already trimmed) sets `key`, bare or quoted.
    fn sets_key(trimmed: &str, key: &str) -> bool {
        [
            format!("{key}:"),
            format!("\"{key}\":"),
            format!("'{key}':"),
        ]
        .iter()
        .any(|spelled| trimmed.starts_with(spelled.as_str()))
    }

    /// A step-level key such as `if` on a step's own lines.
    fn step_has_key(step: &[&str], key: &str) -> bool {
        step.iter().any(|line| {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
            matches!(indent, 6 | 8) && sets_key(trimmed, key)
        })
    }

    /// Workflow lines with shell `\` continuations joined, so a runner split
    /// across lines is read as the command the shell executes.
    fn logical_lines(workflow: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let mut pending = String::new();
        for line in workflow.lines() {
            let trimmed = line.trim();
            match trimmed.strip_suffix('\\') {
                Some(head) => {
                    pending.push_str(head);
                    pending.push(' ');
                }
                None => {
                    pending.push_str(trimmed);
                    lines.push(std::mem::take(&mut pending));
                }
            }
        }
        if !pending.is_empty() {
            lines.push(pending);
        }
        lines
    }

    /// Environment variables that re-policy a runner row without changing its
    /// text: nextest settings, doctest filters, and target runner wrappers.
    fn overrides_runner_environment(trimmed: &str) -> bool {
        trimmed.contains("NEXTEST_")
            || trimmed.contains("RUSTDOCFLAGS")
            || (trimmed.contains("CARGO_TARGET_") && trimmed.contains("_RUNNER"))
    }

    fn required_workflow_runner_violations(workflow: &str) -> Vec<String> {
        let lines = workflow_command_lines(workflow);
        let mut violations = Vec::new();
        for gate in product_gate_definitions() {
            if !lines.contains(&gate.command) {
                violations.push(format!(
                    "{} command `{}` is not executed by the required workflow",
                    gate.id.as_str(),
                    gate.command
                ));
            }
        }
        let runner_rows: BTreeSet<_> = [
            ProductGateId::WorkspaceTests,
            ProductGateId::WorkspaceDocTests,
        ]
        .into_iter()
        .filter_map(|id| {
            product_gate_definitions()
                .into_iter()
                .find(|gate| gate.id == id)
                .map(|gate| gate.command)
        })
        .collect();
        // Any executed line that invokes a test runner must be a declared row.
        for line in logical_lines(workflow) {
            let line = line.strip_prefix("run: ").unwrap_or(&line);
            if !line.starts_with('#') && invokes_test_runner(line) && !runner_rows.contains(line) {
                violations.push(format!(
                    "required workflow runs undeclared test command `{line}`"
                ));
            }
        }
        // Step- and job-level keys that could skip a declared row, ignore
        // its failure, or run it under another shell are rejected by text.
        for step in workflow_steps(workflow) {
            let runs_row = step.iter().any(|line| {
                let line = line.trim();
                runner_rows.contains(line.strip_prefix("run: ").unwrap_or(line))
            });
            if !runs_row {
                continue;
            }
            let name = step[0].trim().trim_start_matches("- ");
            for key in ["if", "continue-on-error", "shell"] {
                if step_has_key(&step, key) {
                    violations.push(format!(
                        "required runner step `{name}` declares `{key}:`, so the row can be skipped, ignored, or run by another shell"
                    ));
                }
            }
            // The doctest row has no guard of its own: its step must be the
            // bare command, so nothing in the step can swallow its exit.
            let doc_row = product_gate_definitions()
                .into_iter()
                .find(|gate| gate.id == ProductGateId::WorkspaceDocTests)
                .map(|gate| gate.command)
                .unwrap_or_default();
            let runs_doc_row = step.iter().any(|line| line.trim().ends_with(doc_row));
            if runs_doc_row
                && !step
                    .iter()
                    .any(|line| line.trim() == format!("run: {doc_row}"))
            {
                violations.push(format!(
                    "required runner step `{name}` must be exactly `run: {doc_row}`"
                ));
            }
        }
        for line in workflow.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            let indent = line.len() - trimmed.len();
            for key in ["if", "continue-on-error"] {
                if indent == 4 && sets_key(trimmed, key) {
                    violations.push(format!(
                        "a job in the required workflow declares `{key}:`, so a skipped or failing row can report success"
                    ));
                }
            }
            if indent <= 4 && sets_key(trimmed, "defaults") {
                violations.push(
                    "required workflow declares `defaults:`, which can replace the runner shell"
                        .to_string(),
                );
            }
            if overrides_runner_environment(trimmed) {
                violations.push(format!(
                    "required workflow re-policies a runner through the environment: `{trimmed}`"
                ));
            }
        }
        violations
    }

    /// Keys that change which tests nextest selects or how it retries them,
    /// wherever they appear in the parsed config, quoted or inline.
    fn collect_selection_keys(value: &toml::Value, path: &str, violations: &mut Vec<String>) {
        match value {
            toml::Value::Table(table) => {
                for (key, child) in table {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    if key == "default-filter" || key == "overrides" {
                        violations.push(format!(
                            "nextest config declares `{child_path}`, which narrows or re-policies the selected tests"
                        ));
                    }
                    collect_selection_keys(child, &child_path, violations);
                }
            }
            toml::Value::Array(items) => {
                for item in items {
                    collect_selection_keys(item, path, violations);
                }
            }
            _ => {}
        }
    }

    fn nextest_config_violations(config: &str) -> Vec<String> {
        let value = match toml::from_str::<toml::Value>(config) {
            Ok(value) => value,
            Err(err) => return vec![format!("nextest config does not parse: {err}")],
        };
        let mut violations = Vec::new();
        collect_selection_keys(&value, "", &mut violations);
        let ci = value.get("profile").and_then(|profile| profile.get("ci"));
        if ci
            .and_then(|ci| ci.get("retries"))
            .and_then(toml::Value::as_integer)
            != Some(0)
        {
            violations.push("`[profile.ci]` must pin `retries = 0`".to_string());
        }
        if ci
            .and_then(|ci| ci.get("junit"))
            .and_then(|junit| junit.get("path"))
            .and_then(toml::Value::as_str)
            != Some("junit.xml")
        {
            violations.push("`[profile.ci.junit]` must write `junit.xml`".to_string());
        }
        violations
    }

    #[test]
    fn required_workflow_executes_exactly_the_declared_runner_rows() {
        assert_eq!(
            required_workflow_runner_violations(REQUIRED_WORKFLOW),
            Vec::<String>::new()
        );
    }

    #[test]
    fn dropping_or_filtering_a_required_runner_row_is_a_violation() {
        let dropped_doctests =
            REQUIRED_WORKFLOW.replace("run: cargo test --workspace --doc", "run: true");
        assert_ne!(
            dropped_doctests, REQUIRED_WORKFLOW,
            "fixture must remove the doctest row"
        );
        assert!(
            required_workflow_runner_violations(&dropped_doctests)
                .iter()
                .any(|violation| violation.contains("product.rust.workspace_doc_tests"))
        );

        let filtered = REQUIRED_WORKFLOW.replace(
            "          cargo nextest run --workspace --profile ci\n",
            "          cargo nextest run --workspace --profile ci -E 'not test(framed_lsp_)'\n",
        );
        assert_ne!(
            filtered, REQUIRED_WORKFLOW,
            "fixture must filter the nextest row"
        );
        let violations = required_workflow_runner_violations(&filtered);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("product.rust.workspace_tests"))
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("undeclared test command"))
        );

        let wrapped = REQUIRED_WORKFLOW.replace(
            "        run: cargo test --workspace --doc\n",
            "        run: |\n          cargo test --workspace --doc\n          timeout 60 cargo test --workspace --lib -- --skip framed_lsp_\n",
        );
        assert_ne!(
            wrapped, REQUIRED_WORKFLOW,
            "fixture must add a wrapped runner line"
        );
        assert!(
            required_workflow_runner_violations(&wrapped)
                .iter()
                .any(|v| v.contains("timeout 60 cargo test"))
        );

        // Respelled runners, a skipped or ignored row, and environment
        // overrides of the `ci` profile each keep the row text intact.
        let doctest_step = "      - name: Required Rust doctests\n";
        let tests_step = "      - name: Required Rust tests\n        id: rust-tests\n";
        let cases = [
            (
                doctest_step,
                "      - name: Required Rust doctests\n        if: false\n",
                "declares `if:`",
            ),
            (
                doctest_step,
                "      - name: Required Rust doctests\n        continue-on-error: true\n",
                "declares `continue-on-error:`",
            ),
            (
                "    timeout-minutes: ${{ inputs.job-timeout-minutes }}\n",
                "    timeout-minutes: ${{ inputs.job-timeout-minutes }}\n    continue-on-error: true\n",
                "a job in the required workflow declares `continue-on-error:`",
            ),
            (
                tests_step,
                "      - name: Required Rust tests\n        id: rust-tests\n        env:\n          NEXTEST_RETRIES: \"3\"\n",
                "through the environment",
            ),
            (
                "        run: cargo test --workspace --doc\n",
                "        run: |\n          cargo test --workspace --doc\n          cargo --locked test --workspace --lib -- --skip framed_lsp_\n",
                "cargo --locked test",
            ),
            (
                "        run: cargo test --workspace --doc\n",
                "        run: |\n          cargo test --workspace --doc\n          cargo +1.95.0 nextest r --workspace -E 'not test(framed_lsp_)'\n",
                "cargo +1.95.0 nextest r",
            ),
            (
                "    timeout-minutes: ${{ inputs.job-timeout-minutes }}\n",
                "    timeout-minutes: ${{ inputs.job-timeout-minutes }}\n    if: ${{ false }}\n",
                "a job in the required workflow declares `if:`",
            ),
            (
                doctest_step,
                "      - name: Required Rust doctests\n        \"if\": false\n",
                "declares `if:`",
            ),
            (
                doctest_step,
                "      - name: Required Rust doctests\n        shell: bash -c 'exit 0' {0}\n",
                "declares `shell:`",
            ),
            (
                "    steps:\n",
                "    defaults:\n      run:\n        shell: bash -c 'exit 0' {0}\n    steps:\n",
                "declares `defaults:`",
            ),
            (
                doctest_step,
                "      - name: Required Rust doctests\n        env:\n          RUSTDOCFLAGS: --test-args=--skip=value_resolution\n",
                "through the environment",
            ),
            (
                tests_step,
                "      - name: Required Rust tests\n        id: rust-tests\n        env:\n          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER: true\n",
                "through the environment",
            ),
            (
                "        run: cargo test --workspace --doc\n",
                "        run: |\n          trap 'exit 0' EXIT\n          cargo test --workspace --doc\n",
                "must be exactly `run: cargo test --workspace --doc`",
            ),
            (
                "        run: cargo test --workspace --doc\n",
                "        run: |\n          cargo test --workspace --doc\n          cargo \\\n            nextest run --workspace -E 'not test(framed_lsp_)'\n",
                "nextest run --workspace -E 'not test(framed_lsp_)'`",
            ),
        ];
        for (anchor, replacement, expected) in cases {
            assert_eq!(
                REQUIRED_WORKFLOW.matches(anchor).count(),
                1,
                "fixture anchor must be unique: {anchor}"
            );
            let edited = REQUIRED_WORKFLOW.replacen(anchor, replacement, 1);
            let violations = required_workflow_runner_violations(&edited);
            assert!(
                violations.iter().any(|v| v.contains(expected)),
                "{expected}: {violations:#?}"
            );
        }
        for spelled in [
            "cargo --locked test --workspace",
            "env CARGO_TERM_COLOR=never cargo t --workspace",
            "cargo --config net.offline=true nextest run --workspace",
            "true && ~/.cargo/bin/cargo-nextest nextest run",
        ] {
            assert!(invokes_test_runner(spelled), "{spelled}");
        }
        for not_a_runner in [
            "cargo nextest --version",
            "cargo xtask test-oracle-report",
            "cargo clippy --workspace --all-targets -- -D warnings",
            "printf 'profile=ci\\ncommand=cargo nextest run --workspace --profile ci\\n'",
        ] {
            assert!(!invokes_test_runner(not_a_runner), "{not_a_runner}");
        }
    }

    #[test]
    fn required_nextest_profile_cannot_filter_retry_or_drop_junit() {
        assert_eq!(
            nextest_config_violations(NEXTEST_CONFIG),
            Vec::<String>::new()
        );

        let filtered =
            format!("{NEXTEST_CONFIG}\n[profile.default]\ndefault-filter = \"not test(slow)\"\n");
        assert!(!nextest_config_violations(&filtered).is_empty());

        let overridden =
            format!("{NEXTEST_CONFIG}\n[[profile.ci.overrides]]\nfilter = \"test(slow)\"\n");
        assert!(!nextest_config_violations(&overridden).is_empty());

        let retried = NEXTEST_CONFIG.replace("retries = 0", "retries = 2");
        assert_ne!(retried, NEXTEST_CONFIG, "fixture must change retries");
        assert!(!nextest_config_violations(&retried).is_empty());

        let no_junit = NEXTEST_CONFIG.replace("path = \"junit.xml\"", "");
        assert_ne!(no_junit, NEXTEST_CONFIG, "fixture must drop the JUnit path");
        assert!(!nextest_config_violations(&no_junit).is_empty());

        // Quoted and inline spellings parse to the same keys nextest reads.
        let quoted = NEXTEST_CONFIG.replace(
            "[profile.ci]\n",
            "[profile.ci]\n\"default-filter\" = \"not test(framed_lsp_)\"\n",
        );
        assert_ne!(quoted, NEXTEST_CONFIG, "fixture must add a quoted filter");
        assert!(
            nextest_config_violations(&quoted)
                .iter()
                .any(|v| v.contains("profile.ci.default-filter"))
        );
        let inline = NEXTEST_CONFIG.replace(
            "[profile.ci]\n",
            "[profile.ci]\noverrides = [{ filter = \"test(slow)\", retries = 3 }]\n",
        );
        assert_ne!(inline, NEXTEST_CONFIG, "fixture must add inline overrides");
        assert!(
            nextest_config_violations(&inline)
                .iter()
                .any(|v| v.contains("profile.ci.overrides"))
        );
        let table_retries = NEXTEST_CONFIG.replace(
            "retries = 0",
            "retries = { backoff = \"fixed\", count = 2 }",
        );
        assert_ne!(table_retries, NEXTEST_CONFIG, "fixture must retry");
        assert!(!nextest_config_violations(&table_retries).is_empty());
        assert!(!nextest_config_violations("[profile.ci\n").is_empty());
    }

    /// Runs the real `Required Rust tests` step body under the Actions
    /// default shell (`bash -eo pipefail`) with `cargo`, `git`, and `rustc`
    /// stubbed, so the fresh-report guard and exit propagation are executed,
    /// not string-matched.
    #[cfg(target_os = "linux")]
    mod required_test_step {
        use std::process::Command;

        use super::REQUIRED_WORKFLOW;

        const STUBS: &str = r#"
cargo() {
  if [ "$1 $2" = "nextest run" ]; then
    if [ -n "${STUB_JUNIT:-}" ]; then printf '%s\n' "$STUB_JUNIT" > target/nextest/ci/junit.xml; fi
    return "$STUB_EXIT"
  fi
  echo "cargo stub $*"
}
git() { echo 0000000000000000000000000000000000000000; }
rustc() { echo "rustc stub"; }
"#;

        fn step_body() -> String {
            crate::extract_workflow_run_blocks(REQUIRED_WORKFLOW)
                .into_iter()
                .find(|block| {
                    block
                        .text
                        .contains("cargo nextest run --workspace --profile ci")
                        && block.text.contains("junit.xml")
                })
                .map(|block| block.text)
                .unwrap_or_default()
        }

        /// Returns the step's exit code and the run context it wrote.
        fn run(
            label: &str,
            stub_exit: u8,
            junit: &str,
            stale: bool,
        ) -> Result<(Option<i32>, String), String> {
            let body = step_body();
            assert!(!body.is_empty(), "Required Rust tests step not found");
            let dir = crate::tests::temp_dir(&format!("required-test-step-{label}"));
            let ci = dir.join("target/nextest/ci");
            std::fs::create_dir_all(&ci).map_err(|err| err.to_string())?;
            if stale {
                let stale_report = r#"<testsuites name="nextest-run" tests="9">"#;
                std::fs::write(ci.join("junit.xml"), stale_report)
                    .map_err(|err| err.to_string())?;
            }
            let script = dir.join("step.sh");
            std::fs::write(&script, format!("{STUBS}\n{body}\n")).map_err(|err| err.to_string())?;
            let output = Command::new("bash")
                .args(["--noprofile", "--norc", "-eo", "pipefail"])
                .arg(&script)
                .current_dir(&dir)
                .env("STUB_EXIT", stub_exit.to_string())
                .env("STUB_JUNIT", junit)
                .env("GITHUB_REPOSITORY", "EffortlessMetrics/ripr-swarm")
                .env("GITHUB_RUN_ID", "1")
                .env("GITHUB_RUN_ATTEMPT", "1")
                .output();
            let context = std::fs::read_to_string(ci.join("run-context.txt")).unwrap_or_default();
            let _ = std::fs::remove_dir_all(&dir);
            let code = output.ok().and_then(|output| output.status.code());
            Ok((code, context))
        }

        const NAMED: &str = r#"<testsuites name="nextest-run" tests="3" failures="0">"#;

        #[test]
        fn a_fresh_report_naming_tests_passes() -> Result<(), String> {
            let (code, context) = run("pass", 0, NAMED, false)?;
            assert_eq!(code, Some(0));
            assert!(context.contains("nextest_exit=0"), "{context}");
            assert!(
                context.contains("command=cargo nextest run --workspace --profile ci"),
                "{context}"
            );
            Ok(())
        }

        #[test]
        fn a_green_run_without_a_fresh_named_report_fails() -> Result<(), String> {
            for (label, junit, stale) in [
                (
                    "zero",
                    r#"<testsuites name="nextest-run" tests="0">"#,
                    false,
                ),
                (
                    "leading-zero",
                    r#"<testsuites name="nextest-run" tests="05">"#,
                    false,
                ),
                ("junk", "not xml", false),
                ("missing", "", false),
                ("stale", "", true),
            ] {
                let (code, _) = run(label, 0, junit, stale)?;
                assert_eq!(code, Some(1), "{label}");
            }
            Ok(())
        }

        #[test]
        fn a_failing_run_keeps_its_exit_code_and_context() -> Result<(), String> {
            let (code, context) = run("fail", 100, NAMED, false)?;
            assert_eq!(code, Some(100));
            assert!(context.contains("nextest_exit=100"), "{context}");
            Ok(())
        }
    }

    #[test]
    fn gate_plan_document_restates_exactly_the_typed_rows() {
        let documented: BTreeSet<(&str, &str)> = GATE_PLAN_DOC
            .lines()
            .filter_map(|line| {
                let cells: Vec<_> = line.split('|').map(str::trim).collect();
                let id = cells.get(1)?.strip_prefix('`')?.strip_suffix('`')?;
                let command = cells.get(2)?.strip_prefix('`')?.strip_suffix('`')?;
                id.starts_with("product.").then_some((id, command))
            })
            .collect();
        let typed: BTreeSet<(&str, &str)> = product_gate_definitions()
            .into_iter()
            .map(|gate| (gate.id.as_str(), gate.command))
            .collect();

        assert_eq!(documented, typed);
    }
}
