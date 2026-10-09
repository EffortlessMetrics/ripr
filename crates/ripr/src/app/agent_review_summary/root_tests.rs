//! #6313: executable advice must retain the native Unix root and redirect.

use super::{build_agent_review_summary_report, render_agent_review_summary_json};
use crate::agent::loop_commands::{WORKFLOW_AGENT_STATUS_ARTIFACT, shell_arg};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Path, PathBuf};

struct OwnedRoot(PathBuf);

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn review_summary_commands_retain_literal_unix_root_and_redirect() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let parent = std::env::temp_dir().join(format!(
        "ripr-review-summary-root-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&parent).map_err(|error| error.to_string())?;
    let _owned = OwnedRoot(parent.clone());
    let root = parent.join("team\\repo 'quoted'");
    let decoy = parent.join("team/repo 'quoted'");
    let foreign = parent.join("foreign");
    for directory in [
        root.join("target/ripr/workflow"),
        decoy.join("target/ripr/workflow"),
        foreign.clone(),
    ] {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    let selected_metadata = std::fs::metadata(&root).map_err(|error| error.to_string())?;
    let decoy_metadata = std::fs::metadata(&decoy).map_err(|error| error.to_string())?;
    assert_ne!(
        (selected_metadata.dev(), selected_metadata.ino()),
        (decoy_metadata.dev(), decoy_metadata.ino())
    );
    let decoy_output = decoy.join(WORKFLOW_AGENT_STATUS_ARTIFACT);
    let decoy_marker = b"immutable slash-decoy artifact";
    std::fs::write(&decoy_output, decoy_marker).map_err(|error| error.to_string())?;

    let script = parent.join("capture.sh");
    let custody = parent.join("argv.bin");
    let output_marker = b"captured advisory output";
    let capture = |command: &str| -> Result<Vec<Vec<u8>>, String> {
        // Custody is separate from stdout: the exact production command
        // redirects stdout to its selected workflow artifact.
        std::fs::write(
            &script,
            format!(
                "ripr() {{ printf '%s\\000' \"$@\" > {}; printf '%s' 'captured advisory output'; }}\n{command}\n",
                shell_arg(&custody.to_string_lossy())
            ),
        )
        .map_err(|error| error.to_string())?;
        let output = std::process::Command::new("sh")
            .arg(&script)
            .current_dir(&foreign)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| format!("required Unix shell failed to start: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "advisory shell capture failed: {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let bytes = std::fs::read(&custody).map_err(|error| error.to_string())?;
        if bytes.last() != Some(&0) {
            return Err("shell custody omitted nonempty NUL-separated argv".to_string());
        }
        Ok(bytes[..bytes.len() - 1]
            .split(|byte| *byte == 0)
            .map(<[u8]>::to_vec)
            .collect())
    };
    let root_argument = |argv: &[Vec<u8>]| -> Result<PathBuf, String> {
        let flag = argv
            .iter()
            .position(|argument| argument.as_slice() == b"--root")
            .ok_or("advisory command omitted --root")?;
        let argument = argv
            .get(flag + 1)
            .ok_or("advisory command omitted its root argument")?;
        Ok(Path::new(std::ffi::OsStr::from_bytes(argument)).to_path_buf())
    };

    // Independently show that display normalization really selects the
    // existing different inode, with all shell setup already valid.
    let wrong = format!(
        "ripr agent status --root {} --json",
        shell_arg(&root.to_string_lossy().replace('\\', "/"))
    );
    let wrong_argv = capture(&wrong)?;
    let wrong_root = root_argument(&wrong_argv)?;
    assert_eq!(
        wrong_root.as_os_str().as_bytes(),
        decoy.as_os_str().as_bytes()
    );
    let wrong_metadata = std::fs::metadata(&wrong_root).map_err(|error| error.to_string())?;
    assert_eq!(
        (wrong_metadata.dev(), wrong_metadata.ino()),
        (decoy_metadata.dev(), decoy_metadata.ino())
    );

    let report = build_agent_review_summary_report(&root, &root);
    let rendered = render_agent_review_summary_json(&report)?;
    let value: serde_json::Value =
        serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
    let next = value["next_command"]["command"]
        .as_str()
        .ok_or("real missing-artifact report omitted continuation")?;
    let status_command = value["surfaces"]
        .as_array()
        .and_then(|surfaces| {
            surfaces
                .iter()
                .find(|surface| surface["name"] == "agent_status")
        })
        .and_then(|surface| surface["summary"].as_str())
        .and_then(|summary| summary.split_once("Command: ").map(|(_, command)| command))
        .ok_or("rendered status surface omitted its executable instruction")?;
    for (label, command) in [("next_command", next), ("status surface", status_command)] {
        let argv = capture(command)?;
        let received_root = root_argument(&argv)?;
        assert_eq!(
            received_root.as_os_str().as_bytes(),
            root.as_os_str().as_bytes(),
            "{label} command lost native Unix identity: {command}"
        );
        let received_metadata =
            std::fs::metadata(&received_root).map_err(|error| error.to_string())?;
        assert_eq!(
            (received_metadata.dev(), received_metadata.ino()),
            (selected_metadata.dev(), selected_metadata.ino()),
            "{label} command selected a different root"
        );
        if label == "next_command" {
            let out_flag = argv
                .iter()
                .position(|argument| argument.as_slice() == b"--out")
                .ok_or("pilot continuation omitted --out")?;
            assert_eq!(
                argv.get(out_flag + 1).map(Vec::as_slice),
                Some(root.join("target/ripr/pilot").as_os_str().as_bytes()),
                "pilot output escaped the selected root"
            );
        }
    }
    assert_eq!(
        std::fs::read(root.join(WORKFLOW_AGENT_STATUS_ARTIFACT))
            .map_err(|error| error.to_string())?,
        output_marker,
        "embedded status advice redirected outside the selected root"
    );
    assert_eq!(
        std::fs::read(&decoy_output).map_err(|error| error.to_string())?,
        decoy_marker,
        "embedded status advice modified the slash-decoy artifact"
    );
    assert!(!foreign.join(WORKFLOW_AGENT_STATUS_ARTIFACT).exists());
    Ok(())
}
