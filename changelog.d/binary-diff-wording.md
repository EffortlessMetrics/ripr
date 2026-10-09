<!-- section: Fixed -->
- `ripr check --diff` on a well-formed diff that holds only binary or file-mode
  changes now says so ("no text hunks to analyze") instead of telling the reader
  to provide a valid unified diff. The outcome is unchanged: `unsupported_input`,
  incomplete, never a clean result.
