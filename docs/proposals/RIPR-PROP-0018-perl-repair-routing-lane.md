# RIPR-PROP-0018: Perl Repair Routing Lane

Status: proposed

Owner: language-adapter / swarm

Created: 2026-06-04

Target campaign: Perl repair routing

Linked specs:

- `RIPR-SPEC-0026`: Language adapter contract
- `RIPR-SPEC-0064`: Perl fact packet contract
- `RIPR-SPEC-0057`: RIPR swarm repair loop
- `RIPR-SPEC-0058`: RIPR swarm external agent handoff
- `RIPR-SPEC-0061`: Lane 1 canonical actionability contract

Linked ADRs:

- [ADR 0018: Perl fact substrate](../adr/0018-perl-lsp-fact-substrate.md)

Linked work items:

- `docs/perl-repair-routing-charter`
- `adr/perl-lsp-fact-substrate`
- `spec/ripr-perl-facts-v1`
- `analysis/perl-adapter-fixture-facts`
- `analysis/perl-owner-gap-identity`
- `integration/perllsp-facts-exporter`
- `analysis/perl-source-test-oracle-facts`
- `analysis/perl-related-test-linking`
- `analysis/perl-strict-actionability`
- `output/perl-repair-cards`
- `output/perl-verify-commands`
- `swarm/perl-agent-packet-export`
- `output/perl-surface-projection`
- `fixtures/perl-dynamic-boundary-corpus`
- `dogfood/perl-real-repo-receipts`
- `metrics/perl-route-quality`
- `campaign/perl-support-tier-decision`

Support-tier impact:

- This proposal does not promote Perl support. Perl starts as
  preview/advisory only. Perl evidence has no default gate authority, public
  badge contribution, baseline authority, RIPR Zero authority, or stable
  support claim until a later support-tier decision is backed by fixtures,
  dogfood, route-quality metrics, receipts, and rollback evidence.

Policy impact:

- Register this proposal and the linked ADR in `policy/doc-artifacts.toml`.
- Keep source `ripr` as one published package. No new `ripr-perl` crate,
  binary, LSP server, or parser package is introduced by this proposal.

## Problem

Perl users need the same repair-routing value that the TypeScript and Python
lanes are targeting: not more findings, not a broad coverage proxy, and not a
claim that RIPR fully understands dynamic language semantics. The useful loop
is smaller:

```text
changed Perl behavior
-> known Perl fact substrate
-> RIPR evidence
-> one canonical gap or named limitation
-> bounded Perl test repair card
-> verify command
-> before/after receipt
```

Without a Perl-specific lane, the product has two failure modes. It can ignore
Perl repos entirely, leaving mixed-language teams without the next missing
proof. Or it can overfit a lightweight syntax guesser and produce
repair-shaped cards that are not safe for real Perl code with dynamic package
loading, generated symbols, monkeypatching, role composition, and test helper
indirection.

The end state is:

```text
RIPR turns a Perl code change into the next smallest missing behavioral proof
a human or agent can safely add, or it names the static limitation that makes
the case non-actionable.
```

## Users and surfaces

Users:

- Perl maintainers who need precise "what test should I add?" guidance;
- reviewers replacing vague "add tests" feedback with a missing
  discriminator;
- coding-agent operators who need one bounded test-repair packet at a time;
- mixed-language repo maintainers who need Perl to use the same receipt and
  support-tier model as Rust, TypeScript, and Python;
- maintainers deciding whether Perl evidence is ready to move beyond preview.

Surfaces:

- `ripr check`, `ripr pilot`, and `ripr first-pr`;
- JSON, Markdown, SARIF, PR summary, generated CI, report packets, and LSP
  projection;
- swarm queue, claim, attempt, ingest, outcome, and receipt surfaces;
- support tiers, capability matrix, traceability, fixture corpus, dogfood, and
  route-quality reports.

## User journeys

### Perl maintainer

The maintainer runs RIPR on a Perl pull request. The report names the changed
package/sub/script owner, the related test evidence from `perl-lsp` facts, and
the missing discriminator. If the current test reaches the owner but only uses
`ok(...)`, the card suggests a bounded `Test::More` or `Test2` exact-value,
field, exception, warning, output, or predicate-boundary assertion and gives a
`prove`, `yath`, `carton exec prove`, or `dzil test` command when the fact
packet supports one.

### Pull-request reviewer

The reviewer sees a Perl advisory card that says what is weakly exposed and
why it matters. If package resolution, dynamic dispatch, test helper semantics,
or generated code make the case unsafe, the PR summary names the limitation
instead of emitting a repair packet.

### Coding agent

The swarm export gives the agent one Perl packet with allowed test files,
forbidden production edits, the target test shape, verify command, receipt
command, stop conditions, and must-not-change constraints. The agent adds or
strengthens one test, proves the command, records the receipt, and stops when
the packet closes, improves, stays unchanged, or is blocked by missing context.

### Editor user

The editor projects the same Perl repair card as CLI and PR output. It may
copy the card, verify command, or agent packet. It does not edit files, run
provider calls, hide stale facts, or treat preview Perl evidence as a gate.

