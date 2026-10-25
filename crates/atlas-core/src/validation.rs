use crate::{AtlasError, ErrorCode};
use chrono::{DateTime, FixedOffset, Utc};
use std::net::IpAddr;

pub fn text(value: &str, limit: usize, allow_empty: bool) -> Result<String, AtlasError> {
    if value.chars().count() > limit || value.chars().any(|c| c < '\u{20}') {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    let result = value.trim();
    if !allow_empty && result.is_empty() {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    Ok(result.to_owned())
}

pub fn hostname(value: &str) -> Result<String, AtlasError> {
    let value = text(value, 253, false)?;
    if value.parse::<IpAddr>().is_ok() {
        return Ok(value);
    }
    // Scoped IPv6 addresses are accepted without permitting SSH option syntax.
    if let Some((address, scope)) = value.split_once('%') {
        if address.parse::<std::net::Ipv6Addr>().is_ok()
            && !scope.is_empty()
            && scope
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))
        {
            return Ok(value);
        }
    }
    let valid = value.trim_end_matches('.').split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.as_bytes()[0].is_ascii_alphanumeric()
            && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
            && label
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    });
    if !valid {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    Ok(value)
}

pub fn coordinate(value: f64) -> Result<f64, AtlasError> {
    if !value.is_finite() || value.abs() > 10_000.0 {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    Ok(value)
}

pub fn observation_time(
    value: &str,
    now: DateTime<Utc>,
) -> Result<DateTime<FixedOffset>, AtlasError> {
    let value = text(value, 64, false)?;
    let parsed = DateTime::parse_from_rfc3339(&value)
        .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
    if parsed > now {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    Ok(parsed)
}
