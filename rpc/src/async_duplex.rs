/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

#[cfg(feature = "schemars")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AsyncRequest, AsyncResponse};

/// Message supporting a two way async RPC interface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AsyncDuplex<Req, Res, Event = Void, K = u64> {
    Request(AsyncRequest<Req, K>),
    Response(AsyncResponse<Res, K>),
    Event { event: Event },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub enum Void {}
