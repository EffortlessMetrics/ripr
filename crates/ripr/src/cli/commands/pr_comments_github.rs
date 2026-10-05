//! `ripr pr-comments existing` and `ripr pr-comments requests` (#5409): the
//! JSON work on either side of the generated workflow's `gh api` calls.
//! Neither command reads a token or the network; the workflow step that
//! holds `GH_TOKEN` makes every call.

use crate::cli::parse::expect_value;
use crate::cli::suggest::unknown_argument;
use crate::output::pr_inline_comment_github::{existing_comments, publish_requests};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use super::non_empty_string_arg;

const EXISTING: &str = "pr-comments existing";
const REQUESTS: &str = "pr-comments requests";
const DEFAULT_RAW: &str = "target/ripr/review/existing-comments.raw.json";
const DEFAULT_EXISTING_OUT: &str = "target/ripr/review/existing-comments.json";
const DEFAULT_PLAN: &str = "target/ripr/review/comment-publish-plan.json";
const DEFAULT_REQUESTS_DIR: &str = "target/ripr/review/publish";
/// One line per call, in order: method, endpoint after `repos/OWNER/REPO/`,
/// request file, and the message to print once the call succeeds,
/// separated by tabs.
pub(crate) const REQUESTS_MANIFEST: &str = "requests.tsv";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingOptions {
    pub(super) root: PathBuf,
    pub(super) raw: PathBuf,
    pub(super) out: PathBuf,
}

pub(super) fn parse_pr_comments_existing_options(
    args: &[String],
) -> Result<ExistingOptions, String> {
    let mut options = ExistingOptions {
        root: PathBuf::from("."),
        raw: PathBuf::from(DEFAULT_RAW),
        out: PathBuf::from(DEFAULT_EXISTING_OUT),
    };
    let mut i = 0usize;
    while i < args.len() {
        let flag = args[i].as_str();
        i += 1;
        match flag {
            "--root" => {
                options.root = PathBuf::from(non_empty_string_arg(args, i, flag, EXISTING)?)
            }
            "--raw" => options.raw = PathBuf::from(non_empty_string_arg(args, i, flag, EXISTING)?),
            "--out" => options.out = PathBuf::from(non_empty_string_arg(args, i, flag, EXISTING)?),
            other => return Err(unknown_argument(EXISTING, other)),
        }
        i += 1;
    }
    Ok(options)
}

pub(super) fn pr_comments_existing(args: &[String]) -> Result<(), String> {
    let options = parse_pr_comments_existing_options(args)?;
    let pages = if options.raw.as_os_str() == "-" {
        // The workflow pipes `gh api` straight in; keep the raw pages beside
        // the normalized file so the uploaded artifact still shows them.
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes)
            .map_err(|err| format!("{EXISTING} could not read standard input: {err}"))?;
        let raw_copy = options.root.join(DEFAULT_RAW);
        if let Some(parent) = raw_copy.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!("{EXISTING} could not create {}: {err}", parent.display())
            })?;
        }
        fs::write(&raw_copy, &bytes)
            .map_err(|err| format!("{EXISTING} could not write {}: {err}", raw_copy.display()))?;
        serde_json::from_slice(&bytes)
            .map_err(|err| format!("{EXISTING}: standard input is not JSON: {err}"))?
    } else {
        read_json(&options.root.join(&options.raw), EXISTING)?
    };
    let existing = existing_comments(&pages);
    let out = options.root.join(&options.out);
    write_json(&out, &existing, EXISTING)?;
    let count = existing["comments"].as_array().map_or(0, Vec::len);
    println!(
        "Captured {count} existing RIPR inline comment(s) in {}.",
        options.out.display()
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequestsOptions {
    pub(super) root: PathBuf,
    pub(super) plan: PathBuf,
    pub(super) out_dir: PathBuf,
    pub(super) pull_request: String,
    pub(super) head_sha: String,
}

pub(super) fn parse_pr_comments_requests_options(
    args: &[String],
) -> Result<RequestsOptions, String> {
    let mut options = RequestsOptions {
        root: PathBuf::from("."),
        plan: PathBuf::from(DEFAULT_PLAN),
        out_dir: PathBuf::from(DEFAULT_REQUESTS_DIR),
        pull_request: String::new(),
        head_sha: String::new(),
    };
    let mut i = 0usize;
    while i < args.len() {
        let flag = args[i].as_str();
        i += 1;
        match flag {
            "--root" => {
                options.root = PathBuf::from(non_empty_string_arg(args, i, flag, REQUESTS)?)
            }
            "--plan" => {
                options.plan = PathBuf::from(non_empty_string_arg(args, i, flag, REQUESTS)?)
            }
            "--out-dir" => {
                options.out_dir = PathBuf::from(non_empty_string_arg(args, i, flag, REQUESTS)?);
            }
            "--pull-request" => options.pull_request = expect_value(args, i, flag)?.to_string(),
            "--head-sha" => options.head_sha = expect_value(args, i, flag)?.to_string(),
            other => return Err(unknown_argument(REQUESTS, other)),
        }
        i += 1;
    }
    // The run replaces request files in this directory, so it must stay a
    // relative path inside --root: no absolute path, no `..`.
    if !options.out_dir.components().all(|part| {
        matches!(
            part,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    }) || options
        .out_dir
        .components()
        .all(|part| part == std::path::Component::CurDir)
    {
        return Err(format!(
            "{REQUESTS} --out-dir must be a relative directory inside --root (got {}); the run replaces its request files",
            options.out_dir.display()
        ));
    }
    if options
        .out_dir
        .to_string_lossy()
        .contains(['\t', '\r', '\n'])
    {
        return Err(format!(
            "{REQUESTS} --out-dir cannot contain a tab or line break: the request manifest is tab-separated"
        ));
    }
    // Both land in an API path or payload; refuse anything but the shapes
    // GitHub uses so a request can never address another endpoint.
    if options.pull_request.is_empty() || !options.pull_request.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!(
            "{REQUESTS} needs --pull-request N with the pull request number (got {:?}); in the generated workflow that is ${{{{ github.event.pull_request.number }}}}",
            options.pull_request
        ));
    }
    if options.head_sha.is_empty() || !options.head_sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "{REQUESTS} needs --head-sha SHA with the pull request head commit (got {:?}); in the generated workflow that is ${{{{ github.event.pull_request.head.sha }}}}",
            options.head_sha
        ));
    }
    Ok(options)
}

