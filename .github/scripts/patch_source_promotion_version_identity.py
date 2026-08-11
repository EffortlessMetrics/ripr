from pathlib import Path


def replace_exact(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new)


verifier = Path("xtask/src/reports/source_promotion_verify.rs")
text = verifier.read_text(encoding="utf-8")

# Compatibility anchor consumed by the already-reviewed one-use workflow.
text = text.replace("METADATA_SURFACES", "RELEASE_METADATA_SURFACES")

schema_old = "ripr.source_promotion_verification.v1"
schema_count = text.count(schema_old)
if schema_count < 2:
    raise SystemExit(f"expected success/failure v1 schema fields, found {schema_count}")
text = text.replace(schema_old, "ripr.source_promotion_verification.v2")

mutation_anchor = '''            let mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
'''
mutation_replacement = '''            let mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["reset", "--quiet", "--hard", &dependency_only])?;
            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
'''
text = replace_exact(
    text,
    mutation_anchor,
    mutation_replacement,
    "changelog adversary base",
)

verifier.write_text(text, encoding="utf-8")

spec = Path("docs/specs/RIPR-SPEC-0149-source-promotion-verifier.md")
spec_text = spec.read_text(encoding="utf-8")
spec_text = replace_exact(
    spec_text,
    "match preflight, J's tree equals the reviewed tree, governed metadata is byte\nidentical to the source parent, and optional merged source main reaches J.\n",
    "match preflight, J's tree equals the reviewed tree, the governed effective\nrelease versions match the source parent, `CHANGELOG.md` remains byte-identical,\nand optional merged source main reaches J.\n",
    "spec behavior contract",
)
spec_text = replace_exact(
    spec_text,
    "The verifier emits deterministic JSON and Markdown receipts containing exact\nidentities, ordered parents, tree and ancestry digests, checks, invalidation\n",
    "The verifier emits deterministic `ripr.source_promotion_verification.v2` JSON\nand Markdown receipts containing exact identities, ordered parents, tree and\nancestry digests, checks, invalidation\n",
    "spec receipt schema",
)
spec_text = replace_exact(
    spec_text,
    "- A valid two-parent J with matching reviewed tree, metadata, ranges, and\n  resolution inventory emits `verified` JSON and Markdown receipts.\n",
    "- A valid two-parent J with matching reviewed tree, release-version identity,\n  source-authoritative changelog bytes, ranges, and resolution inventory emits\n  `verified` JSON and Markdown receipts.\n",
    "spec acceptance wording",
)
spec_text = replace_exact(
    spec_text,
    "  tree-equivalent, preview-tree substitution, release-version or source-authoritative changelog drift, and appended\n  repair heads are rejected with deterministic failure reasons.\n",
    "  tree-equivalent, preview-tree substitution, release-version or\n  source-authoritative changelog drift, and appended repair heads are rejected\n  with deterministic failure reasons.\n",
    "spec line wrapping",
)
spec_text = replace_exact(
    spec_text,
    "canonical manifest and inventory, range/tree/parent adversaries, replacement\nrefs, metadata mutation, caller-state snapshots, structured rejection, and\n",
    "canonical manifest and inventory, range/tree/parent adversaries, replacement\nrefs, release-version and changelog mutation, caller-state snapshots, structured\nrejection, and\n",
    "spec test mapping",
)
spec.write_text(spec_text, encoding="utf-8")

fixture = Path("fixtures/source_promotion_verification/SPEC.md")
fixture_text = fixture.read_text(encoding="utf-8")
fixture_text = replace_exact(
    fixture_text,
    "governed release-version or source-authoritative changelog changes",
    "governed release-version or source-authoritative changelog changes",
    "fixture governed mutation wording",
)
fixture.write_text(fixture_text, encoding="utf-8")

workflow = Path(".github/workflows/source-promotion-version-identity-repair.yml")
if workflow.exists():
    workflow.unlink()

script = Path(".github/scripts/patch_source_promotion_version_identity.py")
if script.exists():
    script.unlink()
