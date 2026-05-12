// Workflow identity, permissions, toolchain setup, and pilot packet generation.
pub(super) const WORKFLOW: &str = r####"name: RIPR

on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  pull-requests: write
  security-events: write

env:
  RIPR_UPLOAD_SARIF: "true"
  RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}
  RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}
  RIPR_COMMENT_MODE: ${{ vars.RIPR_COMMENT_MODE || 'off' }}

jobs:
  ripr:
    name: RIPR advisory reports
    runs-on: ubuntu-latest
    continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - name: Install ripr
        run: cargo install ripr --locked

      - name: Generate RIPR pilot packet
        continue-on-error: true
        run: |
          ripr pilot \
            --root . \
            --out target/ripr/pilot \
            --mode ready \
            --max-seams 5

"####;
