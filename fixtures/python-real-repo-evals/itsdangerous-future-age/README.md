# ItsDangerous future-age regression and test activation

Spec: RIPR-SPEC-0028. Product admission repair: [#5389](https://github.com/EffortlessMetrics/ripr-swarm/issues/5389).

[Upstream issue #126](https://github.com/pallets/itsdangerous/issues/126)
describes old signatures appearing valid when timestamps are interpreted as
being in the future. The exact fix is
`c30678d19e37011890e2374cca04f7789e101793`, with direct parent
`1a9b8d1f9968eb248bda69f39d373a6330b692ba`. The changed age check rejects
negative age with `SignatureExpired` when `max_age` is supplied. This is the
independent semantic justification for the exception oracle; RIPR output does
not establish it.

`input/` preserves 17 complete, byte-identical upstream files: all nine package
modules, the original timed/signer/serializer test files and package initializer,
setup configuration, setup entry point, tox configuration, and the exact
BSD-3-Clause license/copyright. `retained-files.json` binds each file's size,
SHA-256, Git blob, and immutable upstream URL. The parent `timed.py` is retained
in `upstream/broken-timed.py`. Only that module changes between implementations.
This is a bounded test closure, not the complete upstream test suite.

The original tests use inherited pytest fixtures, an autouse frozen clock,
setup/yield teardown, exception assertions and parameterized neighboring tests.
The complete original timed test file executes 97 subjects on the fixed source
in the recorded environment. No upstream source or original test was simplified.

Run with a separate CPython 3.14 environment and the hash-locked dependencies:

```text
python -m pip install --require-hashes -r requirements.lock
python replay.py --work-dir <owned-native-output-directory>
```

The replay creates a fresh child directory on every invocation, copies only
validated input bytes and retains all raw observations. It never replaces an
existing checkout or runs RIPR. Each child has a 30-second timeout. The replay
needs under 1 MiB of source/run outputs, in addition to its Python environment.
The lock describes the modern Windows replay, including pytest's Windows
colorama dependency; it does not recreate the historical upstream environment.
Before execution the driver checks CPython 3.14 on Windows x64, all nine
installed distribution versions against the lock, and imported pytest/freezegun
versions. The receipt records those versions and module initialization digests.

| Control | Fixed | Exact broken parent | What it establishes |
| --- | --- | --- | --- |
| Original future-age test | Pass | Intended exception-assertion failure | Discriminates this actual upstream defect |
| Omit `max_age`, retain exact payload assertion | Pass | Pass | A valid exact oracle observes a different behavior and misses this defect |
| Add method or class `pytest.mark.skip` | One registered, zero bodies | One registered, zero bodies | Registration is not execution |
| Add non-strict `pytest.mark.xfail` | XPASS, exit 0 | Expected failure, exit 0 | A detected assertion failure need not fail verification |
| Original expiry and timestamp neighbors | Two pass | Two pass | Setup and neighboring behavior remain viable |
| Complete original timed test file | 97 pass | Not run | Fixed-source broader test closure only |

The weak/skip/xfail variants are explicitly constructed controls, not historical
upstream changes. `replay.py` records deterministic transformations and binds
the resulting production/test digests. `evidence/native.json` retains 13 native
rows, source/driver/lock identities and JUnit with nonempty subjects and zero
setup errors. Four skip rows execute no bodies; the expected-failure row runs
its body despite pytest using its skipped result bucket. The public JUnit text
is a labeled decoded-text projection that normalizes CRLF to LF and replaces
the absolute run-root text with `[RUN_ROOT]`. Raw
stdout/stderr/JUnit bytes remain in task-owned proof storage and their original
SHA-256 values are retained; no public raw-artifact availability is claimed.

The existing `check-fixture-contracts` route validates these retained identities
and the control matrix. It does not execute Python. Current RIPR execution is
`not_established`; this case is in `native_cases`, outside the existing static
repair/no-action metrics. It is neither an analyzer golden nor a desired permanent
limitation, and it does not count toward installed journeys, blind opportunities,
population rates, mutation adequacy, or support-tier promotion.

Rust uses the existing #5288 repository contract and #5318 verdict corpus;
TypeScript/JavaScript remains advisory preview; Perl remains exporter-dependent
preview with no released analyzer/exporter pair. Those distinct authorities must
be retained as the four-language corpus grows. This Python case does not imply
test-understanding parity across them.

The evaluation missed a distinction between a registered assertion and an
active test whose failure fails verification. The ordinary upstream future-age
test also shows that a valid exact payload oracle can miss an age defect. A
scorecard that credits assertion shape alone would reward both controls
incorrectly. The existing Python scorecards exclude `native_cases`; current
analyzer outcomes and score changes remain unmeasured here.

Acceptance stays on #5389: run the source-fact/classifier regressions on the
exact candidate, observe their intended failure on the pre-repair source, then
graduate the measured skipped/expected-failure and active controls through the
existing evidence-promotion honesty corpus. No golden is changed in advance of
those observations. Correctness requires preserving independent active oracle
credit as well as withholding disabled credit. Confidence requires a source-bound
analyzer run and native assertion location, rather than exit status alone.
User effort should measure whether the visible test pointer and activation stop
reason let an agent select an active test without a dead verification loop.
Resource cost records retained bytes and bounded replay separately; CPU time and
peak memory have not been measured, and fewer checks are not a quality gain.

Further investigations should extend the existing #5318/#5295 Rust corpus and
calibration owners, reuse the #5293 journey and #5400 editor verification work
for TypeScript/Python usability, and keep Perl activation/exporter proof with
#4061/#3216/#3227. The shared #5199 resource owner controls native Cargo
allocation. These links identify existing authorities; they do not claim their
remaining work is complete or create another acceptance tracker.
