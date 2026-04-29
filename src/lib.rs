//! Tiny vsock is a Rust library that provides an abstraction over vsock (`AF_VSOCK`) sockets
//! for communication between hosts and virtual machines or enclaves. It offers a simple API
//! for creating, connecting, binding, listening, accepting, sending, and receiving data through
//! vsock sockets. The library is designed to be efficient and easy to use, with built-in retry
//! mechanisms for connection attempts and chunked data transfer for optimal performance.
//!
//! # Features
//! - `std-io`: Enables implementation of the standard `Read` and `Write` traits for `Vsock`,
//!   allowing it to be used with standard Rust I/O patterns.
//!
//! # Examples
//!
//! - [`enclave-echo-service`](https://github.com/orlowskilp/tiny-vsock/blob/master/examples/enclave-echo-service.rs)
//!   — binds, accepts one
//!   connection, receives data, and echoes it back; run inside the enclave
//! - [`parent-echo-client`](https://github.com/orlowskilp/tiny-vsock/blob/master/examples/parent-echo-client.rs)
//!   — connects to the enclave
//!   service, sends a message, and reads the echo; run on the parent instance
//! - [`std_io_echo`](https://github.com/orlowskilp/tiny-vsock/blob/master/examples/std_io_echo.rs)
//!   — demonstrates `std::io::Read` /
//!   `std::io::Write` via `BufReader` and `flush`; requires `--features std-io`

use anyhow::{Result, anyhow};
#[cfg(feature = "std-io")]
use nix::sys::socket::Shutdown;
use nix::{
    Error as NixError,
    sys::socket::{self, AddressFamily, Backlog, MsgFlags, SockFlag, SockType, VsockAddr},
};
#[cfg(feature = "std-io")]
use std::io::{Error as IoError, Read, Result as IoResult, Write};
use std::{
    os::{
        fd::{AsFd, BorrowedFd, FromRawFd, OwnedFd},
        unix::io::{AsRawFd, RawFd},
    },
    result::Result as StdResult,
    thread::sleep,
    time::Duration,
};

/// Abstraction over a vsock (`AF_VSOCK`) socket for communication between hosts and virtual machines
/// or enclaves.
pub struct Vsock {
    /// Vsock socket file descriptor.
    socket_fd: OwnedFd,
}

impl AsRawFd for Vsock {
    /// Return the raw file descriptor of the Vsock socket.
    fn as_raw_fd(&self) -> RawFd {
        self.socket_fd.as_raw_fd()
    }
}

impl AsFd for Vsock {
    /// Return the Vsock file descriptor as a `BorrowedFd` for use with nix socket operations.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket_fd.as_fd()
    }
}

impl Vsock {
    /// Maximum number of attempts to connect to a Vsock socket before giving up.
    const CONNECT_ATTEMPTS: usize = 5;
    /// CID address to bind to for accepting connections from any CID. This is a convention
    /// in vsock
    pub const ANY_CID_ADDR: u32 = u32::MAX;
    /// CID address used for communication with the Nitro Enclave parent instance. This is a
    /// convention in AWS Nitro Enclaves and is not meant to be used for general vsock
    /// communication outside of Nitro Enclaves.
    pub const PARENT_NE_CID_ADDR: u32 = 3;

    /// Idiomatic method to create a new Vsock instance from a given socket file descriptor. Not meant
    /// for public use.
    fn new(socket_fd: OwnedFd) -> Self {
        Vsock { socket_fd }
    }

    /// Shorthand method to create a new Vsock socket. Not meant for public use.
    fn socket() -> Result<OwnedFd> {
        socket::socket(AddressFamily::Vsock, SockType::Stream, SockFlag::empty(), None).map_err(|err| {
            tracing::error!("Vsock: Create socket failed: {err:#?}");
            anyhow!(err)
        })
    }

    /// Connect to a Vsock socket with a given CID and port and return a vsock handle.
    ///
    /// # Arguments
    ///
    /// * `cid` - CID address to connect to.
    /// * `port` - Port to connect to.
    ///
    /// # Returns
    ///
    /// A `Vsock` instance representing the connected socket or an error if the connection
    /// fails after 5 attempts. The method implements an exponential backoff strategy for
    /// retrying failed connection attempts.
    pub fn connect(cid: u32, port: u32) -> Result<Self> {
        Self::connect_with_max_attempts(cid, port, Self::CONNECT_ATTEMPTS)
    }

