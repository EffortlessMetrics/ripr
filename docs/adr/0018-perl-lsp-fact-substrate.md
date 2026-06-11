# ADR 0018: Perl LSP Fact Substrate

Status: proposed

Date: 2026-06-04

Owner: language-adapter / swarm

Artifact ID: RIPR-ADR-0018

Linked proposal:

- [RIPR-PROP-0018: Perl Repair Routing Lane](../proposals/RIPR-PROP-0018-perl-repair-routing-lane.md)

Linked specs:

- [RIPR-SPEC-0026: Language adapter contract](../specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0064: Perl fact packet contract](../specs/RIPR-SPEC-0064-perl-fact-packet-contract.md)
- [RIPR-SPEC-0057: RIPR swarm repair loop](../specs/RIPR-SPEC-0057-ripr-swarm-repair-loop.md)
- [RIPR-SPEC-0058: RIPR swarm external agent handoff](../specs/RIPR-SPEC-0058-ripr-swarm-external-agent-handoff.md)
- [RIPR-SPEC-0061: Lane 1 canonical actionability contract](../specs/RIPR-SPEC-0061-lane1-canonical-actionability-contract.md)

## Context

Perl repair routing needs richer language facts than a small syntax-first
extractor can safely infer. Real Perl projects rely on package and module
resolution, dynamic dispatch, role composition, test helper indirection,
generated symbols, monkeypatching, and framework conventions. Those are exactly
the cases where a RIPR-owned Perl parser would be most likely to produce
repair-shaped false positives.

The Perl lane's product value is not "RIPR can parse Perl." The useful loop is:

```text
Perl repo / PR
-> perl-lsp fact export
-> ripr PerlAdapter
-> changed package/sub/script owner
-> RIPR evidence
-> canonical Perl gap or named limitation
-> repair card / verify command / receipt
```

RIPR already owns language-neutral evidence routing and user surfaces:
reachability, infection, propagation, revealability, canonical gaps,
actionability, repair packets, verify commands, receipts, support-tier claims,
and projection through CLI, JSON, Markdown, SARIF, PR, CI, LSP, and swarm
surfaces. Perl-specific syntax and semantic facts should come from the Perl
intelligence substrate instead of being recreated inside RIPR.

This ADR records that substrate boundary before any Perl adapter or fact schema
implementation lands. It does not add a dependency, write adapter code, define
the full `ripr-perl-facts-v1` schema, or change current RIPR behavior.

## Decision

Use **`perl-lsp` fact export** as the Perl intelligence substrate for RIPR Perl
repair routing.

`perl-lsp` owns:

- Perl syntax facts;
- semantic owner facts;
- package, module, and workspace resolution facts;
- confidence and provenance;
- test framework facts;
- dynamic-boundary and unsupported-case facts.

RIPR owns:

- deterministic ingestion of `ripr-perl-facts-v1` batch packets;
- language-neutral `PerlAdapter` translation into RIPR evidence;
- reachability, infection, propagation, and revealability routing;
- canonical Perl gap IDs and actionability;
- repair cards, verify commands, agent packets, receipts, and support-tier
  claims;
- projection through existing CLI, JSON, Markdown, SARIF, PR, CI, LSP, and
  swarm surfaces.

The fact exchange must be a deterministic batch packet, not a live LSP protocol
dependency. The intended first contract name is `ripr-perl-facts-v1`.

The first exporter request shape is a batch command:

```text
perl-lsp ripr-facts --schema ripr-perl-facts-v1 --root . --base origin/main --head HEAD --fact-classes owners,changes,tests,oracles --out target/ripr/reports/perl-facts.json
```

RIPR may model and render that request, but it must not make `ripr check`
depend on launching `perl-lsp`, a Perl runtime, package installation, or test
execution. If the exporter is missing or cannot produce a packet, RIPR records
an unavailable/limitation state rather than creating repair guidance.

The batch packet must be fixture-friendly so RIPR can implement and test a
fixture-only `PerlAdapter` before a live exporter is available. Parser-specific
or LSP-specific types must not leak into RIPR's public output schemas or
language-neutral evidence contracts.

## Consequences

Positive:

- RIPR avoids building and maintaining a second Perl parser.
- Perl-specific intelligence stays with the tool that owns Perl workspace and
  module understanding.
- RIPR can remain focused on evidence routing, actionability, repair packets,
  verify commands, receipts, and user surfaces.
- Fixture-only adapter work can start from canned fact packets without running
  a Perl runtime, launching an LSP server, or installing project dependencies.
- Unsupported Perl dynamics can be represented explicitly as limitation facts
  instead of being guessed into repair packets.
- The boundary matches the existing single-package architecture: no new
  published crate, binary, or LSP server is required.

Negative:

- RIPR's Perl usefulness depends on `perl-lsp` exporter quality and fact
  version stability.
- The first working Perl slice needs a fact schema before it can consume real
  projects.
- There is an integration boundary to test: provenance, confidence, file paths,
  owner IDs, and dynamic-boundary labels must be deterministic across tools.
- Some Perl facts may be unavailable until `perl-lsp` grows exporter support,
  which means RIPR must emit limitations instead of repair packets for those
  cases.

## Alternatives Considered

- **Build a RIPR-owned Perl parser.** Rejected. It would duplicate the Perl
  intelligence substrate, likely under-handle dynamic semantics, and turn RIPR
  toward parser ownership instead of repair routing.
- **Bind RIPR directly to the LSP protocol.** Rejected. Live protocol sessions
  introduce editor/runtime state, lifecycle concerns, and nondeterminism into a
  batch analyzer. RIPR needs deterministic packets that can be checked in as
  fixtures and replayed in CI.
- **Consume ad hoc JSON from `perl-lsp` without a versioned contract.**
  Rejected. Repair packets need stable owner IDs, provenance, confidence,
  limitation reasons, verify commands, and receipt movement; an unversioned
  feed would make support-tier claims unauditable.
- **Delay the substrate decision until implementation.** Rejected. Without an
  ADR, early adapter work can accidentally encode a RIPR-owned parser shape or
  live LSP dependency before reviewers can evaluate the boundary.
- **Treat Perl as unsupported until full dynamic semantics are available.**
  Rejected. A preview/advisory route with explicit limitations can still give
  users useful bounded repair cards for common Test::More/Test2/Test::Exception
  shapes without claiming full Perl understanding.

## Revisit Criteria

This ADR should be revisited if:

- `perl-lsp` cannot provide deterministic batch fact packets with provenance
  and confidence;
- a stable, maintained Perl fact exporter emerges that is a better substrate
  and can produce the same packet contract;
- RIPR needs a language-neutral fact field that cannot be represented through
  `ripr-perl-facts-v1`;
- dogfood or false-actionable metrics show the fact boundary creates unsafe
  repair packets even with strict actionability;
- Perl support-tier promotion requires a stronger substrate than the current
  batch fact model.

## Follow-up Specs / Plans

Follow-up work must land PR by PR:

1. Define `ripr-perl-facts-v1` as a deterministic batch fact packet.
2. Add a fixture-only `PerlAdapter` that consumes canned fact packets.
3. Add canonical Perl owner and gap ID rules.
4. Add the `perl-lsp` exporter path.
5. Add source, test, oracle, related-test, and dynamic-boundary facts.
6. Enforce strict Perl actionability before any repair card or agent packet.
7. Project Perl advisory cards through existing output and swarm surfaces.
8. Add false-actionable corpus rows and real-repo dogfood receipts.
9. Make a support-tier decision from route-quality metrics.
