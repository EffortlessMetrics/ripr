# Fixture: typescript_probe_facts

Spec: RIPR-SPEC-0027

## Given

A mixed TypeScript / JavaScript preview workspace opts into the language adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

The changed production lines cover the supported preview probe families:

- predicate boundary;
- return value;
- thrown error path;
- field/object construction;
- call side effect;
- mock interaction;
- output/log text through the existing side-effect family.

The workspace also includes ambiguous const-expression and computed-member-call
changes. Related tests call each owner but use smoke-only assertions so the
adapter can describe missing discriminators without claiming strong proof.

## When

```bash
cargo xtask fixtures typescript_probe_facts
```

or:

```bash
ripr check \
  --root fixtures/typescript_probe_facts/input \
  --diff fixtures/typescript_probe_facts/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter emits syntax-backed probe facts with preview
metadata for `.ts`, `.tsx`, `.js`, and `.jsx` sources. Specific weak findings
include expected sinks, flow sinks where the syntax shape supports one, and a
candidate missing discriminator. Ambiguous const expressions and computed
member calls stay advisory: they do not invent missing-discriminator guidance.

## Must Not

- Invoke `tsc`, `tsserver`, package graph resolution, Jest/Vitest runtime, or
  provider/model calls.
- Generate tests or edit source files.
- Promote TypeScript/JavaScript out of opt-in preview.
- Give TypeScript/JavaScript preview findings default gate, badge, baseline,
  or RIPR Zero authority.
