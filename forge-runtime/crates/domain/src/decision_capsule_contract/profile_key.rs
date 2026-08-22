use std::fmt;

use serde::{
    Serialize,
    ser::{self, Impossible, Serializer},
};

pub(super) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub(super) struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

pub(super) fn error(message: impl Into<String>) -> Error {
    Error(message.into())
}

pub(super) fn check_depth(depth: usize) -> Result<()> {
    if depth <= crate::governance_contract::MAX_DEPTH {
        Ok(())
    } else {
        Err(error("canonical JSON exceeds the depth limit"))
    }
}

fn forbidden_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{0000}'..='\u{001f}'
            | '\u{007f}'..='\u{009f}'
            | '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

pub(super) fn check_text(value: &str) -> Result<()> {
    if value.len() > crate::governance_contract::MAX_STRING_BYTES {
        return Err(error("canonical JSON string exceeds the byte limit"));
    }
    if value.chars().any(forbidden_scalar) {
        return Err(error(
            "canonical JSON string contains a forbidden Unicode scalar",
        ));
    }
    Ok(())
}

pub(super) fn check_key(value: &str) -> Result<()> {
    check_text(value)?;
    let bytes = value.as_bytes();
    if bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
    {
        Ok(())
    } else {
        Err(error("canonical JSON object key is not ASCII snake_case"))
    }
}

pub(super) struct Key;

macro_rules! reject {
    ($name:ident, $type:ty) => {
        fn $name(self, _value: $type) -> Result<()> {
            Err(error("canonical JSON object key must be a string"))
        }
    };
}

impl Serializer for Key {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    reject!(serialize_bool, bool);
    reject!(serialize_i8, i8);
    reject!(serialize_i16, i16);
    reject!(serialize_i32, i32);
    reject!(serialize_i64, i64);
    reject!(serialize_i128, i128);
    reject!(serialize_u8, u8);
    reject!(serialize_u16, u16);
    reject!(serialize_u32, u32);
    reject!(serialize_u64, u64);
    reject!(serialize_u128, u128);
    reject!(serialize_f32, f32);
    reject!(serialize_f64, f64);
    reject!(serialize_char, char);
    reject!(serialize_bytes, &[u8]);

    fn serialize_str(self, value: &str) -> Result<()> {
        check_key(value)
    }

    fn serialize_none(self) -> Result<()> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<()> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_unit(self) -> Result<()> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<()> {
        check_key(variant)
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
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_seq(self, _length: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_tuple(self, _length: usize) -> Result<Self::SerializeTuple> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_map(self, _length: Option<usize>) -> Result<Self::SerializeMap> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeStruct> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn collect_str<T: fmt::Display + ?Sized>(self, _value: &T) -> Result<()> {
        Err(error("canonical JSON object key must be a string"))
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}
