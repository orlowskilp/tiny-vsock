use serial_test::serial;
#[cfg(feature = "std-io")]
use std::io::{Read as _, Write as _};
use std::{sync::Once, thread};
use tiny_vsock::Vsock;
use tracing_subscriber::{EnvFilter, fmt};

static INIT: Once = Once::new();
fn init_tracing() {
    INIT.call_once(|| {
        let subscriber = fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
            )
            .with_ansi(true)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set tracing subscriber");
    });
}

const TEST_CID: u32 = 2;
const TEST_PORT: u32 = 12345;
const TEST_CHUNK_SIZE: usize = 1024;
const TEST_MAX_SIZE: usize = 8 * TEST_CHUNK_SIZE;
const TEST_LOG_MESSAGE_PREFIX: &str = "[Test]";

fn format_byte_vec(bytes: &[u8]) -> String {
    let mut result = "[ ".to_string();
    for byte in bytes {
        result.push_str(&format!("{byte:#04x} "));
    }
    result.push(']');
    result.trim_end().to_string()
}

#[test]
#[serial]
fn test_vsock_connection_valid_transfer_ok() {
    init_tracing();
    let handle = thread::spawn(|| {
        let client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        tracing::info!(
            "{} Connected to vsock server at CID {TEST_CID}, port {TEST_PORT}",
            TEST_LOG_MESSAGE_PREFIX
        );

        let message = b"Hello from the guest!";
        client
            .send(message, TEST_CHUNK_SIZE)
            .expect("Failed to send message to vsock server");
        tracing::info!(
            "{} Sent message to vsock server: {}",
            TEST_LOG_MESSAGE_PREFIX,
            format_byte_vec(message)
        );
    });

    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    tracing::info!(
        "{} Vsock server listening on CID {TEST_CID}, port {TEST_PORT}",
        TEST_LOG_MESSAGE_PREFIX
    );

    sock.listen().expect("Failed to listen on vsock server");
    tracing::info!(
        "{} Vsock server is now listening for connections",
        TEST_LOG_MESSAGE_PREFIX
    );

    let client_sock = Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
    tracing::info!(
        "{} Accepted connection from vsock client",
        TEST_LOG_MESSAGE_PREFIX
    );

    let message = client_sock
        .receive(TEST_MAX_SIZE, TEST_CHUNK_SIZE)
        .expect("Failed to receive from vsock server");
    handle.join().expect("Client thread panicked");
    tracing::info!(
        "{} Received response from vsock server: {}",
        TEST_LOG_MESSAGE_PREFIX,
        format_byte_vec(&message)
    );
    tracing::info!(
        "{} Message in UTF-8: {}",
        TEST_LOG_MESSAGE_PREFIX,
        String::from_utf8_lossy(&message)
    );
}

#[test]
#[should_panic(expected = "Permission denied")]
fn test_vsock_bind_on_reserved_port_fail() {
    init_tracing();
    const RESERVED_PORT: u32 = 10;
    Vsock::bind(RESERVED_PORT).unwrap();
}

#[test]
#[serial]
#[should_panic(expected = "Invalid argument")]
fn test_vsock_connect_to_socket_not_accepting_connections_fail() {
    init_tracing();
    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    tracing::info!(
        "{} Vsock server listening on CID {TEST_CID}, port {TEST_PORT}",
        TEST_LOG_MESSAGE_PREFIX
    );

    Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
}

#[test]
#[serial]
#[should_panic(expected = "Address already in use")]
fn test_vsock_bind_to_used_port_fail() {
    init_tracing();
    let _sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    tracing::info!(
        "{} Vsock server no. 1 bound to port {TEST_PORT}",
        TEST_LOG_MESSAGE_PREFIX
    );

    Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    tracing::info!(
        "{} Vsock server no. 2 bound to port {TEST_PORT}",
        TEST_LOG_MESSAGE_PREFIX
    );
}

#[test]
#[serial]
#[should_panic(expected = "Buffer too small")]
fn test_vsock_connection_chunk_bigger_than_max_size_fail() {
    init_tracing();
    thread::spawn(|| {
        let client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        tracing::info!(
            "{} Connected to vsock server at CID {TEST_CID}, port {TEST_PORT}",
            TEST_LOG_MESSAGE_PREFIX
        );

        let message = b"Hello from the guest!";
        client
            .send(message, TEST_CHUNK_SIZE)
            .expect("Failed to send message to vsock server");
        tracing::info!(
            "{} Sent message to vsock server: {}",
            TEST_LOG_MESSAGE_PREFIX,
            format_byte_vec(message)
        );
    });

    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    tracing::info!(
        "{} Vsock server listening on CID {TEST_CID}, port {TEST_PORT}",
        TEST_LOG_MESSAGE_PREFIX
    );

    sock.listen().expect("Failed to listen on vsock server");
    tracing::info!(
        "{} Vsock server is now listening for connections",
        TEST_LOG_MESSAGE_PREFIX
    );

    let client_sock = Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
    tracing::info!(
        "{} Accepted connection from vsock client",
        TEST_LOG_MESSAGE_PREFIX
    );

    client_sock
        .receive(16, TEST_CHUNK_SIZE)
        .expect("Failed to receive from vsock server");
}

#[test]
#[serial]
#[should_panic(expected = "Connect failed")]
fn test_vsock_connection_noone_listening_fail() {
    init_tracing();
    const MAX_ATTEMPTS: usize = 1;
    Vsock::connect_with_max_attempts(TEST_CID, TEST_PORT, MAX_ATTEMPTS)
        .expect("Failed to connect to vsock server");
}

#[cfg(feature = "std-io")]
#[test]
#[serial]
fn test_std_io_vsock_connection_valid_transfer_ok() {
    init_tracing();
    let handle = thread::spawn(|| {
        let mut client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        tracing::info!(
            "{} Connected to vsock server at CID {TEST_CID}, port {TEST_PORT}",
            TEST_LOG_MESSAGE_PREFIX
        );

        let message = b"Hello from the guest!";
        client
            .write_all(message)
            .expect("Failed to send message to vsock server");
        tracing::info!(
            "{} Sent message to vsock server: {}",
            TEST_LOG_MESSAGE_PREFIX,
            format_byte_vec(message)
        );
        client.flush().expect("Failed to flush vsock client");
        tracing::info!(
            "{} Flushed vsock client to ensure message is sent",
            TEST_LOG_MESSAGE_PREFIX
        );
    });

    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    tracing::info!(
        "{} Vsock server listening on CID {TEST_CID}, port {TEST_PORT}",
        TEST_LOG_MESSAGE_PREFIX
    );

    sock.listen().expect("Failed to listen on vsock server");
    tracing::info!(
        "{} Vsock server is now listening for connections",
        TEST_LOG_MESSAGE_PREFIX
    );

    let mut client_sock =
        Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
    tracing::info!(
        "{} Accepted connection from vsock client",
        TEST_LOG_MESSAGE_PREFIX
    );

    let mut buffer = vec![];
    let read_size = client_sock
        .read_to_end(&mut buffer)
        .expect("Failed to receive from vsock server");
    handle.join().expect("Client thread panicked");
    tracing::info!(
        "{} Read {} bytes from vsock client",
        TEST_LOG_MESSAGE_PREFIX,
        read_size
    );
    tracing::info!(
        "{} Received response from vsock server: {}",
        TEST_LOG_MESSAGE_PREFIX,
        format_byte_vec(&buffer)
    );
    tracing::info!("Message in UTF-8: {}", String::from_utf8_lossy(&buffer));
}
