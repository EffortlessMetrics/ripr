//! Infrastructure translations into normalized convergence observations.
//!
//! Adapters implement the bounded ports. Repository-administration and public
//! release capabilities are intentionally not part of this shared surface.

pub mod clock;
pub mod executor;
pub mod filesystem;
pub mod git;
pub mod github;
