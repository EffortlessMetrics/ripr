//! Shared finite native-limit observation. Profile data confers no analysis authority.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::time::Instant;

const ADDRESS_SPACE_MAX: u64 = 2 * 1024 * 1024 * 1024;
const FILE_MAX: u64 = 256 * 1024 * 1024;
#[cfg(target_os = "linux")]
const LIMITS_MAX: u64 = 16 * 1024;

#[cfg(any(target_os = "linux", test))]
pub(crate) fn limits(text: &str, name: &str) -> Result<(u64, u64), String> {
    let mut rows = text.lines().filter_map(|line| line.strip_prefix(name));
    let row = rows
        .next()
        .ok_or_else(|| format!("experimental worker missing {name}"))?;
    if rows.next().is_some() {
        return Err(format!("experimental worker duplicate {name}"));
    }
    let values: Vec<_> = row.split_whitespace().collect();
    if values.len() != 3 || values[2] != "bytes" {
        return Err(format!("experimental worker malformed {name}"));
    }
    let number = |value: &str| {
        value
            .parse::<u64>()
            .map_err(|error| format!("experimental worker nonfinite {name}: {error}"))
    };
    Ok((number(values[0])?, number(values[1])?))
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn require_limits(text: &str, address_space: u64, file: u64) -> Result<(), String> {
    for (name, expected, maximum) in [
        ("Max address space", address_space, ADDRESS_SPACE_MAX),
        ("Max file size", file, FILE_MAX),
    ] {
        if expected == 0 || expected > maximum || limits(text, name)? != (expected, expected) {
            return Err(format!(
                "experimental worker {name} is not the requested finite soft/hard limit"
            ));
        }
    }
    if limits(text, "Max core file size")? != (0, 0) {
        return Err("experimental worker core-file limit is not zero".to_string());
    }
    Ok(())
}

pub(crate) fn verify_limits(address_space: u64, file: u64) -> Result<(), String> {
    if address_space == 0 || address_space > ADDRESS_SPACE_MAX || file == 0 || file > FILE_MAX {
        return Err("experimental worker invalid finite resource profile".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        let mut text = String::new();
        fs::File::open("/proc/self/limits")
            .map_err(|error| {
                format!("experimental worker resource verification unavailable: {error}")
            })?
            .take(LIMITS_MAX + 1)
            .read_to_string(&mut text)
            .map_err(|error| {
                format!("experimental worker resource verification failed: {error}")
            })?;
        if text.len() as u64 > LIMITS_MAX {
            return Err(
                "experimental worker resource verification exceeds its byte bound".to_string(),
            );
        }
        require_limits(&text, address_space, file)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (address_space, file);
        Err("experimental complete-execution requires qualified Linux resource limits".to_string())
    }
}

/// Read-only actual finite resource observations, not an execution capability.
/// The creating observation and this data do not settle descendants or admit analysis.
#[derive(Debug, PartialEq, Eq)]
pub struct NativeLimitTuple {
    address_space_bytes: u64,
    file_size_bytes: u64,
}

impl NativeLimitTuple {
    pub fn address_space_bytes(&self) -> u64 {
        self.address_space_bytes
    }

    pub fn file_size_bytes(&self) -> u64 {
        self.file_size_bytes
    }

    /// Observe this process under the caller's original cooperative clock.
    pub fn observe(held_deadline: Instant) -> Result<Self, String> {
        observe_native_limits(held_deadline)
    }
}

fn time(held_deadline: Instant) -> Result<(), String> {
    if Instant::now() >= held_deadline {
        Err("native resource observation held deadline exceeded".to_string())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn read_limits_with_deadline(held_deadline: Instant) -> Result<String, String> {
    time(held_deadline)?;
    let mut text = String::new();
    let file = fs::File::open("/proc/self/limits").map_err(|error| {
        format!("experimental worker resource verification unavailable: {error}")
    })?;
    time(held_deadline)?;
    file.take(LIMITS_MAX + 1)
        .read_to_string(&mut text)
        .map_err(|error| format!("experimental worker resource verification failed: {error}"))?;
    time(held_deadline)?;
    if text.len() as u64 > LIMITS_MAX {
        return Err("experimental worker resource verification exceeds its byte bound".to_string());
    }
    // read_to_string completed at actual EOF below the cap; reaching cap+1 refuses.
    Ok(text)
}

#[cfg(any(target_os = "linux", test))]
fn observed_tuple(text: &str) -> Result<NativeLimitTuple, String> {
    let address_space = limits(text, "Max address space")?;
    // Check the first limit before reading the next row, preserving refusal order.
    if address_space.0 == 0
        || address_space.0 > ADDRESS_SPACE_MAX
        || address_space.0 != address_space.1
    {
        return Err(
            "experimental worker Max address space is not the requested finite soft/hard limit"
                .to_string(),
        );
    }
    let file = limits(text, "Max file size")?;
    require_limits(text, address_space.0, file.0)?;
    Ok(NativeLimitTuple {
        address_space_bytes: address_space.0,
        file_size_bytes: file.0,
    })
}

pub(crate) fn observe_native_limits(held_deadline: Instant) -> Result<NativeLimitTuple, String> {
    time(held_deadline)?;
    #[cfg(target_os = "linux")]
    {
        let text = read_limits_with_deadline(held_deadline)?;
        let observed = observed_tuple(&text)?;
        time(held_deadline)?;
        Ok(observed)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("experimental complete-execution requires qualified Linux resource limits".to_string())
    }
}

pub(crate) fn verify_limits_with_deadline(
    address_space: u64,
    file: u64,
    held_deadline: Instant,
) -> Result<(), String> {
    time(held_deadline)?;
    if address_space == 0 || address_space > ADDRESS_SPACE_MAX || file == 0 || file > FILE_MAX {
        return Err("experimental worker invalid finite resource profile".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        let text = read_limits_with_deadline(held_deadline)?;
        require_limits(&text, address_space, file)?;
        time(held_deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("experimental complete-execution requires qualified Linux resource limits".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_verification_refuses_missing_nonfinite_duplicate_and_mismatched_limits()
    -> Result<(), String> {
        let valid = "Max address space         1073741824 1073741824 bytes\nMax file size             16777216 16777216 bytes\nMax core file size        0 0 bytes\n";
        require_limits(valid, 1073741824, 16777216)?;
        for invalid in [
            valid.replace("1073741824 1073741824", "unlimited unlimited"),
            valid.replace("1073741824 1073741824", "1073741824 2147483648"),
            valid.replace("Max address space", "Max unknown"),
            format!("{valid}Max address space 1073741824 1073741824 bytes\n"),
            valid.replace("0 0 bytes", "0 1 bytes"),
        ] {
            let Err(error) = require_limits(&invalid, 1073741824, 16777216) else {
                return Err("invalid native resource limits were accepted".to_string());
            };
            assert!(error.starts_with("experimental worker"), "{error}");
        }
        for address_space in [0, ADDRESS_SPACE_MAX + 1] {
            let Err(error) = require_limits(valid, address_space, 16777216) else {
                return Err("invalid address-space profile was accepted".to_string());
            };
            assert!(
                error.contains("not the requested finite soft/hard limit"),
                "{error}"
            );
        }
        Ok(())
    }

    #[test]
    fn observation_uses_the_existing_parser_and_preserves_lower_finite_limits() -> Result<(), String>
    {
        let text = "Max address space 134217728 134217728 bytes\nMax file size 16777216 16777216 bytes\nMax core file size 0 0 bytes\n";
        let observed = observed_tuple(text)?;
        assert_eq!(observed.address_space_bytes(), 134217728);
        assert_eq!(observed.file_size_bytes(), 16777216);
        require_limits(
            text,
            observed.address_space_bytes(),
            observed.file_size_bytes(),
        )?;
        Ok(())
    }

    #[test]
    fn observation_refuses_unlimited_zero_unequal_and_out_of_bounds_profiles() -> Result<(), String>
    {
        let valid = "Max address space 134217728 134217728 bytes\nMax file size 16777216 16777216 bytes\nMax core file size 0 0 bytes\n";
        for text in [
            valid.replace("134217728 134217728", "unlimited unlimited"),
            valid.replace("134217728 134217728", "0 0"),
            valid.replace("134217728 134217728", "134217728 268435456"),
            valid.replace("134217728 134217728", "2147483649 2147483649"),
            valid.replace("16777216 16777216", "unlimited unlimited"),
            valid.replace("16777216 16777216", "0 0"),
            valid.replace("16777216 16777216", "16777216 33554432"),
            valid.replace("16777216 16777216", "268435457 268435457"),
            valid.replace("0 0 bytes", "0 1 bytes"),
        ] {
            let Err(error) = observed_tuple(&text) else {
                return Err(
                    "nonfinite or mismatched profile produced native tuple data".to_string()
                );
            };
            assert!(error.starts_with("experimental worker"), "{error}");
        }
        Ok(())
    }

    #[test]
    fn observation_refuses_first_invalid_address_space_before_later_malformed_file()
    -> Result<(), String> {
        let text =
            "Max address space 0 0 bytes\nMax file size malformed\nMax core file size 0 0 bytes\n";
        let Err(error) = observed_tuple(text) else {
            return Err("competing invalid native limits were admitted".to_string());
        };
        assert!(
            error.contains("Max address space is not the requested"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn expired_native_observation_refuses_before_profile_or_filesystem() -> Result<(), String> {
        let deadline = Instant::now();
        for result in [
            observe_native_limits(deadline).map(|_| ()),
            verify_limits_with_deadline(0, 0, deadline),
        ] {
            let Err(error) = result else {
                return Err("expired native observation was admitted".to_string());
            };
            assert_eq!(error, "native resource observation held deadline exceeded");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn actual_self_limits_use_the_same_bounded_observer_and_parser() -> Result<(), String> {
        let deadline = Instant::now()
            .checked_add(std::time::Duration::from_secs(5))
            .ok_or("native control clock overflow")?;
        let text = read_limits_with_deadline(deadline)?;
        if text.is_empty() {
            return Err("actual native limit control has no subject".to_string());
        }
        match (observed_tuple(&text), observe_native_limits(deadline)) {
            (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
            (Err(expected), Err(actual)) => assert_eq!(actual, expected),
            (Ok(_), Err(error)) => {
                return Err(format!("actual native observation disagreed: {error}"));
            }
            (Err(error), Ok(_)) => {
                return Err(format!("invalid actual native profile admitted: {error}"));
            }
        }
        Ok(())
    }
}
