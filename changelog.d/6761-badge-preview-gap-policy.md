<!-- section: Fixed -->
- Diff badges no longer count candidate-current preview-language findings
  (`language_status = "preview"`) toward the calibrated RIPR 0 gap headline.
  Preview unknowns remain visible in native `counts.unknowns` and in the
  analyzed denominator; they do not move the RIPR 0 message. Native badge
  JSON field contracts in `docs/OUTPUT_SCHEMA.md` document that exclusion.
  A candidate-current Rust gap still counts (#6761).
