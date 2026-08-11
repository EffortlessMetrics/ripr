from pathlib import Path
import re

verifier = Path("xtask/src/reports/source_promotion_verify.rs")
text = verifier.read_text(encoding="utf-8")

anchor = "use std::process::Command;\n\nconst PREFLIGHT_SCHEMA"
replacement = (
    "use std::process::Command;\n\n"
    "mod version_identity;\n"
    "use version_identity::{verify_release_metadata_identity, RELEASE_METADATA_SURFACES};\n\n"
    "const PREFLIGHT_SCHEMA"
)
if text.count(anchor) != 1:
    raise SystemExit(f"expected one module insertion anchor, found {text.count(anchor)}")
text = text.replace(anchor, replacement)

text, count = re.subn(
    r'const METADATA_SURFACES: &\[&str\] = &\[\n.*?\n\];\n\n',
    "",
    text,
    count=1,
    flags=re.S,
)
if count != 1:
    raise SystemExit(f"expected one METADATA_SURFACES block, replaced {count}")

text = text.replace("METADATA_SURFACES", "RELEASE_METADATA_SURFACES")
text = text.replace("metadata_verified", "release_metadata_verified")
text = text.replace("verify_metadata", "verify_release_metadata")
text = text.replace('"metadata_byte_identity"', '"release_version_identity"')
text = text.replace('"metadata_surfaces"', '"release_metadata_surfaces"')
text = text.replace(
    "metadata byte identity",
    "release-version identity and source-authoritative changelog bytes",
)

old_verify = '''fn verify_release_metadata(repo: &Path, join: &str, source: &str) -> Result<(), String> {
    for path in RELEASE_METADATA_SURFACES {
        let source_bytes = git_bytes(repo, &["show", &format!("{source}:{path}")])?;
        let join_bytes = git_bytes(repo, &["show", &format!("{join}:{path}")])?;
        if source_bytes != join_bytes {
            return Err(format!("release-version identity and source-authoritative changelog bytes failed for {path}"));
        }
    }
    Ok(())
}
'''
new_verify = '''fn verify_release_metadata(repo: &Path, join: &str, source: &str) -> Result<(), String> {
    verify_release_metadata_identity(repo, join, source)
}
'''
if text.count(old_verify) != 1:
    raise SystemExit(f"expected old verify function once, found {text.count(old_verify)}")
text = text.replace(old_verify, new_verify)

first_loop = '''            for path in RELEASE_METADATA_SURFACES {
                let file = root.join(path);
                fs::write(file, "stable\\n").map_err(|error| error.to_string())?;
            }
'''
second_loop = '''            for path in RELEASE_METADATA_SURFACES {
                fs::write(root.join(path), "stable\\n").map_err(|error| error.to_string())?;
            }
'''
if text.count(first_loop) != 1 or text.count(second_loop) != 1:
    raise SystemExit(
        f"metadata fixture loops changed: first={text.count(first_loop)} second={text.count(second_loop)}"
    )
text = text.replace(first_loop, '''            write_release_metadata_fixture(&root, "0.10.1", false)?;
''')
text = text.replace(second_loop, '''            write_release_metadata_fixture(&root, "0.10.1", false)?;
''')

test_anchor = '''    #[test]
    fn identity_arguments_are_exact_and_lowercase() -> Result<(), String> {
'''
if text.count(test_anchor) != 1:
    raise SystemExit(f"expected test helper anchor once, found {text.count(test_anchor)}")
test_helper = r'''    fn write_release_metadata_fixture(
        root: &Path,
        version: &str,
        workspace_inherited: bool,
    ) -> Result<(), String> {
        fs::create_dir_all(root.join("crates/ripr")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("editors/vscode")).map_err(|error| error.to_string())?;
        let workspace = if workspace_inherited {
            format!(
                "[workspace]\nmembers = [\"crates/ripr\"]\n[workspace.package]\nversion = \"{version}\"\n[workspace.dependencies]\nserde = \"1\"\n"
            )
        } else {
            "[workspace]\nmembers = [\"crates/ripr\"]\n".to_string()
        };
        let crate_manifest = if workspace_inherited {
            "[package]\nname = \"ripr\"\nversion.workspace = true\n[dependencies]\nrayon = \"1\"\n".to_string()
        } else {
            format!(
                "[package]\nname = \"ripr\"\nversion = \"{version}\"\n[dependencies]\nserde = \"1\"\n"
            )
        };
        let lock = format!(
            "version = 3\n[[package]]\nname = \"ripr\"\nversion = \"{version}\"\ndependencies = [\"serde\"]\n"
        );
        let package = format!(
            "{{\"name\":\"ripr\",\"version\":\"{version}\",\"scripts\":{{\"compile\":\"tsc\"}}}}\n"
        );
        let package_lock = format!(
            "{{\"name\":\"ripr\",\"version\":\"{version}\",\"lockfileVersion\":3,\"packages\":{{\"\":{{\"name\":\"ripr\",\"version\":\"{version}\",\"dependencies\":{{\"vscode-languageclient\":\"^9\"}}}}}}}}\n"
        );
        fs::write(root.join("Cargo.toml"), workspace).map_err(|error| error.to_string())?;
        fs::write(root.join("crates/ripr/Cargo.toml"), crate_manifest)
            .map_err(|error| error.to_string())?;
        fs::write(root.join("Cargo.lock"), lock).map_err(|error| error.to_string())?;
        fs::write(root.join("editors/vscode/package.json"), package)
            .map_err(|error| error.to_string())?;
        fs::write(root.join("editors/vscode/package-lock.json"), package_lock)
            .map_err(|error| error.to_string())?;
        fs::write(root.join("CHANGELOG.md"), "# Changelog\n")
            .map_err(|error| error.to_string())?;
        Ok(())
    }

'''
text = text.replace(test_anchor, test_helper + test_anchor)

