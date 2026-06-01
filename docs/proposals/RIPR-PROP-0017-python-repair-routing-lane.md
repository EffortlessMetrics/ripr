# RIPR-PROP-0017: Python Repair Routing Lane

Status: accepted

Owner: language-adapter / swarm

Created: 2026-05-29

Target campaign: Python repair routing

Linked specs:

- `RIPR-SPEC-0026`: Language adapter contract
- `RIPR-SPEC-0028`: Python preview static facts
- `RIPR-SPEC-0057`: RIPR swarm repair loop
- `RIPR-SPEC-0058`: RIPR swarm external agent handoff
- `RIPR-SPEC-0061`: Lane 1 canonical actionability contract

Linked ADRs:

- None. Add an ADR only if a later slice changes the language adapter
  boundary, parser substrate, packet authority, or execution model.

Linked work items:

- `docs/python-repair-routing-charter`
- `docs/python-current-state-inventory`
- `analysis/python-project-detection`
- `analysis/python-source-facts`
- `analysis/python-diff-owner-mapping`
- `analysis/python-pytest-oracles`
- `analysis/python-unittest-oracles`
- `analysis/python-related-test-linking`
- `analysis/python-canonical-gap-identity`
- `analysis/python-ripr-evidence-model`
- `analysis/python-repair-classes-v1`
- `output/python-ranking-noise-control`
- `output/python-test-placement-verify`
- `output/python-repair-card-v1`
- `swarm/python-agent-packet-export`
- `cli/python-first-use-path`
- `output/python-surface-projection`
- `ci/python-advisory-mode`
- `lsp/python-repair-card-projection`
- `analysis/python-http-api-pack-v1`
- `analysis/python-cli-output-pack-v1`
- `analysis/python-parametrized-boundaries`
- `analysis/python-existing-test-strengthening`
- `swarm/python-gap-work-queue`
- `swarm/python-agent-result-ingestion`
- `outcome/python-gap-ledger`
- `fixtures/python-false-positive-corpus`
- `dogfood/python-real-repo-evals`
- `metrics/python-repair-routing-quality`
- `campaign/python-usable-alpha-promotion`
- `dogfood/python-stability-evals-v1`

Support-tier impact:

- The charter itself did not promote Python. The later
  [usable-alpha closeout](../handoffs/2026-05-31-python-repair-routing-usable-alpha-closeout.md)
  promotes only the scoped Python repair-routing loop to `usable alpha` with
  fixture, dogfood, receipt, metric, support-tier, and rollback evidence.
  Broader Python static facts remain `preview`.

Policy impact:

- Register this proposal and its implementation plan in
  `policy/doc-artifacts.toml`.

## Problem

Python support should not be judged by whether RIPR can parse Python files or
print Python-labeled findings. The product value is a smaller loop:

```text
changed Python behavior
-> missing or weak behavioral evidence
-> focused test repair card
-> precise verify command
-> before/after receipt showing the canonical gap moved
```

Without that loop, Python preview evidence can become another static report
that reviewers have to interpret manually. The lane should instead make vague
test-quality requests concrete enough for a developer, reviewer, or bounded
coding agent to execute safely.

The end state is:

```text
ripr-swarm turns a Python code change into a small, evidence-backed,
verifiable test-repair task that a human or agent can execute safely.
```

That means Python is the first non-Rust proof that RIPR is a repair-routing
layer for test quality, not a language-specific parser demo or a coverage
replacement.

## Users and surfaces

Users:

- solo developers who need the next test to add;
- PR reviewers who need to replace vague "add tests" comments with a concrete
  missing discriminator;
- CI adopters who need advisory Python evidence without new gate authority;
- coding-agent operators who need narrow, test-only work packets;
- editor users who need the same repair card near the changed behavior;
- maintainers who need support-tier and promotion boundaries to stay honest.

Surfaces:

