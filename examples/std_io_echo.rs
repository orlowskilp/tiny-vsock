//! Demonstrates the `std-io` feature: server reads via `BufReader` + `read_to_string`;
//! client writes via `write_all` + `flush` (flush shuts down the write side to signal EOF).
//!
//! Build and run with the feature enabled:
//! ```shell
//!   cargo run --example std_io_echo --features std-io -- server <port>
//!   cargo run --example std_io_echo --features std-io -- client <cid> <port>
//! ```
//!
//! Without the feature the binary prints a notice and exits.

#[cfg(not(feature = "std-io"))]
fn main() {
    eprintln!("This example requires the `std-io` feature: --features std-io");
}

#[cfg(feature = "std-io")]
fn main() -> anyhow::Result<()> {
    use anyhow::anyhow;
    use std::io::{BufReader, Read, Write};
    use tiny_vsock::Vsock;

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut args = std::env::args().skip(1);
    let role = args
        .next()
        .ok_or_else(|| anyhow!("usage: std_io_echo server <port> | client <cid> <port>"))?;

    match role.as_str() {
        "server" => {
            let port: u32 = args
                .next()
                .ok_or_else(|| anyhow!("usage: std_io_echo server <port>"))?
                .parse()?;

            let listener = Vsock::bind(port)?;
            listener.listen()?;

            tracing::info!(port, "listening");

            let mut conn = listener.accept()?;

            tracing::info!("accepted connection");

            let mut body = String::new();
            BufReader::new(&mut conn).read_to_string(&mut body)?;

            tracing::info!(message = %body, bytes = body.len(), "received");
        }
        "client" => {
            let cid: u32 = args
                .next()
                .ok_or_else(|| anyhow!("usage: std_io_echo client <cid> <port>"))?
                .parse()?;
            let port: u32 = args
                .next()
                .ok_or_else(|| anyhow!("usage: std_io_echo client <cid> <port>"))?
                .parse()?;

            let mut conn = Vsock::connect(cid, port)?;

            tracing::info!(cid, port, "connected");

            conn.write_all(b"Hello via std::io!")?;
            // flush shuts down the write side, allowing the server's read_to_string to complete.
            conn.flush()?;

            tracing::info!("sent and flushed");
        }
        other => {
            return Err(anyhow!(
                "unknown role `{other}`; expected `server` or `client`"
            ));
        }
    }

    Ok(())
}
