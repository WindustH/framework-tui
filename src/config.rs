//! Configuration primitives; schemas and defaults remain with applications.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Boolean setting with an optional automatic mode. Reads legacy on/off strings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Toggle {
  #[default]
  Off,
  On,
  Auto,
}
impl Toggle {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Off => "off",
      Self::On => "on",
      Self::Auto => "auto",
    }
  }
}
impl Serialize for Toggle {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    match self {
      Self::Off => serializer.serialize_bool(false),
      Self::On => serializer.serialize_bool(true),
      Self::Auto => serializer.serialize_str("auto"),
    }
  }
}
impl<'de> Deserialize<'de> for Toggle {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
      Bool(bool),
      Text(String),
    }
    match Value::deserialize(deserializer)? {
      Value::Bool(v) => Ok(if v { Self::On } else { Self::Off }),
      Value::Text(v) => match v.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Ok(Self::On),
        "off" | "false" | "no" | "0" | "" => Ok(Self::Off),
        "auto" => Ok(Self::Auto),
        _ => Err(de::Error::custom("expected boolean, on/off, or auto")),
      },
    }
  }
}
