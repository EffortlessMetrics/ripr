# Source-promotion resolved-tree fixture

Spec: RIPR-SPEC-0150

This is a dedicated receipt-snapshot corpus for
`ripr.source_promotion_resolved_tree_validation.v1`. Its dedicated validator
owns the exact JSON and Markdown members, schema and status checks, semantic
admission checks, and canonical embedded JSON mirrors. It is not a
Given/When/Then analyzer fixture and therefore has no `diff.patch` or
`expected/check.json`.

The J5 final-tree behavioral corpus lives in
`xtask/tests/source_promotion_resolved_tree.rs` and executes the production
`check-network-policy` command against a Git-tracked temporary repository.