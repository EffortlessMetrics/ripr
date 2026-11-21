//! Hexagonal boundary for source-to-swarm convergence.
//!
//! Shared convergence meaning lives here. Repository-specific workflow and
//! administration wrappers remain outside this module and consume these ports.

pub mod adapters;
pub mod architecture;
pub mod commands;
pub mod domain;
pub mod ports;
pub mod types;

/// Stable owner for the first version of the convergence architecture.
pub const ARCHITECTURE_SPEC: &str = "docs/specs/RIPR-SPEC-0167-convergence-architecture.md";

/// Canonical root for deterministic convergence fixtures.
pub const FIXTURE_ROOT: &str = "fixtures/convergence";

/// Canonical non-expiring root for compact convergence receipts and indexes.
pub const DURABLE_RECEIPT_ROOT: &str = ".ripr/convergence";
