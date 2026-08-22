use std::io::{self, Write};

use serde::{
    Serialize,
    de::DeserializeOwned,
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant, Serializer,
    },
};
use sha2::{Digest, Sha256};

use super::{DecisionCapsuleContractError, MAX_CLOSURE_BYTES, invalid, profile};

struct BoundedCounter {
    maximum: usize,
    written: usize,
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    maximum: usize,
}

struct RejectFloats<'a, T: ?Sized>(&'a T);

struct FloatGuard<S>(S);

struct FloatCompound<C>(C);

impl<T: Serialize + ?Sized> Serialize for RejectFloats<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(FloatGuard(serializer))
    }
}

macro_rules! forward_scalar {
    ($name:ident, $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.0.$name(value)
        }
    };
}

impl<S: Serializer> Serializer for FloatGuard<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = FloatCompound<S::SerializeSeq>;
    type SerializeTuple = FloatCompound<S::SerializeTuple>;
    type SerializeTupleStruct = FloatCompound<S::SerializeTupleStruct>;
    type SerializeTupleVariant = FloatCompound<S::SerializeTupleVariant>;
    type SerializeMap = FloatCompound<S::SerializeMap>;
    type SerializeStruct = FloatCompound<S::SerializeStruct>;
    type SerializeStructVariant = FloatCompound<S::SerializeStructVariant>;

    forward_scalar!(serialize_bool, bool);
    forward_scalar!(serialize_i8, i8);
    forward_scalar!(serialize_i16, i16);
    forward_scalar!(serialize_i32, i32);
    forward_scalar!(serialize_i64, i64);
    forward_scalar!(serialize_i128, i128);
    forward_scalar!(serialize_u8, u8);
    forward_scalar!(serialize_u16, u16);
    forward_scalar!(serialize_u32, u32);
    forward_scalar!(serialize_u64, u64);
    forward_scalar!(serialize_u128, u128);
    forward_scalar!(serialize_char, char);

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(<S::Error as serde::ser::Error>::custom(
            "canonical JSON rejects floats",
        ))
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(<S::Error as serde::ser::Error>::custom(
            "canonical JSON rejects floats",
        ))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_str(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_some(&RejectFloats(value))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(name, index, variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_struct(name, &RejectFloats(value))
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0
            .serialize_newtype_variant(name, index, variant, &RejectFloats(value))
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.0.serialize_seq(length).map(FloatCompound)
    }

    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.0.serialize_tuple(length).map(FloatCompound)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.0
            .serialize_tuple_struct(name, length)
            .map(FloatCompound)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.0
            .serialize_tuple_variant(name, index, variant, length)
            .map(FloatCompound)
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.0.serialize_map(length).map(FloatCompound)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.0.serialize_struct(name, length).map(FloatCompound)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.0
            .serialize_struct_variant(name, index, variant, length)
            .map(FloatCompound)
    }

    fn collect_str<T: std::fmt::Display + ?Sized>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.collect_str(value)
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

macro_rules! sequence_guard {
    ($trait:ident, $method:ident) => {
        impl<C: $trait> $trait for FloatCompound<C> {
            type Ok = C::Ok;
            type Error = C::Error;

            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
                self.0.$method(&RejectFloats(value))
            }

            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        }
    };
}

sequence_guard!(SerializeSeq, serialize_element);
sequence_guard!(SerializeTuple, serialize_element);
sequence_guard!(SerializeTupleStruct, serialize_field);
sequence_guard!(SerializeTupleVariant, serialize_field);

impl<C: SerializeMap> SerializeMap for FloatCompound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.0.serialize_key(&RejectFloats(key))
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.0.serialize_value(&RejectFloats(value))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

macro_rules! struct_guard {
    ($trait:ident) => {
        impl<C: $trait> $trait for FloatCompound<C> {
            type Ok = C::Ok;
            type Error = C::Error;

            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), Self::Error> {
                self.0.serialize_field(key, &RejectFloats(value))
            }

            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        }
    };
}

struct_guard!(SerializeStruct);
struct_guard!(SerializeStructVariant);

impl Write for BoundedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "JSON size overflow"))?;
        if next > self.maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON exceeds its document ceiling",
            ));
        }
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "JSON size overflow"))?;
        if next > self.maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON exceeds its document ceiling",
            ));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("bounded JSON allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn premeasure_only<T>(
    value: &T,
    maximum: usize,
) -> Result<usize, DecisionCapsuleContractError>
where
    T: Serialize + ?Sized,
{
    let mut counter = BoundedCounter {
        maximum,
        written: 0,
    };
    serde_json::to_writer(&mut counter, value)
        .map_err(|error| invalid(format!("bounded JSON measurement failed: {error}")))?;
    profile::validate(value)?;
    Ok(counter.written)
}

#[cfg(test)]
thread_local! {
    static CANONICAL_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BOUNDED_CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_allocation() {
    CANONICAL_ALLOCATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_allocation() {}

#[cfg(test)]
fn record_clone() {
    BOUNDED_CLONES.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_clone() {}

#[cfg(test)]
pub(super) fn reset_allocation_calls() {
    CANONICAL_ALLOCATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn allocation_calls() -> usize {
    CANONICAL_ALLOCATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn reset_clone_calls() {
    BOUNDED_CLONES.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn clone_calls() -> usize {
    BOUNDED_CLONES.with(std::cell::Cell::get)
}

pub(super) fn bounded_clone<T>(value: &T, maximum: usize) -> Result<T, DecisionCapsuleContractError>
where
    T: Clone + Serialize,
{
    premeasure_only(value, maximum)?;
    record_clone();
    Ok(value.clone())
}

pub(super) fn canonical_with_max<T>(
    value: &T,
    maximum: usize,
) -> Result<String, DecisionCapsuleContractError>
where
    T: Serialize + ?Sized,
{
    record_allocation();
    let mut captured = BoundedBuffer {
        bytes: Vec::new(),
        maximum,
    };
    serde_json::to_writer(&mut captured, &RejectFloats(value))
        .map_err(|error| invalid(format!("bounded JSON capture failed: {error}")))?;
    profile::reject_duplicate_keys(&captured.bytes)?;
    let stable: serde_json::Value = serde_json::from_slice(&captured.bytes)
        .map_err(|error| invalid(format!("captured JSON failed to decode: {error}")))?;
    profile::validate(&stable)?;
    let canonical = crate::governance_contract::codec::canonical_json(&stable)
        .map_err(|error| invalid(format!("canonical JSON failed: {}", error.message)))?;
    if canonical.len() > maximum {
        Err(invalid(format!(
            "canonical JSON exceeds the {maximum}-byte ceiling"
        )))
    } else {
        Ok(canonical)
    }
}

pub(super) fn domain_digest<T>(
    domain: &[u8],
    value: &T,
    maximum: usize,
) -> Result<String, DecisionCapsuleContractError>
where
    T: Serialize + ?Sized,
{
    let canonical = canonical_with_max(value, maximum)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(canonical.as_bytes());
    Ok(crate::governance_contract::codec::lower_hex(
        &hasher.finalize(),
    ))
}

pub(super) fn decode_typed<T>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, DecisionCapsuleContractError>
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

/// Returns exact compact canonical JSON under the ADR-0092 outer ceiling.
///
/// # Errors
///
/// Returns an error for unsupported JSON values, frozen wire-bound violations, or oversize.
pub fn canonical_json<T>(value: &T) -> Result<String, DecisionCapsuleContractError>
where
    T: Serialize + ?Sized,
{
    canonical_with_max(value, MAX_CLOSURE_BYTES)
}
