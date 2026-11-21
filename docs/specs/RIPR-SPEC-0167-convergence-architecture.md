# RIPR-SPEC-0167: Hexagonal source-to-swarm convergence architecture

Status: accepted

Issue: #3323 (umbrella: `EffortlessMetrics/ripr#1542`)

## Problem

Convergence behavior is currently distributed across orchestration and
repository surfaces without one mechanically enforced dependency direction or
capability boundary. That makes it possible for infrastructure authority,
workflow policy, or duplicate DTOs to become accidental semantic owners.

## Behavior

Continuous source-to-swarm convergence is one internally modular `xtask`
capability. Shared convergence meaning originates in `ripr-swarm` and projects
byte-identically into public `ripr`; source-only workflow, repository
administration, release, registry, marketplace, signing, and publication
adapters remain source authority. No new published crate or workflow-owned
policy surface is introduced.

### Dependency direction

```text
types/domain -> no infrastructure
ports        -> bounded capabilities over normalized types
adapters     -> Git/GitHub/filesystem/executor/clock translation
commands     -> ports plus domain decisions
workflows    -> command transport only
```

Domain decisions—proof eligibility, semantic ownership, projection
disposition, admission, invalidation, landing eligibility, and health—are pure
or deterministic over explicit inputs. Domain and types do not access the
filesystem, processes, GitHub, workflow state, serialized adapter formats, or
wall clock. Commands do not bypass ports to concrete adapters.

## Capability boundary

The shared ports separately own:

- Git graph/tree observation;
- disposable Git object construction;
- GitHub ref/PR/check/review/artifact/protection observation;
- candidate-only ref, PR, and expected-head merge transport;
- content-addressed receipt storage;
- bounded isolated execution;
- clock and lease observation;
- repository-pair and semantic-registry loading.

Observation and mutation are different traits. Candidate transport cannot
change settings, protection, releases, registries, marketplaces, signing, or
publication channels. A workflow credential cannot widen the trait a domain
use case receives.

## Implementation Mapping

| Contract | Owner |
|---|---|
| Architecture and versioned behavior | this spec |
| Rust DTOs and ports | `xtask/src/convergence/{types,ports.rs}` |
| Pure decisions | `xtask/src/convergence/domain/` |
| Infrastructure translation | `xtask/src/convergence/adapters/` |
| Use-case orchestration | `xtask/src/convergence/commands/` |
| Deterministic fixture inputs | `fixtures/convergence/` |
| Compact durable receipts/indexes | `.ripr/convergence/` |
| Source-only workflow wrappers | public `ripr` repository |
| Agent procedures | `.agents/skills/` |

Large external artifacts may be referenced only by stable identity, digest,
schema/media type, retention state, and durable-copy state. An expiring URL is
not authority. Later receipt/schema issues populate `.ripr/convergence/`; this
architecture assigns the owner without fabricating an empty receipt.

## Mechanical enforcement

The existing `cargo xtask check-architecture` gate validates the required
module/spec/fixture owners and rejects:

1. filesystem, process, wall-clock, adapter, command, or JSON dependencies in
   convergence types/domain;
2. filesystem, process, or concrete-adapter bypass in convergence commands;
3. convergence semantic-resolution tokens in workflow YAML;
4. convergence DTO/trait accumulation in `xtask/src/main.rs`;
5. removal of a canonical architecture surface.

The existing workspace-shape gate separately rejects an unreviewed crate. Rust
privacy and the port signatures keep candidate evaluation from obtaining
repository-administration or publication capabilities.

## Acceptance Examples

| Risk | Review surface | Falsifier |
|---|---|---|
| Object identity and authority | `types`, ownership table | mutable ref/current SHA cannot replace stable repository identity |
| Dependency direction | architecture gate and policy | domain imports `std::process` |
| Observation vs mutation | port traits | read-only GitHub port has no mutation method; candidate transport has no settings/release method |
| Shared vs source-only ownership | ruling and adapter boundary | source administration appears in shared ports |
| Failure vocabulary | `EvidenceState` | missing/unavailable evidence cannot default to passed |
| Receipt/schema evolution | durable owner and digest types | expiring URL becomes authority |
| Fixture ownership | `fixtures/convergence/README.md` | canonical fixture root is removed |

## Required Evidence

- compilation of the typed module skeleton and every bounded port;
- unit controls for domain/process coupling, command/adapter bypass,
  workflow-owned semantic decisions, and missing canonical surfaces;
- `cargo xtask check-architecture`, `check-workspace-shape`,
  `check-spec-format`, `check-spec-numbering`, and `check-traceability`;
- the repository's normal `precommit` and review-ready gates.

## Non-Goals

This contract implements no product-proof decision, semantic registry,
projection, transaction, admission, landing, health transition, GitHub App,
source bridge, merge, repository setting, release, tag, registry, marketplace,
signing, or publication action. It establishes the code and authority shape in
which later issues implement and prove those behaviors.

## Test Mapping

- `xtask/src/convergence/architecture.rs::tests`
- `cargo xtask check-architecture`

## Metrics

- `architecture_gate_pass_rate`
- `convergence_port_compile_pass_rate`
