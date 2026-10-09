<!-- section: Changed -->
- Performance: in `ripr pilot` and `ripr check`, the macro-reach fallback for
  `no_static_path` seams builds its `macro_rules!` definition table in one
  pass over the index, and reads each test's and function's macro
  invocations from a table built once, instead of rescanning every indexed
  source for each owner and invoked macro name. Cold pilot on
  rust-analyzer takes 14.3s instead of 22.5s, with byte-identical output.
