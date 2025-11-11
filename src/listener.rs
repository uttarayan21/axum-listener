use axum::serve::Listener;
use std::net::ToSocketAddrs;
#[cfg(unix)]
use tokio::net::UnixListener;

/// A unified listener that can bind to either TCP or Unix Domain Socket addresses.
///
/// This enum allows you to create a single listener type that can handle both TCP and UDS
/// connections transparently. The specific variant is determined at runtime based on the
/// address format provided to [`DuplexListener::bind`].
///
/// # Examples
///
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use axum_listener::listener::DuplexListener;
///
/// // Bind to TCP
/// let tcp_listener = DuplexListener::bind("localhost:8080").await.unwrap();
///
/// // Bind to Unix Domain Socket (on Unix systems)
/// # #[cfg(unix)] {
/// let uds_listener = DuplexListener::bind("unix:/tmp/app.sock").await.unwrap();
/// # }
/// # });
/// ```
///
/// # Platform Support
///
/// - `Tcp` variant is available on all platforms
/// - `Uds` variant is only available on Unix-like systems
pub enum DuplexListener {
    /// A TCP listener for network connections
    Tcp(tokio::net::TcpListener),
    /// A Unix Domain Socket listener for local inter-process communication
    #[cfg(unix)]
    Uds(tokio::net::UnixListener),
}

/// An address that can represent either a TCP socket address or a Unix Domain Socket address.
///
/// This enum is used to represent the local and remote addresses for connections
/// accepted by [`DuplexListener`]. It automatically implements cleanup for UDS
/// socket files when the `remove-on-drop` feature is enabled.
///
/// # Examples
///
/// ```rust
/// use axum_listener::listener::DuplexAddr;
/// use std::str::FromStr;
///
/// // Parse a TCP address
/// let tcp_addr = DuplexAddr::from_str("127.0.0.1:8080").unwrap();
///
/// // Parse a UDS address (on Unix systems)
/// # #[cfg(unix)] {
/// let uds_addr = DuplexAddr::from_str("unix:/tmp/app.sock").unwrap();
/// # }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum DuplexAddr {
    /// A TCP socket address (IPv4 or IPv6)
    Tcp(core::net::SocketAddr),
    /// A Unix Domain Socket address
    #[cfg(unix)]
    Uds(tokio::net::unix::SocketAddr),
}

#[cfg(feature = "remove-on-drop")]
impl Drop for DuplexAddr {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let DuplexAddr::Uds(addr) = self {
            if let Some(path) = addr.as_pathname() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl From<core::net::SocketAddr> for DuplexAddr {
    fn from(addr: core::net::SocketAddr) -> Self {
        DuplexAddr::Tcp(addr)
    }
}

#[cfg(unix)]
impl From<tokio::net::unix::SocketAddr> for DuplexAddr {
    fn from(addr: tokio::net::unix::SocketAddr) -> Self {
        DuplexAddr::Uds(addr)
    }
}

impl core::str::FromStr for DuplexAddr {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let unix_like = s.starts_with("/") || s.starts_with("unix:");
        let has_uds = cfg!(unix);
        let tcp_like = s.to_socket_addrs().is_ok();

        if unix_like && has_uds && !tcp_like {
            #[cfg(unix)]
            {
                let path = s.trim_start_matches("unix:");
                let addr = From::from(std::os::unix::net::SocketAddr::from_pathname(path)?);
                Ok(DuplexAddr::Uds(addr))
            }
            #[cfg(not(unix))]
            {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unix domain sockets are not supported on this platform",
                ))
            }
        } else if tcp_like {
            let addr = s.to_socket_addrs()?.next().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid TCP address")
            })?;
            Ok(DuplexAddr::Tcp(addr))
        } else if unix_like && !has_uds {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unix domain sockets are not supported on this platform",
            ))
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid address format",
            ))
        }
    }
}

/// A trait for types that can be converted to a [`DuplexAddr`].
///
/// This trait enables convenient address binding by allowing various types
/// to be converted to the unified [`DuplexAddr`] type. It's implemented for
/// common address types including strings, socket addresses, and paths.
///
/// # Examples
///
/// ```rust
/// use axum_listener::listener::{ToDuplexAddr, DuplexAddr};
/// use std::net::SocketAddr;
///
/// // String addresses
/// let addr1 = "127.0.0.1:8080".to_duplex_addr().unwrap();
/// let addr2 = "unix:/tmp/app.sock".to_duplex_addr().unwrap();
///
/// // Socket address
/// let socket_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
/// let addr3 = socket_addr.to_duplex_addr().unwrap();
/// ```
pub trait ToDuplexAddr {
    /// Convert this type to a [`DuplexAddr`].
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the address format is invalid or
    /// if Unix Domain Sockets are not supported on the current platform.
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error>;
}