## Success criteria

- Perl facts enter RIPR through deterministic batch fact packets produced by
  `perl-lsp`, not a separate RIPR Perl parser and not live LSP protocol calls.
- A fixture-only `PerlAdapter` can consume canned fact packets before the
  exporter exists.
- Canonical Perl owner IDs and canonical Perl gap IDs are stable across CLI,
  JSON, Markdown, SARIF, PR, LSP, agent packet, and receipt surfaces.
- Actionable Perl gaps require all strict packet fields:
  `canonical_gap_id`, `gap_state = "actionable"`, changed owner, RIPR
  evidence, missing discriminator, repair kind, target
  `Test::More`/`Test2`/`Test::Exception` shape, suggested test location,
  verify command, receipt command, confidence, evidence refs, allowed edit
  boundaries, forbidden edit boundaries, stop-if conditions, and
  must-not-change constraints.
- Perl preview cases missing any strict field emit a named limitation or
  non-actionable advisory state, not a repair packet.
- First repair classes are fixture-backed before surfacing as packets:
  predicate boundary, exact return assertion, exception/error path observer,
  hash/object field assertion, output/warn/log observer, and existing weak
  test strengthening.
- First test facts distinguish strong exact oracles from `ok(...)`, smoke
  tests, mention-only tests, dies-only tests, unknown helpers, and dynamic
  framework indirection.
- Verify command guidance supports the detected Perl test surface when facts
  allow it: `prove`, `yath`, `carton`, and `dzil`.
- Perl remains preview/advisory until a later support-tier decision records
  false-actionable review, dogfood receipts, route-quality metrics, support
  tier evidence, and rollback criteria.

## Proposed shape

Perl plugs into the existing repair-routing spine:

```text
Perl repo / PR
-> perl-lsp fact export
-> ripr PerlAdapter
   -> changed package/sub/script owner
   -> source facts
   -> test facts
   -> oracle facts
   -> dynamic-boundary facts
-> RIPR evidence
   -> reachability
   -> infection signal
   -> propagation signal
   -> revealability signal
-> canonical Perl gap
   -> stable gap ID
   -> actionability or named limitation
   -> missing discriminator
   -> repair route
-> repair card / verify command / agent packet
-> before/after receipt and outcome projection
```

`perl-lsp` owns Perl intelligence: syntax, semantic facts, workspace and module
resolution, confidence, provenance, and dynamic-boundary facts. RIPR owns
evidence routing: reachability, infection, propagation, revealability,
canonical gaps, actionability, repair packets, verify commands, receipts,
support-tier claims, and user surfaces.

The first fact packet contract should be named `ripr-perl-facts-v1`. It should
be deterministic, batch-oriented, and fixture-friendly so RIPR can test the
adapter without launching a Perl runtime or LSP server.

## Repair classes

The first useful repair classes are:

| Repair class | Safe first shape |
| --- | --- |
| Predicate boundary | Add an exact boundary assertion for the changed predicate. |
| Exact return assertion | Replace or strengthen smoke evidence with exact return-value proof. |
| Exception/error path observer | Observe exception class, message, payload, or error-path result. |
| Hash/object field assertion | Assert a specific hash key, object field, or accessor result. |
| Output/warn/log observer | Assert output, warning, log text, or emitted event when that is the contract. |
| Existing weak test strengthening | Strengthen an already-related weak Perl test when edit boundaries are safe. |

The first useful test facts are:

- `Test::More`
- `Test2::V0` and `Test2::Suite`
- `Test::Exception`
- `Test::Fatal`
- `prove`
- `yath`
- `carton`
- `dzil`

Strong exact oracles must be classified separately from broad or weak evidence:
`ok(...)`, smoke tests, mention-only tests, dies-only tests, unknown helpers,
and dynamic framework indirection.

## Preview boundary

Perl starts preview/advisory. This lane must not add:

- default gates;
- public badge contribution;
- RIPR Zero authority;
- runtime mutation proof;
- generated tests;
- source edits;
- provider or model calls;
- "Perl stable" support claims;
- automatic package installation or test execution as a prerequisite for
  static fact ingestion.

Unsupported Perl dynamics become explicit limitations, not repair packets.

## Behavior specs to create or update

- Update `RIPR-SPEC-0026` only when the language-neutral adapter contract needs
  an additive Perl field or vocabulary update.
- Add `RIPR-SPEC-0064: Perl fact packet contract` to define
  `ripr-perl-facts-v1`, owner IDs, test facts, oracle facts, provenance,
  confidence, dynamic-boundary facts, and failure modes.
- Update `RIPR-SPEC-0057`, `RIPR-SPEC-0058`, and `RIPR-SPEC-0061` only when
  Perl packets, external-agent handoffs, or canonical actionability require a
  language-neutral contract change.

## Architecture decisions needed

- ADR 0018 records that `perl-lsp` is the Perl fact substrate and RIPR must not
  build a separate Perl parser or bind analysis to the LSP protocol.

## Implementation campaign shape

Each slice follows the scoped PR contract and should land with one semantic
proof obligation:

