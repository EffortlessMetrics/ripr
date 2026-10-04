# Werkzeug multiple Cookie headers: a real upstream regression

Spec: RIPR-SPEC-0028

## Given

[Werkzeug PR #2065](https://github.com/pallets/werkzeug/pull/2065) includes the
cookie fix commit `57acc4236044803956686c3f0f6ae0cacd2986b0`, whose direct parent
is `2f0d8a7725ef3ac606d022fb16333ed482bebeaf`. The PR also contains an earlier
content-length fix. This case selects **only the later cookie commit**;
`upstream/fix.patch` is the complete original commit patch and
`production.patch` is its unchanged production-file diff section.

The real `werkzeug.sansio.request.Request.cookies` cached property previously
passed only the first Cookie header to `parse_cookie`. Its fixed implementation
joins `Headers.getlist("Cookie")` with semicolons. The original upstream test
sends `Cookie: a=b`, unrelated `Content-Type: text`, then `Cookie: a=c`.

The independent oracle is in `oracle.json`. Reading `req.cookies.get("a")`
as `"b"` is correct on both implementations and therefore ineffective against
this bug. `req.cookies.getlist("a") == ["b", "c"]` distinguishes them. These
expectations were established from upstream semantics before RIPR was run.

## Retained upstream source

`input/` contains 29 complete, byte-identical upstream Python modules (534,943
bytes), the complete upstream test file, original `tests/conftest.py`, original
`setup.cfg`, and exact BSD-3-Clause `LICENSE.rst`. The modules are the observed
Werkzeug import closure of the focused original test file on the recorded
runtime. `upstream/retained-files.json` records every retained upstream SHA-256,
Git blob ID and size. No imports, production functions, tests, decorators, or
conftest were rewritten to simplify the case. Unused modules, documentation,
assets and unrelated tests are not retained. This is not a complete Werkzeug
installation and is not intended for unrelated APIs or full-suite execution.

Before reduction, the full original source archive passed the 5-subject
`tests/sansio/test_request.py` run. The same seven-row native replay ran against
all original `src/` modules and against this retained slice. Their 15 inner
subjects have identical expected outcomes, source/test digests and failure
locations. Original archive URLs/digests, exact commits/tree and retained-file
identities are in `manifest.json`. The original upstream license is also copied
to `upstream/LICENSE.rst`; all copyright and license text is preserved.

## Install-report projection

`evidence/dependency-install-report.projected.json` is a deterministic,
post-capture projection of the pip-generated install report, not a raw receipt
or a new installation. It omits only the seven optional
`install[*].metadata.description` long-form registry descriptions. Every other
parsed field remains identical, including package names/versions, source URLs,
archive hashes, dependency constraints, request flags and runtime environment.
Public package descriptions can contain developer-path examples unrelated to
this case; they are not needed to establish dependency identity.

`evidence/setup.json` records the exact transformation, projected artifact
identity, and each omitted JSON pointer, decoded-string UTF-8 size and SHA-256.
The original pip report remains outside Git in task evidence: 52,953 bytes,
SHA-256 `4b130755d073982c6d41bae627e6269ef8dd269ed239b9f18ebe5d866f87de84`.
The observed pip command still names the raw file it actually wrote. The
projection does not claim that pip directly produced the renamed artifact.
The updated case-manifest digest reflects evidence packaging only; upstream
source, native observations, replay driver and dependency lock are unchanged.

## When

Use CPython 3.12 on Linux x86_64 and explicit task-local dependencies. The lock
is for the **modern replay runtime**, not the 2021 runtime. Historical upstream
pins remain unmodified under `upstream/requirements/`. This focused replay uses
pytest 9.1.1 and pytest-xprocess 1.0.2; it does not install optional dependencies
for unrelated upstream tests.

From the RIPR repository root:

```bash
CASE="$PWD/fixtures/python-real-repo-evals/werkzeug-multiple-cookie"
WORK=$(mktemp -d)
python3.12 -m venv "$WORK/venv"
"$WORK/venv/bin/python" -m pip install --no-cache-dir --require-hashes -r "$CASE/requirements.lock"
"$WORK/venv/bin/python" -m pytest "$CASE/test_replay.py" -q \
  --basetemp "$WORK/replay" --junitxml "$WORK/replay.xml"
```

The seven pytest cases prepare private source copies. Four run the exact child
selector `tests/sansio/test_request.py::test_cookies` through that interpreter;
one runs its complete module, and two run the additional positive controls.
They declare `PYTHONPATH=<owned subject>/src` as source-layout setup and check
that the imported package is exactly that copy. Original conftest loads,
including its `xprocess.ProcessStarter` import; no missing-dependency failure,
zero-selection run, skipped test or import error can count as the behavioral
RED. Every child writes its actual argv, cwd, import identity, source/test
hashes, exit, stdout/stderr, JUnit and subject counts to `observation.json`.

To validate a newly downloaded exact upstream fixed archive before reduction,
set `WERKZEUG_UPSTREAM_SOURCE` to its extracted root for the same command.
Dependency/source setup occurs before either native measurement or static
analysis. RIPR never installs or executes these dependencies or tests.

## Then

The retained and original native replays both establish:

| Source / test | Executed subjects | Result |
| --- | ---: | --- |
| Fixed / complete upstream cookie test | 1 | pass |
| Exact broken parent / complete upstream cookie test | 1 | intended getlist assertion failure: `['b'] != ['b', 'c']` |
| Broken / only getlist assertion removed | 1 | pass; original first-value assertion remains |
| Fixed / only getlist assertion removed | 1 | pass |
| Fixed / complete upstream test file | 5 | pass, including four content-length neighbors |
| Fixed / single-cookie and unrelated-header controls | 3 | pass |
| Broken / same positive controls | 3 | pass |

The seven outer checks pass by demanding these exact outcomes. The intended
inner failure remains recorded as exit 1, not relabeled as a passing test.

## Observed historical RIPR output

After the native oracle, the frozen debug-installed CLI from RIPR source
`3e900f58d3a8d5bec93d2f248c177600b3eafc9a` was invoked by absolute path, with
SHA-256 `36ddf81974367638f22ce3c1369e769c00f8da0b0e18346a70236dfa53e5bbeb`
checked before and after. This is **historical advisory evidence**, not the
current source candidate or a published package. `evidence/analysis-observations.json`
records the exact commands and raw files. Each used a private copy of `input/`
and the same production patch, once with the complete upstream test and once
with only its getlist assertion removed.

Both runs exit 0 and analyze one changed Python file, 2 probes and 2 findings.
Every finding is `static_unknown` / `decorator_indirection`, because the owner
has `@cached_property`. RIPR finds `test_cookies` only by uncertain `same_stem`
proximity; oracle strength and alignment remain unknown. It emits no repair
card or packet and does not distinguish the effective and ineffective tests.
The related node is executable and matches the upstream target, but that is
not proof of resolved descriptor reach or a safe actionable repair target.
No receipt or static improvement is claimed. Full raw output is retained.

This snapshot is **not a golden requiring future RIPR versions to remain
incapable**. Future runs must preserve the native answer key and record a new
analyzer identity. The existing `check-fixture-contracts` route checks retained
bytes and historical native evidence; it does not execute Python, re-run this
historical binary, or qualify current analyzer behavior. The separate pytest
command replays the behavioral control.

## Future capability acceptance

The product goal is to distinguish the effective list assertion from the
ineffective first-value assertion through real descriptor reach and sink
alignment. Do not fix the limitation with a bare `cached_property` name allowlist.
A bounded successor must establish the descriptor/import binding, the receiver
and `Request.cookies` reach, and propagation to the MultiDict list observer.
It must keep unknown, shadowed, rebound or replaced decorators conservative,
and reject wrong receivers, wrong keys and first-value-only observations.
Only then can a fresh exact-binary run claim a correct actionable test target.

Current semantic owners are Python `static_limits.rs`, `related_tests.rs`,
`source_facts.rs`, and `sink_alignment.rs`. Existing transitive-reach issue
[#4568](https://github.com/EffortlessMetrics/ripr-swarm/issues/4568) is adjacent
(function-to-helper edges); closed
[#4765](https://github.com/EffortlessMetrics/ripr-swarm/issues/4765) owns the
method-reach limitation precedent. Neither establishes descriptor/getlist
support. This case proposes a narrow descriptor-reach/sink follow-up under the
Python integration scope [#3552](https://github.com/EffortlessMetrics/ripr-swarm/issues/3552),
without changing analyzer behavior in this patch.

## Must Not

- Do not derive expected runtime behavior from RIPR classifications
- Do not count this curated upstream regression as a blind user, external
  repair opportunity, installed journey, support-tier, package or release pass
- Do not increment #4703's real-opportunity denominator
- Do not count setup/import failures, skips or zero subjects as behavioral RED
- Do not mistake a same-stem node match for resolved reach or actionable repair
- Do not treat static unknown as the permanent desired outcome
- Do not claim the entire Werkzeug suite or historical dependency matrix ran
