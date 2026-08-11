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

setup_block = '''            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
                .map_err(|error| error.to_string())?;
            test_git(&root, &["add", "CHANGELOG.md"])?;
            test_git(&root, &["commit", "--quiet", "-m", "changelog-mutated"])?;
            let changelog_mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["reset", "--quiet", "--hard", &mutated])?;
'''
text = replace_exact(text, setup_block, "", "existing changelog adversary setup")

snapshot_anchor = '''            let before_refs = test_git_output(
'''
pre_snapshot = '''            test_git(&root, &["reset", "--quiet", "--hard", &dependency_only])?;
            let before_refs = test_git_output(
'''
text = replace_exact(
    text,
    snapshot_anchor,
    pre_snapshot,
    "dependency-only caller-state setup",
)

measured_assertion = '''            if verify_release_metadata(&root, &changelog_mutated, &source).is_ok() {
                return Err("source-authoritative changelog mutation was accepted".into());
            }
'''
late_block = '''            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
    .map_err(|error| error.to_string())?;
  test_git(&root, &["add", "CHANGELOG.md"])?;
  test_git(&root, &["commit", "--quiet", "-m", "changelog-mutated"])?;
  let changelog_mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
  if verify_release_metadata(&root, &changelog_mutated, &source).is_ok() {
      return Err("source-authoritative changelog mutation was accepted".into());
  }
'''
text = replace_exact(
    text,
    measured_assertion,
    late_block,
    "late changelog adversary block for trusted workflow relocation",
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

workflow = Path(".github/workflows/source-promotion-version-identity-repair.yml")
if workflow.exists():
    workflow.unlink()

script = Path(".github/scripts/patch_source_promotion_version_identity.py")
if script.exists():
    script.unlink()
