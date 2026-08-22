use std::net::{Ipv4Addr, Ipv6Addr};

use super::{CapabilityGrantContractError, HostKind, invalid};

pub(super) fn text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(forbidden_scalar) {
        Err(invalid(format!("{label} violates the text contract")))
    } else {
        Ok(())
    }
}

pub(super) fn optional_text(
    value: Option<&str>,
    maximum: usize,
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    if let Some(value) = value {
        text(value, maximum, label)?;
    }
    Ok(())
}

pub(super) fn sha256(value: &str, label: &str) -> Result<(), CapabilityGrantContractError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        Err(invalid(format!("{label} is not lowercase SHA-256 hex")))
    } else {
        Ok(())
    }
}

pub(super) fn optional_sha256(
    value: Option<&str>,
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    if let Some(value) = value {
        sha256(value, label)?;
    }
    Ok(())
}

pub(super) fn integer(
    value: i64,
    minimum: i64,
    maximum: i64,
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(invalid(format!("{label} is outside its admitted range")))
    }
}

pub(super) fn base64url(value: &str) -> Result<(), CapabilityGrantContractError> {
    if value.len() < 16
        || value.len() > 16_384
        || value.len() % 4 == 1
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !canonical_base64url_tail(value)
    {
        Err(invalid("proof_base64url is not unpadded base64url text"))
    } else {
        Ok(())
    }
}

fn canonical_base64url_tail(value: &str) -> bool {
    let Some(last) = value.bytes().last().and_then(base64url_value) else {
        return false;
    };
    match value.len() % 4 {
        2 => last.trailing_zeros() >= 4,
        3 => last.trailing_zeros() >= 2,
        _ => true,
    }
}

fn base64url_value(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

pub(super) fn canonical_path(
    value: &str,
    allow_root: bool,
) -> Result<(), CapabilityGrantContractError> {
    text(value, 4_096, "repository path")?;
    if value == "." {
        return if allow_root {
            Ok(())
        } else {
            Err(invalid("repository path cannot name the root"))
        };
    }
    if value.starts_with('/')
        || value.ends_with('/')
        || value.contains(['\\', '*', '?', '[', ']', '{', '}'])
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(invalid(
            "repository path is not canonical repo-relative text",
        ));
    }
    Ok(())
}

pub(super) fn host(value: &str, kind: HostKind) -> Result<(), CapabilityGrantContractError> {
    text(value, 253, "network host")?;
    match kind {
        HostKind::Ipv4 => require_canonical_ip::<Ipv4Addr>(value, "IPv4"),
        HostKind::Ipv6 => require_canonical_ipv6(value),
        HostKind::Dns => validate_dns(value),
    }
}

fn require_canonical_ipv6(value: &str) -> Result<(), CapabilityGrantContractError> {
    let parsed = value
        .parse::<Ipv6Addr>()
        .map_err(|_| invalid("network host is not IPv6"))?;
    if parsed.to_ipv4_mapped().is_some() {
        return Err(invalid("IPv4-mapped IPv6 network hosts are forbidden"));
    }
    if parsed.to_string() == value {
        Ok(())
    } else {
        Err(invalid("network host is not canonical IPv6"))
    }
}

fn require_canonical_ip<T>(value: &str, label: &str) -> Result<(), CapabilityGrantContractError>
where
    T: std::str::FromStr + ToString,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| invalid(format!("network host is not {label}")))?;
    if parsed.to_string() == value {
        Ok(())
    } else {
        Err(invalid(format!("network host is not canonical {label}")))
    }
}

fn validate_dns(value: &str) -> Result<(), CapabilityGrantContractError> {
    if is_canonical_ipv4(value) {
        return Err(invalid("canonical IPv4 hosts must use host_kind ipv4"));
    }
    if value != value.to_ascii_lowercase() || value.ends_with('.') || value.contains('*') {
        return Err(invalid("network host is not canonical DNS"));
    }
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(invalid("network host is not canonical DNS"));
        }
    }
    Ok(())
}

fn is_canonical_ipv4(value: &str) -> bool {
    value
        .parse::<Ipv4Addr>()
        .is_ok_and(|parsed| parsed.to_string() == value)
}

fn forbidden_scalar(value: char) -> bool {
    matches!(value, '\u{0000}'..='\u{001f}' | '\u{007f}')
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}
