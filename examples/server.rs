//! Vsock server that binds to a port, accepts one connection, and receives a message.
//!
//! Run with:
//! ```shell
//!   cargo run --example server -- <port>
//! ```
//!
//! Pair with `client` example:
//! ```shell
//!   cargo run --example client -- <cid> <port>
//! ```

use anyhow::{Result, anyhow};
use tiny_vsock::Vsock;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let port: u32 = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: server <port>"))?
        .parse()?;

    let listener = Vsock::bind(port)?;
    listener.listen()?;

    tracing::info!(port, "listening for connections");

    let conn = listener.accept()?;

    tracing::info!("accepted connection");

    // 64 KiB total cap; 4 KiB chunks match typical vsock hypervisor page size.
    let data = conn.receive(65536, 4096)?;

    let message = String::from_utf8_lossy(&data);
    tracing::info!(message = %message, bytes = data.len(), "received");

    Ok(())
}
