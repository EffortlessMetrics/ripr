# Clippy Lint Policy

This repo enforces a stricter Clippy / Rust lint profile than the popular
defaults. The bar applies to **production code and tests**; we do not have
test carve-outs for `unwrap`, `expect`, `panic!`, `dbg!`, or unchecked
indexing.

The lint set is recorded in two places:

- `Cargo.toml` (`[workspace.lints.rust]` and `[workspace.lints.clippy]`) —
  the actual enforcement, inherited by every workspace member through
  `[lints] workspace = true`.
- `policy/clippy-lints.toml` — the machine-readable ledger of every lint we
  enforce today (`status = "active"`), every lint we have intentionally
  deferred until a follow-up migration (`status = "deferred"`), and every
  lint we plan to enable when a future toolchain becomes the workspace MSRV
  (`status = "planned"`, `activate_when_msrv = "..."`).

`cargo xtask check-lint-policy` cross-checks the two so the bar cannot drift.

## What the policy is for

Lints earn a slot in this profile when they map to a **failure mode**, not a
matter of taste:

| Bucket               | What it prevents                                                           |
| -------------------- | -------------------------------------------------------------------------- |
| Panic rails          | `unwrap`, `expect`, `panic!`, indexing, string slicing, unreachable paths  |
| Silent-failure rails | Dropped futures, ignored `Result`, erased error sources                    |
| Async / concurrency  | Locks held across `.await`, RefCell guards across `.await`, oversized fut. |
| Unsafe / memory      | `mem::forget`, undocumented unsafe, multi-op unsafe blocks                 |
| Numeric              | Lossy casts, float equality, NaN-to-int, invalid upcast comparisons        |
| Filesystem / IO      | Suspicious open options, absolute-path joins that overwrite the path      |
| API / trait          | Infallible `TryFrom`, fallible `From`, non-iterator `iter()`               |
| Governance           | `#[allow]` -> `#[expect(..., reason = "...")]`, no blanket category opts   |

We also include a small set of "good-taste" lints (e.g. `format_in_format_args`,
`to_string_in_format_args`) where the lint points at one obvious better
shape, the false-positive rate is low, and the result is easier to review.
We deliberately do **not** enable `pedantic` or `restriction` wholesale.

## Tests are not a panic playground

Evan Schwartz's lint config explicitly allows `unwrap`, `panic`, `expect`,
`dbg`, and indexing in tests. That is a defensible default for many teams,
but it is not our bar.

In ripr, tests should use **fallible setup**:

```rust
#[test]
fn parses_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = std::fs::read_to_string("tests/fixtures/input.rs")?;
    let parsed = parse(&fixture)?;
    assert_eq!(parsed.items.len(), 3);
    Ok(())
}
```

instead of:

```rust
#[test]
fn parses_fixture() {
    let fixture = std::fs::read_to_string("tests/fixtures/input.rs").unwrap();
    let parsed = parse(&fixture).unwrap();
    assert_eq!(parsed.items.len(), 3);
}
```

`assert!` / `assert_eq!` remain the test harness's failure mechanism. The
line we draw is: **assertions may panic; setup, parsing, IO, indexing, and
fixture plumbing may not.**

This is enforced today through the existing `cargo xtask check-no-panic-family`
gate, backed by `.ripr/no-panic-allowlist.toml`. The Clippy lints
`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, and
`clippy::indexing_slicing` are recorded in the ledger as **deferred** so
they can be flipped to `deny` once the migration sweep lands. We do not
add a `clippy.toml` file with `allow-*-in-tests = true` toggles.

## Suppressions

We require `#[expect(<lint>, reason = "<why>")]` over `#[allow(<lint>)]` for
new suppressions. The ledger records this as the active suppression style;
the `clippy::allow_attributes_without_reason` and `clippy::allow_attributes`
lints are deferred only until the legacy `#[allow(deprecated)]` sites in the
LSP backend are migrated. Existing suppressions are tracked in
`.ripr/allow-attributes.txt` and policed by `cargo xtask check-allow-attributes`.

## Promoting deferred lints

A deferred lint moves to active in its own focused PR:

1. Migrate the affected sites (e.g. introduce a `test-support` helper with
   `ensure!` / `ensure_eq!` for fallible test assertions when promoting
   `panic` and friends).
2. Flip `status = "active"` for the lint in `policy/clippy-lints.toml`.
3. Add the lint to `[workspace.lints.*]` in `Cargo.toml` at the recorded
   level.
4. Run `cargo xtask check-lint-policy && cargo clippy --workspace --all-targets -- -D warnings`.

`cargo xtask check-lint-policy` will fail if any active lint is missing
from `Cargo.toml`, if any deferred lint is recorded as `expect`, if any
planned lint shows up in `Cargo.toml` before its `activate_when_msrv` is
reached, or if `policy.msrv` disagrees with `workspace.package.rust-version`.

## Promoting planned lints (toolchain bumps)

Planned lints are gated on the workspace MSRV. When the bar is bumped to
1.94 or 1.95, the upgrade is mechanical:

```text
1. Update rust-toolchain.toml + workspace.package.rust-version.
2. Update policy.msrv in policy/clippy-lints.toml.
3. Promote planned lints whose activate_when_msrv now passes to active.
4. Add them to [workspace.lints.*].
5. cargo xtask check-lint-policy
6. cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The Rust 1.95 entry that matters structurally is `clippy::disallowed_fields`,
which lets us encode architectural seams (constructors / accessors / ports)
through `clippy.toml` instead of code review.

## Why a ledger and not just `Cargo.toml`?

Plain `Cargo.toml` only records what is enforced **today**. Rust toolchain
upgrades arrive on their own cadence, and several of the lints we want
(notably the Rust 1.94 `same_length_and_capacity` and the Rust 1.95
`disallowed_fields`) need to wait for the MSRV bump. Without a ledger,
those decisions live only in chat history or a PR description that nobody
reads later. The ledger keeps the bar discoverable at the repo root.
