use serial_test::serial;
#[cfg(feature = "std-io")]
use std::io::{Read as _, Write as _};
use std::thread;
use tiny_vsock::Vsock;

const TEST_CID: u32 = 2;
const TEST_PORT: u32 = 12345;
const TEST_CHUNK_SIZE: usize = 1024;
const TEST_MAX_SIZE: usize = 8 * TEST_CHUNK_SIZE;

#[test]
#[serial]
fn test_vsock_connection_valid_transfer_ok() {
    let handle = thread::spawn(|| {
        let client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        println!("Connected to vsock server at CID {TEST_CID}, port {TEST_PORT}");

        let message = b"Hello from the guest!";
        client
            .send(message, TEST_CHUNK_SIZE)
            .expect("Failed to send message to vsock server");
        println!("Sent message to vsock server: {:#x?}", message);
    });

    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    println!(
        "Vsock server listening on CID {}, port {}",
        TEST_CID, TEST_PORT
    );

    sock.listen().expect("Failed to listen on vsock server");
    println!("Vsock server is now listening for connections");

    let client_sock = Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
    println!("Accepted connection from vsock client");

    let message = client_sock
        .receive(TEST_MAX_SIZE, TEST_CHUNK_SIZE)
        .expect("Failed to receive from vsock server");
    handle.join().expect("Client thread panicked");
    println!("Received response from vsock server: {:#x?}", message);
    println!("Message in UTF-8: {}", String::from_utf8_lossy(&message));
}

#[test]
#[serial]
#[should_panic(expected = "Buffer too small")]
fn test_vsock_connection_chunk_bigger_than_max_size_fail() {
    thread::spawn(|| {
        let client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        println!("Connected to vsock server at CID {TEST_CID}, port {TEST_PORT}");

        let message = b"Hello from the guest!";
        client
            .send(message, TEST_CHUNK_SIZE)
            .expect("Failed to send message to vsock server");
        println!("Sent message to vsock server: {:#x?}", message);
    });

    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    println!(
        "Vsock server listening on CID {}, port {}",
        TEST_CID, TEST_PORT
    );

    sock.listen().expect("Failed to listen on vsock server");
    println!("Vsock server is now listening for connections");

    let client_sock = Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
    println!("Accepted connection from vsock client");

    client_sock
        .receive(16, TEST_CHUNK_SIZE)
        .expect("Failed to receive from vsock server");
}

#[test]
#[serial]
#[should_panic(expected = "Connect failed")]
fn test_vsock_connection_noone_listening_fail() {
    const MAX_ATTEMPTS: usize = 1;
    Vsock::connect_with_max_attempts(TEST_CID, TEST_PORT, MAX_ATTEMPTS)
        .expect("Failed to connect to vsock server");
}

#[cfg(feature = "std-io")]
#[test]
#[serial]
fn test_std_io_vsock_connection_valid_transfer_ok() {
    let handle = thread::spawn(|| {
        let mut client =
            Vsock::connect(TEST_CID, TEST_PORT).expect("Failed to connect to vsock server");
        println!("Connected to vsock server at CID {TEST_CID}, port {TEST_PORT}");

        let message = b"Hello from the guest!";
        client
            .write_all(message)
            .expect("Failed to send message to vsock server");
        println!("Sent message to vsock server: {:#x?}", message);
        client.flush().expect("Failed to flush vsock client");
        println!("Flushed vsock client to ensure message is sent");
    });

    let sock = Vsock::bind(TEST_PORT).expect("Failed to bind vsock server");
    println!(
        "Vsock server listening on CID {}, port {}",
        TEST_CID, TEST_PORT
    );

    sock.listen().expect("Failed to listen on vsock server");
    println!("Vsock server is now listening for connections");

    let mut client_sock =
        Vsock::accept(&sock).expect("Failed to accept connection from vsock client");
    println!("Accepted connection from vsock client");

    let mut buffer = vec![];
    let read_size = client_sock
        .read_to_end(&mut buffer)
        .expect("Failed to receive from vsock server");
    handle.join().expect("Client thread panicked");
    println!("Read {} bytes from vsock client", read_size);
    println!("Received response from vsock server: {:#x?}", buffer);
    println!("Message in UTF-8: {}", String::from_utf8_lossy(&buffer));
}
