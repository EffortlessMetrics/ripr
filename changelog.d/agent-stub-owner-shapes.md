<!-- section: Fixed -->
- Agent stub: `ripr agent stub` now writes a compiling stub for four owner
  shapes it refused with reasons that did not name the blocker. A file with
  several inline `#[cfg(test)]` modules gets the stub in the one that already
  names the owner, else the nearest one after it, else the nearest before it
  (was `ambiguous_test_module`). A method of an impl with only lifetime
  generics (`impl<'a> Parser<'a>`, `impl Parser<'_>`) binds its receiver as
  `Parser<'_>` (was `owner_unsupported`). A trait-impl method is called as
  `<Type as Trait>::method(..)` with the trait as the impl header writes it,
  and `Self::Assoc` types are spelled `<Type as Trait>::Assoc` (was
  `owner_trait_method`). A changed field of the struct literal the owner
  returns directly gets a stub asserting the whole return value (was
  `field_type_unresolved`). Impls with type or const generics are refused
  as `owner_generic_impl`, and a field of a literal the owner does not
  return as `field_not_returned`. Test modules gated by more than
  `cfg(test)` are never chosen, and an impl local to a function body or
  `const` block, or an owner behind a cfg in its own file that a plain
  `cargo test` build may not enable, is refused as `owner_unsupported`
  ([#5471](https://github.com/EffortlessMetrics/ripr-swarm/issues/5471)).
