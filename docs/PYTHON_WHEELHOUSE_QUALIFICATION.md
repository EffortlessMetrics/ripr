# Python wheelhouse qualification

`Python Wheelhouse Qualification` is an Actions-only rehearsal lane for
[#4631](https://github.com/EffortlessMetrics/ripr-swarm/issues/4631). It
installs candidate wheels from a local wheelhouse with pip and uv, then
evaluates a fail-closed aggregate. It is not a publisher and does not replace
[#4490](https://github.com/EffortlessMetrics/ripr-swarm/issues/4490),
[#4626](https://github.com/EffortlessMetrics/ripr-swarm/issues/4626), or the
generic cross-channel DTO in
[#4630](https://github.com/EffortlessMetrics/ripr-swarm/issues/4630).

## Invocation

Dispatch `.github/workflows/python-wheelhouse-qualification.yml` with:

- `candidate_sha`: the full 40-character immutable candidate commit SHA.

The job checks out that SHA with `contents: read` and `persist-credentials:
false`, stages `dist/wheels` or `packaging/python/dist` when present, installs
through isolated pip and uv consumers, and retains receipts as an Actions
artifact for 5 days. Staging must yield exactly one wheel; zero or mixed
filenames leave the required rows missing. The workflow hashes that one staged
path rather than the first glob match.

Network isolation is an independent `urllib.request.urlopen('https://pypi.org')`
probe after `PIP_NO_INDEX`/`UV_OFFLINE` are set. Source-checkout reachability
is whether `${GITHUB_WORKSPACE}/Cargo.toml` is readable from the consumer
working directory. This lane does not add a sandbox or filesystem unmount, so
those probes typically remain true on GitHub-hosted runners and fail closed.

## Gate

`cargo xtask qualify-python-wheelhouse --receipts <file-or-dir>` is the
authority. A required linux/x86_64 pip or uv row can pass only when it records
exact wheel and payload digests, a matching wheel filename across required
passed rows, a nonzero subject count, no Cargo/source checkout/ambient `ripr`
escape, a detected planted wrong-PATH binary, and network disabled after
staging. Failed, `not_run`, unsupported, missing, zero-subject, or mismatched
rows cannot satisfy the aggregate.

When this candidate has no staged wheels, or more than one, the required rows
stay missing and the aggregate cannot pass. That is the honest result until
#4490 produces a single wheelhouse artifact. Installed-use analysis remains
#4626; a wheel install without a nonzero discriminator still cannot pass.

## Claim boundary

A passing run would establish that the named candidate's selected linux/x86_64
wheel installed from a local wheelhouse through pip and uv with the isolation
controls above. It does not establish PyPI publication, a compatibility floor,
a support-tier promotion, or the five-target matrix. A green pip `--no-index`
install is not network isolation; a PATH that omits the checkout is not a
source-checkout barrier.
