<!-- section: Changed -->
- Function/test bodies and probe-shape text share the file's source
  allocation as spans instead of one `String` per body. Peak `check`
  RSS falls about 80MB on a mid-sized workspace (719MB to 637MB on
  the #5415 repro) with byte-identical output; the file-fact cache
  moves to schema 1.21, and out-of-range, split-character, or legacy
  bare-string payloads fail the decode so the entry cold-recomputes
  (#5415).
