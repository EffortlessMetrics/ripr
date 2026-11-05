//! Nonpublishing final-subject contract fixtures use the real archive/assembler.
use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

const VERSION: &str = "1.2.3";
const REPOSITORY: &str = "EffortlessMetrics/ripr";
const CANDIDATE_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CANDIDATE_TREE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn fixture(root: &Path) -> Result<(), String> {
    write(&root.join("LICENSE-MIT"), "mit")?;
    write(&root.join("LICENSE-APACHE"), "apache")?;
    for target in super::release_server::release_server_target_set() {
        let (executable, archive) = if target.ends_with("windows-msvc") {
            ("ripr.exe", "zip")
        } else {
            ("ripr", "tar.gz")
        };
        write(
            &root
                .join("target")
                .join(target)
                .join("release")
                .join(executable),
            target,
        )?;
        super::release_server::release_server_archive(&[
            "--version".into(),
            VERSION.into(),
            "--target".into(),
            target.into(),
            "--executable".into(),
            executable.into(),
            "--archive".into(),
            archive.into(),
        ])?;
        let path = root
            .join("dist")
            .join(format!("ripr-server-v{VERSION}-{target}.receipt.json"));
        let mut receipt: Value =
            serde_json::from_slice(&fs::read(&path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        // Synthetic source identities are explicitly fixture data, never provenance.
        receipt["repository"] = REPOSITORY.into();
        receipt["candidate_sha"] = CANDIDATE_SHA.into();
        receipt["candidate_tree"] = CANDIDATE_TREE.into();
        receipt["toolchain_file_sha256"] = "c".repeat(64).into();
        receipt["cargo_lock_sha256"] = "d".repeat(64).into();
        write(
            &path,
            serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?,
        )?;
    }
    super::release_server::release_server_manifest(&[
        "--version".into(),
        VERSION.into(),
        "--repository".into(),
        REPOSITORY.into(),
    ])
}

fn command_args(output: &str) -> Vec<String> {
    [
        "release-final-server-subjects",
        "--version",
        VERSION,
        "--repository",
        REPOSITORY,
        "--candidate-sha",
        CANDIDATE_SHA,
        "--candidate-tree",
        CANDIDATE_TREE,
        "--dist",
        "dist",
        "--out",
        output,
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[test]
fn release_final_subject_inventory_command_uses_assembler_contract() -> Result<(), String> {
    crate::tests::with_temp_cwd("final-server-subject-command", |root| {
        fixture(root)?;
        let result = crate::dispatch::execute(crate::command::XtaskCommand::parse(command_args(
            "prepared",
        )));
        if cfg!(not(unix)) {
            assert!(
                result
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.contains("single-link identity is unavailable")),
                "unsupported single-link identity must fail closed"
            );
            let receipt = read_json(&root.join("prepared/final-server-subjects.receipt.json"))?;
            assert_eq!(receipt["disposition"], "rejected");
            assert_eq!(receipt["release_upload_eligible"], false);
            for field in [
                "inventory_sha256",
                "provenance_inputs_sha256",
                "subject_checksums_sha256",
            ] {
                assert_eq!(receipt.get(field), Some(&Value::Null), "{field}");
            }
            return Ok(());
        }
        result?;
        let inventory: Value = serde_json::from_slice(
            &fs::read(root.join("prepared/final-server-subjects.json"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(inventory["subjects"].as_array().map(Vec::len), Some(12));
        let receipt: Value = serde_json::from_slice(
            &fs::read(root.join("prepared/final-server-subjects.receipt.json"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(receipt["disposition"], "inventoried");
        assert_eq!(receipt["release_upload_eligible"], false);
        assert_eq!(receipt["provenance_verified"], false);
        assert_eq!(receipt["attestation_attempted"], false);
        assert_eq!(receipt["credential_requested"], false);
        assert_eq!(receipt["publication_mutation_attempted"], false);
        let subjects = inventory["subjects"].as_array().ok_or("missing subjects")?;
        for (kind, count) in [
            ("server_archive", 5),
            ("archive_checksum", 5),
            ("server_manifest", 1),
            ("aggregate_checksum", 1),
        ] {
            assert_eq!(
                subjects
                    .iter()
                    .filter(|subject| subject["kind"] == kind)
                    .count(),
                count
            );
        }
        let checksums = fs::read_to_string(root.join("prepared/final-server-subjects.sha256"))
            .map_err(|error| error.to_string())?;
        assert_eq!(checksums.lines().count(), 12);
        let request = read_json(&root.join("prepared/final-server-provenance-inputs.json"))?;
        assert_eq!(request["subjects"].as_array().map(Vec::len), Some(12));
        assert_eq!(request["execution_state"], "not_requested");
        assert!(request["action_identity"].is_null());
        assert_eq!(request["verification_observed"], false);
        assert_eq!(request["release_upload_eligible"], false);
        let markdown = fs::read_to_string(root.join("prepared/final-server-subjects.receipt.md"))
            .map_err(|error| error.to_string())?;
        for (field, name, label) in [
            (
                "inventory_sha256",
                "final-server-subjects.json",
                "Inventory",
            ),
            (
                "provenance_inputs_sha256",
                "final-server-provenance-inputs.json",
                "Provenance inputs",
            ),
            (
                "subject_checksums_sha256",
                "final-server-subjects.sha256",
                "Subject checksums",
            ),
        ] {
            let bytes =
                fs::read(root.join("prepared").join(name)).map_err(|error| error.to_string())?;
            assert!(!bytes.is_empty(), "{name} must contain prepared bytes");
            let digest = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(
                receipt.get(field).and_then(Value::as_str),
                Some(digest.as_str()),
                "receipt must bind the exact raw bytes of {name}"
            );
            assert!(markdown.contains(&format!("{label} SHA-256: `{digest}`")));
            let mut changed = bytes;
            changed.push(b'\n');
            let changed_digest = format!("{:x}", Sha256::digest(&changed));
            assert_ne!(
                receipt.get(field).and_then(Value::as_str),
                Some(changed_digest.as_str()),
                "the original receipt must not bind changed {name} bytes"
            );
        }
        Ok(())
    })
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

#[test]
fn release_final_subject_inventory_help_is_nonpublishing() -> Result<(), String> {
    let help = crate::command::help_message(&["release-final-server-subjects".into()])?;
    assert!(help.contains("report_only"));
    assert!(help.contains("without attestation, credentials, or publication authority"));
    let entry = crate::command::command_catalog()
        .into_iter()
        .find(|entry| entry.command.starts_with("release-final-server-subjects "))
        .ok_or("final server subject command is missing from the catalog")?;
    assert_eq!(entry.mutability, "report_only");
    assert!(!entry.judgment_required);
    assert!(!entry.ci_enforced);
    Ok(())
}

#[cfg(unix)]
mod inventory_controls {
    use super::*;

    fn run(args: Vec<String>) -> Result<(), String> {
        crate::dispatch::execute(crate::command::XtaskCommand::parse(args))
    }

    fn set_flag(args: &mut [String], flag: &str, value: &str) -> Result<(), String> {
        let index = args
            .iter()
            .position(|item| item == flag)
            .ok_or("fixture flag missing")?;
        let field = args
            .get_mut(index + 1)
            .ok_or("fixture flag value missing")?;
        *field = value.into();
        Ok(())
    }

    fn rejected(
        root: &Path,
        output: &str,
        args: Vec<String>,
        expected: &str,
    ) -> Result<(), String> {
        let error = match run(args) {
            Err(error) => error,
            Ok(()) => {
                return Err("expected final-subject rejection, got successful preparation".into());
            }
        };
        assert!(
            error.contains(expected),
            "expected rejection category: {expected}"
        );
        let receipt = read_json(&root.join(output).join("final-server-subjects.receipt.json"))?;
        assert_eq!(receipt["disposition"], "rejected");
        assert_eq!(receipt["release_upload_eligible"], false);
        assert_eq!(receipt["provenance_verified"], false);
        assert_eq!(receipt["attestation_attempted"], false);
        assert_eq!(receipt["credential_requested"], false);
        assert_eq!(receipt["publication_mutation_attempted"], false);
        assert!(
            receipt["failures"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
        assert!(
            receipt["observed_staging_files"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
        for (field, name) in [
            ("inventory_sha256", "final-server-subjects.json"),
            (
                "provenance_inputs_sha256",
                "final-server-provenance-inputs.json",
            ),
            ("subject_checksums_sha256", "final-server-subjects.sha256"),
        ] {
            assert_eq!(receipt.get(field), Some(&Value::Null), "{field}");
            assert!(!root.join(output).join(name).exists(), "{name}");
        }
        assert!(
            root.join(output)
                .join("final-server-subjects.receipt.md")
                .is_file()
        );
        Ok(())
    }

    #[test]
    fn release_final_subject_inventory_rejects_changed_final_bytes() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-changed", |root| {
            fixture(root)?;
            for (index, (name, expected)) in [
                (
                    "ripr-server-v1.2.3-x86_64-unknown-linux-gnu.tar.gz",
                    "digest mismatch",
                ),
                (
                    "ripr-server-manifest-v1.2.3.json",
                    "accepted assembler output",
                ),
                ("SHA256SUMS", "accepted assembler output"),
                (
                    "ripr-server-assembly-v1.2.3.receipt.json",
                    "accepted assembler output",
                ),
                (
                    "ripr-server-v1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256",
                    "checksum mismatch",
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let path = root.join("dist").join(name);
                let original = fs::read(&path).map_err(|error| error.to_string())?;
                let mut changed = original.clone();
                let byte = changed.first_mut().ok_or("fixture unexpectedly empty")?;
                *byte ^= 1;
                write(&path, changed)?;
                let output = format!("changed-{index}");
                rejected(root, &output, command_args(&output), expected)?;
                write(&path, original)?;
            }
            Ok(())
        })
    }

    #[test]
    fn release_final_subject_inventory_rejects_noncanonical_sidecar() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-sidecar", |root| {
            fixture(root)?;
            let path = root.join("dist/ripr-server-v1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256");
            let original = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            write(&path, format!(" {original}"))?;
            // The assembler trims this sidecar. Final preparation still binds
            // the exact sidecar bytes selected by the public uploader.
            rejected(
                root,
                "sidecar",
                command_args("sidecar"),
                "exact canonical checksum",
            )
        })
    }

    #[test]
    fn release_final_subject_inventory_rejects_missing_and_extra_inputs() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-missing-extra", |root| {
            fixture(root)?;
            let path = root.join("dist/ripr-server-v1.2.3-x86_64-unknown-linux-gnu.tar.gz");
            let original = fs::read(&path).map_err(|error| error.to_string())?;
            fs::remove_file(&path).map_err(|error| error.to_string())?;
            rejected(root, "missing", command_args("missing"), "unavailable")?;
            write(&path, original)?;
            write(&root.join("dist/unreviewed.log"), "extra")?;
            rejected(root, "extra", command_args("extra"), "unrecognized staged")?;
            fs::remove_file(root.join("dist/unreviewed.log")).map_err(|error| error.to_string())?;
            write(&root.join("dist/checksums.txt"), "legacy")?;
            rejected(
                root,
                "legacy",
                command_args("legacy"),
                "unrecognized staged",
            )?;
            assert_eq!(
                fs::read(root.join("dist/checksums.txt")).map_err(|error| error.to_string())?,
                b"legacy"
            );
            Ok(())
        })
    }

    #[test]
    fn release_final_subject_inventory_rejects_bad_receipt_and_identity() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-identity", |root| {
            fixture(root)?;
            let path = root.join("dist/ripr-server-v1.2.3-x86_64-unknown-linux-gnu.receipt.json");
            let original = fs::read(&path).map_err(|error| error.to_string())?;
            write(&path, b"{broken")?;
            rejected(
                root,
                "malformed",
                command_args("malformed"),
                "malformed release server receipt",
            )?;
            let mut receipt: Value =
                serde_json::from_slice(&original).map_err(|error| error.to_string())?;
            receipt["schema_version"] = "unsupported".into();
            write(
                &path,
                serde_json::to_vec(&receipt).map_err(|error| error.to_string())?,
            )?;
            rejected(
                root,
                "schema",
                command_args("schema"),
                "unsupported release server receipt schema",
            )?;
            write(&path, original)?;
            let mut args = command_args("identity");
            set_flag(&mut args, "--candidate-sha", &"e".repeat(40))?;
            rejected(root, "identity", args, "expected source identity")?;
            let mut args = command_args("tree");
            set_flag(&mut args, "--candidate-tree", &"f".repeat(40))?;
            rejected(root, "tree", args, "expected source identity")
        })
    }

    #[test]
    fn release_final_subject_inventory_rejects_links_and_nonregular_inputs() -> Result<(), String> {
        use std::os::unix::fs::symlink;
        crate::tests::with_temp_cwd("final-subject-links", |root| {
            fixture(root)?;
            let path = root.join("dist/SHA256SUMS");
            fs::hard_link(&path, root.join("outside-checksums"))
                .map_err(|error| error.to_string())?;
            rejected(
                root,
                "hardlink",
                command_args("hardlink"),
                "hard-linked/aliased",
            )?;
            fs::remove_file(root.join("outside-checksums")).map_err(|error| error.to_string())?;
            symlink(&path, root.join("dist/link")).map_err(|error| error.to_string())?;
            rejected(
                root,
                "symlink",
                command_args("symlink"),
                "symlink or nonregular",
            )?;
            fs::remove_file(root.join("dist/link")).map_err(|error| error.to_string())?;
            fs::create_dir(root.join("dist/nested")).map_err(|error| error.to_string())?;
            rejected(
                root,
                "directory",
                command_args("directory"),
                "symlink or nonregular",
            )
        })
    }

    #[test]
    fn release_final_subject_inventory_is_root_and_enumeration_independent() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-determinism", |root| {
            fixture(root)?;
            run(command_args("first"))?;
            fs::create_dir(root.join("relocated")).map_err(|error| error.to_string())?;
            let mut paths = fs::read_dir(root.join("dist"))
                .map_err(|error| error.to_string())?
                .map(|entry| {
                    entry
                        .map(|entry| entry.path())
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            paths.sort();
            for path in paths.into_iter().rev() {
                fs::copy(
                    &path,
                    root.join("relocated")
                        .join(path.file_name().ok_or("fixture has no basename")?),
                )
                .map_err(|error| error.to_string())?;
            }
            let mut args = command_args("second");
            set_flag(
                &mut args,
                "--dist",
                &root.join("relocated").to_string_lossy(),
            )?;
            run(args)?;
            for name in [
                "final-server-subjects.json",
                "final-server-provenance-inputs.json",
                "final-server-subjects.sha256",
                "final-server-subjects.receipt.json",
                "final-server-subjects.receipt.md",
            ] {
                assert_eq!(
                    fs::read(root.join("first").join(name)).map_err(|error| error.to_string())?,
                    fs::read(root.join("second").join(name)).map_err(|error| error.to_string())?,
                    "{name}"
                );
            }
            Ok(())
        })
    }

    #[test]
    fn release_final_subject_inventory_output_and_option_boundaries() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-output", |root| {
            fixture(root)?;
            write(&root.join("existing/sentinel"), "keep")?;
            assert!(
                run(command_args("existing"))
                    .err()
                    .is_some_and(|error| error.contains("output must be a fresh directory"))
            );
            assert_eq!(
                fs::read(root.join("existing/sentinel")).map_err(|error| error.to_string())?,
                b"keep"
            );
            assert_eq!(
                run(command_args("dist/prepared")).err().as_deref(),
                Some("output must be outside the staging directory")
            );
            assert!(!root.join("dist/prepared").exists());
            let mut args = command_args("unauthorized");
            args.extend(["--authorize-publication".into(), "true".into()]);
            assert_eq!(
                run(args).err().as_deref(),
                Some(
                    "unknown final-subject option `--authorize-publication`; this command cannot request attestation or publication"
                )
            );
            assert!(!root.join("unauthorized").exists());
            let mut args = command_args("duplicate");
            args.extend(["--version".into(), VERSION.into()]);
            assert_eq!(
                run(args).err().as_deref(),
                Some("duplicate final-subject option `--version`")
            );
            let mut args = command_args("uppercase");
            set_flag(&mut args, "--candidate-sha", &"A".repeat(40))?;
            assert_eq!(
                run(args).err().as_deref(),
                Some("candidate SHA must be 40 lowercase hexadecimal characters")
            );
            let mut args = command_args("repository");
            set_flag(&mut args, "--repository", "somewhere/else")?;
            assert_eq!(
                run(args).err().as_deref(),
                Some("final server subjects require the source repository EffortlessMetrics/ripr")
            );
            let mut args = command_args("traversal");
            set_flag(&mut args, "--dist", "../dist")?;
            assert_eq!(
                run(args).err().as_deref(),
                Some("staging/output paths must not contain parent traversal")
            );
            Ok(())
        })
    }

    #[test]
    fn release_final_subject_inventory_bounds_metadata_and_staging() -> Result<(), String> {
        crate::tests::with_temp_cwd("final-subject-bounds", |root| {
            fixture(root)?;
            let path = root.join("dist/oversized.receipt.json");
            fs::File::create(&path)
                .map_err(|error| error.to_string())?
                .set_len(4 * 1024 * 1024 + 1)
                .map_err(|error| error.to_string())?;
            rejected(
                root,
                "metadata-bound",
                command_args("metadata-bound"),
                "metadata exceeds",
            )?;
            fs::remove_file(path).map_err(|error| error.to_string())?;
            for index in 0..65 {
                write(&root.join("dist").join(format!("extra-{index}")), "extra")?;
            }
            rejected(
                root,
                "file-bound",
                command_args("file-bound"),
                "64-file bound",
            )
        })
    }
}
