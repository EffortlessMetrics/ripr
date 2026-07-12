# RIPR-SPEC-0112: Bounded Subprocess Adapter Boundary

Status: accepted

## Problem

Syntax-only call probes over-classify a bounded, receipt-producing subprocess
adapter as an arbitrary call or an unconnected effect. That creates an
unactionable strict-exposure result for adapters such as ub-review's `curl`
request path while leaving genuinely dynamic process execution on the normal
strict path.

## Behavior

The Rust diff probe builder may classify a changed line as the existing
`side_effect` family only when all of these conditions hold in its owning
function:

- the process command is exactly one literal command from the allowlist;
- the command constructs arguments through `.arg(...)` or `.args(...)`;
- a timeout bound is visible (`--max-time`, `timeout_sec`, or `.timeout(...)`);
- output is captured through `.output()`, `.status()`, or a spawned child passed
  to `wait_for_child_output_files`;
- failure cleanup visibly includes process termination/wait and receipt-file
  cleanup, with an explicit error path; and
- shell dispatch and shell-style `-c` arguments are absent.

The initial allowlist contains only the literal `curl` command. The rule is
deny-by-default: dynamic command names, missing bounds, shell dispatch, and
other subprocess shapes retain the existing probe classification and strict
zero behavior.

This is a classification boundary, not a runtime-safety claim. The output
continues to use the existing `side_effect` probe family, preserving the exact
changed file, line, probe id, and exposure class in JSON receipts. No finding
is deleted solely because the adapter is bounded.

## Required Evidence

- `crates/ripr/src/analysis/probes/subprocess.rs` contains the allowlist,
  deny-by-default recognizer, positive adapter fixture, and dynamic/shell/
  unbounded negative cases.
- `crates/ripr/src/analysis/probes/diff.rs` routes only qualifying added lines
  through the boundary; removed lines retain the ordinary path.
- `probes_for_file_emits_side_effect_for_bounded_adapter_line` pins the actual
  changed-file probe output, including argument construction, timeout/error
  handling, child capture, and receipt cleanup.

## Non-Goals

- Do not classify arbitrary shell strings or dynamic process execution as
  bounded.
- Do not suppress findings in a consumer or relax strict-zero policy.
- Do not claim that static exposure analysis proves runtime subprocess safety.
- Do not change GitHub review or receipt semantics.

## Validation

```text
cargo test -p ripr subprocess --locked
cargo clippy -p ripr --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

## Acceptance Examples

### Bounded adapter

```text
ProcessCommand::new("curl")
  + literal arguments
  + --max-time / timeout_sec
  + spawned output capture
  + kill/wait and receipt-file cleanup
  -> probe family: side_effect
```

The emitted probe retains its original changed path, line, id, expression, and
later exposure class in JSON output.

### Dynamic or shell command

```text
ProcessCommand::new(command_name)
ProcessCommand::new("curl").arg("-c").arg(script)
ProcessCommand::new("curl") without a timeout or cleanup path
-> ordinary probe classification; no boundary promotion
```

## Test Mapping

| Requirement | Test |
|---|---|
| Literal allowlisted command and bounded adapter shape | `bounded_literal_curl_adapter_is_an_observable_effect` |
| Dynamic, shell, and unbounded commands remain strict | `dynamic_or_shell_commands_remain_unclassified` |
| Actual diff probe preserves path/line and emits `side_effect` | `probes_for_file_emits_side_effect_for_bounded_adapter_line` |

## Implementation Mapping

| Surface | Responsibility |
|---|---|
| `crates/ripr/src/analysis/probes/subprocess.rs` | Deny-by-default boundary and fixture tests |
| `crates/ripr/src/analysis/probes/diff.rs` | Apply boundary only to added changed lines |
| `docs/STATIC_EXPOSURE_MODEL.md` | Document bounded outbound adapters as `side_effect` |
| `docs/specs/README.md` and `.ripr/traceability.toml` | Index and traceability |

## Metrics

- `unit_test_pass_rate`: the three boundary tests pass.
- No new output class or schema field is introduced.
- Unqualified subprocess probes remain eligible for existing strict exposure
  counts; this slice does not lower strict-zero policy thresholds.
