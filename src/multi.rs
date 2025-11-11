use crate::listener::{DuplexAddr, DuplexListener, DuplexStream, ToDuplexAddr};
use axum::serve::Listener;

/// A listener that can accept connections on multiple underlying listeners.  
/// There is no guarantee which listener will accept the next connection if multiple are ready.  
/// This uses [`futures::future::select_all`] to wait on all listeners simultaneously.
pub struct MultiListener {
    pub listeners: Vec<DuplexListener>,
}

#[derive(Debug, Clone)]
pub struct MultiAddr {
    pub addrs: Vec<DuplexAddr>,
}

pub struct MultiStream {
    pub streams: Vec<DuplexStream>,
}

impl MultiListener {
    /// Binds a new multi-listener to the given addresses.
    /// Each address in the `MultiAddr` will be bound to a separate underlying listener.
    /// If any of the bindings fail, the entire operation will fail.
    ///
    /// # Arguments
    /// * `addresses` - A `MultiAddr` containing the addresses to bind to.
    /// ```rust
    /// # tokio_test::block_on(async {
    /// use axum_listener::multi::MultiListener;
    /// let multi_listener = MultiListener::bind(["localhost:8080", "unix:/tmp/socket.sock"]).await.unwrap();
    /// # });
    /// ```
    pub async fn bind<I: IntoIterator<Item = A>, A: ToDuplexAddr>(
        addresses: I,
    ) -> Result<Self, std::io::Error> {
        let listeners = futures::future::join_all(addresses.into_iter().map(DuplexListener::bind))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MultiListener { listeners })
    }

    pub async fn accept(&self) -> Result<(DuplexStream, DuplexAddr), std::io::Error> {
        let (out, idx, _rest) =
            futures::future::select_all(self.listeners.iter().map(|listener| listener._accept()))
                .await;
        tracing::trace!("Accepted connection on multi-listener from index {}", idx);
        out
    }
}

impl axum::serve::Listener for MultiListener {
    type Io = DuplexStream;
    type Addr = MultiAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (out, _index, _rest) =
            futures::future::select_all(self.listeners.iter_mut().map(|listener| {
                Box::pin(async move {
                    match listener {
                        DuplexListener::Tcp(ref mut listener) => {
                            let (io, addr) = Listener::accept(listener).await;
                            (DuplexStream::Tcp(io), DuplexAddr::Tcp(addr))
                        }
                        #[cfg(unix)]
                        DuplexListener::Uds(ref mut listener) => {
                            let (io, addr) = Listener::accept(listener).await;
                            (DuplexStream::Uds(io), DuplexAddr::Uds(addr))
                        }
                    }
                })
            }))
            .await;
        tracing::trace!("Accepted connection on multi-listener from {}", _index);
        (out.0, MultiAddr { addrs: vec![out.1] })
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.listeners
            .iter()
            .map(|listener| listener.local_addr())
            .collect::<Result<Vec<_>, _>>()
            .map(|addrs| MultiAddr { addrs })
    }
}
