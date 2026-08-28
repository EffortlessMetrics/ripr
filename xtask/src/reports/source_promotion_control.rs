//! Typed admission, exact-join construction, and candidate-ref publication.
//!
//! These commands are the only source-owned authority for converting one
//! terminal-green resolved-tree packet into an unreferenced exact join object
//! and, later, publishing that exact object behind an expected-state guard.

include!("source_promotion_control/core.rs");
include!("source_promotion_control/io.rs");
include!("source_promotion_control/admission.rs");
include!("source_promotion_control/construction.rs");
include!("source_promotion_control/publication.rs");
include!("source_promotion_control/tests.rs");