    /// Connect to a Vsock socket with a given CID and port and return a vsock handle.
    ///
    /// # Arguments
    ///
    /// * `cid` - CID address to connect to.
    /// * `port` - Port to connect to.
    /// * `max_attempts` - Maximum number of attempts to connect before giving up.
    ///
    /// # Returns
    ///
    /// A `Vsock` instance representing the connected socket or an error if the connection
    /// fails after the specified number of attempts. The method implements an exponential
    /// backoff strategy for retrying failed connection attempts.
    pub fn connect_with_max_attempts(cid: u32, port: u32, max_attempts: usize) -> Result<Self> {
        for i in 0..max_attempts {
            let vsock = Self::new(Self::socket()?);
            match socket::connect(vsock.as_raw_fd(), &VsockAddr::new(cid, port)) {
                Ok(_) => return Ok(vsock),
                Err(err) => {
                    tracing::warn!("Vsock: Connect attempt {} failed: {err:#?}, retrying...", i + 1)
                }
            }
            // Exponentially backoff before retrying to connect to the socket
            sleep(Duration::from_secs(1 << i));
        }

        tracing::error!("Vsock: Connect failed after {max_attempts} attempts");
        Err(anyhow!("Vsock: Connect failed"))
    }

    /// Bind to a Vsock socket with a given port and return a vsock handle.
    ///
    /// # Arguments
    ///
    /// * `port` - Port to bind to.
    ///
    /// # Returns
    ///
    /// A `Vsock` instance representing the bound socket, or an error if the bind operation fails.
    pub fn bind(port: u32) -> Result<Self> {
        let socket_fd = Vsock::socket()?;
        let sock_addr = VsockAddr::new(Self::ANY_CID_ADDR, port);
        socket::bind(socket_fd.as_raw_fd(), &sock_addr)
            .map_err(|err| {
                tracing::error!("Vsock: Bind failed: {err:#?}");
                anyhow!(err)
            })
            .map(|_| {
                tracing::debug!("Vsock: Bound to port {port}");
                Self::new(socket_fd)
            })
    }

    /// Listen for incoming connections on a Vsock socket.
    pub fn listen(&self) -> Result<()> {
        const MAX_QUEUE_LEN: i32 = 128;
        socket::listen(&self.as_fd(), Backlog::new(MAX_QUEUE_LEN)?).map_err(|err| {
            tracing::error!("Vsock: Listen failed: {err:#?}");
            anyhow!(err)
        })
    }

    /// Accept an incoming connection on a Vsock socket and return a new Vsock instance
    /// representing the accepted connection.
    pub fn accept(&self) -> Result<Self> {
        socket::accept(self.as_raw_fd())
            .map_err(|err| {
                tracing::error!("Vsock: Accept failed: {err:#?}");
                anyhow!(err)
            })
            .map(|raw_fd| {
                // Safety: We own the raw fd returned by accept
                unsafe { Self::new(OwnedFd::from_raw_fd(raw_fd)) }
            })
    }

    /// Send a slice of bytes through a Vsock socket. The method makes assumptions about the
    /// transport chunk size to optimize performance.
    ///
    /// # Arguments
    ///
    /// * `data` - Slice of bytes to be sent through the Vsock socket entirely.
    /// * `chunk_size` - Size of each chunk to send through the socket.
    ///
    /// # Returns
    ///
    /// Empty result indicating success or error if the operation fails.
    pub fn send(&self, data: &[u8], chunk_size: usize) -> Result<()> {
        Self::send_loop(data, chunk_size, |chunk| socket::send(self.as_raw_fd(), chunk, MsgFlags::empty()))
    }

