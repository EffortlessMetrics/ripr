# Report Packet Index Fixture Corpus

These files pin the Campaign 25 report-packet index corpus for
`RIPR-SPEC-0024`.

They are static fixture artifacts for the future `ripr reports index` producer.
The producer must index explicit existing artifacts only. It must not rerun
analysis, inspect source to fill missing fields, edit source, generate tests,
call providers, run mutation testing, change recommendation ranking, change
gate policy, publish inline comments, or change CI blocking behavior.

Files:

- `corpus.json` records packet-shaped input states and expected report-packet
  index summaries for the bounded cases in RIPR-SPEC-0024.
- `<case>/index.json` and `<case>/index.md` pin the expected report output for
  each route.

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
reason vocabulary, Markdown headings, and advisory limits. The producer and
later generated CI projection should use this corpus as the regression
contract.
