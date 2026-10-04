# RIPR-SPEC-0203: Global verbose flag contract

Status: proposed

Owner: product / cli

Created: 2026-10-02

Linked issues:

- #5009 (this slice)
- #2610 (any-position global `--verbose` extraction)
- #4825 (the `ripr help --json` machine route and its strict grammar)

Support-tier impact:

- None. The flag only adds stderr diagnostics; it accepts no authority,
  starts no server, and writes nothing. The MCP protocol stream on stdout
  stays clean because the diagnostic goes to stderr. See
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md).

Policy impact:

- None. No new process, network, or file-policy surface.

## Problem

`--verbose` / `-v` was a global flag the CLI accepted in any argv position,
extracted before every command parser, but documented nowhere a user could
find it. Worse, the two pre-dispatch extraction sites disagreed
(#5009): `cli/mod.rs` removed only the first occurrence while
`startup.rs` removed all occurrences before MCP routing. The same flag
therefore obeyed two rules: `ripr -v -v mcp --stdio` routed while
`ripr -v -v check` failed with `unknown check argument "-v"`, and
`ripr check --base -v` silently consumed the value slot of `--base`. The
per-command flag/help parity gate (#2342, revived by #4317) could never
catch the disclosure gap because the flag bypasses every command parser
and `cli/mod.rs` was not in the gate's `PARSER_SOURCES`.

## Behavior

- One owner, one pass: `parse::extract_global_verbose` is the single
  authority that removes the global flag from argv. Both the CLI dispatch
  (`cli::run`) and the binary startup route (`startup::routed_mcp_args`,
  which must strip before MCP routing) call the same function, re-exported
  as `ripr::cli::extract_global_verbose`.
- Stripping rule: every occurrence of `-v` or `--verbose` is removed, so a
  repeated flag is idempotent — `ripr -v -v <command>` behaves exactly like
  `ripr -v <command>` on the MCP route and every other command family.
- Disclosure: the flag is documented on the `ripr help --all` reference
  with what verbose mode adds (stderr pipeline diagnostics: analyzed
  languages, mode, probe/finding counts) and the any-position contract.
  A parity-style gate mines the owner function's body with the same
  scanner the per-command gate uses and requires every accepted spelling
  to appear on that reference, so a new global spelling cannot land
  without a disclosure in the same PR.
- Value position: extraction is deliberately flag-arity blind (routing
  the global flag through per-command parsers is a non-goal), so a
  `-v`/`--verbose` token is always the global flag and can never serve as
  another flag's value. `ripr check --base -v` enables verbose and lets
  `check` report its own `missing value for --base`. The limitation is
  disclosed on the same help surface rather than guessed from the previous
  token, which would break the appended-position contract for boolean
  flags (`ripr check --quiet -v`, `ripr mcp --stdio -v`).
- The #4825 contract is unchanged: a version request never emits the
  verbose diagnostic, and combining the flag with `ripr help --json` fails
  closed with the strict usage error before any document bytes or stderr
  diagnostic leak.

## Required Evidence

- Unit tests on the extraction owner: all-positions removal, idempotent
  repeat, lookalike tokens left untouched.
- Dispatch-path tests through `cli::run`: a repeated flag reaches the
  command parser; the value-position boundary reports the command's own
  missing-value error; the `help --json` refusal fires identically for one
  flag, a repeat, and the appended spelling.
- Startup-path tests through `routed_mcp_args`: a repeat routes to the
  same MCP argv as a single flag, for both the direct and `help mcp`
  routes.
- Disclosure gate: the owner's accepted spellings mined from source must
  each appear on the `ripr help --all` reference, which must also disclose
  the any-position contract and the stderr destination.

## Non-Goals

- New verbose diagnostics or a `--debug`/`--trace` tier.
- Changing what verbose mode currently prints.
- Routing `--verbose` through per-command parsers.
- Arity inference from the previous argv token.

## Acceptance Examples

- `ripr -v -v check` and `ripr -v -v mcp --stdio` both succeed with
  verbose enabled and one `ripr: verbose mode enabled` diagnostic.
- `ripr check --base -v` enables verbose and exits with
  `missing value for --base` from `check`'s own parser.
- `ripr help --all` contains a Global flags entry naming `-v` and
  `--verbose`, the stderr diagnostics they add, and the any-position
  contract.
- `ripr -v help --json` and `ripr help --json --verbose` both fail closed
  with `usage: ripr help --json (this route accepts no other arguments)`
  and no verbose diagnostic on stderr.
- `ripr --verbose --version` prints only the version line with empty
  stderr.

## Test Mapping

- `crates/ripr/src/cli/parse.rs::tests::extract_global_verbose_removes_every_occurrence_in_any_position`
- `crates/ripr/src/cli/parse.rs::tests::extract_global_verbose_is_idempotent_for_a_repeated_flag`
- `crates/ripr/src/cli/parse.rs::tests::extract_global_verbose_keeps_lookalike_and_plain_tokens`
- `crates/ripr/src/cli/mod.rs::tests::run_treats_a_repeated_global_verbose_as_one_enable`
- `crates/ripr/src/cli/mod.rs::tests::run_keeps_the_global_verbose_out_of_flag_value_position`
- `crates/ripr/src/cli/mod.rs::tests::run_rejects_verbose_on_the_machine_discovery_route`
- `crates/ripr/src/startup.rs::tests::startup_extracts_the_global_verbose_flag_in_any_position`
- `crates/ripr/src/startup.rs::tests::startup_treats_a_repeated_global_verbose_as_one_enable`
- `crates/ripr/src/cli/help.rs::tests::global_verbose_spellings_are_documented_on_help_all`
- `crates/ripr/tests/cli_smoke.rs` (version precedence and the
  `help --json` fail-closed contracts exercised end to end)

## Implementation Mapping

- `crates/ripr/src/cli/parse.rs` — `extract_global_verbose`, the single
  stripping owner.
- `crates/ripr/src/cli/mod.rs` — dispatch calls the owner before
  `parse_args`; owns the #4825 machine-route refusal.
- `crates/ripr/src/startup.rs` — MCP route detection skips verbose tokens;
  the MCP route strips through the same owner before `mcp::run`.
- `crates/ripr/src/cli/help/overview.rs` — the `HELP_ALL` Global flags
  entry.
- `crates/ripr/src/cli/help.rs` — the disclosure gate.
- `policy/public_api.txt` — the `ripr::cli::extract_global_verbose`
  re-export allowlist entry.

## Metrics

- unit_test_pass_rate
