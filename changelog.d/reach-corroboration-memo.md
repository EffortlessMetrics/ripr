<!-- section: Changed -->
- Performance: in `ripr pilot` and `ripr check`, transitive-witness corroboration (#6009)
  masks each test body once and remembers each test's receiver-type answer,
  and resolves the reaching functions' `impl` self types once per entry
  instead of once per test. Cold pilot on rust-analyzer takes about 20s
  instead of 78s (35s CPU instead of 282s), with byte-identical output.
