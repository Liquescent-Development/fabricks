//! TCP runtime types.

use std::net::SocketAddr;

use tokio::net::TcpStream;

/// A TCP connection received by the daemon.
#[derive(Debug)]
pub struct TcpConnection {
    /// The TCP stream.
    pub stream: TcpStream,

    /// The peer's socket address.
    pub peer_addr: SocketAddr,
}

impl TcpConnection {
    /// Create a new TCP connection.
    #[must_use]
    pub const fn new(stream: TcpStream, peer_addr: SocketAddr) -> Self {
        Self { stream, peer_addr }
    }
}
