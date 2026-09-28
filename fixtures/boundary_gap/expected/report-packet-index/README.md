# Report Packet Index Fixture Corpus

These files pin the Campaign 25 report-packet index corpus for
`RIPR-SPEC-0024`.

Each case ships a real packet tree and a producer-generated golden. The dogfood
gate copies `<case>/packet/` to a scratch directory, runs the corpus
`canonical_command` there, and compares what `ripr reports index` produced to
`<case>/index.json` and `<case>/index.md`. Nothing here is hand-written: a
golden is re-blessed by running the command, not by editing the file.

The producer must index explicit existing artifacts only. It must not rerun
analysis, inspect source to fill missing fields, edit source, generate tests,
call providers, run mutation testing, change recommendation ranking, change
gate policy, publish inline comments, or change CI blocking behavior.

Files:

- `corpus.json` records the one `canonical_command` every case renders with,
  and per case the `packet_root` to render, the goldens to compare against, and
  the expected report-packet index summary for the bounded cases in
  RIPR-SPEC-0024.
- `<case>/packet/` is the producer input: the artifact tree a repository would
  have under `target/`, laid out the way `ripr reports index` documents.
- `<case>/index.json` and `<case>/index.md` are the rendered output of that
  packet, with `generated_at` pinned to `unix_ms:0` because the renderer stamps
  its own clock.

The corpus intentionally covers:

- complete packet;
- sparse advisory packet;
- missing PR review front panel;
- blocked gate with gate-decision authority preserved;
- missing assistant proof;
- missing validation receipts;
- coverage/grip-present packet.

Case directories:

- `complete-packet/`
- `sparse-advisory/`
- `missing-front-panel/`
- `blocked-gate/`
- `missing-assistant-proof/`
- `missing-receipts/`
- `coverage-grip-present/`

Each case pins status, missing-surface counts, warning/failure counts,
start-here availability, gate-authority presence, group vocabulary, missing
reason vocabulary, Markdown headings, and advisory limits. Because the goldens
are compared byte for byte against a live render, a change to any rendered
label or grouping fails the gate until the goldens are regenerated.

## Regenerating a golden

Run the corpus `canonical_command` with the case's `packet_root` as the working
directory, then copy `target/ripr/reports/index.json` and `index.md` into the
case directory and set `generated_at` back to `unix_ms:0`.
