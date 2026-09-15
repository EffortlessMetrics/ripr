pub(super) const AGENT_HELP: &str = r#"Create a bounded repair transaction for a coding agent and inspect its evidence.

Usage: ripr agent <subcommand>

Primary workflow:
  repair    Run the before/edit/after repair transaction and its verification phase for one seam.
  status    Report existing agent-loop artifacts and the exact next command.

Advanced and compatibility workflows:
  start      Write a source-edit-free workflow manifest for one seam.
  brief      Rank a working-set brief for the agent-active router.
  packet     Expand one visible seam into the existing agent seam packet JSON.
  verify     Compare before/after repo-exposure JSON for agent verification.
  verify-execute
             Execute one validated producer-owned direct verify route.
  receipt    Summarize one seam from agent verify JSON for review handoff.
  review-summary
             Join agent-loop artifacts into a compact review packet.

Ordinary repair path:
  ripr agent repair --seam-id ID --phase before
  # edit one focused test outside RIPR
  ripr agent repair --attempt ID --phase after

Run `ripr agent repair --help` for the primary repair workflow. Run
`ripr agent status --help` to inspect an interrupted or incomplete local loop.
The lower-level `start`, `brief`, `packet`, `verify`, `verify-execute`,
`receipt`, and `review-summary` help surfaces remain available for explicit
control, debugging, and compatibility.
"#;
pub(super) const AGENT_START_HELP: &str = r#"Start a source-edit-free workflow packet for one selected change.

Usage: ripr agent start [--root PATH] --seam-id ID [--out PATH]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --seam-id ID     Select one visible seam by ID.
  --out PATH       Workflow output directory (relative to --root). Defaults to target/ripr/workflow.

The start command writes a source-edit-free workflow packet for one seam:
workflow.json, commands.md, and agent-brief.json. The packet contains artifact
paths and shared command templates for the before snapshot, packet, brief,
after snapshot, verify, and receipt steps. It remains advisory and static; it
does not call an LLM API, run mutation testing, generate tests, edit files,
change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_BRIEF_HELP: &str = r#"Write a bounded brief for a coding agent over the current diff or change.

