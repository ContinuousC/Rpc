/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct TraceCtx(Option<HashMap<String, String>>);

impl TraceCtx {
    #[cfg(not(feature = "opentelemetry"))]
    pub(crate) fn current() -> Self {
        Self(None)
    }

    #[cfg(feature = "opentelemetry")]
    pub(crate) fn current() -> Self {
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        let span = tracing::Span::current();
        let ctx = span.context();
        let mut trace_ctx = TraceCtx::default();
        opentelemetry::global::get_text_map_propagator(|injector| {
            injector.inject_context(&ctx, &mut trace_ctx)
        });
        trace_ctx
    }

    #[cfg(not(feature = "opentelemetry"))]
    pub(crate) fn set_parent(&self) {}

    #[cfg(feature = "opentelemetry")]
    pub(crate) fn set_parent(&self) {
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        opentelemetry::global::get_text_map_propagator(|extractor| {
            let ctx = extractor.extract(self);
            let span = tracing::Span::current();
            span.set_parent(ctx);
        });
    }
}

#[cfg(feature = "opentelemetry")]
impl opentelemetry::propagation::Injector for TraceCtx {
    fn set(&mut self, key: &str, value: String) {
        let ctx = self.0.get_or_insert_with(HashMap::new);
        ctx.insert(key.to_string(), value);
    }
}

#[cfg(feature = "opentelemetry")]
impl opentelemetry::propagation::Extractor for TraceCtx {
    fn get(&self, key: &str) -> Option<&str> {
        let value =
            self.0.as_ref().and_then(|ctx| Some(ctx.get(key)?.as_str()));
        value
    }
    fn keys(&self) -> Vec<&str> {
        self.0.as_ref().map_or_else(Vec::new, |ctx| {
            ctx.keys().map(|s| s.as_str()).collect()
        })
    }
}
