pub(crate) fn pep440_version(native: &str) -> Result<String, String> {
    if native.contains('+') {
        return Err(
            "build metadata is not accepted because registry versions must map one-to-one"
                .to_string(),
        );
    }

    let (release, prerelease) = match native.split_once('-') {
        Some((release, prerelease)) => (release, Some(prerelease)),
        None => (native, None),
    };
    validate_release_triplet(release)?;

    let Some(prerelease) = prerelease else {
        return Ok(release.to_string());
    };
    let mut parts = prerelease.split('.');
    let label = parts
        .next()
        .ok_or_else(|| "missing prerelease label".to_string())?;
    let ordinal = parts
        .next()
        .ok_or_else(|| "prerelease must include a numeric ordinal".to_string())?;
    if parts.next().is_some() {
        return Err("prerelease must have exactly `<label>.<ordinal>`".to_string());
    }
    validate_numeric_identifier(ordinal, "prerelease ordinal")?;
    let pep440_label = match label {
        "alpha" => "a",
        "beta" => "b",
        "rc" => "rc",
        other => {
            return Err(format!(
                "unsupported prerelease label `{other}`; supported labels are alpha, beta, and rc"
            ));
        }
    };
    Ok(format!("{release}{pep440_label}{ordinal}"))
}

fn validate_release_triplet(release: &str) -> Result<(), String> {
    let mut parts = release.split('.');
    for field in ["major", "minor", "patch"] {
        let part = parts
            .next()
            .ok_or_else(|| "release must have exactly MAJOR.MINOR.PATCH".to_string())?;
        validate_numeric_identifier(part, field)?;
    }
    if parts.next().is_some() {
        return Err("release must have exactly MAJOR.MINOR.PATCH".to_string());
    }
    Ok(())
}

fn validate_numeric_identifier(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{field} must be an ASCII decimal integer"));
    }
    if value.len() > 1 && value.starts_with('0') {
        return Err(format!("{field} must not contain a leading zero"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_versions_map_deterministically() -> Result<(), String> {
        assert_eq!(pep440_version("0.11.0")?, "0.11.0");
        assert_eq!(pep440_version("0.11.0-alpha.1")?, "0.11.0a1");
        assert_eq!(pep440_version("0.11.0-beta.2")?, "0.11.0b2");
        assert_eq!(pep440_version("0.11.0-rc.3")?, "0.11.0rc3");
        Ok(())
    }

    #[test]
    fn unsupported_or_lossy_versions_are_rejected() {
        for version in [
            "0.11",
            "0.11.0-preview.1",
            "0.11.0-rc",
            "0.11.0-rc.01",
            "0.11.0+build.1",
        ] {
            assert!(pep440_version(version).is_err(), "accepted `{version}`");
        }
    }
}
