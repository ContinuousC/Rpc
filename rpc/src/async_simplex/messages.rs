/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

#[cfg(feature = "schemars")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{GenericValue, TraceCtx};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct AsyncRequest<T, K = u64> {
    pub req_id: K,
    pub request: T,
    //#[cfg(feature = "opentelemetry")]
    #[serde(default)]
    pub(crate) trace_ctx: TraceCtx,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct AsyncResponse<T, K = u64> {
    pub req_id: K,
    pub response: T,
}

impl<K, V> AsyncResponse<V, K>
where
    V: GenericValue,
{
    pub fn from_send_error<E>(
        err: mpsc::error::TrySendError<AsyncRequest<V>>,
    ) -> AsyncResponse<V>
    where
        E: From<String> + Serialize + 'static,
    {
        match err {
            mpsc::error::TrySendError::Full(AsyncRequest {
                req_id, ..
            })
            | mpsc::error::TrySendError::Closed(AsyncRequest {
                req_id, ..
            }) => AsyncResponse {
                req_id,
                response: V::serialize_from::<Result<(), E>>(Err(err
                    .to_string()
                    .into()))
                .unwrap(),
            },
        }
    }
}
