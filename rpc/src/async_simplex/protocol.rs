/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::marker::PhantomData;

use crate::Protocol;

#[derive(Clone, Copy, Debug)]
pub struct AsyncProto<P>(PhantomData<P>);

impl<P: Protocol> Protocol for AsyncProto<P> {
    //     type Req = AsyncRequest<P::Req>;
    //     type Res = AsyncResponse<P::Res>;
}