    /// Core send loop parameterized over the underlying transport operation.
    ///
    /// Extracted to enable unit testing of the chunking and position-tracking logic without
    /// a real socket file descriptor. The public `send` method delegates here, passing a
    /// closure that calls `socket::send`.
    ///
    /// # Arguments
    ///
    /// * `data` - Slice of bytes to be sent entirely.
    /// * `chunk_size` - Maximum bytes handed to `transport` per call.
    /// * `transport` - Called repeatedly with successive slices of `data`; returns the number
    ///   of bytes consumed, `0` for a remote close, or a [`NixError`] on failure.
    fn send_loop(
        data: &[u8], chunk_size: usize, mut transport: impl FnMut(&[u8]) -> StdResult<usize, NixError>,
    ) -> Result<()> {
        let mut position = 0;
        loop {
            let left = position;
            let right = left + chunk_size.min(data.len() - left);
            position += match transport(&data[left..right]) {
                Ok(0) => {
                    tracing::warn!("Vsock: Remote closed connection, total bytes sent: {position}");
                    break Ok(());
                }
                Ok(data_len) => {
                    tracing::trace!("Vsock: Bytes sent: {data_len}");
                    data_len
                }
                // Interrupt signal: non-critical retry
                Err(NixError::EINTR) => {
                    tracing::warn!("Vsock: Send interrupted by EINTR, retrying...");
                    continue;
                }
                Err(err) => {
                    tracing::error!("Vsock: Send failed: {err:#?}");
                    break Err(anyhow!(err));
                }
            };
            if position == data.len() {
                tracing::debug!("Vsock: Send completed, total bytes sent: {position}");
                break Ok(());
            }
            if position > data.len() {
                tracing::error!("Vsock: Send exceeded data length");
                break Err(anyhow!("Vsock: Send exceeded data length"));
            }
        }
    }

    /// Receive bytes from a Vsock socket. The method reads data in chunks up to the specified
    /// maximum buffer size. The chunk size can be configured for optimal performance.
    ///
    /// # Arguments
    ///
    /// * `max_data_size` - Total capacity of the receive buffer in bytes; must be greater than or
    ///   equal to `chunk_size`.
    /// * `chunk_size` - Size of each chunk to read from the socket in bytes.
    ///
    /// # Returns
    ///
    /// A `Vec<u8>` containing the received bytes on success, truncated to the number of bytes
    /// actually received. Returns an error if `max_data_size` is less than `chunk_size`, if the
    /// buffer fills completely before the peer closes the connection, or if the underlying socket
    /// operation fails.
    pub fn receive(&self, max_data_size: usize, chunk_size: usize) -> Result<Vec<u8>> {
        if max_data_size < chunk_size {
            tracing::error!("Vsock: Buffer length less than chunk size: {max_data_size} < {chunk_size}");
            return Err(anyhow!("Vsock: Buffer too small"));
        }
        Self::receive_loop(max_data_size, chunk_size, |buf| socket::recv(self.as_raw_fd(), buf, MsgFlags::empty()))
    }

    /// Core receive loop parameterized over the underlying transport operation.
    ///
    /// Extracted to enable unit testing of the chunking and position-tracking logic without
    /// a real socket file descriptor. The public `receive` method validates arguments and then
    /// delegates here, passing a closure that calls `socket::recv`.
    ///
    /// # Arguments
    ///
    /// * `max_data_size` - Total capacity of the receive buffer; must be `>= chunk_size` (the
    ///   caller is responsible for enforcing this precondition before calling `receive_loop`).
    /// * `chunk_size` - Maximum bytes requested from `transport` per call.
    /// * `transport` - Called repeatedly with a mutable slice of the buffer; returns the number
    ///   of bytes written, `0` for a remote close, or a [`NixError`] on failure.
    fn receive_loop(
        max_data_size: usize, chunk_size: usize, mut transport: impl FnMut(&mut [u8]) -> StdResult<usize, NixError>,
    ) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; max_data_size];
        let mut position = 0;
        loop {
            let left = position;
            let right = left + chunk_size.min(max_data_size - left);
            let recv_data_len = match transport(&mut buffer[left..right]) {
                Ok(0) => {
                    tracing::warn!("Vsock: Remote closed connection, total bytes received: {position}");
                    break Ok(buffer[..position].to_vec());
                }
                Ok(data_len) => {
                    tracing::trace!("Vsock: Bytes received: {data_len}");
                    data_len
                }
                // Interrupt signal: non-critical retry
                Err(NixError::EINTR) => {
                    tracing::warn!("Vsock: Recv interrupted by EINTR, retrying...");
                    continue;
                }
                Err(err) => {
                    tracing::error!("Vsock: Recv failed: {err:#?}");
                    break Err(anyhow!(err));
                }
            };
            position += recv_data_len;
            if recv_data_len < chunk_size {
                tracing::debug!("Vsock: Recv completed, total bytes received: {position}");
                break Ok(buffer[..position].to_vec());
            }
            if position >= max_data_size {
                tracing::error!("Vsock: Recv buffer full");
                break Err(anyhow!("Vsock: Recv buffer full"));
            }
        }
    }
}