impl ToDuplexAddr for &str {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        self.parse()
    }
}

impl ToDuplexAddr for String {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        self.as_str().to_duplex_addr()
    }
}

impl ToDuplexAddr for core::net::SocketAddr {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        Ok(DuplexAddr::Tcp(*self))
    }
}

#[cfg(unix)]
impl ToDuplexAddr for tokio::net::unix::SocketAddr {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        Ok(DuplexAddr::Uds(self.clone()))
    }
}

impl ToDuplexAddr for DuplexAddr {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        Ok(self.clone())
    }
}

impl ToDuplexAddr for &DuplexAddr {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        Ok((*self).clone())
    }
}

#[cfg(unix)]
impl ToDuplexAddr for &std::path::Path {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        Ok(DuplexAddr::Uds(From::from(
            std::os::unix::net::SocketAddr::from_pathname(self)?,
        )))
    }
}

#[cfg(unix)]
impl ToDuplexAddr for std::path::PathBuf {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        self.as_path().to_duplex_addr()
    }
}

impl DuplexListener {
    /// Creates a new [`DuplexListener`] bound to the specified address.
    ///
    /// This method accepts any type that implements [`ToDuplexAddr`], allowing
    /// for flexible address specification. The listener type (TCP or UDS) is
    /// automatically determined based on the address format.
    ///
    /// # Arguments
    ///
    /// * `address` - An address that can be converted to [`DuplexAddr`]
    ///
    /// # Returns
    ///
    /// Returns a [`DuplexListener`] bound to the specified address, or an error
    /// if binding fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # tokio_test::block_on(async {
    /// use axum_listener::listener::DuplexListener;
    ///
    /// // Bind to TCP address
    /// let listener = DuplexListener::bind("localhost:8080").await.unwrap();
    ///
    /// // Bind to UDS address (Unix only)
    /// # #[cfg(unix)] {
    /// let listener = DuplexListener::bind("unix:/tmp/app.sock").await.unwrap();
    /// # }
    /// # });
    /// ```
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - The address format is invalid
    /// - The address is already in use
    /// - Permission is denied for the requested address
    /// - Unix Domain Sockets are not supported on the current platform
    pub async fn bind<A: ToDuplexAddr>(address: A) -> Result<Self, std::io::Error> {
        let address = address.to_duplex_addr()?;
        match address {
            DuplexAddr::Tcp(addr) => {
                let listener = tokio::net::TcpListener::bind(addr).await?;
                Ok(DuplexListener::Tcp(listener))
            }
            #[cfg(unix)]
            DuplexAddr::Uds(ref addr) => {
                let path = addr.as_pathname().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "UDS address does not have a valid pathname",
                    )
                })?;
                let listener = UnixListener::bind(path)?;
                Ok(DuplexListener::Uds(listener))
            }
            #[cfg(not(unix))]
            DuplexAddr::Uds(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unix domain sockets are not supported on this platform",
            )),
        }
    }

    /// Accepts a new incoming connection from this listener.
    ///
    /// This method will wait for a connection to be established and return
    /// a stream and address representing the connection.
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - [`DuplexStream`]: The stream for communicating with the client
    /// - [`DuplexAddr`]: The address of the connected client
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # tokio_test::block_on(async {
    /// use axum_listener::listener::DuplexListener;
    ///
    /// let listener = DuplexListener::bind("localhost:8080").await.unwrap();
    ///
    /// // Accept a connection
    /// let (stream, addr) = listener.accept().await.unwrap();
    /// println!("Accepted connection from: {:?}", addr);
    /// # });
    /// ```
    ///
    /// # Errors
    ///
    /// This method can fail if there's an I/O error while accepting the connection.
    pub async fn accept(&self) -> Result<(DuplexStream, DuplexAddr), std::io::Error> {
        match self {
            DuplexListener::Tcp(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((DuplexStream::Tcp(stream), DuplexAddr::Tcp(addr)))
            }
            #[cfg(unix)]
            DuplexListener::Uds(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((DuplexStream::Uds(stream), DuplexAddr::Uds(addr)))
            }
        }
    }

    pub(crate) fn _accept(
        &self,
    ) -> impl core::future::Future<Output = Result<(DuplexStream, DuplexAddr), std::io::Error>>
           + Unpin
           + use<'_> {
        Box::pin(async move {
            match self {
                DuplexListener::Tcp(listener) => {
                    let (stream, addr) = listener.accept().await?;
                    Ok((DuplexStream::Tcp(stream), DuplexAddr::Tcp(addr)))
                }
                #[cfg(unix)]
                DuplexListener::Uds(listener) => {
                    let (stream, addr) = listener.accept().await?;
                    Ok((DuplexStream::Uds(stream), DuplexAddr::Uds(addr)))
                }
            }
        })
    }
}

