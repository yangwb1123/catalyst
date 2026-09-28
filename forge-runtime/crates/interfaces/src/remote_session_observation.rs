use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::Path,
};

use serde_json::{Map, Value};

use super::{RemoteError, placement, validation};

#[path = "remote_session_observation_shape.rs"]
mod shape;

#[path = "remote_session_observation/render.rs"]
mod render;
#[path = "remote_session_observation/request.rs"]
mod request;
#[path = "remote_session_observation/response.rs"]
mod response;

pub(super) use render::render_human;
pub(super) use request::validate_request;
pub(super) use response::validate_response;

type CandidateDevices = BTreeMap<String, (String, Value)>;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEVICES: usize = 128;

/// Reads the caller supplied request used by the authenticated session
/// observation route. The request contains no credentials, leases, or target
/// selection; it is only a bounded declaration for a pure preview.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote session observation input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI session observation preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

fn has_exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.iter().all(|field| object.contains_key(*field))
}

fn string_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RemoteError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError(format!("remote session observation {field} is invalid")))
}

fn validate_owner(value: &Value) -> Result<(), RemoteError> {
    let object = value
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation owner is invalid".into()))?;
    if !has_exact_fields(object, ["issuer", "subject", "tenant_id"])
        || ["issuer", "subject", "tenant_id"]
            .iter()
            .any(|field| object[*field].as_str().is_none_or(str::is_empty))
    {
        return Err(RemoteError(
            "remote session observation owner is invalid".into(),
        ));
    }
    Ok(())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote session observation stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read remote session observation input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "remote session observation input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read remote session observation input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote session observation input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote session observation input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
