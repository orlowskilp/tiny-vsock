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
const TEST_MESSAGE: &[u8] = b"We're the knights who say ni!";

#[test]
#[serial]
fn test_vsock_connection_valid_transfer_ok() {
    init_tracing();
    let client_handle = thread::spawn(|| {
        let client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        client
            .send(TEST_MESSAGE, TEST_CHUNK_SIZE)
            .expect("Failed to send message to vsock server");
    });

    let socket = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    socket.listen().expect("Failed to listen on vsock server");

    let client_socket =
        Vsock::accept(&socket).expect("Failed to accept connection from vsock client");
    let recv_msg = client_socket
        .receive(TEST_MAX_SIZE, TEST_CHUNK_SIZE)
        .expect("Failed to receive from vsock server");
    client_handle.join().expect("Client thread panicked");
    assert_eq!(recv_msg, TEST_MESSAGE);
}

#[test]
#[should_panic(expected = "Permission denied")]
fn test_vsock_bind_on_reserved_port_fail() {
    const RESERVED_PORT: u32 = 10;
    init_tracing();
    Vsock::bind(RESERVED_PORT).unwrap();
}

#[test]
#[serial]
#[should_panic(expected = "Invalid argument")]
fn test_vsock_connect_to_socket_not_accepting_connections_fail() {
    init_tracing();
    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
}

#[test]
#[serial]
#[should_panic(expected = "Address already in use")]
fn test_vsock_bind_to_used_port_fail() {
    init_tracing();
    let _sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
}

#[test]
#[serial]
#[should_panic(expected = "Buffer too small")]
fn test_vsock_connection_chunk_bigger_than_max_size_fail() {
    const TINY_BUFFER_SIZE: usize = 16;
    init_tracing();
    thread::spawn(|| {
        let client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        client
            .send(TEST_MESSAGE, TEST_CHUNK_SIZE)
            .expect("Failed to send message to vsock server");
    });

    let socket = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    socket.listen().expect("Failed to listen on vsock server");

    let client_socket =
        Vsock::accept(&socket).expect("Failed to accept connection from vsock client");
    client_socket
        .receive(TINY_BUFFER_SIZE, TEST_CHUNK_SIZE)
        .unwrap();
}

#[test]
#[serial]
#[should_panic(expected = "Connect failed")]
fn test_vsock_connection_noone_listening_fail() {
    const LOW_MAX_ATTEMPTS: usize = 1;
    init_tracing();
    Vsock::connect_with_max_attempts(TEST_CID, TEST_PORT, LOW_MAX_ATTEMPTS).unwrap();
}

#[test]
#[serial]
#[should_panic(expected = "Invalid argument")]
fn test_vsock_listen_on_already_connected_socket_fail() {
    init_tracing();
    let client_handle = thread::spawn(|| {
        Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
    });

    let socket = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    socket.listen().expect("Failed to listen on vsock server");

    let client_socket =
        Vsock::accept(&socket).expect("Failed to accept connection from vsock client");
    client_handle.join().expect("Client thread panicked");
    client_socket.listen().unwrap();
}

#[cfg(feature = "std-io")]
#[test]
#[serial]
fn test_std_io_vsock_connection_valid_transfer_ok() {
    init_tracing();
    let handle = thread::spawn(|| {
        let mut client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        client
            .write_all(TEST_MESSAGE)
            .expect("Failed to send message to vsock server");
        client.flush().expect("Failed to flush vsock client");
    });

    let socket = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    socket.listen().expect("Failed to listen on vsock server");

    let mut client_sock =
        Vsock::accept(&socket).expect("Failed to accept connection from vsock client");

    let mut buffer = vec![];
    client_sock
        .read_to_end(&mut buffer)
        .expect("Failed to receive from vsock server");
    handle.join().expect("Client thread panicked");
    assert_eq!(buffer, TEST_MESSAGE);
}