/// A stream that can be either a TCP stream or a Unix Domain Socket stream.
///
/// This enum provides a unified interface for both TCP and UDS connections,
/// implementing the necessary async I/O traits to work seamlessly with Axum
/// and other async frameworks.
///
/// # Examples
///
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use axum_listener::listener::DuplexListener;
///
/// let listener = DuplexListener::bind("localhost:8080").await.unwrap();
/// let (stream, _addr) = listener.accept().await.unwrap();
///
/// // The stream can be used with Axum or any other async framework
/// // that works with tokio's AsyncRead + AsyncWrite traits
/// println!("Accepted connection from: {:?}", _addr);
/// # });
/// ```
pub enum DuplexStream {
    /// A TCP stream for network connections
    Tcp(tokio::net::TcpStream),
    /// A Unix Domain Socket stream for local inter-process communication
    #[cfg(unix)]
    Uds(tokio::net::UnixStream),
}

impl From<tokio::net::TcpStream> for DuplexStream {
    fn from(stream: tokio::net::TcpStream) -> Self {
        DuplexStream::Tcp(stream)
    }
}

#[cfg(unix)]
impl From<tokio::net::UnixStream> for DuplexStream {
    fn from(stream: tokio::net::UnixStream) -> Self {
        DuplexStream::Uds(stream)
    }
}

impl tokio::io::AsyncRead for DuplexStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DuplexStream::Tcp(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            #[cfg(unix)]
            DuplexStream::Uds(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for DuplexStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            DuplexStream::Tcp(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
            #[cfg(unix)]
            DuplexStream::Uds(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DuplexStream::Tcp(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            #[cfg(unix)]
            DuplexStream::Uds(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DuplexStream::Tcp(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            #[cfg(unix)]
            DuplexStream::Uds(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl axum::serve::Listener for DuplexListener {
    type Io = DuplexStream;
    type Addr = DuplexAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self {
            DuplexListener::Tcp(listener) => {
                let (io, addr) = Listener::accept(listener).await;
                (DuplexStream::Tcp(io), DuplexAddr::Tcp(addr))
            }
            #[cfg(unix)]
            DuplexListener::Uds(listener) => {
                let (io, addr) = Listener::accept(listener).await;
                (DuplexStream::Uds(io), DuplexAddr::Uds(addr))
            }
        }
    }

    fn local_addr(&self) -> Result<Self::Addr, std::io::Error> {
        match self {
            DuplexListener::Tcp(listener) => Listener::local_addr(listener).map(DuplexAddr::Tcp),
            #[cfg(unix)]
            DuplexListener::Uds(listener) => Listener::local_addr(listener).map(DuplexAddr::Uds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_tcp_bind() {
        let listener = DuplexListener::bind("localhost:8080").await;
        assert!(listener.is_ok());
        if let DuplexListener::Tcp(tcp_listener) = listener.unwrap() {
            let addr = tcp_listener.local_addr().unwrap();
            assert_eq!(addr.port(), 8080);
        } else {
            panic!("Expected TCP listener");
        }
    }

    #[tokio::test]
    async fn test_uds_bind() {
        #[cfg(unix)]
        {
            let listener = DuplexListener::bind("/tmp/test.sock").await;
            assert!(listener.is_ok());
            if let DuplexListener::Uds(uds_listener) = listener.unwrap() {
                let addr = uds_listener.local_addr().unwrap();
                assert_eq!(addr.as_pathname().unwrap(), "/tmp/test.sock");
            } else {
                panic!("Expected UDS listener");
            }
        }
    }
}
