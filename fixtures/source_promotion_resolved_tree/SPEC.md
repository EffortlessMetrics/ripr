# Source-promotion resolved-tree fixture

This fixture pins the canonical rejected and validated receipt shapes for
`ripr.source_promotion_resolved_tree_validation.v1`. The fixture state contains
no real source/W7/tree identity and makes no validation claim; it exists to
prove deterministic key sets, explicit `not_run` reasons, fixed timeout bounds,
and byte-stable JSON/Markdown rendering.

The J5 final-tree behavioral corpus lives in
`xtask/tests/source_promotion_resolved_tree.rs` and executes the production
`check-network-policy` command against a Git-tracked temporary repository.
