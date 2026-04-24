use anyhow::{Result, anyhow};

// Port for the echo service.
const PORT: u32 = 5000;
// CID of the parent instance (AWS Nitro Enclave convention).
const CID: u32 = 3;

#[cfg(not(feature = "std-io"))]
fn main() {
    eprintln!("This example requires the `std-io` feature: --features std-io");
}

#[cfg(feature = "std-io")]
fn main() -> Result<()> {
    use std::io::{BufReader, Read, Write};
    use tiny_vsock::Vsock;

    let role = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: std_io_echo server | client"))?;

    match role.as_str() {
        "server" => {
            let listener = Vsock::bind(PORT)
                .inspect(|_| tracing::info!("Bound to port {PORT}"))
                .inspect_err(|err| tracing::error!("Socket bind failed: {err:#?}"))?;
            listener
                .listen()
                .inspect_err(|err| tracing::error!("Socket listen failed: {err:#?}"))?;

            let mut conn = listener
                .accept()
                .inspect(|_| tracing::info!("Accepted connection"))
                .inspect_err(|err| tracing::error!("Socket accept failed: {err:#?}"))?;

            let mut body = String::new();
            BufReader::new(&mut conn)
                .read_to_string(&mut body)
                .inspect(|bytes| tracing::info!("Received {bytes} bytes: {body}"))
                .inspect_err(|err| tracing::error!("Read failed: {err:#?}"))?;
        }
        "client" => {
            let mut conn = Vsock::connect(CID, PORT)
                .inspect(|_| tracing::info!("Connected to CID {CID} on port {PORT}"))
                .inspect_err(|err| tracing::error!("Socket connect failed: {err:#?}"))?;

            conn.write_all(b"Hello via std::io!")
                .inspect(|_| tracing::info!("Sent message"))
                .inspect_err(|err| tracing::error!("Write failed: {err:#?}"))?;
            // flush shuts down the write side, signalling EOF to the server's read_to_string.
            conn.flush()
                .inspect_err(|err| tracing::error!("Flush failed: {err:#?}"))?;
        }
        other => {
            return Err(anyhow!(
                "unknown role `{other}`; expected `server` or `client`"
            ));
        }
    }

    Ok(())
}
