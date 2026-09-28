//! Pure convergence decisions over explicit typed inputs.
//!
//! Each module is an owned seam for a later behavior slice. No module may read
//! the filesystem, execute a process, call GitHub, or observe wall-clock time.

pub mod admission;
pub mod health;
pub mod landing;
pub mod product_proof;
pub mod projection;
pub mod semantic_registry;
pub mod transaction;
