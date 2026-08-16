# Fixture: lsp_agent_protocol

Spec: RIPR-SPEC-0131

## Purpose

This fixture corpus pins the capability-only v0.1 wire contract for generic
headless clients. It tests vocabulary and envelope shape without pretending
that any reserved request handler exists.

## Given

- `capability.json` is the initialize capability projection.
- `requests/` contains one valid envelope for every reserved request.
- `errors/` contains one valid envelope for every reserved error kind.
- `success-envelope.json` keeps snapshot, input, profile, and budget identities
  distinct and includes explicit edit boundaries and non-claims.
- `negative/` contains unsupported-version and unsupported-profile examples.

## When

A generic client validates the examples against the repository-owned schemas in
`schemas/ripr/`.

## Then

The valid examples are deterministic and closed over the v0.1 vocabulary. The
negative examples are rejected visibly. No example advertises a supported
request, source edit, continuation, progress, cancellation, or autonomous
repair.

## Must Not

- Treat a reserved request as implemented.
- Collapse snapshot, input, profile, and budget identities.
- Treat a limitation or non-claim as evidence of runtime adequacy.
- Infer a repair edit from a missing edit boundary.

## Non-claims

These fixtures do not prove transport behavior, request lifecycle behavior,
snapshot freshness, complete evidence retrieval, or repair usefulness.
