use std::fmt;

use serde::{
    Serialize,
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant, Serializer,
    },
};

use super::{
    DecisionCapsuleContractError, invalid,
    profile_key::{self, Error, Result},
};

#[derive(Clone, Copy)]
struct Profile {
    depth: usize,
}

impl Profile {
    fn scalar(self) -> Result<()> {
        profile_key::check_depth(self.depth)
    }

    fn child(self) -> Self {
        Self {
            depth: self.depth + 1,
        }
    }
}

struct Sequence {
    child: Profile,
    count: usize,
}

impl Sequence {
    fn new(depth: usize, length: Option<usize>) -> Result<Self> {
        profile_key::check_depth(depth)?;
        if length.is_some_and(|count| count > crate::governance_contract::MAX_ARRAY_ITEMS) {
            return Err(profile_key::error(
                "canonical JSON array exceeds the item limit",
            ));
        }
        Ok(Self {
            child: Profile { depth }.child(),
            count: 0,
        })
    }

    fn add<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.count += 1;
        if self.count > crate::governance_contract::MAX_ARRAY_ITEMS {
            return Err(profile_key::error(
                "canonical JSON array exceeds the item limit",
            ));
        }
        value.serialize(self.child)
    }
}

struct Object {
    child: Profile,
    count: usize,
    pending_value: bool,
}

impl Object {
    fn new(depth: usize, length: Option<usize>) -> Result<Self> {
        profile_key::check_depth(depth)?;
        if length.is_some_and(|count| count > crate::governance_contract::MAX_OBJECT_FIELDS) {
            return Err(profile_key::error(
                "canonical JSON object exceeds the field limit",
            ));
        }
        Ok(Self {
            child: Profile { depth }.child(),
            count: 0,
            pending_value: false,
        })
    }

    fn add_field<T: Serialize + ?Sized>(&mut self, key: &str, value: &T) -> Result<()> {
        self.count += 1;
        if self.count > crate::governance_contract::MAX_OBJECT_FIELDS {
            return Err(profile_key::error(
                "canonical JSON object exceeds the field limit",
            ));
        }
        profile_key::check_key(key)?;
        value.serialize(self.child)
    }
}

struct DisplayProfile {
    bytes: usize,
}

