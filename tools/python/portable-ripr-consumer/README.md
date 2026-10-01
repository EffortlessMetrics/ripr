# Portable native RIPR consumer packet

Offline bootstrap for Python-capable environments that already have a
qualified native `ripr` payload and **do not** have Cargo, rustc, or
execution-kernel network access.

This is a packet consumer, not a second semantic replay model, package
system, or agent SDK.

## Packet layout

```text
portable-ripr-consumer/
  manifest.json
  ripr            # or ripr.exe
  run.py
  README.md
```

The packet is supplied as a mounted or extracted directory. `run.py` does not
download or rebuild it.

## Invoke

```bash
python3 run.py \
  --packet /path/to/packet \
  --subject-root /path/to/subject \
  --out /path/to/out \
  --operation check \
  --diff /path/to/change.diff \
  --foreign-cwd /tmp \
  --require-nonzero-subjects
```

`--packet`, `--subject-root`, and `--out` are required. The payload is always
the packet-relative executable resolved to an absolute path. PATH is never
used to select `ripr`. `--operation pilot` passes `--out` to the payload and
classifies `out/pilot-summary.json`, not human terminal stdout. `--diff` is
valid only with `--operation check`. Manifest `argv_template` verbs must
match the allowlisted operation.

## What this proves

- Packet and payload SHA-256 match the manifest before launch.
- Only allowlisted operations (`check`, `pilot`) run.
- Fail-closed classes: digest mismatch, missing executable, incompatible
  payload, timeout, malformed or partial product JSON, typed product
  limitation, zero subjects when required, subject-tree drift, unwritable
  output, launch failure.
- Product `analysis_outcome` kinds are the producer-owned set
  (`complete_with_findings`, `partial_with_limitations`,
  `unsupported_input`, `analysis_failed`, and the other complete kinds).
  Invented kinds fail closed.
- `--subject-digest` is checked before and after launch. A subject tree
  larger than the digest scanner cap is `environment_unavailable`, not
  identity drift.
- A compact receipt at `out/packet-consumption-receipt.json` keeps packet,
  payload, and subject identity separate.

## What this does not prove

- Runtime mutation outcomes. RIPR does not make those claims.
- That an unbuilt RIPR source change was compiled.
- Wheel/npm channel qualification (#4493) or final native payload identity
  (#4521).
- Precompiled Rust-test replay (#4714).

## Authority

Product JSON is reused as-is. The consumer checks `schema_version` and
projects counts; it does not rewrite classifications. Stage the packet from
an explicit binary with repository tests in `xtask` / `crates/ripr`.
