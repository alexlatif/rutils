// use crate::logger::GlobalLog;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

use crate::testing::prelude::*;

/// Include this in a test to turn on logging globally.
#[fixture]
#[once]
pub fn logging(#[default(Level::TRACE)] level: Level) {
    panic_on_err!({
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env().add_directive(level.into()))
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .init();
        Ok::<(), error_stack::Report<AnyErr>>(())
    })
}
