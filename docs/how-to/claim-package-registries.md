# Claim the package identities and prepare publishing

This is the maintainer setup guide for the
[PyPI/npm distribution plan](../PYPI_NPM_DISTRIBUTION.md).
Track non-secret progress in
[ripr #1780](https://github.com/EffortlessMetrics/ripr/issues/1780).
No account, package or environment is created by this document.

## What to claim

| Service | Intended identity | When it is secured |
| --- | --- | --- |
| npm | Organization `effortlessmetrics` | When you control the organization and its scope |
| PyPI | Project `ripr-rs` | When the registry accepts the first genuine publication under your ownership |
| npm | `@effortlessmetrics/ripr` and native packages below | Each exists after its first genuine publication inside your scope |
| TestPyPI | Optional separate project/publisher | Rehearsal only; never reserves production PyPI |

The product and executable remain **ripr**. No repository/editor rename is
needed. The unrelated PyPI project already uses `ripr`; never advertise
`pip install ripr` or `uvx ripr` as installation of this product.

An absent public package page is not assurance that a name is accepted. Confirm
ownership and conflicts in the authenticated service UI; do not automatically
pick a different scope or spelling when a check fails.

## 1. Secure npm organization ownership now

Sign in to your individual npm account. Confirm you already own the organization,
or use profile -> **Add an Organization** and choose `effortlessmetrics` with the
public-packages plan. npm organization names define their package scopes;
GitHub organization ownership does not create the npm organization. See
[organization creation](https://docs.npmjs.com/creating-an-organization/) and
[scope ownership](https://docs.npmjs.com/about-scopes/).

Enable strong 2FA, keep recovery material outside repositories and appoint an
appropriate backup owner. Record the public organization URL and ownership
confirmation in #1780, not passwords, tokens, recovery codes or authenticated
page captures.

The intended package inventory is:

```text
@effortlessmetrics/ripr
@effortlessmetrics/ripr-linux-x64-gnu
@effortlessmetrics/ripr-linux-arm64-gnu
@effortlessmetrics/ripr-darwin-x64
@effortlessmetrics/ripr-darwin-arm64
@effortlessmetrics/ripr-win32-x64-msvc
```

You do not need to publish six empty packages to protect these scoped names.
Control the organization, then publish genuine tested packages when ready.
Neither unscoped npm `ripr` nor an npm package called `ripr-rs` is required by
this design. Future target packages stay within the same controlled scope.

### Do not use Teams -> Add Packages to create them

The organization Teams screen manages access to **existing** packages. It is not
a package-creation or name-reservation surface. Seeing `Packages 0` and an empty
package selector before first publication is expected; entering `ripr` there
does not create `@effortlessmetrics/ripr`.

npm automatically creates a `developers` team for an organization and, by
default, gives it read/write access to newly created packages in the organization
scope. Review that policy and membership, but do not treat the team form as the
bootstrap path. The package family is created only by the genuine publication
transaction in [ripr #1784](https://github.com/EffortlessMetrics/ripr/issues/1784).

The organization page alone does not prove that the signed-in account is an
owner. Confirm your role under organization membership/settings and retain only
a non-secret owner/team confirmation in #1780.

## 2. Prepare PyPI; distinguish setup from reservation

Sign in/create your PyPI account, verify email, configure 2FA and retain private
recovery material. Under account **Publishing**, prepare a pending GitHub
publisher for the source workflow:

| Field | Value |
| --- | --- |
| PyPI project | `ripr-rs` |
| GitHub owner | `EffortlessMetrics` |
| Repository | `ripr` |
| Workflow filename | `publish-pypi.yml` |
| GitHub environment | `pypi` |

Source PR
[`ripr#1783`](https://github.com/EffortlessMetrics/ripr/pull/1783) establishes
that exact filename/environment as a deliberately non-publishing contract. It
must be reviewed and merged before the tuple is treated as present on the source
default branch.

A
[pending publisher](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/)
creates the project only on its first accepted publication. **It does not
reserve the name**, and another registrant can take the name in the meantime.

The [first functional prerelease task](https://github.com/EffortlessMetrics/ripr/issues/1782)
is deliberately separate from full npm/matrix completion. Qualify an honest
platform subset and a useful native CLI, obtain exact source/version/artifact
publication approval, then publish and install it back from production PyPI.
Do not upload a dummy package just to hold the name;
[PEP 541](https://peps.python.org/pep-0541/) treats name-squatting projects as
invalid. A limited real prerelease is different from a placeholder.

Python [name normalization](https://packaging.python.org/en/latest/specifications/name-normalization/)
treats `ripr-rs`, `ripr_rs` and `ripr.rs` as the same normalized project name.
They are not three independent reservations. Optional
[TestPyPI](https://packaging.python.org/en/latest/guides/using-testpypi/)
setup is separate and does not secure production ownership.

## 3. Create and protect the source GitHub environments

The intended source-only workflow identities are:

| Registry | GitHub owner/repository | Workflow filename | Environment | Current source PR |
| --- | --- | --- | --- | --- |
| PyPI | `EffortlessMetrics/ripr` | `publish-pypi.yml` | `pypi` | [#1783](https://github.com/EffortlessMetrics/ripr/pull/1783) |
| npm | `EffortlessMetrics/ripr` | `publish-npm.yml` | `npm` | [#1785](https://github.com/EffortlessMetrics/ripr/pull/1785) |

Both current workflows are intentionally incapable of publication. They use
manual dispatch and `contents: read` only, with no package retrieval/build,
`id-token: write`, staging or upload step. They establish exact source-owned
workflow paths without extending release authority.

In `EffortlessMetrics/ripr`, use **Settings -> Environments -> New environment**
to create `pypi` and `npm`. For each environment:

1. add a required reviewer and enable prevention of self-review where the
   available plan/repository settings support it;
2. restrict deployment branches/tags to the reviewed release refs used by the
   release transaction rather than allowing every branch;
3. add no registry token or long-lived publication secret;
4. save a non-secret screenshot or text receipt only if it contains no account,
   session, token or recovery material.

Create the environments explicitly before any workflow relies on them. A
workflow reference can otherwise create an unprotected environment implicitly.
Environment existence is not publication authorization, and environment approval
cannot replace exact candidate/artifact authorization.

Use filename only in registry forms, not `.github/workflows/...`. Give OIDC
permission only to the eventual publishing/staging job. Build/qualification jobs
should not receive registry-write authority. Never bind either publisher to
`ripr-swarm`.

[PyPI publisher use](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
and [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/) describe
the provider-specific setup. Saving a form is not proof that publishing works.
After verification, consider disabling unnecessary token-based publication;
revoke only credentials created for this bootstrap, not unrelated existing ones.

## 4. Bootstrap npm, then configure staged publishing

The first public npm delivery is tracked in
[ripr #1784](https://github.com/EffortlessMetrics/ripr/issues/1784). Its default
publication order is:

```text
qualified selected native package(s)
-> public registry readback of every exact payload version
-> @effortlessmetrics/ripr launcher
-> independent clean public installs on every advertised target
```

The launcher must never become public while one of its declared exact optional
dependencies is missing or conflicting.

The proposed normal **subsequent-release** path is:

```text
qualified tarball -> source OIDC staging -> human inspection and 2FA approval
                  -> public hash readback -> clean installed-use check
```

[npm staged publishing](https://docs.npmjs.com/staged-publishing/) cannot create
a brand-new package. First publish each inspected real native tarball, then the
launcher, through a separately authorized maintainer bootstrap with explicit
public access and a prerelease tag such as `next`. Keep that tag off stable
`latest`. Do not claim OIDC provenance for a manual bootstrap without evidence.

After each package exists, open that package's **Settings -> Trusted Publisher**
and add a GitHub Actions publisher with:

| Field | Value |
| --- | --- |
| GitHub organization/user | `EffortlessMetrics` |
| Repository | `ripr` |
| Workflow filename | `publish-npm.yml` |
| Environment | `npm` |
| Allowed actions | stage only; do not enable direct publish without a separate decision |

Configure the tuple separately for the launcher and every published native
payload package. Review/approve and verify native packages before approving the
launcher that references them. A staged package is not yet public.

As checked on 2026-09-28, staging requires npm >=11.15.0 and Node >=22.14.0;
GitHub OIDC publishing uses GitHub-hosted runners. Pin reviewed compatible tools
in the implementation. Those publisher minimums do not establish consumer
runtime minimums. OIDC does not authorize general npm administration or human
stage approval; `npm whoami` is not an OIDC readiness check.

Only after the staged path is proved should package publishing access be tightened
to require 2FA and disallow traditional tokens. Do not strand the package family
by removing the only proven publication route first.

The [publishing issue](https://github.com/EffortlessMetrics/ripr/issues/1781)
owns exact-artifact authorization, reusable staging machinery, partial
publication and fix-forward. Do not retry a conflicting immutable package with
different bytes or publish a launcher before its payloads exist.

## Record progress without secrets

Keep account readiness, scope ownership, pending/configured publishers, project
creation, publication and independent verification separate. Record public URLs,
owner confirmation, exact non-secret publisher tuples, selected artifact hashes
and verification receipts in the source issues. Never attach tokens, recovery
codes, credentials, full environment dumps or authenticated HTML captures.

The immediate human actions are npm account/organization-role confirmation,
strong 2FA, PyPI pending-publisher completion and protected GitHub environment
creation. Package-level npm trusted publishers come **after** #1784 creates the
real packages. No broader release, unrelated setting change or publication is
authorized merely by completing this checklist.