pub(super) fn pr_comments_requests(args: &[String]) -> Result<(), String> {
    let options = parse_pr_comments_requests_options(args)?;
    let plan = read_json(&options.root.join(&options.plan), REQUESTS)?;
    let planned = publish_requests(&plan, &options.pull_request, &options.head_sha);

    refuse_linked_out_dir(&options.root, &options.out_dir)?;
    let out_dir = options.root.join(&options.out_dir);
    // Stale request files from an earlier run must never be replayed. Only
    // files this command writes are removed, so a mistyped --out-dir cannot
    // lose anything else.
    if out_dir.is_dir() {
        let entries = fs::read_dir(&out_dir)
            .map_err(|err| format!("{REQUESTS} could not read {}: {err}", out_dir.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|err| format!("{REQUESTS} could not read {}: {err}", out_dir.display()))?
                .path();
            let owned = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_request_file);
            // Not followed: a link left under a request file name, dangling
            // or not, is removed so the write below creates a plain file.
            let removable =
                fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.file_type().is_dir());
            if owned && removable {
                fs::remove_file(&path).map_err(|err| {
                    format!("{REQUESTS} could not remove {}: {err}", path.display())
                })?;
            }
        }
    }
    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("{REQUESTS} could not create {}: {err}", out_dir.display()))?;
    let mut manifest = String::new();
    for (index, request) in planned.requests.iter().enumerate() {
        let file = options.out_dir.join(format!(
            "{:02}-{}.json",
            index + 1,
            request.method.to_ascii_lowercase()
        ));
        write_json(&options.root.join(&file), &request.payload, REQUESTS)?;
        // The workflow splits each line on tabs, so no field may carry a tab
        // or line break; the message is last and only ever printed.
        manifest.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            request.method,
            manifest_field(&request.endpoint),
            manifest_field(&file.display().to_string()),
            manifest_field(&request.message)
        ));
    }
    let manifest_path = out_dir.join(REQUESTS_MANIFEST);
    fs::write(&manifest_path, manifest).map_err(|err| {
        format!(
            "{REQUESTS} could not write {}: {err}",
            manifest_path.display()
        )
    })?;
    for note in &planned.notes {
        println!("{note}");
    }
    Ok(())
}

