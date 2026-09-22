use std::collections::HashSet;

use serde::{de, de::DeserializeSeed};

pub(crate) fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    UniqueJson.deserialize(&mut decoder)?;
    decoder.end()
}

struct UniqueJson;

impl<'de> DeserializeSeed<'de> for UniqueJson {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> de::Visitor<'de> for UniqueJson {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence.next_element_seed(UniqueJson)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut object: A) -> Result<(), A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            object.next_value_seed(UniqueJson)?;
        }
        Ok(())
    }
}
