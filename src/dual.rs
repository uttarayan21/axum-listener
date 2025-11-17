use axum::serve::Listener;
use std::net::ToSocketAddrs;
#[cfg(unix)]
use tokio::net::UnixListener;

/// A unified listener that can bind to either TCP or Unix Domain Socket addresses.
///
/// This enum allows you to create a single listener type that can handle both TCP and UDS
/// connections transparently. The specific variant is determined at runtime based on the
/// address format provided to [`DualListener::bind`].
///
/// # Examples
///
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use axum_listener::listener::DualListener;
///
/// // Bind to TCP
/// let tcp_listener = DualListener::bind("localhost:8080").await.unwrap();
///
/// // Bind to Unix Domain Socket (on Unix systems)
/// # #[cfg(unix)] {
/// let uds_listener = DualListener::bind("unix:/tmp/app.sock").await.unwrap();
/// # }
/// # });
/// ```
///
/// # Platform Support
///
/// - `Tcp` variant is available on all platforms
/// - `Uds` variant is only available on Unix-like systems
#[derive(Debug)]
pub enum DualListener {
    /// A TCP listener for network connections
    Tcp(tokio::net::TcpListener),
    /// A Unix Domain Socket listener for local inter-process communication
    #[cfg(unix)]
    Uds(tokio::net::UnixListener),
}

/// An address that can represent either a TCP socket address or a Unix Domain Socket address.
///
/// This enum is used to represent the local and remote addresses for connections
/// accepted by [`DualListener`]. It automatically implements cleanup for UDS
/// socket files when the `remove-on-drop` feature is enabled.
///
/// # Examples
///
/// ```rust
/// use axum_listener::listener::DualAddr;
/// use std::str::FromStr;
///
/// // Parse a TCP address
/// let tcp_addr = DualAddr::from_str("127.0.0.1:8080").unwrap();
///
/// // Parse a UDS address (on Unix systems)
/// # #[cfg(unix)] {
/// let uds_addr = DualAddr::from_str("unix:/tmp/app.sock").unwrap();
/// # }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum DualAddr {
    /// A TCP socket address (IPv4 or IPv6)
    Tcp(core::net::SocketAddr),
    /// A Unix Domain Socket address
    #[cfg(unix)]
    Uds(tokio::net::unix::SocketAddr),
}

impl From<core::net::SocketAddr> for DualAddr {
    fn from(addr: core::net::SocketAddr) -> Self {
        DualAddr::Tcp(addr)
    }
}

#[cfg(unix)]
impl From<tokio::net::unix::SocketAddr> for DualAddr {
    fn from(addr: tokio::net::unix::SocketAddr) -> Self {
        DualAddr::Uds(addr)
    }
}

impl core::str::FromStr for DualAddr {
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
                Ok(DualAddr::Uds(addr))
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
            Ok(DualAddr::Tcp(addr))
        } else if unix_like && !has_uds {
            Err(std::io::Error::other(
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

/// A trait for types that can be converted to a [`DualAddr`].
///
/// This trait enables convenient address binding by allowing various types
/// to be converted to the unified [`DualAddr`] type. It's implemented for
/// common address types including strings, socket addresses, and paths.
///
/// # Examples
///
/// ```rust
/// use axum_listener::listener::{ToDualAddr, DualAddr};
/// use std::net::SocketAddr;
///
/// // String addresses
/// let addr1 = "127.0.0.1:8080".to_dual_addr().unwrap();
/// let addr2 = "unix:/tmp/app.sock".to_dual_addr().unwrap();
///
/// // Socket address
/// let socket_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
/// let addr3 = socket_addr.to_dual_addr().unwrap();
/// ```
pub trait ToDualAddr {
    /// Convert this type to a [`DualAddr`].
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the address format is invalid or
    /// if Unix Domain Sockets are not supported on the current platform.
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error>;
}

impl ToDualAddr for &str {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        self.parse()
    }
}

impl ToDualAddr for String {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        self.as_str().to_dual_addr()
    }
}

impl ToDualAddr for core::net::SocketAddr {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        Ok(DualAddr::Tcp(*self))
    }
}