/// The lexical check in the parser keeps `--out-dir` under `--root` only if
/// no existing component is a link: a symlink or junction there would send
/// the cleanup and the writes outside the workspace, so refuse it.
fn refuse_linked_out_dir(root: &Path, out_dir: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for part in out_dir.components() {
        current.push(part);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(format!(
                    "{REQUESTS} could not inspect {}: {err}",
                    current.display()
                ));
            }
        };
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(format!(
                "{REQUESTS} --out-dir cannot pass through a link ({} is one); the run replaces request files there",
                current.display()
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

/// `requests.tsv` or an `NN-method.json` request file.
fn is_request_file(name: &str) -> bool {
    if name == REQUESTS_MANIFEST {
        return true;
    }
    let Some((index, rest)) = name.split_once('-') else {
        return false;
    };
    index.len() >= 2
        && index.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(rest, "patch.json" | "post.json")
}

fn manifest_field(text: &str) -> String {
    text.replace(['\t', '\r', '\n'], " ")
}

fn read_json(path: &Path, command: &str) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("{command} could not read {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("{command}: {} is not JSON: {err}", path.display()))
}

fn write_json(path: &Path, value: &Value, command: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("{command} could not create {}: {err}", parent.display()))?;
    }
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("{command} could not render {}: {err}", path.display()))?;
    text.push('\n');
    fs::write(path, text)
        .map_err(|err| format!("{command} could not write {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{is_request_file, parse_pr_comments_requests_options, pr_comments_requests};

    #[test]
    fn only_request_files_are_cleared() {
        for owned in [
            "requests.tsv",
            "01-patch.json",
            "12-post.json",
            "100-post.json",
        ] {
            assert!(is_request_file(owned), "{owned}");
        }
        for other in [
            "lib.rs",
            "1-post.json",
            "01-get.json",
            "ab-post.json",
            "01-post.json.bak",
        ] {
            assert!(!is_request_file(other), "{other}");
        }
    }

    fn parse(out_dir: &str) -> Result<String, String> {
        let args: Vec<String> = [
            "--pull-request",
            "7",
            "--head-sha",
            "abc",
            "--out-dir",
            out_dir,
        ]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
        parse_pr_comments_requests_options(&args)
            .map(|options| options.out_dir.display().to_string())
    }

    /// The run clears --out-dir, so only a dedicated directory under --root
    /// may be named.
    #[test]
    fn requests_out_dir_must_stay_inside_the_root() {
        assert_eq!(
            parse("target/ripr/review/publish"),
            Ok("target/ripr/review/publish".to_string())
        );
        assert_eq!(parse("./out"), Ok("./out".to_string()));
        for escaping in ["/tmp/publish", "../sibling", "target/../..", ".", "a\tb"] {
            assert!(parse(escaping).is_err(), "{escaping} was accepted");
        }
    }

    /// A `publish` symlink to a directory outside --root is refused before
    /// anything there is removed or written.
    #[cfg(unix)]
    #[test]
    fn requests_refuse_an_out_dir_that_links_outside_the_root() -> Result<(), String> {
        use std::fs;
        let base = std::env::temp_dir().join(format!(
            "ripr-pr-comments-link-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let root = base.join("root");
        let external = base.join("external");
        let review = root.join("target/ripr/review");
        fs::create_dir_all(&review).map_err(|err| err.to_string())?;
        fs::create_dir_all(&external).map_err(|err| err.to_string())?;
        fs::write(review.join("comment-publish-plan.json"), "{}").map_err(|err| err.to_string())?;
        let sentinel = external.join("requests.tsv");
        fs::write(&sentinel, "keep\n").map_err(|err| err.to_string())?;
        std::os::unix::fs::symlink(&external, review.join("publish"))
            .map_err(|err| err.to_string())?;

        let args: Vec<String> = [
            "--root",
            &root.display().to_string(),
            "--pull-request",
            "7",
            "--head-sha",
            "abc",
        ]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
        let result = pr_comments_requests(&args);
        let kept = fs::read_to_string(&sentinel).map_err(|err| err.to_string());
        let entries = fs::read_dir(&external).map(Iterator::count);
        let _ = fs::remove_dir_all(&base);

        let err = match result {
            Ok(()) => return Err("a linked --out-dir was accepted".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("cannot pass through a link"), "{err}");
        assert_eq!(kept?, "keep\n");
        assert_eq!(entries.map_err(|err| err.to_string())?, 1);
        Ok(())
    }

    /// A dangling `requests.tsv` link inside the request directory is
    /// removed, not written through.
    #[cfg(unix)]
    #[test]
    fn requests_replace_a_linked_manifest_instead_of_writing_through_it() -> Result<(), String> {
        use std::fs;
        let base = std::env::temp_dir().join(format!(
            "ripr-pr-comments-manifest-link-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let root = base.join("root");
        let outside = base.join("outside.tsv");
        let publish = root.join("target/ripr/review/publish");
        fs::create_dir_all(&publish).map_err(|err| err.to_string())?;
        fs::write(
            root.join("target/ripr/review/comment-publish-plan.json"),
            "{}",
        )
        .map_err(|err| err.to_string())?;
        std::os::unix::fs::symlink(&outside, publish.join("requests.tsv"))
            .map_err(|err| err.to_string())?;

        let args: Vec<String> = [
            "--root",
            &root.display().to_string(),
            "--pull-request",
            "7",
            "--head-sha",
            "abc",
        ]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
        let result = pr_comments_requests(&args);
        let leaked = outside.exists();
        let manifest_is_plain = fs::symlink_metadata(publish.join("requests.tsv"))
            .map(|metadata| metadata.file_type().is_file());
        let _ = fs::remove_dir_all(&base);

        result?;
        assert!(!leaked, "the manifest was written through the link");
        assert!(manifest_is_plain.map_err(|err| err.to_string())?);
        Ok(())
    }
}
