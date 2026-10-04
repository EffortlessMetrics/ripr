use super::contract::ProductContract;

pub(super) fn validate_single_binary(
    path: &str,
    text: &str,
    product: &ProductContract,
    violations: &mut Vec<String>,
) {
    let value: toml::Value = match toml::from_str(text) {
        Ok(value) => value,
        Err(_) => return,
    };
    let package = value.get("package");
    let autobins = package
        .and_then(|table| table.get("autobins"))
        .and_then(toml::Value::as_bool);
    if autobins != Some(false) {
        violations.push(format!(
            "{path}: package.autobins must be false so implicit src/bin targets cannot bypass the distribution contract"
        ));
    }

    let binaries = value
        .get("bin")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if binaries.len() != 1 {
        violations.push(format!(
            "{path}: distribution crate must declare exactly one [[bin]], found {}",
            binaries.len()
        ));
        return;
    }
    let binary = &binaries[0];
    let name = binary.get("name").and_then(toml::Value::as_str);
    let binary_path = binary.get("path").and_then(toml::Value::as_str);
    if name != Some(product.binary.as_str()) || binary_path != Some("src/main.rs") {
        violations.push(format!(
            "{path}: sole [[bin]] must be name = `{}` path = `src/main.rs`, got name = {:?} path = {:?}",
            product.binary, name, binary_path
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::distribution::{CONTRACT_PATH, parse_distribution_contract};

    const CONTRACT_TEXT: &str = include_str!("../../../../policy/distribution.toml");
    const CRATE_TEXT: &str = include_str!("../../../../crates/ripr/Cargo.toml");

    #[test]
    fn actual_crate_has_one_explicit_binary() -> Result<(), String> {
        let contract = parse_distribution_contract(CONTRACT_PATH, CONTRACT_TEXT)?;
        let mut violations = Vec::new();
        validate_single_binary(
            "crates/ripr/Cargo.toml",
            CRATE_TEXT,
            &contract.product,
            &mut violations,
        );
        assert!(violations.is_empty(), "{violations:#?}");
        Ok(())
    }

    #[test]
    fn extra_binary_and_autodiscovery_are_rejected() -> Result<(), String> {
        let contract = parse_distribution_contract(CONTRACT_PATH, CONTRACT_TEXT)?;
        let mutated = CRATE_TEXT
            .replace("autobins = false\n", "")
            .replace(
                "[[bin]]\nname = \"ripr\"\npath = \"src/main.rs\"",
                "[[bin]]\nname = \"ripr\"\npath = \"src/main.rs\"\n\n[[bin]]\nname = \"helper\"\npath = \"src/bin/helper.rs\"",
            );
        let mut violations = Vec::new();
        validate_single_binary(
            "crates/ripr/Cargo.toml",
            &mutated,
            &contract.product,
            &mut violations,
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("package.autobins must be false"))
        );
        assert!(
            violations.iter().any(|violation| {
                violation.contains("must declare exactly one [[bin]], found 2")
            })
        );
        Ok(())
    }
}
