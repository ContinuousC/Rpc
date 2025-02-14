/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::error::Error;

use serde::Serialize;

pub trait FromSerialize: Sized {
    type Error: Error;
    fn from_serialize<T: Serialize>(value: T) -> Result<Self, Self::Error>;
}

#[cfg(feature = "serde_json")]
impl FromSerialize for serde_json::Value {
    type Error = serde_json::Error;
    fn from_serialize<T: Serialize>(value: T) -> Result<Self, Self::Error> {
        serde_json::to_value(value)
    }
}

#[cfg(feature = "serde_cbor")]
impl FromSerialize for serde_cbor::Value {
    type Error = serde_cbor::Error;
    fn from_serialize<T: Serialize>(value: T) -> Result<Self, Self::Error> {
        serde_cbor::value::to_value(value)
    }
}
