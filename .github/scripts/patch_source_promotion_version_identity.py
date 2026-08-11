from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new)


verifier = Path("xtask/src/reports/source_promotion_verify.rs")
text = verifier.read_text(encoding="utf-8")

# Compatibility anchor consumed by the trusted one-use workflow before this
# script runs. It is a no-op on the final product tree.
text = text.replace("METADATA_SURFACES", "RELEASE_METADATA_SURFACES")

text = replace_once(
    text,
    "Changing the preflight bytes, resolution manifest, exact join, parent identities, reviewed tree, or verified main invalidates this receipt.",
    "Changing the preflight bytes, resolution manifest, exact join, parent identities, reviewed tree, governed release-version identity, source-authoritative CHANGELOG.md bytes, or verified main invalidates this receipt.",
    "receipt invalidation identity",
)
text = replace_once(
    text,
    "This receipt proves Git graph and byte identity only; it does not adjudicate conflicts, product correctness, release readiness, or publication.",
    "This receipt proves the exact Git graph, reviewed-tree identity, governed release-version identity, and source-authoritative CHANGELOG.md bytes only; it does not adjudicate conflicts, product correctness, release readiness, or publication.",
    "receipt claim boundary",
)

# Re-express the already-correct changelog adversary in the temporary shape
# consumed by the trusted workflow's caller-state relocation step. Rustfmt runs
# immediately after that step, and these temporary bytes never enter the final
# candidate.
current_setup = '''            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
                .map_err(|error| error.to_string())?;
            test_git(&root, &["add", "CHANGELOG.md"])?;
            test_git(&root, &["commit", "--quiet", "-m", "changelog-mutated"])?;
            let changelog_mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["reset", "--quiet", "--hard", &mutated])?;
'''
text = replace_once(text, current_setup, "", "current changelog adversary setup")

measured_assertion = '''            if verify_release_metadata(&root, &changelog_mutated, &source).is_ok() {
                return Err("source-authoritative changelog mutation was accepted".into());
            }
'''
workflow_late_block = '''            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
    .map_err(|error| error.to_string())?;
  test_git(&root, &["add", "CHANGELOG.md"])?;
  test_git(&root, &["commit", "--quiet", "-m", "changelog-mutated"])?;
  let changelog_mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
  if verify_release_metadata(&root, &changelog_mutated, &source).is_ok() {
      return Err("source-authoritative changelog mutation was accepted".into());
  }
'''
text = replace_once(
    text,
    measured_assertion,
    workflow_late_block,
    "trusted-workflow changelog relocation input",
)

verifier.write_text(text, encoding="utf-8")

workflow = Path(".github/workflows/source-promotion-version-identity-repair.yml")
if workflow.exists():
    workflow.unlink()

script = Path(".github/scripts/patch_source_promotion_version_identity.py")
if script.exists():
    script.unlink()
