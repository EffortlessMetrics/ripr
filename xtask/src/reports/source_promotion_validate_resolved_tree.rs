//! Source-trusted validation of one exact reviewed source/W7 tree.
//!
//! The command is compiled from the held source parent, validates the complete
//! preflight and resolution contracts, materializes the reviewed tree without
//! moving an authoritative ref, and executes the source-owned governance
//! catalog with bounded retained evidence.

include!("source_promotion_validate_resolved_tree/core.rs");
include!("source_promotion_validate_resolved_tree/validation.rs");
include!("source_promotion_validate_resolved_tree/materialization.rs");
include!("source_promotion_validate_resolved_tree/receipt.rs");
include!("source_promotion_validate_resolved_tree/io.rs");
include!("source_promotion_validate_resolved_tree/tests.rs");
