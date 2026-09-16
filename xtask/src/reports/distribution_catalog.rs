//! Release distribution catalog producer (`#1682` packaging half of PR A).
//!
//! The catalog is the installed extension's release authority: it binds one
//! product version and server generation to its allowed public placements
//! plus the exact manifest digest the downloader must observe. The tracked
//! `editors/vscode/distribution.json` stays a development template; this
//! producer emits one candidate catalog outside tracked source for release
//! packaging to stage.

use std::fs;
use std::path::Path;

use super::release_server::{normalize_product_version, required_release_arg, sha256_bytes};

const MAX_MANIFEST_BYTES: u64 = 1_048_576;

pub(crate) fn release_distribution_catalog(args: &[String]) -> Result<(), String> {
    let product_version = normalize_product_version(&required_release_arg(
        args,
        "product-version",
        "PRODUCT_VERSION",
    )?)?;
    let channel = required_release_arg(args, "channel", "DISTRIBUTION_CHANNEL")?;
    if channel != "stable" && channel != "rc" {
        return Err(format!(
            "unsupported distribution channel `{channel}`; expected `stable` or `rc`"
        ));
    }
    let stable_tag = required_release_arg(args, "stable-tag", "STABLE_TAG")?;
    let rc_tag = optional_release_arg(args, "rc-tag", "RC_TAG");
    let manifest_path = required_release_arg(args, "manifest", "DISTRIBUTION_MANIFEST")?;
    let repository = required_release_arg(args, "repository", "REPOSITORY")?;
    let out = optional_release_arg(args, "out", "DISTRIBUTION_CATALOG_OUT")
        .unwrap_or_else(|| "dist/ripr-distribution-catalog.json".to_string());

    let expected_stable_tag = format!("v{product_version}");
    if stable_tag != expected_stable_tag {
        return Err(format!(
            "stable placement `{stable_tag}` must equal the product tag `{expected_stable_tag}`"
        ));
    }
    let (release_tag, release_ref, fallback_placements) = if channel == "stable" {
        let mut fallbacks = Vec::new();
        if let Some(rc_tag) = rc_tag {
            fallbacks.push(rc_placement(&product_version, &rc_tag)?);
        }
        (
            stable_tag.clone(),
            format!("refs/tags/{stable_tag}"),
            fallbacks,
        )
    } else {
        let rc_tag = rc_tag.ok_or_else(|| {
            "an RC catalog requires --rc-tag as the preferred placement".to_string()
        })?;
        let preferred = rc_placement(&product_version, &rc_tag)?;
        (
            preferred.clone(),
            format!("refs/tags/{preferred}"),
            Vec::new(),
        )
    };

    let manifest_bytes = read_bounded_manifest(&manifest_path)?;
    let manifest_sha256 = sha256_bytes(&manifest_bytes);
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|err| format!("release manifest `{manifest_path}` is not valid JSON: {err}"))?;
    let manifest_object = manifest
        .as_object()
        .ok_or_else(|| format!("release manifest `{manifest_path}` must be a JSON object"))?;
    let manifest_version = manifest_object
        .get("product_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!("release manifest `{manifest_path}` has no product_version string")
        })?;
    if manifest_version != product_version {
        return Err(format!(
            "release manifest product `{manifest_version}` does not match catalog product `{product_version}`"
        ));
    }
    if manifest_object
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some("2")
    {
        return Err(format!(
            "release manifest `{manifest_path}` must use placement-independent schema 2"
        ));
    }
    let distribution_generation = digest_field(manifest_object, "distribution_generation")?;
    let target_set_digest = manifest_object
        .get("target_set")
        .and_then(serde_json::Value::as_object)
        .map(|target_set| digest_field(target_set, "digest"))
        .transpose()?
        .ok_or_else(|| {
            format!("release manifest `{manifest_path}` has no target_set.digest string")
        })?;
    let manifest_file_name = Path::new(&manifest_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("release manifest path `{manifest_path}` has no file name"))?;
    let expected_manifest_file = format!("ripr-server-manifest-v{product_version}.json");
    if manifest_file_name != expected_manifest_file {
        return Err(format!(
            "release manifest file `{manifest_file_name}` must equal `{expected_manifest_file}`"
        ));
    }
    let manifest_repository = manifest_object
        .get("source_repository")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!("release manifest `{manifest_path}` has no source_repository string")
        })?;
    let source_repository = normalize_source_repository(&repository)?;
    if normalize_manifest_repository(manifest_repository)? != source_repository.owner_repo {
        return Err(format!(
            "catalog repository `{repository}` does not match manifest source `{manifest_repository}`"
        ));
    }

    let catalog = serde_json::json!({
        "schema": 2,
        "productVersion": product_version,
        "channel": channel,
        "releaseTag": release_tag,
        "releaseRef": release_ref,
        "fallbackPlacements": fallback_placements.iter().map(|placement| {
            serde_json::json!({
                "channel": "rc",
                "releaseTag": placement,
                "releaseRef": format!("refs/tags/{placement}"),
            })
        }).collect::<Vec<_>>(),
        "manifestFile": expected_manifest_file,
        "sourceRepository": source_repository.url,
        "distributionGeneration": distribution_generation,
        "manifestSha256": manifest_sha256,
        "targetSetDigest": target_set_digest,
        "producer": {
            "tool": "xtask release-distribution-catalog",
            "schema": "distribution-catalog/1",
        },
    });
    let catalog_text = serde_json::to_string_pretty(&catalog)
        .map_err(|err| format!("failed to render distribution catalog: {err}"))?;
    let catalog_text = format!("{catalog_text}\n");
    let out_path = Path::new(&out);
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(out_path, &catalog_text)
        .map_err(|err| format!("failed to write {}: {err}", out_path.display()))?;
    eprintln!(
        "wrote {} generation={distribution_generation} manifest_sha256={manifest_sha256}",
        out_path.display(),
    );
    Ok(())
}

