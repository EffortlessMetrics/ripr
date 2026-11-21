# LSP Transport Bounds

Issue #2034. The bounded ingress/concurrency/egress policy for
`ripr lsp --stdio` and the evidence for each bound. The claim boundary is
narrow: this hardens the stdio transport against resource exhaustion and
malformed clients. It does not sandbox the operating system, prove
resistance to every denial-of-service technique, or authorize untrusted
network exposure.

## Enforcement points

tower-lsp-server 0.23 constructs its `LanguageServerCodec` inside
`Server::serve` and exposes no max-payload knob, so the framing cap cannot be
installed by configuring the codec. The sanctioned generic seam is
`Server::new(stdin, stdout, socket)`, which accepts any `AsyncRead + Unpin` /
`AsyncWrite` pair. ripr installs byte-transparent adapters there
(`crates/ripr/src/lsp/transport_bounds.rs`, wired in
`crates/ripr/src/lsp.rs::serve_streams`):

- `BoundedStdinReader` observes the `Content-Length` framing as bytes pass
  through (never modifying them) and trips on an oversized header block or an
  oversized declared `Content-Length`. A trip delivers one bounded
  `io::Error`; tokio-util's `FramedRead` fuses after the first decode error,
  so tower-lsp emits exactly one bounded `-32700` JSON-RPC response with a
  null id and the read loop ends — the same behavior class as a malformed
  frame (pinned in `crates/ripr/tests/lsp_lifecycle.rs` since #2030).
- `BoundedStdoutWriter` enforces the write-stall deadline below. On expiry it
  errors the write half and trips a shared session flag that wakes the stdin
  adapter to report EOF, so the server terminates cleanly instead of wedging
  on a dead reader.
- `Server::concurrency_level` is set explicitly so the in-flight request
  bound is owned and reviewed rather than inherited silently.

Typed payload bounds live in `crates/ripr/src/lsp/payload_bounds.rs` and run
at handler entry in `crates/ripr/src/lsp/backend.rs`, before any config load,
analysis, refresh scheduling, Git, filesystem, or subprocess work, and before
snapshot fast paths. Rejection is a bounded JSON-RPC `-32602`
(`InvalidParams`) error whose message names the bound — never attacker
input.

## Limits and how they were chosen

| Bound | Value | Justification |
| --- | --- | --- |
| Max message body (`Content-Length`) | 16 MiB | Largest legitimate ingress class is `didOpen`/`didSave` full document text; multi-MiB generated sources are plausible. Egress is independently bounded by the diagnostic budget (500 items / 64 KiB serialized caps in `lsp/diagnostic_budget.rs`), so ingress dominates; 16 MiB gives wide headroom while capping the bytes one frame can force the codec's read buffer to hold. |
| Max framing header block | 8 KiB | Legitimate headers are under ~100 bytes (`Content-Length` plus optional `Content-Type`); bounds a client streaming header bytes forever. |
| In-flight request concurrency | 4 | tower-lsp's own implicit default, made explicit. Must stay above 1 so the built-in `$/cancelRequest` notification and lifecycle messages stay serviceable under load. |
| Write-stall deadline | 120 s | Any successful write resets the clock; only a reader that has completely stopped draining stdout trips it. Converts a permanent bounded-memory wedge into clean process termination. |
| `initialization_options` | 64 KiB (size estimate) | Only a handful of known keys are read (`lsp/config.rs`). |
| `previousResultIds` | 4096 entries; 4096 B/URI; 1024 B/value | One entry per tracked document must tolerate monorepo pull-diagnostic sessions; bounds the URI-set clone and per-document scan. |
| `executeCommand` arguments | 8 entries; 64 KiB total (size estimate) | Every RIPR command takes zero or one argument object; bounds all downstream identifiers (gap/seam/snapshot ids) transitively. |
| JSON nesting depth | 128 | Enforced by serde_json's default recursion limit; the `unbounded_depth` feature is not enabled anywhere in the workspace. Over-deep bodies are bounded codec errors. |

## Composition with existing budget machinery

These bounds compose with, and do not duplicate, the existing authorities:

- expensive analysis work is already funneled through the refresh scheduler
  (`lsp/refresh_scheduler.rs`: one active attempt plus coalesced pending);
- egress payloads are already bounded by the diagnostic budget
  (`lsp/diagnostic_budget.rs`);
- cancellation is the tower-lsp built-in `$/cancelRequest` plus the ripr
  cancellation substrate (`analysis/cancellation.rs`), both of which stay
  serviceable because request concurrency is above 1;
- tower-lsp internally bounds queued request futures at 100 with read-loop
  backpressure — bounded, though the constant is not configurable without
  forking the transport.

## Named residuals

- **Frame-level recovery does not exist on the wire.** tokio-util fuses the
  framed stream after the first decode error, so any framing violation —
  including an ingress bound trip — ends the session after one bounded
  `-32700`. This is deliberate ("terminate cleanly when framing cannot be
  recovered") and pinned by tests.
- **Duplicate `Content-Length` headers are last-wins** in the vendored codec;
  `BoundedStdinReader` mirrors last-wins so both layers agree on which value
  governs the cap.
- **Unknown extra headers are warn-only** inside the vendored codec; the
  header-block byte cap bounds their cost.
- **`initialize.processId` is accepted but not monitored** (#2030). A dead
  client holding stdin open with output pending now trips the write-stall
  deadline; a fully idle half-open session is still only terminated by
  client EOF. Full lifecycle cleanup is #2030's scope.
- **The queued-future bound (100)** is a tower-lsp internal constant,
  bounded but not configurable without forking the transport.
- **`riprAgent/*` requests are capability-only** in this slice
  (`lsp/agent_protocol.rs`); there is no live request family to bound. When
  handlers land they must register payload bounds in
  `lsp/payload_bounds.rs` first.
- **Slow-reader egress** cannot be given a per-message write deadline
  without forking the transport; the enforced posture is bounded memory
  (rendezvous response channel) plus the session-level write-stall trip.
- The LSP surface manifest gate (#1995) has not landed, so this document is
  the durable record of the limits; when the manifest exists these bounds
  should be represented there.

## Evidence

- `crates/ripr/src/lsp/transport_bounds.rs::tests` — adapter unit tests plus
  two in-process end-to-end tests over the real `serve_streams` composition:
  an oversized frame yields one bounded `-32700` then a clean stop, and a
  client that stops reading trips the write-stall deadline and ends the
  session instead of wedging it.
- `crates/ripr/src/lsp/payload_bounds.rs::tests` — typed bound unit tests.
- `crates/ripr/tests/lsp_lifecycle.rs` section 9 (issue #2034) — real-binary
  adversarial cases: oversized declared `Content-Length` with no body,
  oversized header block, integer-overflow and negative `Content-Length`,
  deeply nested JSON, giant `previousResultIds` with duplicates, oversized
  result-id values, oversized `executeCommand` arguments (bytes and count),
  oversized `initialization_options` without session poisoning, a concurrent
  request burst with `$/cancelRequest`, and exit during request load. Each
  negative asserts a bounded response, no attacker echo, a responsive server
  afterwards (or prompt clean termination where framing cannot recover), and
  no orphan process.