- `ripr pilot`, `ripr first-pr`, and `ripr check`;
- JSON, Markdown, SARIF, PR summaries, generated CI, and report packets;
- LSP/editor diagnostics, hovers, and copy actions;
- `ripr-swarm` packet, queue, attempt, ingest, and outcome surfaces;
- before/after receipts and canonical gap ledgers;
- support tiers, capability matrix, traceability, fixture corpus, and dogfood
  reports.

## User journeys

### Solo developer

The developer runs `ripr pilot --root .` in a Python repo. RIPR reports the
detected Python project, names the top repairable gap, explains the current test
evidence, gives the missing discriminator, suggests the nearest test file and
test shape, and prints the verify and receipt commands. The developer edits the
test, runs the verify command, runs the receipt command, and sees the canonical
gap close or downgrade.

### PR reviewer

The reviewer sees a PR summary that does not say "needs tests" in general. It
says which changed Python owner lacks a specific discriminator, why the current
related test is weak or missing, and which test shape would expose the behavior.
If no high-confidence repair exists, the summary says no actionable Python gap
instead of inflating the finding count.

### CI/advisory mode

Generated CI can run Python preview evidence cheaply and safely. It uploads the
same repair-card artifacts as the CLI, keeps preview language groups advisory,
does not use secrets or unsafe runner assumptions for fork PRs, and leaves
configured gate authority with explicit gate-decision artifacts.

### Coding agent / swarm task

The swarm export turns one canonical gap into one bounded packet. The packet
names the allowed test files, forbidden production files, missing
discriminator, test shape, verify command, receipt command, and stop
conditions. The agent adds one test, runs the command, attaches the receipt, and
stops if imports, fixtures, expected values, or edit scope are unresolved.

### LSP/editor user

The editor projects the same report as the CLI. Diagnostics appear only for
high-signal changed behavior with missing evidence. Hover explains the gap, and
code actions copy the repair card, pytest skeleton, agent packet, or related
test path. The editor does not silently edit files or hide stale report state.

## Success criteria

- `ripr pilot --root .` works in normal Python repos without a Cargo workspace.
- `ripr first-pr --root . --base origin/main --head HEAD` can surface a top
  Python repairable gap when the diff and evidence support one.
- Python findings use the same evidence spine as Rust:
  language facts -> changed behavior owner -> RIPR evidence -> canonical gap
  -> repair card -> verify command -> receipt.
- Every actionable Python finding answers:
  changed owner, changed behavior, current test evidence, missing
  discriminator, recommended test shape, suggested location, verify command,
  and receipt command.
- Every non-actionable Python case has a named stop reason or static limit.
- Agent packets are test-edit bounded, deterministic, and explicit about
  allowed files, forbidden files, stop conditions, verify, and receipt.
- Before/after outcome receipts can show opened, closed, unchanged, improved,
  regressed, and newly introduced Python canonical gaps.
- Output surfaces share canonical gap IDs across CLI, JSON, Markdown, SARIF,
  PR summary, LSP, and agent packets.
- Support-tier language stays preview/advisory until promotion evidence lands
  through a dedicated support-tier update.

## Support contract

Python support uses these tiers:

| Tier | Meaning for Python |
| --- | --- |
| `unsupported` | RIPR may route around the repo or emit an unavailable-adapter/config limitation. No Python repair cards. |
| `preview` | Opt-in syntax-first Python evidence is visible, advisory, and not a gate input. Broader Python static facts still live here. |
| `usable alpha` | Common pytest/unittest repair-routing loops are fixture-backed, dogfooded, receipt-backed, and surfaced consistently, but remain static/advisory. |
| `stable` | Reserved for a later policy-approved claim with stronger evidence, long-running dogfood, low false-actionable rate, rollback proof, and support-tier signoff. |

Promotion out of `preview` requires explicit evidence. It cannot be inferred
from parser coverage, a single fixture, generated CI grouping, or the presence
of Python-labeled output.

## Proposed shape

