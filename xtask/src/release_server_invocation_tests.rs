use super::{ServerIdentity, producer_args, upload_with};
use crate::command::XtaskCommand;
use crate::dispatch::execute;
use std::fs;

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn source(root: &std::path::Path, version: &str) -> Result<(), String> {
    fs::create_dir_all(root.join("crates/ripr")).map_err(|error| error.to_string())?;
    fs::write(
        root.join("crates/ripr/Cargo.toml"),
        "[package]\nname = \"ripr\"\nversion.workspace = true\n",
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        root.join("Cargo.toml"),
        format!("[workspace.package]\nversion = \"{version}\"\n"),
    )
    .map_err(|error| error.to_string())
}

#[test]
fn given_product_when_rc_placement_is_separate_then_identity_preserves_both() -> Result<(), String>
{
    // #1466/#1631 product/ref contract; this fixture grants no publication authority.
    let identity = ServerIdentity::from_source("0.11.0", "v0.11.0-rc.2")?;
    assert_eq!(identity.product_version, "0.11.0");
    assert_eq!(identity.placement_version, "0.11.0-rc.2");
    for placement in ["0.11.0", "v0.11.0"] {
        assert_eq!(
            ServerIdentity::from_source("0.11.0", placement)?.product_version,
            "0.11.0"
        );
    }
    Ok(())
}

#[test]
fn given_wrong_source_or_noncanonical_placement_then_identity_rejects() {
    for (product, placement) in [
        ("0.11.0-rc.2", "v0.11.0-rc.2"),
        ("0.11.0-alpha.2", "v0.11.0-rc.2"),
        ("0.11.0", "v0.12.0-rc.2"),
        ("0.11.0", "v0.11.0-rc.0"),
        ("0.11.0", "v0.11.0-rc.02"),
        ("0.11.0", "v0.11.0-rc.-1"),
        ("0.11.0", "v0.11.0-rc.2+build"),
        ("0.11.0", "v0.11.0-beta.2"),
        ("0.11.0", "vv0.11.0-rc.2"),
        ("0.11.0", " v0.11.0-rc.2"),
    ] {
        assert!(
            ServerIdentity::from_source(product, placement).is_err(),
            "{product} / {placement}"
        );
    }
}

#[test]
fn given_source_when_placement_args_are_adapted_then_product_is_source_bound() -> Result<(), String>
{
    crate::tests::with_temp_cwd("server-source-bound-args", |root| {
        source(root, "0.11.0")?;
        for placement in ["--release-version=v0.11.0-rc.2", "--release-version=0.11.0"] {
            assert_eq!(
                producer_args(&arguments(&[placement, "--target", "fixture"]))?,
                arguments(&["--target", "fixture", "--version", "0.11.0"])
            );
        }
        assert_eq!(
            producer_args(&arguments(&["--version", "0.11.0"]))?,
            arguments(&["--version", "0.11.0"])
        );
        for values in [
            vec!["--release-version"],
            vec!["--release-version="],
            vec!["--release-version", "v0.11.0-rc.2", "--version", "0.11.0"],
            vec!["--release-version=v0.11.0-rc.2", "--version=0.11.0"],
            vec![
                "--release-version=v0.11.0-rc.2",
                "--release-version=v0.11.0-rc.2",
            ],
        ] {
            assert!(producer_args(&arguments(&values)).is_err(), "{values:?}");
        }
        source(root, "0.12.0")?;
        assert!(producer_args(&arguments(&["--release-version", "v0.11.0-rc.2"])).is_err());
        fs::remove_file(root.join("crates/ripr/Cargo.toml")).map_err(|error| error.to_string())?;
        assert!(producer_args(&arguments(&["--release-version", "v0.11.0-rc.2"])).is_err());
        Ok(())
    })
}

#[test]
fn given_rc_input_when_legacy_upload_is_requested_then_it_stops_before_transport()
-> Result<(), String> {
    for version in ["v0.11.0-rc.2", "0.11.0-rc.2", "0.11.0-alpha.2"] {
        let mut attempted = false;
        let error = upload_with(&arguments(&["--version", version]), |_| {
            attempted = true;
            Ok(())
        })
        .err()
        .ok_or_else(|| format!("legacy upload accepted {version}"))?;
        assert!(
            error.contains("exact RC authorization/transport is required"),
            "{error}"
        );
        assert!(!attempted, "RC reached fake transport: {version}");
    }
    let stable_args = arguments(&["--version", "v0.11.0"]);
    let mut attempted = false;
    upload_with(&stable_args, |received| {
        assert_eq!(received, stable_args);
        attempted = true;
        Ok(())
    })?;
    assert!(attempted, "stable invocation did not reach fake transport");
    Ok(())
}

