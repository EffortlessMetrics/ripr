<!-- section: Performance -->
- `ripr pilot`: the evidence context builds its import and owner-name
  tables once and derives its three target-affinity helper tables from one
  pass, instead of rescanning every source's `use` statements up to four
  times. Over two cold runs each, pilot wall time fell from 31.7s to 26.9s
  on bevy, 26.1s to 22.0s on nushell and 17.3s to 15.9s on rust-analyzer,
  with byte-identical output.