1. `docs/perl-repair-routing-charter`: proposal/PRD and ADR.
2. `spec/ripr-perl-facts-v1`: deterministic batch fact packet schema.
3. `analysis/perl-adapter-fixture-facts`: fixture-only adapter consuming
   canned fact packets.
4. `analysis/perl-owner-gap-identity`: canonical owner and gap IDs.
5. `integration/perllsp-facts-exporter`: `perl-lsp` exporter path for the
   fact packet.
6. `analysis/perl-source-test-oracle-facts`: first source/test/oracle fact
   extraction.
7. `analysis/perl-related-test-linking`: related-test and probe
   classification.
8. `analysis/perl-strict-actionability`: fail closed unless strict packet
   fields exist.
9. `output/perl-repair-cards`: repair cards, verify commands, and agent
   packets.
10. `output/perl-surface-projection`: CLI, JSON, Markdown, SARIF, PR, CI, LSP,
   and swarm projection.
11. `fixtures/perl-dynamic-boundary-corpus`: false-actionable and dynamic
   boundary fixtures.
12. `dogfood/perl-real-repo-receipts`: real Perl repo dogfood with
   before/after receipts.
13. `metrics/perl-route-quality`: route-quality metrics and support-tier
   decision packet.

## Evidence plan

Evidence must scale with the claim:

- proposal and ADR prove only product direction and architecture boundary;
- fact-packet spec fixtures prove deterministic packet parse, provenance,
  confidence, and unsupported-dynamic reporting;
- adapter fixtures prove owner extraction, test facts, oracle facts,
  canonical gap IDs, limitations, and strict actionability without launching
  `perl-lsp`;
- exporter fixtures prove `perl-lsp` can produce the batch packet
  deterministically;
- output-contract tests prove Perl cards and limitations project through CLI,
  JSON, Markdown, SARIF, PR, CI, LSP, agent packet, and receipt surfaces
  without schema forks;
- false-actionable fixtures prove dynamic package loading, monkeypatching,
  generated symbols, role composition, opaque custom helpers, and framework
  indirection fail closed;
- dogfood receipts prove at least one real Perl gap closes or improves before
  any support-tier promotion is considered;
- route-quality metrics report top-1/top-3 actionable precision,
  verify-command validity, related-test-location rate, concrete-discriminator
  rate, false-actionable rate, static-limit distribution, and receipt closure
  rate.

## Alternatives considered

- Build Perl parsing and semantic analysis inside RIPR. Rejected because
  `perl-lsp` already owns the Perl fact substrate and a second parser would
  split provenance, confidence, and dynamic-boundary behavior.
- Make `ripr check` launch a live Perl LSP session. Rejected for the first lane
  because deterministic packet replay is easier to fixture, review, and keep
  preview/advisory.
- Generate Perl tests directly from weak evidence. Rejected because this lane
  must route bounded repair packets or named limitations, not create
  unreceipted edits.

## Risks

- `perl-lsp` facts could drift from RIPR expectations. Mitigation: define a
  versioned batch packet, keep fixtures in RIPR, and require provenance and
  confidence fields.
- RIPR could accidentally build a second Perl parser. Mitigation: ADR 0018
  makes `perl-lsp` the substrate and limits RIPR to fact consumption and
  evidence routing.
- Dynamic Perl cases could look actionable. Mitigation: strict packet fields
  are mandatory; missing facts produce named limitations.
- Preview output could imply gate authority. Mitigation: every Perl surface
  stays preview/advisory until a separate support-tier PR changes that claim.
- Agent packets could permit broad edits. Mitigation: packets require allowed
  and forbidden edit boundaries, stop-if conditions, and must-not-change
  constraints.
- Verify commands could overclaim local tool availability. Mitigation: command
  guidance comes from facts and is advisory; missing runner facts produce a
  limitation or setup diagnostic, not a repair packet.

## Non-goals

- No separate RIPR Perl parser.
- No live LSP protocol dependency in `ripr check`.
- No Perl runtime, package install, or test execution dependency by default.
- No runtime mutation execution.
- No generated Perl tests.
- No automatic source edits.
- No provider/model calls.
- No broad line-coverage queue.
- No default CI blocking, public badge contribution, baseline authority, or
  RIPR Zero authority for Perl preview evidence.
- No stable Perl support claim.
- No repair packet for unsupported dynamic dispatch, unresolved package/module
  resolution, unknown helpers, generated code, or framework indirection without
  explicit fixture-backed facts.

## Exit criteria

This proposal can move to `accepted` only after:

- the Perl fact packet spec is merged;
- the fixture-only `PerlAdapter` consumes canned `ripr-perl-facts-v1` packets;
- the `perl-lsp` exporter path exists or the proposal is superseded with a
  recorded reason;
- strict Perl actionability and false-actionable fixtures are in place;
- repair cards, verify commands, agent packets, and receipt projection share
  canonical Perl gap IDs;
- real Perl dogfood records at least one useful repair card and one
  before/after receipt outcome;
- route-quality metrics support the selected support-tier decision;
- public docs clearly state what Perl preview evidence proves and does not
  prove.
