# Trusted, Bounded Actionability

*Notes from the TypeScript actionable-flip campaign — 2026-06-13*

There's a temptation, when you build a static analyzer, to chase a number. More
coverage. More findings. More languages lit up green. The TypeScript
preview->actionable wave taught us — again, and from a new angle — that the
number is the wrong thing to chase. What `ripr` actually sells is a smaller,
stranger, more valuable thing: it knows exactly what it can and cannot act on,
and it never lies about where that line is.

## Trusted bounded actionability beats coverage

`ripr`'s job is to look at a diff and say whether the existing tests contain a
discriminator that would notice if the changed behavior were wrong. The honest
answer is frequently "I can't tell." That answer is not a failure. The failure
mode that actually destroys trust is the opposite: telling an agent
`repair_packet_ready: true` and handing it a packet that is wrong.

That asymmetry is the whole product. A missed finding is a gap — annoying,
recoverable, and visible later. A false `repair_packet_ready: true` is a *lie the
consumer cannot detect from the output alone*, because the packet looks complete
by construction. So the cardinal sin isn't under-reporting. It's a confident
packet that doesn't hold. The campaign encoded this as a fail-closed flip:
`repair_packet_ready` is computed through one shared Rust validator,
`validate_agent_gap_record_packet`, and the projection threads through an
`Option<GapRecord>` whose `None` collapses to `false`. There is no parallel
TypeScript validator that could disagree. The flip is true only when the single
authority says every required field — repair route, verification commands,
allowed edit surface, receipt command — is present. Bounded, and trusted
*because* it's bounded.

## Usability exposes dishonesty

The most interesting bug of the wave wasn't a crash. It was a contradiction that
had been sitting in the data the whole time, invisible, until we made it usable.

The projection slice rendered the gap record into the human-readable surface. The
moment the output actually relabeled "why not actionable" into "why actionable,"
the projection surfaced a finding that simultaneously said *actionable* and
*evidence needed*. Those two can't both be true. The contradiction had always
existed in the latent record — we just couldn't see it until a human-facing
projection forced the two fields next to each other. The fix (in
`preview_actionability.rs`) made the flip atomic: when `repair_packet_ready`
holds, it replaces the "why not" reasons with the resolved repair action, flips
the category to `complete_repair_packet`, and empties `evidence_needed` in the
same move. The lesson generalizes: easy-to-use and honest are not two goals.
Usability is the pressure that makes latent dishonesty legible. If your output is
too dry to read, your contradictions get to hide.

## Preview, but actionable

The wave shipped a milestone without shipping a release decision. That sounds
like a contradiction too, until you treat the support tier as a *dial* rather
than a *switch*.

The old mental model was binary: a feature is shipped or it isn't, gating or it
isn't. The TypeScript repair packet is "preview" — delegatable to agents that opt
in, advisory in LSP hover and code actions, never holding gate authority. That
tier let real consumers exercise a real, validated artifact while the question
"do we cut a release for this?" stayed untouched. The dial dissolved the
shipped/unshipped binary: the work could be trusted at a bounded level of
authority without anyone making the irreversible call. Trust became a quantity
you tune, not a door you open.

## We built RIPR by being RIPR

The most uncanny part: the development process turned out to be isomorphic to the
product. The same bug — *a proxy mistaken for the artifact* — recurred at every
level of the work.

`ripr` exists because "the test mentions the changed thing" is not "the test
would observe the changed thing being wrong." That's a proxy read as the real
signal. And while building it, we kept making the same shape of mistake one rung
up:

- An **error string read as data** instead of as a failure.
- **"All gates pass"** taken as proof, when the gate had simply not looked.
- The **output's status divorced from its content** — `actionable` next to
  `evidence needed`.
- A **test that mentions** the behavior standing in for a test that **observes**
  it.

Every one of those is the discriminator problem wearing a different hat. The
discipline that fixed them is the same discipline `ripr` enforces on its users:
verify the artifact, fail closed, real producers only. We didn't just build a
tool that distrusts proxies. We had to *become* a team that distrusts proxies to
finish it.

## Two campaigns converged

While this wave ran, a separate campaign (#1173) was tightening when `ripr` is
allowed to say `exposed` — fixing an over-credit where reach plus a
strong-but-orthogonal oracle was wrongly read as discrimination. It arrived,
independently, at the same sentence we'd written for the actionable flip:
*discrimination, not coverage.* One campaign was about not over-claiming a repair
packet; the other about not over-claiming exposure. Neither borrowed the slogan
from the other; both derived it from first contact with the failure. When two
people dig from opposite ends and meet in the middle, that's evidence the tunnel
was always there. The principle is real, not a slogan we talked ourselves into.

## A multi-agent system on single-writer primitives

One last, humbling note for the record. We ran this as a swarm — many agents,
concurrent PRs — and the concurrency scaled right past our coordination
primitives. Two campaigns reached for `RIPR-SPEC-0086` at the same time and
collided; the fix was a manual renumber to `0087`. Merges, governed by the
up-to-date-branch rule, serialized into something close to livelock. The work
parallelized beautifully; the *bookkeeping* — spec numbers, the merge queue, the
single `main` everyone wanted to write — did not. We built a parallel system on
top of single-writer artifacts and then acted surprised when the single writers
became the bottleneck. The campaign succeeded anyway, but the seam is now
visible, and naming it is the first half of fixing it.

---

The through-line is one idea seen from five sides: **a system earns trust by
knowing its own boundary and refusing to lie about it.** That's the product. It
turned out to also be the only way to build the product.
