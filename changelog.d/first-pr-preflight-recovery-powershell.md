<!-- section: Fixed -->
- `ripr first-pr` preflight recovery (a missing base or head, an empty diff or a
  diff that cannot be read) now carries its commands as separate
  `recovery_commands` in `start-here.json`, with the reason in
  `recovery_guidance`. `start-here.md` prints each single-line step in its own
  code span, with a labelled PowerShell form for supported translations. A
  command that runs unchanged is marked as such, and an unsupported or
  multiline command is disclosed or withheld. For supported forms, an
  apostrophe or backtick in a root or branch stays one argument in Bash and
  PowerShell. The missing-base fetch names the repository with `git -C`,
  fetches with an explicit refspec and runs before the rerun. Older packets
  keep their `next_command` presentation.
