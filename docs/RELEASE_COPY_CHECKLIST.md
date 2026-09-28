# Release Copy Checklist

Public release copy should describe the shipped product, not the publishing
process. Check the README, package page, and editor listing alongside release
notes before publishing. Keep recovery details in [CHANGELOG.md](../CHANGELOG.md)
and [Release](RELEASE.md), not the public release body.

Run this review before finalizing release notes, triggering
`publish-extension.yml`, or running `cargo publish`. Fix inaccurate copy or
stale artifacts before publishing.

## README and onboarding

- [ ] Lead with the product and a specific reason to use it. Show a concrete
      example and one complete first-use path before explaining the full model.
- [ ] Put installation requirements beside the installation that needs them.
      Do not interrupt the product introduction with prerequisites or diagnostics.
- [ ] State the assumptions needed by an example. Use ordinary defaults where
      they work; explain alternate bases and other controls in the relevant guide.
- [ ] Keep the prose direct: name the actor and action, explain necessary terms
      where they arise, and remove repetition and administrative noun phrases.
- [ ] Remove self-description such as "this README is a front door" and mandatory
      glossaries. Let the document's order and examples explain the product.
- [ ] Keep internal release, campaign, and agent-operation detail in contributor
      documentation. Link to the specific user task, not an internal index.
- [ ] Match installation, examples, output, and support claims to the same channel.
      Label development-only steps and link to versioned release instructions.
- [ ] Obtain every identifier and input in the shown workflow. In particular,
      do not pass a `check` probe ID to `agent repair` as a seam ID.
- [ ] Use captured output or a checked fixture. Label excerpts and illustrative
      examples; do not invent a cleaner CLI, timings, or successful outcomes.
- [ ] Review rendered pages and follow their links. Confirm one useful run and
      the handling of an empty or limited result with the advertised build.
- [ ] Keep root, crate, and editor introductions consistent in meaning, while
      tailoring their examples to the reader. Do not copy the whole README into
      every surface.

Concision means removing unnecessary reader effort, not hiding limitations,
removing useful context, or enforcing an arbitrary word count. These are review
questions, not new wording, heading, or badge-count gates.

## GitHub Release body

- [ ] The body describes the **shipped release**: what users get and what changed
      since the last public version.
- [ ] Publishing attempts, failures, and recovery commands stay in the changelog's
      `Release recovery` section and [Release recovery](RELEASE.md#recovery).
- [ ] The first paragraph describes the user benefit before internal vocabulary.
      Use [Terminology](TERMINOLOGY.md) for the public-to-internal mapping.
- [ ] Runtime mutation words such as `killed` and `survived` appear only with real
      runtime data. Static claims retain conservative classifications.
- [ ] Positioning remains static mutation-exposure analysis: draft-time guidance
      about weak testing evidence before execution-backed mutation confirmation.
      Do not claim that ripr replaces mutation testing or detects an unrelated
      class of signal.
- [ ] Preview-language copy separates what ships from what is supported. Check
      [Support tiers](status/SUPPORT_TIERS.md) for language and workflow scope;
      do not imply Rust parity or default gate eligibility.
- [ ] Counts name their basis. Static audit counts are not coverage, runtime
      mutation results, or test-adequacy claims.

## VS Marketplace and Open VSX

- [ ] `editors/vscode/package.json` uses a readable `displayName` consistent with
      the opener; the current target is `ripr: Static Mutation Exposure`.
- [ ] The package `description` explains the user benefit, not the internal model.
- [ ] The extension README starts with what the user can do. Use absolute links
      for documentation reached from store listings.
- [ ] Rebuild and inspect the VSIX after copy changes. The published artifact
      must contain the updated title, description, and README.
- [ ] Read the title and description in both store previews, not just the source.

## Crate metadata and README

- [ ] The Cargo `description` explains the tool to a package user.
- [ ] The package README provides an installed-user path; source-checkout
      examples and contributor validation are clearly separate.
- [ ] The root README explains the product before the RIPR model and links to
      [Terminology](TERMINOLOGY.md) when introducing precise vocabulary.
- [ ] Badge labels and destinations describe the actual metric and repository.
      Live badges may use live endpoints. Manual badges need a nearby hidden
      comment naming the source and last-checked date. Pending services must be
      labeled pending or omitted until available.
- [ ] Badge layout changes do not change generated counts or imply stronger
      coverage, mutation, correctness, or release claims.

## Install instructions

- [ ] Versions named in README, editor README, and [Quickstart](QUICKSTART.md)
      are available from the named channel at publish time.
- [ ] `cargo install ripr` resolves to the release being described, or the command
      pins the intended version explicitly. A source version is not publication.
- [ ] `cargo install --path crates/ripr` is a source-build option, not a hidden
      requirement for an ordinary package install.
- [ ] The extension's documented bundled, cached, and downloaded server choices
      match the assets actually available for that extension version.
- [ ] Missing matching assets or unsupported hosts are stated before the user
      relies on automatic installation.

## Public vocabulary

- [ ] Explain the task before introducing seams, discriminators, oracle strength,
      grip, canonical gaps, or internal report names.
- [ ] Use precise terms in specs, schemas, metrics, fixtures, and technical
      explanations. A term's presence is not a defect when it does useful work.
- [ ] State limitations beside the affected claim. Do not overclaim first and
      retract it later, or repeat the same disclaimer throughout the page.
- [ ] Keep generated summaries and CLI help aligned with the documented command
      roles. Detailed flags belong in help and task references.

## Release assets and dependent channels

- [ ] The GitHub Release contains the full documented asset set: VSIX, server
      manifest, per-target server archives, and checksums.
- [ ] Server archive checksums match the manifest.
- [ ] Trigger dependent marketplace and crates.io publication only after verifying
      the GitHub Release assets. Dependent workflows must consume the right set.
- [ ] `publish-extension.yml` uses the rebuilt VSIX, not an older artifact attached
      to the same release.

## When something slips

Before dependent publication, fix the copy or rebuild the artifact and retry.
Keep the retry narrative out of the public release body.

After publication:

1. Open a focused fix PR and follow [Release recovery](RELEASE.md#recovery).
2. Record the recovery in the changelog's `Release recovery` section.
3. Leave the tag in place. Update public release copy only to describe the
   shipped product, not the repair process.
4. Add installation smoke evidence when user-visible behavior changed; see
   [Installation verification](INSTALLATION_VERIFICATION.md).

## Automated guard

`cargo xtask check-product-copy` scans the public files registered in
`xtask/src/policy/product_copy.rs` for selected internal terms. The current
implementation skips vocabulary scanning for a whole file when it contains
`TERMINOLOGY.md` anywhere. The `product_copy` unit tests exercise that rule.

A passing result does not establish plain language, good section order, readable
rendering, or working examples. A terminology link helps navigation; it is not
proof that the text explains the user's task. Review those properties directly.
Use existing link, README-state, and documentation checks for their mechanical
contracts; do not freeze prose to compensate for missing editorial review.

## Origin

This checklist grew from the v0.5.0 release review: publishing-recovery prose
appeared before product information, an older VSIX carried stale storefront
copy, and a marketplace-count badge used an unreliable live route. Apply those
lessons without turning the product introduction into a release runbook.
