use serde_json::Value;

const SECRET_TRIPWIRES: [&str; 18] = [
    "api_key",
    "apikey",
    "api_token",
    "access_token",
    "auth_token",
    "password",
    "passwd",
    "secret",
    "credential",
    "bearer ",
    "private_key",
    "begin rsa private",
    "begin private key",
    "ghp_",
    "gho_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
];

pub(super) fn check_git_sha(field: &str, value: &str) -> Result<(), String> {
    let ok = value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{field} must be a bare lowercase 40-character git SHA"
        ))
    }
}

pub(super) fn check_sha256_digest(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{field} must use the sha256:<hex> digest form"));
    };
    let ok = hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{field} must be sha256: followed by 64 lowercase hex characters"
        ))
    }
}

pub(super) fn reject_secret_tokens(value: &Value, field: &str) -> Result<(), String> {
    match value {
        Value::String(text) => reject_secret_text(field, text),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                reject_secret_tokens(item, &format!("{field}[{index}]"))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, item) in map {
                reject_secret_tokens(item, &format!("{field}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn reject_secret_text(field: &str, text: &str) -> Result<(), String> {
    let lowered = text.to_ascii_lowercase();
    for tripwire in SECRET_TRIPWIRES {
        if lowered.contains(tripwire) {
            return Err(format!(
                "{field} must not carry credential or secret material"
            ));
        }
    }
    Ok(())
}

pub(super) fn require_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must be nonempty"))
    } else {
        Ok(())
    }
}
