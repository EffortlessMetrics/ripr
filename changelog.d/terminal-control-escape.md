<!-- section: Fixed -->
- Human reports (`ripr check`, `--format human-full`, `ripr explain`) now print
  terminal control characters from repository text as `\u{XX}`. An assertion
  message or test source holding ESC/OSC sequences, BEL, a bare CR or a bidi
  override previously reached the terminal verbatim and could clear the screen,
  retitle the window or reorder what the reader saw. Newlines and tabs are
  kept; JSON and SARIF are unchanged.
