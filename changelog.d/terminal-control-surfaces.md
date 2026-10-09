<!-- section: Fixed -->
- Repository text with control or bidi characters no longer reaches the
  terminal raw through `--format github` annotations, command-failure lines
  on stderr, or any stderr notice or warning that quotes a config value, path or
  ref (#6309; every library `eprintln!` now escapes by default). Printed drill-in commands spell such characters as POSIX `"$(printf '\ooo')"`
  segments, so a pasted `ripr explain --root ...` still names the same
  directory.
