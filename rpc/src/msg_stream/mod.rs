/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

#[cfg(feature = "serde_cbor")]
mod cbor;
#[cfg(feature = "serde_json")]
mod json;
mod stream;

mod protocol;
mod split;
#[cfg(test)]
mod test;
mod varint;
// mod wrapper;

#[cfg(feature = "serde_cbor")]
pub use cbor::{CborReadStream, CborStream, CborWriteStream};
#[cfg(feature = "serde_json")]
pub use json::{JsonReadStream, JsonStream, JsonWriteStream};
pub use protocol::Protocol;
pub use split::Split;
pub use stream::{MsgReadStream, MsgStream, MsgWriteStream};
// #[cfg(feature = "serde_cbor")]
// pub use wrapper::CborStreamWrapper;
// #[cfg(feature = "serde_json")]
// pub use wrapper::JsonStreamWrapper;
// pub use wrapper::{PlainStreamWrapper, StreamWrapper, TlsConnector};
