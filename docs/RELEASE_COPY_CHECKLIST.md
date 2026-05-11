# Release Copy Checklist

Public release copy is its own surface. A clean release-prep PR can still ship
with a release page that describes the *publishing process* instead of the
shipped product, a marketplace listing that uses internal vocabulary, or a
README badge that overstates current install state. This checklist captures the
v0.5.0 lessons so the next release does not drift back.

Run through it before:

- finalizing the GitHub Release notes,
- triggering `publish-extension.yml`,
- the manual `cargo publish`.

If a row fails, fix the copy or the artifact before publishing. Recovery
narrative goes in [CHANGELOG.md](../CHANGELOG.md) and [RELEASE.md](RELEASE.md),
not in the public release body.

## GitHub Release body

- [ ] The body describes the **shipped release** — what users get and what
      changed since the last public version.
- [ ] The body does **not** describe the publishing process, retry attempts,
      mid-publish failures, or recovery commands. Those belong in
      [CHANGELOG.md](../CHANGELOG.md) under the version's `Release recovery`
      section and in [RELEASE.md](RELEASE.md) under `Recovery`.
- [ ] First paragraph says what the user gets in plain language before any
      internal terms (seam, discriminator, oracle, grip, canonical gap,
      front panel, evidence record). The
      [Terminology bridge](TERMINOLOGY.md) is the mapping the rest of the
      product follows.
- [ ] Runtime mutation language (`killed`, `survived`, `adequate`) appears
      only if real runtime data is part of this release. Static evidence
      stays inside the conservative classifications listed in
      [Terminology bridge](TERMINOLOGY.md).

## VS Marketplace and Open VSX

- [ ] `editors/vscode/package.json` `displayName` is user-readable and
      consistent with the README opener. The current target is
      `ripr: Static Mutation Exposure`.
- [ ] `editors/vscode/package.json` `description` is plain-language
      marketplace copy and does not lead with internal vocabulary.
- [ ] `editors/vscode/README.md` opener says what the extension does for the
      user before any internal terms. Marketplace renders Markdown but does
      not resolve relative links, so the terminology bridge link is the
      absolute GitHub URL.
- [ ] VSIX **package metadata is checked before publish**: rebuild the VSIX
      after any copy change so the marketplace artifact carries the new
      title and description, not a stale build attached to an earlier tag.
- [ ] Marketplace title and description are reviewed by reading them in the
      VS Marketplace and Open VSX listing previews, not only in
      `package.json`.

## Crate metadata and README

- [ ] `crates/ripr/Cargo.toml` `description` reads as user-facing marketplace
      copy on crates.io, not internal model language.
- [ ] Root `README.md` first screen leads with plain language; the RIPR
      acronym and model description appear below the fold with a link to
      [Terminology bridge](TERMINOLOGY.md).
- [ ] README badges disclose their **freshness model**:
      - Live badges (e.g. Open VSX downloads, codecov, crates.io version)
        may use live Shields routes.
      - Manual badges (e.g. VS Marketplace install count) must carry a
        hidden HTML comment naming the source and last-checked date.
      - Pending badges (services that are not live yet) must say "pending"
        in the badge label or be hidden until the service exists.
      - Publisher-metric badges must say so in the comment near the badge
        so a future maintainer does not replace them with a live route.

## Install instructions

- [ ] Install commands in the README, `editors/vscode/README.md`, and
      [Quickstart](QUICKSTART.md) are true at publish time:
      - The named version is reachable from crates.io / VS Marketplace /
        Open VSX **now**, not after a delayed propagation step.
      - `cargo install ripr` resolves to the version this release is
        documenting (or the docs explicitly say `--version X.Y.Z`).
      - The fallback `cargo install --path crates/ripr` is documented as
        a fallback, not the required first-run path.
- [ ] The bundled-server / cached-server / GitHub Release download chain
      documented in [Editor extension](EDITOR_EXTENSION.md) matches the
      assets actually attached to this release.

## Public vocabulary

- [ ] Public release copy says what users get before any internal terms
      appear. The user-facing column in the
      [Terminology bridge](TERMINOLOGY.md) is the canonical phrasing for
      first-hour surfaces.
- [ ] Internal vocabulary (seams, discriminators, oracle strength, grip,
      canonical gap, evidence record, front panel, assistant-loop health)
      stays inside specs, `docs/OUTPUT_SCHEMA.md`, metrics, fixtures,
      `docs/IMPLEMENTATION_CAMPAIGNS.md`, and `CHANGELOG.md` entries that
      describe internal contracts.
- [ ] Section headings in the GitHub Release body, generated CI summary,
      and CLI help follow the reviewer-friendly wording landed in
      `#727` / `#729` / `#730`.

## Release assets and dependent channels

- [ ] GitHub Release contains the full asset surface documented in
      [RELEASE.md](RELEASE.md): VSIX, server manifest, per-target server
      archives, and checksums.
- [ ] Server archive checksums match the entries in the server manifest.
- [ ] Dependent publish channels (marketplaces, crates.io) are only
      triggered after the GitHub Release asset set is verified. The
      publish workflows download assets from the Release; if the Release
      is incomplete, dependent publishes will inherit the gap.
- [ ] The `publish-extension.yml` run is gated on the post-fix VSIX, not on
      the pre-fix VSIX that may still be attached to the original tag.

## When something slips

If a slip is caught before the dependent publish, fix the copy or rebuild
the artifact and retry. Do **not** rewrite the GitHub Release body to
describe the retry; that is publishing process narrative.

If a slip is caught after the dependent publish:

1. Open a focused fix PR on `main`.
2. Land the fix, follow the fix-forward recovery in
   [RELEASE.md → Recovery](RELEASE.md#recovery).
3. Document the recovery in `CHANGELOG.md` under the version's
   `Release recovery` section.
4. Leave the GitHub Release tag in place. Update the body **only** to
   reflect the now-shipped product, not the recovery story.
5. Add an `INSTALLATION_VERIFICATION.md`-style smoke artifact when the
   user-visible behavior changed.

## Origin

This checklist captures the v0.5.0 release lessons:

- The original release body described the publishing-recovery sequence
  before describing the shipped product.
- A pre-fix VSIX was on track to be published to VS Marketplace with the
  stale `Rust Test-Oracle Gaps` title until the storefront audit in
  `#727` and the rebuild was scheduled.
- README install count was first wired to a live VS Marketplace Shields
  route that the marketplace does not reliably serve.

Future releases run this checklist before each public surface is touched.
