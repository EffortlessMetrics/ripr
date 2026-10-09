# MCP journey benchmarks: B2 happy path, B3 negative authority

Executable oracle: `crates/ripr/tests/agentic_bench_mcp.rs`.
Machine contract: `spec.json`. Static pre-refresh probe script: `b3a.probes.ndjson`.

## Fixture

`input/base` is a minimal Cargo package committed on branch `main`;
`input/journey/src/lib.rs` closes the predicate (`>` to `>=`) on branch
`journey`. The harness copies `input/` into a temp dir, commits both
branches with git, and points `ripr mcp --stdio --root <fixture>` at it.
Analysis diffs the resolved default base (`main`) against `journey`, so the
refresh commits a snapshot with the boundary gap. Data stays small: four
source files, no generated output.

## B2: happy-path journey

One stdio session drives the full tool sequence: status, refresh,
list_gaps, get_gap, prepare_repair (twice), get_repair_attempt,
get_receipt_status. The oracle requires: refresh `completed` with a
non-empty snapshot id; status re-binds that id; list_gaps counts
consistent (`items.len == selected`, `omitted_items.len == omitted`,
`selected + omitted == total`, `selected <= eligible <= total`,
`selected_bytes <= complete_bytes`); at least one gap; get_gap binds the
snapshot, item, and changed behavior; the two prepares are identical; and
the repair chain binds attempt, receipt, and the gap link when the
producer reports ready (or typed `attempt_not_found` for unknown ids when
not ready).

## B3: negative authority

Fresh sessions per probe group: pre-refresh (`no_snapshot` for
list/get/prepare, `attempt_not_found` for attempt/receipt reads),
stale-snapshot ids (`stale_snapshot` plus the current id), unknown item
and attempt ids (`item_not_found`, `attempt_not_found`), and a pipelined
refresh-plus-followers batch that must stay sequential and coherent
(refresh completes, every follower binds that snapshot). Every failure
must be a typed envelope (`code`, `detail`, `recovery`), never a silent
null; every stdout frame must fit the advertised 131072-byte response
bound; stderr stays empty; and the fixture tree gains nothing outside
refresh cache (no repair-attempt or receipt artifacts).

## Anti-gaming twins

Replay twin: preparing a different item (or the same item under a stale
snapshot) must NOT equal the first prepare document, so a constant
responder fails. Oversize twin: a ~200 KB argument (under the input cap)
must fail closed with a typed code and a bounded reply while the session
keeps serving.

## Excitation limits

`analysis_in_flight` is unreachable over one stdio connection by
transport design: the `Admission` gate holds one typed request until its
reply frame flushes, so pipelined followers always observe the completed
snapshot. It is guarded by the pipelined-coherence probe plus the
existing session unit tests that set `in_flight` directly.
`result_too_large` needs a single evidence document over 131072 bytes,
unreachable on this small fixture (the shared budget caps `list_gaps` at
65536 bytes). It is guarded by the advertised-bound pin, the every-frame
audit, and the oversize twin instead of direct excitation.

## Run

```sh
cargo test -p ripr --test agentic_bench_mcp
```

Manual pre-refresh probes against a built fixture root:

```sh
ripr mcp --stdio --root <fixture> < benchmarks/agentic/mcp-journey/b3a.probes.ndjson
```