#[cfg(feature = "std-io")]
impl Write for Vsock {
    /// Write a slice of bytes to the Vsock socket. The method assumes that the entire buffer can be sent
    /// in one call for optimal performance. For larger buffers, the `send` method with chunking should
    /// be used instead.
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        loop {
            match socket::send(self.as_raw_fd(), buf, MsgFlags::empty()) {
                Ok(0) => {
                    tracing::warn!("Vsock: Remote closed connection, total bytes sent: 0");
                    break Ok(0);
                }
                Ok(data_len) => {
                    tracing::trace!("Vsock: Bytes sent: {data_len}");
                    break Ok(data_len);
                }
                // Interrupt signal: non-critical retry
                Err(NixError::EINTR) => {
                    tracing::warn!("Vsock: Send interrupted by EINTR, retrying...");
                    continue;
                }
                Err(errno) => {
                    tracing::error!("Vsock: Send failed: {errno:#?}");
                    break Err(IoError::other(errno));
                }
            }
        }
    }

    /// Shut down the write side of the Vsock socket, signaling EOF to the peer.
    ///
    /// This allows the peer's `read_to_end` or `read_to_string` calls to return cleanly.
    /// After this call, any further attempt to write to the socket will fail.
    ///
    /// **Note**: This operation is irreversible — the socket cannot be written to afterwards.
    fn flush(&mut self) -> IoResult<()> {
        socket::shutdown(self.as_raw_fd(), Shutdown::Write).map_err(|err| {
            tracing::error!("Vsock: Shutdown write failed: {err:?}");
            IoError::other(err)
        })
    }
}

