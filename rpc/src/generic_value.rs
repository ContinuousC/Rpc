/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::error::Error;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait GenericValue:
    Serialize + DeserializeOwned + Sized + Clone + Send + Sync + 'static
{
    type Error: Error + Send + Sync + 'static;
    fn deserialize_into<T>(self) -> Result<T, Self::Error>
    where
        T: for<'de> Deserialize<'de>;
    fn serialize_from<T>(value: T) -> Result<Self, Self::Error>
    where
        T: Serialize;
}

#[cfg(feature = "serde_json")]
impl GenericValue for serde_json::Value {
    type Error = serde_json::Error;

    fn deserialize_into<T>(self) -> Result<T, Self::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(self)
    }

    fn serialize_from<T>(value: T) -> Result<Self, Self::Error>
    where
        T: Serialize,
    {
        serde_json::to_value(value)
    }
}

#[cfg(feature = "serde_cbor")]
impl GenericValue for serde_cbor::Value {
    type Error = serde_cbor::Error;

    fn deserialize_into<T>(self) -> Result<T, Self::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_cbor::value::from_value(self)
    }

    fn serialize_from<T>(value: T) -> Result<Self, Self::Error>
    where
        T: Serialize,
    {
        serde_cbor::value::to_value(value)
    }
}
