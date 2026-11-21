# Exit Codes

This reference describes the current source tree. Check `ripr --version` and
the installed command's `--help` when using an older release; the
[Quickstart](QUICKSTART.md) includes a compatible `0.10.0` first-check path.

RIPR uses `0` for command completion and `2` for a reported error or a
command-specific policy rejection. **The exit code is not a finding
classification.** In particular, `ripr check` is advisory: it can report weak
or missing static evidence and still exit with `0`.

| Code | Meaning | What it does not establish |
| --- | --- | --- |
| `0` | The command completed according to its own contract. | That findings are absent, the requested behavior was fully analyzed, tests executed, or a policy approved the change. |
| `2` | RIPR reported an input, configuration, analysis, output, or command-specific policy error. | Whether the failure was an analysis problem or a policy rejection; inspect stderr and any valid report. |

Shell launch failures and process termination can produce other statuses.
Treat any unexpected nonzero status as a failed invocation and inspect its
diagnostics; do not assume a particular shell failure will use `1`.

## `ripr check`: analysis is advisory

Successful analysis and rendering return `0`, regardless of the findings'
static-exposure labels. An `exposed` finding does not make the command fail,
and neither does a `weakly_exposed` finding by itself. Use the report to decide
what to inspect or repair.

An empty result also needs context. Check the selected diff, omitted tracked
edits, warnings, and any partial or limited state before interpreting it.
[Choose the change to analyze](QUICKSTART.md#choose-the-change-to-analyze)
explains committed, working-tree, and saved-diff inputs.

Examples of errors that return `2` include an invalid argument or config,
an unresolved base, an unreadable diff, and an analysis or output-write
failure. Some input-limit failures emit a structured JSON limitation before
returning `2`; other errors occur before JSON is written. A redirected file
can therefore be empty, incomplete, or an error report. Its existence is not
evidence of successful analysis.

## `ripr gate evaluate`: a separate policy decision

`ripr gate evaluate` consumes evidence and applies the selected gate mode.
Its default `visible-only` mode records advisory evidence. Blocking modes and
an explicitly supplied exception policy can reject a change; the CLI returns
`2` for a `blocked` or `config_error` decision.
Invalid arguments, missing required inputs, and report I/O errors can also
return `2`.

When evaluation completes, inspect the generated gate-decision JSON and
Markdown for the mode, decision, input status, and reasons. A successful
`visible-only` invocation does not establish readiness to block merges.
See [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) and
[Blocking readiness](BLOCKING_READINESS.md).

## `ripr doctor`: required checks and advisory probes

`ripr doctor` and `ripr doctor --json` return `0` when the required checks pass
and `2` when they fail. Core checks include the selected root, Cargo manifest,
configuration, and availability of Git, Cargo, and a supported Rust compiler.
A missing `ripr.toml` uses built-in defaults and is not itself a failure.

Primary runtime probes are required when their language is enabled in the
effective configuration, even if no files for that language were detected.
Detected-but-disabled preview languages and optional test or package runners
remain advisory. Read each probe's requirement and the overall status; an
optional missing tool is not automatically a command failure.

Doctor checks development prerequisites as well as CLI setup. Installing a
prebuilt binary does not install the project toolchain that doctor probes.

## Capture failures without losing the diagnosis

This Bash example preserves the command status and keeps diagnostics separate
from JSON. Run it from the project checkout with the intended base available:

```bash
check_status=0
ripr check --base origin/main --format json > check.json 2> check.stderr || check_status=$?
if [ "$check_status" -ne 0 ]; then
  cat check.stderr >&2
  exit "$check_status"
fi
# Inspect check.json and any warnings in check.stderr before consuming the evidence.
```

In an advisory CI workflow, retain the captured status, stderr, and any output
as diagnostic artifacts even if the workflow continues. Validate a report
before passing it to a downstream consumer. `continue-on-error` changes the
workflow's handling of a failure; it does not turn an unsuccessful analysis
into valid evidence. See [CI strategy](CI.md).

For an unexpected `ripr: internal error (this is a bug)` message, preserve
stderr, the RIPR version, and the reproduction command when reporting the
failure. The top-level panic boundary also maps such a failure to `2`.
