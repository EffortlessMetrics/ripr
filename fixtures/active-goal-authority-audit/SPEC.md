# Active-goal authority audit fixture

Spec: RIPR-SPEC-0128

## Purpose

This corpus proves RIPR-SPEC-0128 without changing active-goal authority.

## Inputs

- `consumers.toml` contains reviewed classification, ownership, compatibility,
  target behavior, and proof rules.
- `issue-contracts.json` captures issue-contract consumers without treating the
  capture as live GitHub state.
- `hidden-singleton/` contains a singleton reader and legacy `ready` value but
  no `.ripr/goals/active.toml`.
- `historical-reference/` contains a source-linked historical reference with
  no current authority.

## Expected behavior

The current repository and historical fixture have no unclassified consumers.
The hidden singleton fixture remains blocked because removing the manifest does
not remove its reader. Its `LEGACY_STATUS` must remain exactly `ready`; changing
or removing that token breaks the negative control, and the token alone grants
no authority. Equivalent inputs preserve the semantic digest.

## Given

A reviewed inventory, captured issue contracts, and positive and negative
repository-reader examples.

## When

The deterministic active-goal authority audit scans the fixture or repository.

## Then

Every discovered consumer is classified or reported as a migration blocker.

## Must Not

Legacy status must not authorize work, and historical text must not become live
authority.

## Non-claims

These fixtures do not select work, authorize mutation, read live GitHub state,
or prove the #1701 migration complete.
