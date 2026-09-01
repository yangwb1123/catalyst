use std::{
    fmt,
    io::{self, Write},
};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};

use super::{
    MAX_ARRAY_ITEMS, MAX_JSON_DEPTH, MAX_OBJECT_FIELDS, MAX_STRING_BYTES,
    PlatformCoreContractError, invalid,
};

pub(super) fn decode_typed<T>(bytes: &[u8], maximum: usize) -> Result<T, PlatformCoreContractError>
where
    T: DeserializeOwned + Serialize,
{
    let value = decode_strict_value(bytes, maximum)?;
    let canonical = canonical_value(&value, maximum)?;
    if canonical.as_bytes() != bytes {
        return Err(invalid("input is not exact compact canonical JSON"));
    }
    let typed = serde_json::from_value(value)
        .map_err(|error| invalid(format!("typed JSON decode failed: {error}")))?;
    if canonical_typed(&typed, maximum)?.as_bytes() != bytes {
        return Err(invalid("input is missing required exact fields"));
    }
    Ok(typed)
}

pub(super) fn canonical_typed<T>(
    value: &T,
    maximum: usize,
) -> Result<String, PlatformCoreContractError>
where
    T: Serialize,
{
    let encoded = encode_bounded(value, maximum)?;
    let detached = decode_strict_value(encoded.as_bytes(), maximum)?;
    canonical_value(&detached, maximum)
}

pub(super) fn canonical_value(
    value: &Value,
    maximum: usize,
) -> Result<String, PlatformCoreContractError> {
    let mut remaining = maximum;
    validate_value(value, 1, &mut remaining)?;
    encode_bounded(value, maximum)
}

pub(super) fn canonical_object(
    value: &Map<String, Value>,
    maximum: usize,
) -> Result<String, PlatformCoreContractError> {
    let mut remaining = maximum;
    claim_node(&mut remaining)?;
    validate_object(value, 1, &mut remaining)?;
    encode_bounded(value, maximum)
}

pub(super) fn validate_text(
    value: &str,
    label: &str,
    maximum: usize,
    nonempty: bool,
) -> Result<(), PlatformCoreContractError> {
    if value.len() > maximum || nonempty && value.is_empty() {
        return Err(invalid(format!(
            "{label} must be UTF-8 text within 1..{maximum} bytes"
        )));
    }
    if value.chars().any(forbidden_scalar) {
        return Err(invalid(format!(
            "{label} contains a forbidden Unicode scalar"
        )));
    }
    Ok(())
}

pub(super) fn validate_schema_name(
    value: &str,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    validate_text(value, label, 128, true)?;
    let parts: Vec<&str> = value.split('.').collect();
    if !(3..=6).contains(&parts.len())
        || parts
            .iter()
            .any(|part| part.is_empty() || part.len() > 32 || !lower_token(part))
    {
        return Err(invalid(format!(
            "{label} must contain three to six lowercase namespace segments"
        )));
    }
    Ok(())
}

pub(super) fn validate_hash(value: &str, label: &str) -> Result<(), PlatformCoreContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(format!("{label} must be a lowercase bare SHA-256")));
    }
    Ok(())
}

pub(super) fn lower_token(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn decode_strict_value(bytes: &[u8], maximum: usize) -> Result<Value, PlatformCoreContractError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!("JSON byte length must be 1..{maximum}")));
    }
    precheck_depth(bytes)?;
    let strict: StrictValue = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid strict JSON: {error}")))?;
    let mut remaining = maximum;
    validate_value(&strict.0, 1, &mut remaining)?;
    Ok(strict.0)
}

fn validate_value(
    value: &Value,
    depth: usize,
    remaining: &mut usize,
) -> Result<(), PlatformCoreContractError> {
    if depth > MAX_JSON_DEPTH {
        return Err(invalid(format!("JSON depth exceeds {MAX_JSON_DEPTH}")));
    }
    claim_node(remaining)?;
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) if number.as_i64().is_some() => Ok(()),
        Value::Number(_) => Err(invalid("JSON numbers must be signed int64 integers")),
        Value::String(text) => validate_text(text, "JSON string", MAX_STRING_BYTES, false),
        Value::Array(items) => validate_array(items, depth, remaining),
        Value::Object(object) => validate_object(object, depth, remaining),
    }
}

