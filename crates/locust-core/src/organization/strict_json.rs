//! Reject duplicate keys before typed deserialization can silently replace them.
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::{Diagnostic, escape};

struct Json<'a> {
    path: String,
    duplicate: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for Json<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Json<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON data")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.into()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Json {
            path: format!("{}/{}", self.path, values.len()),
            duplicate: self.duplicate,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let path = format!("{}/{}", self.path, escape(&key));
            if values.contains_key(&key) {
                *self.duplicate = Some(path);
                return Err(de::Error::custom(format!("duplicate JSON key {key:?}")));
            }
            let value = map.next_value_seed(Json {
                path,
                duplicate: self.duplicate,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

pub(super) fn parse(source: &str) -> Result<Value, Box<Diagnostic>> {
    let mut duplicate = None;
    let mut reader = serde_json::Deserializer::from_str(source);
    let value = Json {
        path: String::new(),
        duplicate: &mut duplicate,
    }
    .deserialize(&mut reader);
    let value = value.and_then(|value| reader.end().map(|()| value));
    value.map_err(|error| Box::new(Diagnostic::error(
        if duplicate.is_some() { "duplicate_key" } else { "invalid_json" }, "load", duplicate.as_deref().unwrap_or(""),
        error.to_string(), "Supply one complete JSON object with unique field names; parser resource-limit errors are reported without truncating the document.")))
}