#[cfg(feature = "std-io")]
impl Read for Vsock {
    /// Read bytes from the Vsock socket into a provided buffer. The method assumes that the buffer is large
    /// enough to hold the incoming data for optimal performance. For larger buffers, the `receive` method
    /// with chunking and data size cap should be used instead.
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        loop {
            match socket::recv(self.as_raw_fd(), buf, MsgFlags::empty()) {
                Ok(0) => {
                    tracing::warn!("Vsock: Remote closed connection, total bytes received: 0");
                    break Ok(0);
                }
                Ok(data_len) => {
                    tracing::trace!("Vsock: Bytes received: {data_len}");
                    break Ok(data_len);
                }
                // Interrupt signal: non-critical retry
                Err(NixError::EINTR) => {
                    tracing::warn!("Vsock: Recv interrupted by EINTR, retrying...");
                    continue;
                }
                Err(errno) => {
                    tracing::error!("Vsock: Recv failed: {errno:#?}");
                    break Err(IoError::other(errno));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::errno::Errno;

    // ── receive() input validation ────────────────────────────────────────────

    // The validation check in `receive` is pure Rust and runs before any vsock
    // syscall. We can reach it by wrapping a real (but non-vsock) OS file
    // descriptor obtained from `nix::unistd::pipe`, which is always available.
    // The `Vsock` wrapper is dropped at the end of each test, closing its fd.
    fn make_pipe_vsock() -> Vsock {
        use nix::unistd::pipe;
        // `pipe()` returns (read_end, write_end); we use the read end.
        let (read_fd, write_fd) = pipe().expect("pipe() failed in test");
        // Explicitly drop the write end so it doesn't leak.
        drop(write_fd);
        Vsock::new(read_fd)
    }

    #[test]
    #[should_panic(expected = "Buffer too small")]
    fn receive_returns_error_when_max_size_less_than_chunk_size_fail() {
        const MAX_DATA_SIZE: usize = 16;
        const CHUNK_SIZE: usize = 32;
        let vsock = make_pipe_vsock();
        vsock.receive(MAX_DATA_SIZE, CHUNK_SIZE).unwrap();
    }

    #[test]
    #[should_panic(expected = "Buffer too small")]
    fn receive_returns_error_when_max_size_equals_zero_and_chunk_size_nonzero_fail() {
        const MAX_DATA_SIZE: usize = 0;
        const MIN_NONZERO_CHUNK_SIZE: usize = 1;
        let vsock = make_pipe_vsock();
        vsock.receive(MAX_DATA_SIZE, MIN_NONZERO_CHUNK_SIZE).unwrap();
    }

    #[test]
    #[should_panic(expected = "Buffer too small")]
    fn receive_returns_error_when_max_size_one_less_than_chunk_size_fail() {
        const CHUNK_SIZE: usize = 1024;
        const MAX_DATA_SIZE: usize = CHUNK_SIZE - 1;
        let vsock = make_pipe_vsock();
        vsock.receive(MAX_DATA_SIZE, CHUNK_SIZE).unwrap();
    }

    // ── connect_with_max_attempts() zero-iteration path ───────────────────────

    #[test]
    #[should_panic(expected = "Connect failed")]
    fn connect_with_zero_attempts_returns_error_immediately_fail() {
        const PORT: u32 = 12345;
        Vsock::connect_with_max_attempts(u32::MAX, PORT, 0).unwrap();
    }

    // ── send_loop() unit tests (fake transport) ───────────────────────────────

    #[test]
    fn send_loop_delivers_entire_payload_in_one_chunk_ok() {
        const CHUNK_SIZE: usize = 64;
        let data = b"hello";
        let mut received: Vec<u8> = Vec::new();
        Vsock::send_loop(data, CHUNK_SIZE, |chunk| {
            received.extend_from_slice(chunk);
            Ok(chunk.len())
        })
        .unwrap();
        assert_eq!(received, data);
    }

    #[test]
    fn send_loop_splits_payload_across_multiple_chunks_ok() {
        const CHUNK_SIZE: usize = 3;
        const PAYLOAD_LEN: u8 = 10;
        let data: Vec<u8> = (0u8..PAYLOAD_LEN).collect();
        let mut calls: Vec<Vec<u8>> = Vec::new();
        Vsock::send_loop(&data, CHUNK_SIZE, |chunk| {
            calls.push(chunk.to_vec());
            Ok(chunk.len())
        })
        .unwrap();
        // 10 bytes / chunk_size 3  →  slices of 3, 3, 3, 1
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0], &data[0..3]);
        assert_eq!(calls[1], &data[3..6]);
        assert_eq!(calls[2], &data[6..9]);
        assert_eq!(calls[3], &data[9..10]);
    }

    #[test]
    fn send_loop_retries_on_eintr_and_still_completes_ok() {
        const CHUNK_SIZE: usize = 64;
        let data = b"retry";
        let mut call_count = 0usize;
        // Return EINTR on the first call, succeed on the second.
        Vsock::send_loop(data, CHUNK_SIZE, |chunk| {
            call_count += 1;
            if call_count == 1 { Err(Errno::EINTR) } else { Ok(chunk.len()) }
        })
        .unwrap();
        assert_eq!(call_count, 2);
    }

    #[test]
    fn send_loop_stops_cleanly_when_transport_returns_zero_ok() {
        const CHUNK_SIZE: usize = 64;
        let data = b"goodbye";
        let mut call_count = 0usize;
        Vsock::send_loop(data, CHUNK_SIZE, |_| {
            call_count += 1;
            Ok(0) // peer closed
        })
        .unwrap();
        // A zero-byte send is treated as a graceful close, not an error.
        assert_eq!(call_count, 1);
    }

    #[test]
    #[should_panic(expected = "ECONNRESET")]
    fn send_loop_propagates_transport_error_fail() {
        const CHUNK_SIZE: usize = 64;
        let data = b"oops";
        Vsock::send_loop(data, CHUNK_SIZE, |_| Err(Errno::ECONNRESET)).unwrap();
    }

    #[test]
    fn send_loop_with_empty_data_calls_transport_once_with_empty_slice_ok() {
        // When data is empty: data.len() == 0, so left == right == 0 on the
        // first (and only) iteration. The transport receives &[] and returns
        // Ok(0), which triggers the "remote closed" branch — Ok(()) is returned.
        const CHUNK_SIZE: usize = 8;
        let data: &[u8] = b"";
        let mut call_count = 0usize;
        Vsock::send_loop(data, CHUNK_SIZE, |chunk| {
            call_count += 1;
            assert!(chunk.is_empty(), "transport must receive an empty slice");
            Ok(chunk.len()) // 0 → remote-closed branch
        })
        .unwrap();
        assert_eq!(call_count, 1);
    }

    // ── receive_loop() unit tests (fake transport) ────────────────────────────

    #[test]
    fn receive_loop_collects_single_partial_chunk_on_short_read_ok() {
        // A `recv_data_len < chunk_size` read signals end-of-stream.
        const MAX_DATA_SIZE: usize = 64;
        const CHUNK_SIZE: usize = 16;
        let payload = b"hi";
        let result = Vsock::receive_loop(MAX_DATA_SIZE, CHUNK_SIZE, |buf| {
            let n = payload.len().min(buf.len());
            buf[..n].copy_from_slice(&payload[..n]);
            Ok(n) // n < chunk_size → loop exits
        });
        assert_eq!(result.unwrap(), payload);
    }

    #[test]
    fn receive_loop_reassembles_multiple_full_chunks_followed_by_short_read_ok() {
        // Three full chunks of 4 bytes each, then a short final read.
        const MAX_DATA_SIZE: usize = 64;
        const CHUNK_SIZE: usize = 4;
        const PAYLOAD_LEN: u8 = 13;
        let payload: Vec<u8> = (0u8..PAYLOAD_LEN).collect();
        let mut pos = 0usize;
        let result = Vsock::receive_loop(MAX_DATA_SIZE, CHUNK_SIZE, |buf| {
            let remaining = payload.len() - pos;
            let n = remaining.min(buf.len());
            buf[..n].copy_from_slice(&payload[pos..pos + n]);
            pos += n;
            Ok(n)
        });
        assert_eq!(result.unwrap(), payload);
    }

    #[test]
    fn receive_loop_stops_cleanly_on_remote_close_returning_zero_ok() {
        const MAX_DATA_SIZE: usize = 64;
        const CHUNK_SIZE: usize = 8;
        let result = Vsock::receive_loop(MAX_DATA_SIZE, CHUNK_SIZE, |_| {
            Ok(0) // Zero bytes received before remote closed → empty vec.
        });
        assert_eq!(result.unwrap(), b"");
    }

    #[test]
    fn receive_loop_retries_on_eintr_and_still_completes_ok() {
        const MAX_DATA_SIZE: usize = 64;
        const CHUNK_SIZE: usize = 16;
        let payload = b"interrupt";
        let mut call_count = 0usize;
        let result = Vsock::receive_loop(MAX_DATA_SIZE, CHUNK_SIZE, |buf| {
            call_count += 1;
            if call_count == 1 {
                Err(Errno::EINTR)
            } else {
                let n = payload.len().min(buf.len());
                buf[..n].copy_from_slice(&payload[..n]);
                Ok(n)
            }
        });
        let result = result.unwrap();
        assert_eq!(call_count, 2);
        assert_eq!(result, payload);
    }

    #[test]
    #[should_panic(expected = "ECONNRESET")]
    fn receive_loop_returns_error_when_transport_fails_fail() {
        const MAX_DATA_SIZE: usize = 64;
        const CHUNK_SIZE: usize = 8;
        Vsock::receive_loop(MAX_DATA_SIZE, CHUNK_SIZE, |_| Err(Errno::ECONNRESET)).unwrap();
    }

    #[test]
    #[should_panic(expected = "Recv buffer full")]
    fn receive_loop_returns_buffer_full_error_when_capacity_exhausted_fail() {
        // Transport always returns a full chunk, so position will reach max_data_size.
        let max_data_size = 8;
        let chunk_size = 4;
        Vsock::receive_loop(max_data_size, chunk_size, |buf| Ok(buf.len())).unwrap();
    }

    #[test]
    fn receive_loop_last_chunk_is_clamped_to_remaining_capacity_ok() {
        // max_data_size = 10, chunk_size = 4 → chunks of 4, 4, 2.
        // Verify the final slice handed to transport has length 2, not 4.
        let max_data_size = 10usize;
        let chunk_size = 4usize;
        let mut chunk_lengths: Vec<usize> = Vec::new();
        // Transport fills each chunk fully (simulating a full read) except the last
        // where the slice is already smaller than chunk_size, so the loop exits.
        let result = Vsock::receive_loop(max_data_size, chunk_size, |buf| {
            chunk_lengths.push(buf.len());
            Ok(buf.len())
        });
        // After two full chunks (8 bytes), the third window is 2 bytes — shorter
        // than chunk_size — so the loop detects buffer full and returns an error
        // (position 8 >= max_data_size 10 is false after chunk 2; the third call
        // fills 2 bytes returning 2 < 4, so the short-read branch fires first).
        // What matters for this test: the third chunk is 2 bytes, not 4.
        assert!(chunk_lengths.len() >= 3);
        assert_eq!(chunk_lengths[2], 2);
        // Result is Ok because the short-read branch fires before buffer-full.
        result.unwrap();
    }
}