fn validate_array(
    items: &[Value],
    depth: usize,
    remaining: &mut usize,
) -> Result<(), PlatformCoreContractError> {
    if items.len() > MAX_ARRAY_ITEMS {
        return Err(invalid(format!(
            "JSON array exceeds {MAX_ARRAY_ITEMS} items"
        )));
    }
    for item in items {
        validate_value(item, depth + 1, remaining)?;
    }
    Ok(())
}

fn validate_object(
    object: &Map<String, Value>,
    depth: usize,
    remaining: &mut usize,
) -> Result<(), PlatformCoreContractError> {
    if object.len() > MAX_OBJECT_FIELDS {
        return Err(invalid(format!(
            "JSON object exceeds {MAX_OBJECT_FIELDS} fields"
        )));
    }
    for (key, value) in object {
        validate_text(key, "JSON object key", MAX_STRING_BYTES, true)?;
        validate_value(value, depth + 1, remaining)?;
    }
    Ok(())
}

fn claim_node(remaining: &mut usize) -> Result<(), PlatformCoreContractError> {
    if *remaining == 0 {
        return Err(invalid("JSON aggregate occurrence budget exceeded"));
    }
    *remaining -= 1;
    Ok(())
}

fn encode_bounded<T: Serialize>(
    value: &T,
    maximum: usize,
) -> Result<String, PlatformCoreContractError> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        maximum,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| invalid(format!("canonical JSON encoding failed: {error}")))?;
    String::from_utf8(writer.bytes)
        .map_err(|error| invalid(format!("canonical JSON was not UTF-8: {error}")))
}

struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other(format!(
                "canonical JSON exceeds {} bytes",
                self.maximum
            )));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn forbidden_scalar(value: char) -> bool {
    matches!(value, '\u{0000}'..='\u{001f}' | '\u{007f}'..='\u{009f}')
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn precheck_depth(bytes: &[u8]) -> Result<(), PlatformCoreContractError> {
    let (mut depth, mut quoted, mut escaped) = (0_usize, false, false);
    for byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
        } else if *byte == b'"' {
            quoted = true;
        } else if matches!(*byte, b'{' | b'[') {
            depth += 1;
            if depth > MAX_JSON_DEPTH {
                return Err(invalid(format!("JSON depth exceeds {MAX_JSON_DEPTH}")));
            }
        } else if matches!(*byte, b'}' | b']') {
            depth = depth.saturating_sub(1);
        }
    }
    Ok(())
}

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor)
    }
}

struct StrictValueVisitor;

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = StrictValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON containing signed int64 numbers")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(Number::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        i64::try_from(value)
            .map(|value| StrictValue(Value::Number(Number::from(value))))
            .map_err(|_| E::custom("JSON integer exceeds signed int64"))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("floating JSON numbers are forbidden"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if sequence
            .size_hint()
            .is_some_and(|size| size > MAX_ARRAY_ITEMS)
        {
            return Err(de::Error::custom(format!(
                "JSON array exceeds {MAX_ARRAY_ITEMS} items"
            )));
        }
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<StrictValue>()? {
            if values.len() >= MAX_ARRAY_ITEMS {
                return Err(de::Error::custom(format!(
                    "JSON array exceeds {MAX_ARRAY_ITEMS} items"
                )));
            }
            values.push(value.0);
        }
        Ok(StrictValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        if map.size_hint().is_some_and(|size| size > MAX_OBJECT_FIELDS) {
            return Err(de::Error::custom(format!(
                "JSON object exceeds {MAX_OBJECT_FIELDS} fields"
            )));
        }
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.len() >= MAX_OBJECT_FIELDS {
                return Err(de::Error::custom(format!(
                    "JSON object exceeds {MAX_OBJECT_FIELDS} fields"
                )));
            }
            let value = map.next_value::<StrictValue>()?;
            if values.insert(key.clone(), value.0).is_some() {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
        }
        Ok(StrictValue(Value::Object(values)))
    }
}
