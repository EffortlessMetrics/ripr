<!-- section: Changed -->
- `ripr check` no longer refuses a diff whose changed package alone holds more
  than 1,200 Rust files. The 1,200 value now only decides when Draft/Fast
  narrows dependent packages (`RIPR_DIFF_NARROW_INDEX_FILES`), and the
  `diff_scope_oversized` memory guard (`RIPR_MAX_DIFF_INDEX_FILES`) defaults to
  10,000 files. An edit in wasm-bindgen's 1,781-file `web-sys` crate was
  refused and now runs in 2.5 s cold and 1.7 s warm at 141 MB.
  The editor sidecar uses the same limits, so LSP refreshes on 1,200 to
  10,000-file packages now produce diagnostics instead of a refusal.
