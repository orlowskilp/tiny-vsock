//! Vsock client that connects to a server and sends a fixed message.
//!
//! Run with:
//! ```shell
//!   cargo run --example client -- <cid> <port>
//! ```
//!
//! Pair with `server` example:
//! ```shell
//!   cargo run --example server -- <port>
//! ```

use anyhow::{Result, anyhow};
use tiny_vsock::Vsock;

const MESSAGE: &[u8] = b"Hello from tiny-vsock client!";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut args = std::env::args().skip(1);
    let cid: u32 = args
        .next()
        .ok_or_else(|| anyhow!("usage: client <cid> <port>"))?
        .parse()?;
    let port: u32 = args
        .next()
        .ok_or_else(|| anyhow!("usage: client <cid> <port>"))?
        .parse()?;

    // 3 attempts is enough for a local dev environment; raise for flaky hypervisor paths.
    let conn = Vsock::connect_with_max_attempts(cid, port, 3)?;

    tracing::info!(cid, port, "connected");

    conn.send(MESSAGE, 4096)?;

    tracing::info!(bytes = MESSAGE.len(), "sent");

    Ok(())
}