impl fmt::Write for DisplayProfile {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > crate::governance_contract::MAX_STRING_BYTES
            || profile_key::check_text(value).is_err()
        {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

macro_rules! scalar {
    ($name:ident, $type:ty) => {
        fn $name(self, _value: $type) -> Result<()> {
            self.scalar()
        }
    };
}

impl Serializer for Profile {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Sequence;
    type SerializeTuple = Sequence;
    type SerializeTupleStruct = Sequence;
    type SerializeTupleVariant = Sequence;
    type SerializeMap = Object;
    type SerializeStruct = Object;
    type SerializeStructVariant = Object;

    scalar!(serialize_bool, bool);
    scalar!(serialize_i8, i8);
    scalar!(serialize_i16, i16);
    scalar!(serialize_i32, i32);
    scalar!(serialize_i64, i64);
    scalar!(serialize_u8, u8);
    scalar!(serialize_u16, u16);
    scalar!(serialize_u32, u32);

    fn serialize_i128(self, value: i128) -> Result<()> {
        self.scalar()?;
        i64::try_from(value)
            .map(|_| ())
            .map_err(|_| profile_key::error("canonical JSON permits signed int64 only"))
    }

    fn serialize_u64(self, value: u64) -> Result<()> {
        self.serialize_u128(u128::from(value))
    }

    fn serialize_u128(self, value: u128) -> Result<()> {
        self.scalar()?;
        if value <= i64::MAX as u128 {
            Ok(())
        } else {
            Err(profile_key::error(
                "canonical JSON permits signed int64 only",
            ))
        }
    }

    fn serialize_f32(self, _value: f32) -> Result<()> {
        Err(profile_key::error("canonical JSON rejects floats"))
    }

    fn serialize_f64(self, _value: f64) -> Result<()> {
        Err(profile_key::error("canonical JSON rejects floats"))
    }

    fn serialize_char(self, value: char) -> Result<()> {
        let mut bytes = [0; 4];
        self.serialize_str(value.encode_utf8(&mut bytes))
    }

    fn serialize_str(self, value: &str) -> Result<()> {
        self.scalar()?;
        profile_key::check_text(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<()> {
        let mut sequence = Sequence::new(self.depth, Some(value.len()))?;
        for byte in value {
            sequence.add(byte)?;
        }
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.scalar()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.scalar()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.scalar()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        self.scalar()?;
        profile_key::check_key(variant)?;
        value.serialize(self.child())
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq> {
        Sequence::new(self.depth, length)
    }

    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple> {
        Sequence::new(self.depth, Some(length))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Sequence::new(self.depth, Some(length))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.scalar()?;
        profile_key::check_key(variant)?;
        Sequence::new(self.depth + 1, Some(length))
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap> {
        Object::new(self.depth, length)
    }

    fn serialize_struct(self, _name: &'static str, length: usize) -> Result<Self::SerializeStruct> {
        Object::new(self.depth, Some(length))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.scalar()?;
        profile_key::check_key(variant)?;
        Object::new(self.depth + 1, Some(length))
    }

    fn collect_str<T: fmt::Display + ?Sized>(self, value: &T) -> Result<()> {
        self.scalar()?;
        let mut output = DisplayProfile { bytes: 0 };
        fmt::write(&mut output, format_args!("{value}"))
            .map_err(|_| profile_key::error("canonical JSON string profile differs"))
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

macro_rules! sequence_impl {
    ($trait:ident, $method:ident) => {
        impl $trait for Sequence {
            type Ok = ();
            type Error = Error;

            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
                self.add(value)
            }

            fn end(self) -> Result<()> {
                Ok(())
            }
        }
    };
}

sequence_impl!(SerializeSeq, serialize_element);
sequence_impl!(SerializeTuple, serialize_element);
sequence_impl!(SerializeTupleStruct, serialize_field);
sequence_impl!(SerializeTupleVariant, serialize_field);

impl SerializeMap for Object {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        if self.pending_value {
            return Err(profile_key::error("canonical JSON map key has no value"));
        }
        self.count += 1;
        if self.count > crate::governance_contract::MAX_OBJECT_FIELDS {
            return Err(profile_key::error(
                "canonical JSON object exceeds the field limit",
            ));
        }
        key.serialize(profile_key::Key)?;
        self.pending_value = true;
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        if !self.pending_value {
            return Err(profile_key::error("canonical JSON map value has no key"));
        }
        value.serialize(self.child)?;
        self.pending_value = false;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.pending_value {
            Err(profile_key::error("canonical JSON map key has no value"))
        } else {
            Ok(())
        }
    }
}

macro_rules! object_impl {
    ($trait:ident) => {
        impl $trait for Object {
            type Ok = ();
            type Error = Error;

            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<()> {
                self.add_field(key, value)
            }

            fn end(self) -> Result<()> {
                Ok(())
            }
        }
    };
}

object_impl!(SerializeStruct);
object_impl!(SerializeStructVariant);

pub(super) fn validate<T: Serialize + ?Sized>(
    value: &T,
) -> std::result::Result<(), DecisionCapsuleContractError> {
    value
        .serialize(Profile { depth: 1 })
        .map_err(|error| invalid(format!("canonical JSON profile failed: {error}")))
}

mod captured {
    use std::{collections::HashSet, fmt};

    use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

    struct UniqueJson;

    macro_rules! ignore_scalar {
        ($($method:ident($value:ty)),+ $(,)?) => {$(
            fn $method<E: de::Error>(self, _: $value) -> Result<(), E> { Ok(()) }
        )+};
    }

    impl<'de> DeserializeSeed<'de> for UniqueJson {
        type Value = ();

        fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
            decoder.deserialize_any(self)
        }
    }

    impl<'de> Visitor<'de> for UniqueJson {
        type Value = ();

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a JSON value with unique object keys")
        }

        ignore_scalar!(
            visit_bool(bool),
            visit_i64(i64),
            visit_u64(u64),
            visit_f64(f64),
            visit_str(&str)
        );

        fn visit_unit<E: de::Error>(self) -> Result<(), E> {
            Ok(())
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
            while sequence.next_element_seed(UniqueJson)?.is_some() {}
            Ok(())
        }

        fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<(), A::Error> {
            let mut keys = HashSet::new();
            while let Some(key) = object.next_key::<String>()? {
                if !keys.insert(key) {
                    return Err(de::Error::custom("duplicate JSON object key"));
                }
                if keys.len() > crate::governance_contract::MAX_OBJECT_FIELDS {
                    return Err(de::Error::custom(
                        "canonical JSON object exceeds the field limit",
                    ));
                }
                object.next_value_seed(UniqueJson)?;
            }
            Ok(())
        }
    }

    pub(super) fn reject(bytes: &[u8]) -> Result<(), serde_json::Error> {
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        UniqueJson.deserialize(&mut decoder)?;
        decoder.end()
    }
}

pub(super) fn reject_duplicate_keys(
    bytes: &[u8],
) -> std::result::Result<(), DecisionCapsuleContractError> {
    captured::reject(bytes)
        .map_err(|error| invalid(format!("captured JSON key validation failed: {error}")))
}
