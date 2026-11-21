# Rust Usable Gap Projection Plan

This directory records the source-of-truth rails for the Rust Usable Gap
Projection lane. The lane is closed; these files exist so future agents can
recover the shipped behavior from repo artifacts instead of chat history.

## Documents

- [Implementation plan](implementation-plan.md) - PR-sized sequence that shipped
  the lane and the validation attached to each slice.
- [Agent route](agent-context-route.md) - read-first packet, touched surfaces,
  stop conditions, and proof commands for future gap-projection repair work.

## Role Model

| Artifact | Job |
| --- | --- |
| Proposal | explains why Rust gap projection exists and what user problem it solves |
| Spec | defines the evidence-to-gap and gap-ledger behavior contracts |
| Plan | sequences scoped implementation and proof commands |
| Manifest | records the closed execution state and landed work items |
| Traceability | maps specs to code, fixtures, outputs, metrics, and docs |
| Policy ledgers | own suppression, waiver, badge, and gate authority |
| Closeout | records shipped proof, non-changes, limits, and restart context |

Specs and output contracts remain the behavior source of truth. This plan does
not create analyzer truth, gate authority, schema authority, or editor behavior.

## Current State

The lane is closed by
[Rust Usable Gap Projection closeout](../../docs/handoffs/2026-05-15-rust-usable-gap-projection-closeout.md).

The durable proposal is
[RIPR-PROP-0006](../../docs/proposals/RIPR-PROP-0006-rust-usable-gap-projection.md).

The behavior contracts are
[RIPR-SPEC-0045](../../docs/specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
and
[RIPR-SPEC-0046](../../docs/specs/RIPR-SPEC-0046-gap-decision-ledger.md).

