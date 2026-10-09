<!-- section: Changed -->
- The install-time notes now separate the two source routes. A workspace build
  (the git install in the README) uses the workspace release profile
  (`lto = true`, `codegen-units = 1`); a `cargo build --release` there took 12.5
  minutes on a 4-core container. The packaged 0.11.0 crate carries no profile
  section, so a crates.io install should build with cargo's default release
  profile; a local `cargo build --release` of the packaged crate took 4.6
  minutes there (not a crates.io install, which 0.11.0 does not have yet). About
  700 of 907 CPU-seconds are the `ripr` crate itself, not its dependencies
  (#5311).
