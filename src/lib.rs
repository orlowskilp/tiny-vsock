//! Tiny vsock is a Rust library that provides an abstraction over vsock (`AF_VSOCK`) sockets
//! for communication between hosts and virtual machines or enclaves. It offers a simple API
//! for creating, connecting, binding, listening, accepting, sending, and receiving data through
//! vsock sockets. The library is designed to be efficient and easy to use, with built-in retry
//! mechanisms for connection attempts and chunked data transfer for optimal performance.
//!
//! # Features
//! - `std-io`: Enables implementation of the standard `Read` and `Write` traits for `Vsock`,
//!   allowing it to be used with standard Rust I/O patterns.

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
        socket::socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::empty(),
            None,
        )
        .map_err(|err| {
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
                    tracing::warn!(
                        "Vsock: Connect attempt {} failed: {err:#?}, retrying...",
                        i + 1
                    )
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
    /// A `Vsock` instance representing the bound socket or an error if the bind operation.
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
        let mut position = 0;
        loop {
            let left = position;
            let right = left + chunk_size.min(data.len() - left);
            position += match socket::send(self.as_raw_fd(), &data[left..right], MsgFlags::empty())
            {
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

    /// Receive bytes from a Vsock socket. The method makes assumptios about the maximum data size
    /// to be received in each chunk. The chunk size can be configured for optimal performance.
    ///
    /// # Arguments
    ///
    /// * `max_data_size` - Total size of the buffer to receive data into.
    /// * `chunk_size` - Size of each chunk to read from the socket.
    ///
    /// # Returns
    ///
    /// Vector of bytes received from the Vsock socket or error if the operation fails, or data
    /// exceeds buffer size. This is to prevent buffer overflow attacks.
    pub fn receive(&self, max_data_size: usize, chunk_size: usize) -> Result<Vec<u8>> {
        if max_data_size < chunk_size {
            tracing::error!(
                "Vsock: Buffer length less than chunk size: {max_data_size} < {chunk_size}"
            );
            return Err(anyhow!("Vsock: Buffer too small"));
        }
        let mut buffer = vec![0u8; max_data_size];
        let mut position = 0;
        loop {
            let left = position;
            let right = left + chunk_size.min(max_data_size - left);
            let recv_data_len = match socket::recv(
                self.as_raw_fd(),
                &mut buffer[left..right],
                MsgFlags::empty(),
            ) {
                Ok(0) => {
                    tracing::warn!(
                        "Vsock: Remote closed connection, total bytes received: {position}"
                    );
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

    /// Flush the Vsock socket. Since vsock is a stream-oriented socket, flush typically ensures
    /// all data is sent. We shutdown the write side to signal EOF, allowing read_to_end() to work properly.
    ///
    /// **Note**: After a socket is flushed you can no longer write to it!
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
