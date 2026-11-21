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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProductGateId {
    Formatting,
    WorkspaceCheck,
    Clippy,
    WorkspaceTests,
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
            "cargo nextest run --workspace",
            "the workspace test suite passes",
            "does not prove mutation resistance",
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
            row.definition.id == ProductGateId::Dependencies
                && row.selection == ProductGateSelection::NoOp
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
}
