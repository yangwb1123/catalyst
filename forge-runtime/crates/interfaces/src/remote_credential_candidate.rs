//! Strict client boundary for the injected, metadata-only credential
//! lifecycle candidate.  The caller supplies the reviewed request image;
//! this module never derives owner data, mints material, or retries the POST.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde_json::Value;

use crate::remote_command::RemoteError;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote credential-candidate input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote credential-candidate input is invalid JSON".into()))?;
    crate::device_credential_candidate_command::validate_remote_request(&value)
        .map_err(|error| RemoteError(error.to_string()))?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI credential-candidate preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn validate_response(value: &Value, request: &Value) -> Result<(), RemoteError> {
    crate::device_credential_candidate_command::validate_remote_response(value, request)
        .map(|_| ())
        .map_err(|error| RemoteError(error.to_string()))
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> Result<(), RemoteError> {
    let output = crate::device_credential_candidate_command::decode_remote_response(value)
        .map_err(|error| RemoteError(error.to_string()))?;
    crate::device_credential_candidate_command::write_remote_output(&output, writer).map_err(
        |error| {
            RemoteError(format!(
                "credential-candidate response could not be rendered: {error}"
            ))
        },
    )
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote credential-candidate stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read remote credential-candidate input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "remote credential-candidate input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read remote credential-candidate input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote credential-candidate input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote credential-candidate input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
