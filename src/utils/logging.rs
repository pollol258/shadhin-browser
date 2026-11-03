//! Tracing subscriber setup with environment filter support

use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initializes the application-wide tracing subscriber
pub fn init_logging() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("shadhin_browser=debug,warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .try_init()
        .ok();

    Ok(())
}
