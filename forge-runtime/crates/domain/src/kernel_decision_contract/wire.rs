use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{KernelDecisionContractError, invalid};

pub(super) fn canonical_with_max<T>(
    value: &T,
    maximum: usize,
) -> Result<String, KernelDecisionContractError>
where
    T: Serialize + ?Sized,
{
    let node = serde_json::to_value(value)
        .map_err(|error| invalid(format!("canonical JSON failed: {error}")))?;
    reject_control_scalars(&node)?;
    let canonical = crate::governance_contract::codec::canonical_json(value)
        .map_err(|error| invalid(format!("canonical JSON failed: {}", error.message)))?;
    if canonical.is_empty() || canonical.len() > maximum {
        Err(invalid(format!(
            "canonical JSON byte length must be 1..={maximum}"
        )))
    } else {
        Ok(canonical)
    }
}

fn reject_control_scalars(value: &Value) -> Result<(), KernelDecisionContractError> {
    match value {
        Value::String(text) if text.chars().any(char::is_control) => Err(invalid(
            "canonical JSON string contains a forbidden Unicode scalar",
        )),
        Value::Array(items) => items.iter().try_for_each(reject_control_scalars),
        Value::Object(fields) => {
            for (key, member) in fields {
                if key.chars().any(char::is_control) {
                    return Err(invalid(
                        "canonical JSON string contains a forbidden Unicode scalar",
                    ));
                }
                reject_control_scalars(member)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn decode_typed<T>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, KernelDecisionContractError>
where
    T: DeserializeOwned + Serialize,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!("JSON byte length must be 1..={maximum}")));
    }
    let typed: T = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid typed JSON: {error}")))?;
    let canonical = canonical_with_max(&typed, maximum)?;
    if canonical.as_bytes() != bytes {
        return Err(invalid("input is not exact compact canonical JSON"));
    }
    Ok(typed)
}

/// Returns exact compact canonical JSON under the outer closure ceiling.
///
/// # Errors
///
/// Returns an error for non-int64 JSON values, forbidden scalars, excessive nesting, or size.
pub fn canonical_json<T>(value: &T) -> Result<String, KernelDecisionContractError>
where
    T: Serialize + ?Sized,
{
    canonical_with_max(value, super::MAX_CLOSURE_BYTES)
}
