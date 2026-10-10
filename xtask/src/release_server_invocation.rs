use std::path::Path;

use crate::reports::release::read_crate_version;
use crate::reports::release_server;

#[derive(Debug, Eq, PartialEq)]
struct ServerIdentity {
    product_version: String,
    placement_version: String,
}

impl ServerIdentity {
    // Identity binding only. This is not release-event or publication authority.
    // #1607/#1631 and #1644/#1646 still own those unfinished contracts.
    fn from_source(product: &str, placement: &str) -> Result<Self, String> {
        let product_version = release_server::normalize_product_version(product)?;
        let placement_version = release_server::normalize_release_version(placement);
        if placement != placement_version && placement != format!("v{placement_version}") {
            return Err(format!("release placement `{placement}` is not canonical"));
        }
        if placement_version != product_version {
            let prefix = format!("{product_version}-rc.");
            let number = placement_version.strip_prefix(&prefix).ok_or_else(|| {
                format!("release placement `{placement}` must name source product `{product_version}` or its canonical rc.N placement")
            })?;
            if number.is_empty()
                || number.starts_with('0')
                || !number.chars().all(|character| character.is_ascii_digit())
            {
                return Err(format!(
                    "release placement `{placement}` must carry a positive canonical RC number"
                ));
            }
        }
        Ok(Self {
            product_version,
            placement_version,
        })
    }
}

fn producer_args(args: &[String]) -> Result<Vec<String>, String> {
    let mut placement = None;
    let mut forwarded = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        let value = if argument == "--release-version" {
            index += 1;
            Some(
                args.get(index)
                    .ok_or_else(|| {
                        "--release-version requires a placement version or tag".to_string()
                    })?
                    .clone(),
            )
        } else {
            argument
                .strip_prefix("--release-version=")
                .map(str::to_string)
        };
        if let Some(value) = value {
            if placement.replace(value).is_some() {
                return Err("--release-version must be supplied exactly once".to_string());
            }
        } else {
            forwarded.push(argument.clone());
        }
        index += 1;
    }
    let Some(placement) = placement else {
        // Preserve existing explicit product/RAW_VERSION producer invocations.
        return Ok(forwarded);
    };
    if forwarded
        .iter()
        .any(|argument| argument == "--version" || argument.starts_with("--version="))
    {
        return Err("--release-version and --version are mutually exclusive".to_string());
    }
    let product = read_crate_version(Path::new("crates/ripr/Cargo.toml"), Path::new("Cargo.toml"))
        .ok_or_else(|| "cannot read the checked-out ripr source product version".to_string())?;
    let identity = ServerIdentity::from_source(&product, &placement)?;
    eprintln!(
        "server subject product={} placement={} (identity binding only)",
        identity.product_version, identity.placement_version
    );
    forwarded.extend(["--version".to_string(), identity.product_version]);
    Ok(forwarded)
}

pub(super) fn archive(args: &[String]) -> Result<(), String> {
    release_server::release_server_archive(&producer_args(args)?)
}

pub(super) fn manifest(args: &[String]) -> Result<(), String> {
    release_server::release_server_manifest(&producer_args(args)?)
}

pub(super) fn upload(args: &[String]) -> Result<(), String> {
    let placement = release_server::required_release_arg(args, "version", "RAW_VERSION")?;
    // Do not make the legacy opportunistic create/clobber uploader reachable
    // for RCs as a side effect of repairing the nonpublishing producers.
    // Exact RC authorization and transport remain #1631/#1644/#1646 work.
    release_server::normalize_product_version(&placement).map_err(|error| {
        format!("legacy server upload cannot publish RC placement; exact RC authorization/transport is required: {error}")
    })?;
    release_server::release_upload_assets(args)
}

#[cfg(test)]
#[path = "release_server_invocation_tests.rs"]
mod tests;
