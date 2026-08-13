# Source-promotion preflight fixture

Spec: RIPR-SPEC-0148

## Given

Source and swarm each add one commit after a common base, and both commits
edit the same path. The release operation must retain both commit histories.

## When

The source-promotion preflight is run with the two complete parent SHAs and
the two repository roots.

## Then

The receipt reports a `two_parent_join` mode, one first-parent commit in each
range, and a dry-merge conflict inventory.

## Must Not

The preflight must not change either authoritative checkout, construct a join,
or report a fast-forward mode for the diverged input.

This fixture also describes the minimum discriminating case for the
`source-promotion preflight` receipt: source and swarm each add one commit
after a common base, both edit the same path, and the result must remain a
two-parent join with a reported dry-merge conflict. The fixture is metadata
for the Rust test; it is not a release candidate or a checked-in repository
pair.
