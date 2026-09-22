use super::{API_VERSION, MAX_REQUEST_ID_BYTES, WRITE_API_VERSION};

pub(crate) fn validate_header(
    api_version: &str,
    request_id: &str,
) -> Result<(), (String, &'static str)> {
    validate_header_for_version(api_version, request_id, API_VERSION)
}

pub(crate) fn validate_write_header(
    api_version: &str,
    request_id: &str,
) -> Result<(), (String, &'static str)> {
    validate_header_for_version(api_version, request_id, WRITE_API_VERSION)
}

fn validate_header_for_version(
    api_version: &str,
    request_id: &str,
    expected_version: &str,
) -> Result<(), (String, &'static str)> {
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_BYTES
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(("invalid".into(), "invalid_request_id"));
    }
    if api_version != expected_version {
        return Err((request_id.to_owned(), "unsupported_version"));
    }
    Ok(())
}
