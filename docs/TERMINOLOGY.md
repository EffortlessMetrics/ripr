# Terminology

`ripr` uses plain language on its public surfaces (storefront, README,
Quickstart, editor) and a precise internal vocabulary in its specs, schemas,
JSON, and report artifacts. This page is the bridge between the two, so the
internal model stays teachable instead of leaked.

If you only care about the day-to-day language, the plain-language column is
all you need. If you are reading JSON, writing fixtures, or working on the
analyzer itself, you will want the internal term.

## Bridge

| Public / first-hour phrase | Internal term | Meaning |
| --- | --- | --- |
| changed code where the nearby tests may not actually catch the behavior | seam / finding | the location `ripr` is worried about |
| assertion or check that would catch the changed behavior | discriminator | the test evidence `ripr` is looking for |
| nearby test to imitate | related test | candidate test context used to suggest the next focused test |
| the evidence got better after adding a test | movement | before/after static change observed by `ripr outcome` |
| top test gap | top actionable seam | the highest-priority seam in the current pilot/report |
| no focused test gap found | `no-actionable-seam` | stable status ID emitted when analysis ran cleanly but produced no actionable recommendation |
| recommended next test | `first-useful-action` | the advisory `target/ripr/reports/first-useful-action.json` projection |
| static classifications: exposed, weak, unrevealed, no static path, unknown | `exposed` / `weakly_exposed` / `reachable_unrevealed` / `no_static_path` / `infection_unknown` / `propagation_unknown` / `static_unknown` | conservative static-exposure labels — `ripr` does not use mutation-runtime words such as `killed` or `survived` outside calibration reports |
| ripr's view of how well a behavior is tested | grip (seam-grip class) | seam-native classification across the five RIPR stages, surfaced as `SeamGripClass` in JSON |
| identity for a behavior gap (so it does not move when line numbers change) | canonical gap identity | hash of owner / kind / flow sink / missing discriminator / assertion shape used by ledgers, baselines, and gate comparison |
| PR review summary | `pr-review front-panel` / report packet index | composed advisory CI artifact summarizing PR guidance, first useful action, assistant proof, ledger, baseline, gate, calibration, coverage/grip, and receipt |
| agent proof status | `assistant-loop health` | advisory summary of existing assistant proof reports |
| receipt for a focused test | `agent receipt` | provenance plus bounded next-action guidance for one focused test |
| under-the-hood model | RIPR: **Reachability**, **Infection**, **Propagation**, **Revealability** | the four stages `ripr` evaluates statically per probe / seam |

## Where the precise vocabulary lives

The internal terms are stable in:

- [Output schema](OUTPUT_SCHEMA.md) — JSON contracts for findings, repo
  exposure, agent packets, and calibration reports.
- [Static exposure model](STATIC_EXPOSURE_MODEL.md) — the domain model.
- [Specs](specs/README.md) — RIPR-SPEC documents pin behavior; `RIPR-SPEC-0021`
  pins the seam-native `evidence_record` projection.
- [Capability matrix](CAPABILITY_MATRIX.md) and [Metrics](METRICS.md).
- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md) — full build
  history of the surfaces in the current product.

Renaming the public-facing copy of any of these terms does **not** rename the
JSON field, status ID, command, schema, or spec it maps to.

## When to use which

Use the **plain-language** column in:

- the marketplace storefront and `editors/vscode/README.md`,
- the root `README.md` first screen,
- [Quickstart](QUICKSTART.md),
- [Editor extension](EDITOR_EXTENSION.md) status copy and first-use steps,
- generated CI advisory summary headings (work in progress).

Use the **internal** column in:

- `docs/specs/**`,
- `docs/OUTPUT_SCHEMA.md`,
- metrics, fixtures, and report schemas,
- implementation docs and campaign notes,
- `CHANGELOG.md` entries that describe internal contracts.

Linking this page from a spec is unnecessary — specs are already the
authority on internal terms.
