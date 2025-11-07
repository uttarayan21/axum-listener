use axum::serve::Listener;
use std::net::ToSocketAddrs;
use tokio::net::UnixListener;

pub enum DuplexListener {
    Tcp(tokio::net::TcpListener),
    Uds(tokio::net::UnixListener),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum DuplexAddr {
    Tcp(core::net::SocketAddr),
    Uds(tokio::net::unix::SocketAddr),
}

#[cfg(feature = "remove-on-drop")]
impl Drop for DuplexAddr {
    fn drop(&mut self) {
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
            let path = s.trim_start_matches("unix:");
            let addr = From::from(std::os::unix::net::SocketAddr::from_pathname(path)?);
            Ok(DuplexAddr::Uds(addr))
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

pub trait ToDuplexAddr {
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

impl ToDuplexAddr for &std::path::Path {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        Ok(DuplexAddr::Uds(From::from(
            std::os::unix::net::SocketAddr::from_pathname(self)?,
        )))
    }
}

impl ToDuplexAddr for std::path::PathBuf {
    fn to_duplex_addr(&self) -> Result<DuplexAddr, std::io::Error> {
        self.as_path().to_duplex_addr()
    }
}

impl DuplexListener {
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
        }
    }
}

pub enum DuplexStream {
    Tcp(tokio::net::TcpStream),
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
            DuplexStream::Uds(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DuplexStream::Tcp(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            DuplexStream::Uds(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            DuplexStream::Tcp(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
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
