<!-- section: Changed -->
- The Rust include boundary note (`include!` with a dynamic path) now ends with
  the same orientation sentence as the module composition note: it describes
  indexed context, not a finding, and needs no action unless expected evidence
  is missing. It printed on every `check` and `pilot` of crates such as
  thiserror and libc with no sign it could be ignored.
