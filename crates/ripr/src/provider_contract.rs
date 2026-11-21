//! Exact-snapshot RIPR provider contracts for external proof orchestration (#3298).
//!
//! The contract exposes RIPR-owned static evidence. It does not run project
//! tests, mutation tools, proof packs, builds, network clients, or source edits.

mod model;
mod validate;

pub use model::*;

#[cfg(test)]
mod tests;
