use anyhow::Result;
use lib::Vsock;
use tiny_vsock as lib;

// These parameters should be passed as arguments to the enclave echo service.

// Port number to bind the enclave echo service to.
const PORT: u32 = 8080;
// Maximum size of data to receive from the parent. No more than this amount of data will
// be received in a single call to `receive()`. If the limit is exceeded, `receive()` will
// return an error.
const MAX_DATA_SIZE: usize = 8 * 1024;
// Size of each chunk of data to send or receive. The chunk size cannot be larger than the
// maximum data size, and it must be a positive integer.
const CHUNK_SIZE: usize = 512;

fn main() -> Result<()> {
    // Bind the enclave socket to the specified port and listen for incoming connections.
    let enclave_socket = Vsock::bind(PORT)
        .inspect(|_| tracing::info!("Socket bind successful on port {PORT}"))
        .inspect_err(|err| tracing::error!("Socket bind failed: {err:#?}"))?;
    enclave_socket
        .listen()
        .inspect(|_| tracing::info!("Incomming connection..."))
        .inspect_err(|err| tracing::error!("Socket listen failed: {err:#?}"))?;

    // Accept a single connection from the parent.
    let parent_socket = enclave_socket
        .accept()
        .inspect(|_| tracing::info!("Accepted new connection"))
        .inspect_err(|err| tracing::error!("Socket accept failed: {err:#?}"))?;

    // Receive data from the parent and send it back.
    let data_buffer = parent_socket
        .receive(MAX_DATA_SIZE, CHUNK_SIZE)
        .inspect(|data| tracing::info!("Received {} bytes of data", data.len()))
        .inspect_err(|err| tracing::error!("Socket recv failed: {err:#?}"))?;
    parent_socket
        .send(&data_buffer, CHUNK_SIZE)
        .inspect_err(|err| tracing::error!("Socket send failed: {err:#?}"))
        .map(|_| {
            tracing::info!(
                "Sent {} bytes of data. Closing connection",
                data_buffer.len()
            )
        })
}
