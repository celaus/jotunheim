use serde::Deserialize;
use serde::{
    de::{self, Unexpected},
    Deserializer, Serializer,
};

pub fn int_from_bool<S>(data: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match &data {
        true => serializer.serialize_u8(1),
        false => serializer.serialize_u8(0),
    }
}

pub fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    match u8::deserialize(deserializer)? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(de::Error::invalid_value(
            Unexpected::Unsigned(other as u64),
            &"zero or one",
        )),
    }
}