#[test]
fn given_old_conflation_when_archive_or_manifest_runs_then_product_guard_rejects()
-> Result<(), String> {
    crate::tests::with_temp_cwd("server-old-conflation", |root| {
        source(root, "0.11.0")?;
        let archive_args = arguments(&[
            "--version",
            "v0.11.0-rc.2",
            "--target",
            "x86_64-pc-windows-msvc",
            "--executable",
            "ripr.exe",
            "--archive",
            "zip",
        ]);
        let manifest_args =
            arguments(&["--version", "v0.11.0-rc.2", "--repository", "fixture/ripr"]);
        for command in [
            XtaskCommand::ReleaseServerArchive(archive_args),
            XtaskCommand::ReleaseServerManifest(manifest_args),
        ] {
            let error = execute(command)
                .err()
                .ok_or("old conflation was accepted")?;
            assert!(
                error.contains("must not carry a release-channel suffix"),
                "{error}"
            );
        }
        assert!(!root.join("dist").exists());
        assert!(!root.join("package").exists());
        Ok(())
    })
}

#[test]
fn given_rc_workflow_input_when_real_producers_run_then_subjects_keep_product_identity()
-> Result<(), String> {
    crate::tests::with_temp_cwd("server-rc-real-producers", |root| {
        source(root, "0.11.0")?;
        fs::write(root.join("LICENSE-MIT"), "fixture MIT license")
            .map_err(|error| error.to_string())?;
        fs::write(root.join("LICENSE-APACHE"), "fixture Apache license")
            .map_err(|error| error.to_string())?;
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.95.0\"\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(root.join("Cargo.lock"), "# fixture lock identity\n")
            .map_err(|error| error.to_string())?;
        crate::run("git", &["init", "--quiet"])?;
        crate::run(
            "git",
            &[
                "add",
                "Cargo.toml",
                "crates/ripr/Cargo.toml",
                "rust-toolchain.toml",
                "Cargo.lock",
            ],
        )?;
        crate::run(
            "git",
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "synthetic producer fixture",
            ],
        )?;
        for target in crate::reports::release_server::release_server_target_set() {
            let (executable, format) = if target.contains("windows") {
                ("ripr.exe", "zip")
            } else {
                ("ripr", "tar.gz")
            };
            let binary_dir = root.join("target").join(target).join("release");
            fs::create_dir_all(&binary_dir).map_err(|error| error.to_string())?;
            // Archive-content fixture, deliberately not an executable qualification subject.
            fs::write(binary_dir.join(executable), b"synthetic archive subject\n")
                .map_err(|error| error.to_string())?;
            execute(XtaskCommand::ReleaseServerArchive(arguments(&[
                "--release-version",
                "v0.11.0-rc.2",
                "--target",
                target,
                "--executable",
                executable,
                "--archive",
                format,
            ])))?;
            assert!(
                root.join("dist")
                    .join(format!("ripr-server-v0.11.0-{target}.{format}"))
                    .is_file()
            );
        }
        execute(XtaskCommand::ReleaseServerManifest(arguments(&[
            "--release-version",
            "v0.11.0-rc.2",
            "--repository",
            "fixture/ripr",
        ])))?;
        let manifest_path = root.join("dist/ripr-server-manifest-v0.11.0.json");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        assert_eq!(manifest["product_version"], "0.11.0");
        let assets = manifest["assets"]
            .as_object()
            .ok_or("manifest assets are missing")?;
        assert_eq!(assets.len(), 5);
        for asset in assets.values() {
            let subject = asset["subject"]
                .as_str()
                .ok_or("asset subject is missing")?;
            assert!(subject.starts_with("ripr-server-v0.11.0-"), "{subject}");
            assert!(!subject.contains("-rc."), "{subject}");
        }
        let sums =
            fs::read_to_string(root.join("dist/SHA256SUMS")).map_err(|error| error.to_string())?;
        assert!(sums.contains("ripr-server-manifest-v0.11.0.json"));
        assert!(!sums.contains("-rc."));
        Ok(())
    })
}

#[test]
fn given_legacy_workflow_then_archive_and_manifest_transport_separate_placement()
-> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../.github/workflows/release-server-binaries.yml");
    let workflow = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let producers: Vec<_> = workflow
        .lines()
        .filter(|line| {
            line.contains("run:")
                && (line.contains("release-server-archive ")
                    || line.contains("release-server-manifest "))
        })
        .collect();
    assert_eq!(producers.len(), 2);
    for invocation in producers {
        assert!(
            invocation.contains("--release-version \"$RIPR_RELEASE_VERSION\""),
            "{invocation}"
        );
        assert!(
            !invocation.contains("--version \"$RIPR_RELEASE_VERSION\""),
            "{invocation}"
        );
    }
    Ok(())
}