Python plugs into the existing language-neutral spine:

```text
Python repo / PR
-> Python adapter
   -> project detection
   -> source facts
   -> test facts
   -> framework facts where statically safe
-> evidence graph
   -> changed owner
   -> reachability
   -> infection signal
   -> propagation signal
   -> revealability signal
-> canonical gap
   -> stable ID
   -> missing proof
   -> actionability
   -> stop reason
-> CLI / PR / agent / LSP surfaces
-> outcome receipt
```

The adapter remains syntax-first and conservative. Framework-shaped repair
cards are allowed when the static shape is clear, such as simple FastAPI/Flask
client assertions, Click/Typer output assertions, dataclass/dict field
assertions, and parameterized pytest boundary cases. Dynamic routing,
unresolved imports, opaque fixtures, monkeypatch-only behavior, decorator-heavy
semantics, metaprogramming, and generated code stay limited until a later
fixture-backed slice makes them safe.

## End-of-lane product target

A Python user should eventually be able to run:

```bash
ripr pilot --root .
ripr first-pr --root . --base origin/main --head HEAD
```

and receive a compact repair card like:

```text
Top Python repairable gap

Changed owner:
  app/pricing.py::calculate_discount

Changed behavior:
  if amount >= threshold:

Current test evidence:
  tests/test_pricing.py reaches calculate_discount
  existing tests assert a successful discount
  no test observes the equality boundary

Missing discriminator:
  amount == threshold

Recommended repair:
  Add pytest case:
    test_calculate_discount_threshold_boundary

Verify:
  pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary

Receipt:
  ripr outcome --before .ripr/before.json --after .ripr/after.json
```

The exact formatting can change under output specs, but the questions answered
by the card are part of this proposal's product contract.

## Done means the loop works

Python is not done when RIPR merely parses files, routes `*.py`, or emits
preview findings. It is done for this lane only when the loop works:

```text
Python PR
-> RIPR detects changed behavior
-> RIPR finds weak or missing test evidence
-> RIPR emits one or more high-confidence repair cards
-> human or agent adds a focused test
-> verify command passes
-> RIPR shows the canonical gap closed or improved
-> PR summary and receipt preserve the improvement
```

One excellent top finding is more valuable than a long noisy list. The lane
should optimize for top finding quality, repairability, closure, and agent
safety.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Treat Python support as parser completion. | Parser coverage does not give users the next test to add, a verify command, or a receipt-backed outcome. |
| Emit broad Python review warnings first. | Noisy warnings recreate vague "add tests" feedback instead of bounded repair work. |
| Generate Python tests automatically. | Generated tests and source edits are outside the default RIPR and swarm authority boundary. |
| Make Python preview evidence gate-eligible immediately. | Preview evidence needs fixture, dogfood, false-actionable, receipt, policy, and rollback evidence before promotion. |
| Build a Python-only tool path. | Python should plug into the existing evidence spine so future languages use the same contracts and surfaces. |

## Behavior specs to create or update

The first implementation slices should reuse existing specs where possible:

- `RIPR-SPEC-0026`: language-neutral adapter boundary.
- `RIPR-SPEC-0028`: Python syntax-first static facts.
- `RIPR-SPEC-0057`: swarm repair-loop packet readiness.
- `RIPR-SPEC-0058`: external-agent handoff packet boundary.
- `RIPR-SPEC-0061`: canonical actionability requirements.

Create or update narrower behavior specs only when a later PR changes a public
contract, output schema, LSP payload, swarm packet schema, receipt shape,
support-tier claim, or promotion criterion.

## Architecture decisions needed

No ADR is needed for this charter. Add an ADR only if later work changes:

- parser substrate or dependency boundary;
- import graph or project model authority;
- generated-test or source-edit policy;
- packet execution authority;
- language adapter ownership;
- output schema ownership across languages.

## Implementation campaign shape

