use anyhow::Result;
use lib::Vsock;
use tiny_vsock as lib;

// These parameters should be passed as arguments to the parent echo client.

// Port number to connect to the enclave echo service. This must match the port number that
// the enclave echo service is listening on.
const PORT: u32 = 8080;
// Context ID of the enclave to connect to. This must match the context ID that the enclave.
// The context ID is set while launching the enclave has to be more than 3. The value 16 is
// commonly used for the first enclave, but it can be different based on your setup.
const CID: u32 = 16;
// Maximum size of data to receive from the enclave. No more than this amount of data will
// be received in a single call to `receive()`. If the limit is exceeded, `receive()` will
// return an error.
const MAX_DATA_SIZE: usize = 8 * 1024;
// Size of each chunk of data to send or receive. The chunk size cannot be larger than the
// maximum data size, and it must be a positive integer.
const CHUNK_SIZE: usize = 512;
// Message to send to the enclave. This can be any byte array and it cannot not exceed the
// maximum data size.
const MESSAGE: &[u8] = b"Hello from the parent!";

fn main() -> Result<()> {
    // Connect to the enclave echo service using the specified context ID and port number.
    let enclave_socket = Vsock::connect(CID, PORT)
        .inspect(|_| tracing::info!("Connected to enclave on port {PORT} with context ID {CID}"))
        .inspect_err(|err| tracing::error!("Socket connect failed: {err:#?}"))?;

    // Send a message to the enclave and receive the echoed message back.
    enclave_socket
        .send(MESSAGE, CHUNK_SIZE)
        .inspect(|_| tracing::info!("Sent {} bytes of data", MESSAGE.len()))
        .inspect_err(|err| tracing::error!("Socket send failed: {err:#?}"))?;
    enclave_socket
        .receive(MAX_DATA_SIZE, CHUNK_SIZE)
        .map(|data| {
            let message = String::from_utf8_lossy(&data);
            tracing::info!("Received message from enclave: {message}")
        })
        .inspect_err(|err| tracing::error!("Socket recv failed: {err:#?}"))
}