old_mutation = '''            fs::write(root.join("Cargo.toml"), "changed\\n").map_err(|error| error.to_string())?;
            test_git(&root, &["add", "Cargo.toml"])?;
            test_git(&root, &["commit", "--quiet", "-m", "mutated"])?;
            let mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
'''
new_mutation = '''            write_release_metadata_fixture(&root, "0.10.1", true)?;
            test_git(&root, &["add", "."])?;
            test_git(
                &root,
                &["commit", "--quiet", "-m", "dependency-and-layout-change"],
            )?;
            let dependency_only = test_git_output(&root, &["rev-parse", "HEAD"])?;
            if verify_release_metadata(&root, &dependency_only, &source).is_err() {
                return Err(
                    "dependency/layout changes with unchanged effective versions were rejected"
                        .into(),
                );
            }
            write_release_metadata_fixture(&root, "0.10.2", true)?;
            test_git(&root, &["add", "."])?;
            test_git(&root, &["commit", "--quiet", "-m", "version-mutated"])?;
            let mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
'''
if text.count(old_mutation) != 1:
    raise SystemExit(f"expected metadata mutation block once, found {text.count(old_mutation)}")
text = text.replace(old_mutation, new_mutation)

old_assertions = '''            if verify_release_metadata(&root, &source, &source).is_err() {
                return Err("unchanged metadata was rejected".into());
            }
            if verify_release_metadata(&root, &mutated, &source).is_ok() {
                return Err("metadata byte mutation was accepted".into());
            }
'''
new_assertions = '''            if verify_release_metadata(&root, &source, &source).is_err() {
                return Err("unchanged release metadata was rejected".into());
            }
            if verify_release_metadata(&root, &mutated, &source).is_ok() {
                return Err("governed release-version mutation was accepted".into());
            }
            fs::write(root.join("CHANGELOG.md"), "# Changelog\\nchanged\\n")
                .map_err(|error| error.to_string())?;
            test_git(&root, &["add", "CHANGELOG.md"])?;
            test_git(&root, &["commit", "--quiet", "-m", "changelog-mutated"])?;
            let changelog_mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            if verify_release_metadata(&root, &changelog_mutated, &source).is_ok() {
                return Err("source-authoritative changelog mutation was accepted".into());
            }
'''
if text.count(old_assertions) != 1:
    raise SystemExit(f"expected metadata assertions once, found {text.count(old_assertions)}")
text = text.replace(old_assertions, new_assertions)

verifier.write_text(text, encoding="utf-8")

spec = Path("docs/specs/RIPR-SPEC-0149-source-promotion-verifier.md")
spec_text = spec.read_text(encoding="utf-8")
spec_text = spec_text.replace(
    "governed metadata is byte identical to the source parent",
    "the governed effective release versions match the source parent and CHANGELOG.md remains byte-identical",
)
spec_text = spec_text.replace(
    "metadata byte identity",
    "release-version identity and source-authoritative changelog bytes",
)
spec_text = spec_text.replace(
    "metadata drift",
    "release-version or source-authoritative changelog drift",
)
spec.write_text(spec_text, encoding="utf-8")

fixture = Path("fixtures/source_promotion_verification/SPEC.md")
fixture_text = fixture.read_text(encoding="utf-8")
fixture_text = fixture_text.replace(
    "governed metadata changes",
    "governed release-version or source-authoritative changelog changes",
)
fixture_text = fixture_text.replace(
    "its tree and\nmetadata are unchanged",
    "its tree, release-version identity, and source-authoritative changelog bytes are unchanged",
)
fixture.write_text(fixture_text, encoding="utf-8")

workflow = Path(".github/workflows/source-promotion-version-identity-repair.yml")
if workflow.exists():
    workflow.unlink()

script = Path(".github/scripts/patch_source_promotion_version_identity.py")
if script.exists():
    script.unlink()