#[cfg(unix)]
impl ToDualAddr for tokio::net::unix::SocketAddr {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        Ok(DualAddr::Uds(self.clone()))
    }
}

impl ToDualAddr for DualAddr {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        Ok(self.clone())
    }
}

impl ToDualAddr for &DualAddr {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        Ok((*self).clone())
    }
}

#[cfg(unix)]
impl ToDualAddr for &std::path::Path {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        Ok(DualAddr::Uds(From::from(
            std::os::unix::net::SocketAddr::from_pathname(self)?,
        )))
    }
}

#[cfg(unix)]
impl ToDualAddr for std::path::PathBuf {
    fn to_dual_addr(&self) -> Result<DualAddr, std::io::Error> {
        self.as_path().to_dual_addr()
    }
}

impl DualListener {
    /// Creates a new [`DualListener`] bound to the specified address.
    ///
    /// This method accepts any type that implements [`ToDualAddr`], allowing
    /// for flexible address specification. The listener type (TCP or UDS) is
    /// automatically determined based on the address format.
    ///
    /// # Arguments
    ///
    /// * `address` - An address that can be converted to [`DualAddr`]
    ///
    /// # Returns
    ///
    /// Returns a [`DualListener`] bound to the specified address, or an error
    /// if binding fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # tokio_test::block_on(async {
    /// use axum_listener::listener::DualListener;
    ///
    /// // Bind to TCP address
    /// let listener = DualListener::bind("localhost:8080").await.unwrap();
    ///
    /// // Bind to UDS address (Unix only)
    /// # #[cfg(unix)] {
    /// let listener = DualListener::bind("unix:/tmp/app.sock").await.unwrap();
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
    pub async fn bind<A: ToDualAddr>(address: A) -> Result<Self, std::io::Error> {
        let address = address.to_dual_addr()?;
        match address {
            DualAddr::Tcp(addr) => {
                let listener = tokio::net::TcpListener::bind(addr).await?;
                Ok(DualListener::Tcp(listener))
            }
            #[cfg(unix)]
            DualAddr::Uds(ref addr) => {
                let path = addr.as_pathname().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "UDS address does not have a valid pathname",
                    )
                })?;
                let listener = UnixListener::bind(path)?;
                Ok(DualListener::Uds(listener))
            }
            #[cfg(not(unix))]
            DualAddr::Uds(_) => Err(std::io::Error::new(
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
    /// - [`DualStream`]: The stream for communicating with the client
    /// - [`DualAddr`]: The address of the connected client
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # tokio_test::block_on(async {
    /// use axum_listener::listener::DualListener;
    ///
    /// let listener = DualListener::bind("localhost:8080").await.unwrap();
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
    pub async fn accept(&self) -> Result<(DualStream, DualAddr), std::io::Error> {
        match self {
            DualListener::Tcp(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((DualStream::Tcp(stream), DualAddr::Tcp(addr)))
            }
            #[cfg(unix)]
            DualListener::Uds(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((DualStream::Uds(stream), DualAddr::Uds(addr)))
            }
        }
    }

    pub(crate) fn _accept_unpin(
        &self,
    ) -> impl core::future::Future<Output = Result<(DualStream, DualAddr), std::io::Error>>
    + Unpin
    + use<'_> {
        Box::pin(async move {
            match self {
                DualListener::Tcp(listener) => {
                    let (stream, addr) = listener.accept().await?;
                    Ok((DualStream::Tcp(stream), DualAddr::Tcp(addr)))
                }
                #[cfg(unix)]
                DualListener::Uds(listener) => {
                    let (stream, addr) = listener.accept().await?;
                    Ok((DualStream::Uds(stream), DualAddr::Uds(addr)))
                }
            }
        })
    }
    pub(crate) async fn _accept_axum(&mut self) -> (DualStream, DualAddr) {
        match self {
            DualListener::Tcp(listener) => {
                let (stream, addr) = Listener::accept(listener).await;
                (DualStream::Tcp(stream), DualAddr::Tcp(addr))
            }
            #[cfg(unix)]
            DualListener::Uds(listener) => {
                let (stream, addr) = Listener::accept(listener).await;
                (DualStream::Uds(stream), DualAddr::Uds(addr))
            }
        }
    }

    pub(crate) fn _accept_axum_unpin(
        &mut self,
    ) -> impl core::future::Future<Output = (DualStream, DualAddr)> + Unpin + use<'_> {
        Box::pin(async move {
            match self {
                DualListener::Tcp(listener) => {
                    let (stream, addr) = Listener::accept(listener).await;
                    (DualStream::Tcp(stream), DualAddr::Tcp(addr))
                }
                #[cfg(unix)]
                DualListener::Uds(listener) => {
                    let (stream, addr) = Listener::accept(listener).await;
                    (DualStream::Uds(stream), DualAddr::Uds(addr))
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
/// use axum_listener::listener::DualListener;
///
/// let listener = DualListener::bind("localhost:8080").await.unwrap();
/// let (stream, _addr) = listener.accept().await.unwrap();
///
/// // The stream can be used with Axum or any other async framework
/// // that works with tokio's AsyncRead + AsyncWrite traits
/// println!("Accepted connection from: {:?}", _addr);
/// # });
/// ```
pub enum DualStream {
    /// A TCP stream for network connections
    Tcp(tokio::net::TcpStream),
    /// A Unix Domain Socket stream for local inter-process communication
    #[cfg(unix)]
    Uds(tokio::net::UnixStream),
}

impl From<tokio::net::TcpStream> for DualStream {
    fn from(stream: tokio::net::TcpStream) -> Self {
        DualStream::Tcp(stream)
    }
}

#[cfg(unix)]
impl From<tokio::net::UnixStream> for DualStream {
    fn from(stream: tokio::net::UnixStream) -> Self {
        DualStream::Uds(stream)
    }
}

impl tokio::io::AsyncRead for DualStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DualStream::Tcp(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            #[cfg(unix)]
            DualStream::Uds(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for DualStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            DualStream::Tcp(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
            #[cfg(unix)]
            DualStream::Uds(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DualStream::Tcp(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            #[cfg(unix)]
            DualStream::Uds(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DualStream::Tcp(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            #[cfg(unix)]
            DualStream::Uds(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl axum::serve::Listener for DualListener {
    type Io = DualStream;
    type Addr = DualAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        self._accept_axum().await
    }

    fn local_addr(&self) -> Result<Self::Addr, std::io::Error> {
        match self {
            DualListener::Tcp(listener) => Listener::local_addr(listener).map(DualAddr::Tcp),
            #[cfg(unix)]
            DualListener::Uds(listener) => Listener::local_addr(listener).map(DualAddr::Uds),
        }
    }
}

const _: () = {
    use super::DualAddr;
    use axum::extract::connect_info::Connected;
    impl Connected<DualAddr> for DualAddr {
        fn connect_info(remote_addr: DualAddr) -> Self {
            remote_addr
        }
    }
    use axum::serve;

    impl Connected<serve::IncomingStream<'_, DualListener>> for DualAddr {
        fn connect_info(stream: serve::IncomingStream<'_, DualListener>) -> Self {
            stream.remote_addr().clone()
        }
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_tcp_bind() {
        let listener = DualListener::bind("localhost:8080").await;
        assert!(listener.is_ok());
        if let DualListener::Tcp(tcp_listener) = listener.unwrap() {
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
            let listener = DualListener::bind("/tmp/test.sock").await;
            assert!(listener.is_ok());
            if let DualListener::Uds(uds_listener) = listener.unwrap() {
                let addr = uds_listener.local_addr().unwrap();
                assert_eq!(
                    addr.as_pathname().unwrap(),
                    std::path::Path::new("/tmp/test.sock")
                );
            } else {
                panic!("Expected UDS listener");
            }
        }
    }
}