Usage: ripr agent brief [--root PATH] (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json [--max-seams N]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --diff PATH      Select a diff file and line-level working set.
  --base REV       Derive the working set from a base revision.
  --files PATHS    Comma-separated repo-relative file paths.
  --seam-id ID     Select one visible seam by ID.
  --json           Required until a human brief surface exists.
  --max-seams N    Requested seam cap. Defaults to 3 and cannot exceed 10.

When --files or --base produces a working set that matches no seams, the brief
falls back to all repo-actionable seams. This scope-widening is disclosed in
the warnings[] field of the JSON output — check warnings[] to confirm whether
you received a scoped result or a repo-wide fallback.

This parser is the first implementation seam for RIPR-SPEC-0010. The brief
router remains advisory and static; it does not run mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_PACKET_HELP: &str = r#"Write a per-change handoff packet for a coding agent.

Usage: ripr agent packet [--root PATH] --seam-id ID --json
       ripr agent packet [--root PATH] --gap-ledger PATH --gap-id ID --json

Options:
  --root PATH        Workspace root. Defaults to current directory.
  --seam-id ID       Select one visible seam by ID.
  --gap-ledger PATH  Explicit gap decision ledger JSON.
  --gap-id ID        Select a GapRecord by gap_id or canonical_gap_id.
  --json             Required until a human packet surface exists.

The packet command expands a seam selected by `ripr agent brief` into the
existing agent-seam-packets-json envelope with one packet. With `--gap-ledger`,
it renders one agent packet from an explicit agent-packet-eligible GapRecord
without rerunning analysis. It remains advisory and static; it does not run
mutation testing, generate tests, edit files, change cache behavior, or touch
LSP/MCP surfaces.
"#;
pub(super) const AGENT_VERIFY_HELP: &str = r#"Verify static-evidence movement between a before and after snapshot.

Usage: ripr agent verify [--root PATH] --before PATH --after PATH --json

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --before PATH    Before `repo-exposure-json` snapshot.
  --after PATH     After `repo-exposure-json` snapshot.
  --json           Required until a human verify surface exists.

The verify command compares two saved static repo-exposure artifacts and emits
an agent-focused before/after summary. Snapshot paths must resolve under
`--root`. The command remains advisory and static; it does not run analysis,
mutation testing, generate tests, edit files, change cache behavior, or touch
LSP/MCP surfaces.

Movement vocabulary (#2648): the JSON output uses two related fields:
  - `change` (per-item): improved | changed | regressed | unchanged | new | resolved
  - `gap_movement` (per-item): closed | persisted | new
Precedence: `change: improved` means the grip class moved up; `gap_movement: closed`
means the gap disappeared entirely. A gap can persist even when grip improves.
"#;
pub(super) const AGENT_VERIFY_EXECUTE_HELP: &str = r#"Execute one validated producer-owned verification route.

Usage: ripr agent verify-execute [--root PATH] --packet PATH --result-json PATH --authorize --json [--cancel-after-ms N]

Options:
  --root PATH          Workspace root. Defaults to current directory.
  --packet PATH        One producer-emitted agent packet JSON under --root.
  --result-json PATH   New result path under --root; existing files are refused.
  --authorize          Required explicit authorization for process execution.
  --cancel-after-ms N  Optional bounded cancellation request.
  --json               Required machine-readable output.

Only the exact direct, no-network, no-write `ripr agent verify` route is
executable. Authority is layered: the packet's typed `command_specs.verify` must
be reproducible from its own `verification_commands`, so the route you read is
the route that runs; and both `--before` and `--after` must pass repo-exposure
provenance validation, after which the canonical route is recomputed from those
validated artifacts and must match the packet. The packet chooses which
producer artifacts to compare; it never authors the command.

The command uses the current ripr binary, a bounded timeout, output caps, and
repository currentness checks. The child environment is reduced to a disclosed
platform floor (PATH plus platform essentials); ambient credentials are dropped,
though HOME is passed through so host Git configuration still applies. Only the
owned child is terminated.

Every parsed attempt, including refusals, emits typed JSON on stdout; usage
errors stay on stderr. Exit status is 0 when a bounded observation was committed
-- including an observed command failure -- and nonzero when none was. It
records process evidence only; it does not issue receipts, run mutation testing,
prove adequacy, or grant gate or merge authority.
"#;
pub(super) const AGENT_RECEIPT_HELP: &str = r#"Write a provenance receipt with bounded next-action guidance for one change.

Usage: ripr agent receipt [--root PATH] --verify-json PATH --seam-id ID --json [--test NAME] [--command CMD] [--out PATH]

Options:
  --root PATH         Workspace root. Defaults to current directory.
  --verify-json PATH  JSON emitted by `ripr agent verify`.
  --seam-id ID        Select one seam from the verify JSON.
  --json              Required until a human receipt surface exists.
  --test NAME         Optional focused test added or changed by the agent.
  --command CMD       Optional verification command that was run. Repeatable.
  --out PATH          Write the JSON receipt to a file instead of stdout.

The receipt command narrows a saved agent verify artifact to one seam and adds
handoff metadata for review. The verify JSON path and the before/after snapshot
paths named inside it must resolve under `--root`; receipt provenance hashes
those three artifacts without rerunning analysis. It remains advisory and
static; it does not run analysis, mutation testing, generate tests, edit files,
change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_STATUS_HELP: &str = r#"Report local agent-loop artifact state and the next command to run.

Usage: ripr agent status [--root PATH] [--json]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --json           Emit the machine-readable status report. Human Markdown is the default.

The status command reads existing agent-loop artifacts under target/ripr only
and reports which before snapshot, after snapshot, brief, packet, verify, and
receipt files are present or missing. It may recover a seam_id from those
artifacts and emits the next command to run for missing inputs. It remains
advisory and static; it does not run analysis, mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_REVIEW_SUMMARY_HELP: &str = r#"Summarize agent-loop artifacts into a compact review packet.

Usage: ripr agent review-summary [--root PATH] [--json]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --json           Emit the machine-readable review summary. Human Markdown is the default.

The review-summary command reads existing agent-loop artifacts and joins agent
status, receipt, workflow, operator cockpit, repo exposure, LSP cockpit when
present, and local CI artifact state into a compact review packet. It remains
advisory and static; it does not run analysis, mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;
pub(super) const AGENT_REPAIR_HELP: &str = r#"Run the before/edit/after repair transaction and its verification phase for one named gap.

Usage: ripr agent repair [--root PATH] --seam-id ID --phase before
       ripr agent repair [--root PATH] (--attempt ID|--seam-id ID) --phase after
       ripr agent repair [--root PATH] --attempt ID --phase verify
           [--verify-authorized --verify-authority ID] [--verify-rollback]

Options:
  --root PATH          Workspace root. Defaults to current directory.
  --seam-id ID         Select one visible seam by ID; required for `before` and
                       the compatibility selector for `after`.
  --attempt ID         Select one durable repair attempt; valid for `after`
                       and `verify` (verify accepts only `--attempt`).
  --phase before|after|verify
                       Which phase of the repair loop to run.
  --python-repair-trust-manifest PATH
                       Bind this attempt to an accepted Python repair-trust
                       selection manifest (RIPR-SPEC-0176); `before` only.
  --python-repair-trust-attempt ID
                       The selection attempt identity to bind; requires the
                       manifest flag.
  --edit-authorized    Explicitly authorize the bounded test-only edit for
                       this attempt; requires an authority.
  --edit-authority ID  The operator or agent identity the authorization is
                       recorded under; requires --edit-authorized.
  --verify-authorized  Explicitly authorize the bounded verification run for
                       this attempt; `verify` only; requires an authority.
  --verify-authority ID
                       The operator or agent identity the verification is
                       re-affirmed under (the same authority that authorized
                       the edit); requires --verify-authorized.
  --verify-rollback    After the observation, restore the applied edit and
                       record the rollback proof; `verify` only; requires the
                       verification authorization.

The ordinary repair path is:

  ripr agent repair --seam-id ID --phase before
  # edit one focused test outside RIPR
  ripr agent repair --attempt ID --phase after

Use `--attempt ID` for the normal after phase. `--seam-id ID` remains a
compatibility route and fails closed when multiple awaiting attempts share a
seam.

The before phase writes the pre-edit repo-exposure snapshot and repair packet.
The after phase writes the post-edit snapshot, persists static verification
JSON, and emits a receipt. RIPR owns the evidence plumbing; the human or
external agent owns the test edit.

With the Python repair-trust flags, the before phase verifies the selection
row by digest (manifest digest, row selection digest, current HEAD, exact
test-only target agreement with the packet) and stages the binding into the
durable attempt; the after phase re-verifies the same digests before recording
the applied edit and requires the same explicit authorization. A binding
drift, ambiguity, unsafe surface, or missing authorization fails before any
edit is recorded; repository HEAD drift between the phases is instead owned
by the durable finish, which records the typed stale state. The driver
records no verification result, no static movement, and no closure.

The verify phase (--attempt ID --phase verify --verify-authorized
--verify-authority ID) revalidates every retained identity of the applied
trust-bound attempt, executes only the packet's producer-owned typed verify
route through the bounded execution rails, reruns the analysis against the
exact post-edit state, compares the native-identity movement before/after,
optionally proves the rollback, and publishes one immutable candidate
receipt. Execution and static movement stay separate observations; the
receipt claims no lifecycle, acceptance, or closure state and no repair
correctness.

Lower-level `start`, `brief`, `packet`, `verify`, `receipt`, `status`, and
`review-summary` commands remain available for explicit control and debugging.
The repair command does not generate or apply tests, execute mutation testing,
or declare the repository safe to merge.
"#;