fn rc_placement(product_version: &str, rc_tag: &str) -> Result<String, String> {
    if rc_tag.is_empty() {
        return Err("RC placement tag must not be empty".to_string());
    }
    let suffix = rc_tag
        .strip_prefix(&format!("v{product_version}-rc."))
        .ok_or_else(|| {
            format!("RC placement `{rc_tag}` must tag product `{product_version}` as `v{product_version}-rc.N`")
        })?;
    if suffix != "0" && (suffix.starts_with('0') || !suffix.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!(
            "RC placement `{rc_tag}` carries a non-canonical RC number"
        ));
    }
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "RC placement `{rc_tag}` carries a non-canonical RC number"
        ));
    }
    Ok(rc_tag.to_string())
}

fn digest_field(
    object: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<String, String> {
    let digest = object
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("release manifest has no `{name}` string"))?;
    if digest.len() != 64
        || !digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(format!(
            "release manifest field `{name}` must be a 64-character lowercase hex digest"
        ));
    }
    Ok(digest.to_string())
}

fn read_bounded_manifest(path: &str) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("release manifest `{path}` is unavailable: {err}"))?;
    if !metadata.file_type().is_file() {
        return Err(format!("release manifest `{path}` is not a regular file"));
    }
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "release manifest `{path}` exceeds the {MAX_MANIFEST_BYTES}-byte bound"
        ));
    }
    fs::read(path).map_err(|err| format!("failed to read release manifest `{path}`: {err}"))
}

struct NormalizedRepository {
    url: String,
    owner_repo: String,
}

fn normalize_source_repository(repository: &str) -> Result<NormalizedRepository, String> {
    let repository = repository.trim();
    if let Some(rest) = repository.strip_prefix("https://") {
        let rest = rest.trim_end_matches('/');
        if rest.contains(['?', '#', '@', ' ', '\t', '\n']) {
            return Err(format!(
                "source repository `{repository}` must not carry credentials, query, or fragment"
            ));
        }
        let mut segments = rest.split('/');
        let host = segments.next().unwrap_or_default();
        let owner = segments.next().unwrap_or_default();
        let name = segments.next().unwrap_or_default();
        if host.is_empty() || owner.is_empty() || name.is_empty() || segments.next().is_some() {
            return Err(format!(
                "source repository `{repository}` must be an HTTPS owner/repo URL"
            ));
        }
        if rest.contains(':') {
            return Err(format!(
                "source repository `{repository}` must not carry a port or scheme suffix"
            ));
        }
        return Ok(NormalizedRepository {
            url: format!("https://{host}/{owner}/{name}"),
            owner_repo: format!("{owner}/{name}"),
        });
    }
    let mut segments = repository.split('/');
    let owner = segments.next().unwrap_or_default();
    let name = segments.next().unwrap_or_default();
    if owner.is_empty() || name.is_empty() || segments.next().is_some() {
        return Err(format!(
            "source repository `{repository}` must be `owner/repo` or an HTTPS owner/repo URL"
        ));
    }
    Ok(NormalizedRepository {
        url: format!("https://github.com/{owner}/{name}"),
        owner_repo: format!("{owner}/{name}"),
    })
}

fn normalize_manifest_repository(manifest_repository: &str) -> Result<String, String> {
    normalize_source_repository(manifest_repository).map(|normalized| normalized.owner_repo)
}

fn optional_release_arg(args: &[String], flag: &str, env_name: &str) -> Option<String> {
    let flag_name = format!("--{flag}");
    for window in args.windows(2) {
        if window[0] == flag_name {
            return Some(window[1].clone());
        }
    }
    let inline_prefix = format!("{flag_name}=");
    for arg in args {
        if let Some(value) = arg.strip_prefix(&inline_prefix) {
            return Some(value.to_string());
        }
    }
    std::env::var(env_name).ok()
}