The durable PR-sized sequence lives in
[`plans/python-repair-routing/implementation-plan.md`](../../plans/python-repair-routing/implementation-plan.md).

The campaign milestones are:

| Milestone | Work items | User value |
| --- | --- | --- |
| A. Python is recognized | PR 1-5 | RIPR can run on a Python repo without pretending it is Rust. |
| B. Python has real evidence | PR 6-12 | RIPR can identify changed Python behavior and distinguish strong tests from weak tests. |
| C. Python produces repair cards | PR 13-15 | RIPR gives the next test to add. |
| D. Python works in daily workflows | PR 16-19 | CLI, PR, CI, and editor show the same guidance. |
| E. Python becomes application-useful | PR 20-23 | Common API, CLI, field, and parameterized-test shapes become useful. |
| F. Swarm turns it into leverage | PR 24-26 | RIPR creates safe parallel test-repair work and proves what closed. |
| G. Promotion | PR 27-30 | Python support is honest, measured, and ready to promote if evidence supports it. |

## Evidence plan

Evidence must scale with the support claim:

- fixture matrix for basic function, predicate boundary, changed return,
  changed exception, dict/object/dataclass field, pytest exact assertion,
  pytest smoke assertion, unittest assertion, HTTP/API optional shape,
  CLI/output optional shape, and dynamic unsupported cases;
- positive and negative fixtures for each repair class;
- output-contract tests for human, JSON, Markdown, SARIF, PR summary, LSP, and
  agent packet surfaces when those surfaces change;
- dogfood on controlled and real Python repos before promotion;
- metrics for top-1/top-3 actionable precision, verify-command validity,
  concrete-discriminator rate, related-test-location rate, false-actionable
  rate, crash rate, static-limit distribution, and receipt closure rate;
- before/after receipts proving at least one real Python gap closes before
  claiming `usable alpha`.

## Risks

- Parser work could masquerade as user value. Mitigation: the support contract
  defines repair-loop closure as done.
- Preview output could imply Rust parity. Mitigation: keep `language_status =
  "preview"` visible and require explicit promotion evidence.
- Framework-shaped cards could overclaim runtime behavior. Mitigation: only
  emit them for clear static shapes; otherwise name the limitation.
- Agent packets could permit broad edits. Mitigation: require allowed test
  files, forbidden production files, one-test scope, stop conditions, verify,
  and receipt.
- Noisy findings could reduce trust. Mitigation: rank a small number of
  high-confidence repair cards and treat "no actionable Python gaps" as a valid
  result.
- Receipts could be skipped. Mitigation: do not claim improvement from verify
  success alone; require before/after evidence movement.

## Non-goals

- No full Python semantic understanding.
- No default runtime mutation execution.
- No arbitrary import execution.
- No typechecker, virtualenv, package install, or runtime test runner
  dependency by default.
- No generated tests by default.
- No automatic source edits.
- No provider or model calls.
- No coverage replacement.
- No framework omniscience.
- No default CI blocking, gate promotion, RIPR Zero inclusion, or baseline
  eligibility for Python preview evidence.
- No stable Python support marketing before fixture, dogfood, metric, receipt,
  and support-tier evidence exists.
- No agent tasks that can freely edit production source.
- No release, publish, signing, marketplace, or source-repo authority changes
  in `ripr-swarm`.

## Exit criteria

This proposal moved to `accepted` after:

- Python repair-routing slices land through at least a documented usable-alpha
  promotion or an explicit superseding closeout;
- the linked implementation plan records PRs, proof commands, remaining
  limits, and next work;
- a dedicated support-tier PR promoted only the scoped Python repair-routing
  loop with evidence while broader Python static facts remained `preview`;
- dogfood evidence includes at least one useful real Python repair card and at
  least one receipt-backed closed or improved canonical gap;
- public docs state what Python repair-routing proves and does not prove;
- source `ripr` release/distribution authority remains separate from
  `ripr-swarm` development work.
