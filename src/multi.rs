use crate::listener::{DuplexAddr, DuplexListener, DuplexStream, ToDuplexAddr};
use axum::serve::Listener;

/// A listener that can accept connections on multiple underlying listeners simultaneously.
///
/// This struct allows you to bind to multiple addresses (TCP and/or Unix Domain Sockets)
/// and accept connections from any of them. When multiple listeners are ready to accept
/// connections, there is no guarantee which one will be selected first.
///
/// # Implementation Details
///
/// Internally, this uses [`futures::future::select_all`] to wait on all listeners
/// simultaneously, ensuring efficient polling of all underlying listeners.
///
/// # Examples
///
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use axum::{Router, routing::get};
/// use axum_listener::multi::MultiListener;
///
/// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
///
/// // Bind to multiple TCP addresses
/// let addresses = ["127.0.0.1:8080", "127.0.0.1:8081"];
/// let listener = MultiListener::bind(addresses).await.unwrap();
/// axum::serve(listener, router).await.unwrap();
/// # });
/// ```
///
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use axum_listener::multi::MultiListener;
///
/// // Mix TCP and Unix Domain Socket addresses (on Unix systems)
/// # #[cfg(unix)] {
/// let addresses = ["localhost:8080", "unix:/tmp/app.sock"];
/// let listener = MultiListener::bind(addresses).await.unwrap();
/// # }
/// # });
/// ```
pub struct MultiListener {
    /// The underlying listeners that this multi-listener manages
    pub listeners: Vec<DuplexListener>,
}

/// An address collection representing the local addresses of a [`MultiListener`].
///
/// This struct contains all the addresses that the multi-listener is bound to.
/// It's returned by the [`axum::serve::Listener::local_addr`] method implementation
/// for [`MultiListener`].
///
/// # Examples
///
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use axum_listener::multi::MultiListener;
/// use axum::serve::Listener;
///
/// let addresses = ["127.0.0.1:8080", "127.0.0.1:8081"];
/// let listener = MultiListener::bind(addresses).await.unwrap();
/// let multi_addr = listener.local_addr().unwrap();
/// println!("Bound to {} addresses", multi_addr.addrs.len());
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct MultiAddr {
    /// The collection of addresses that the multi-listener is bound to
    pub addrs: Vec<DuplexAddr>,
}

/// A stream collection for multi-listener connections.
///
/// This struct is currently not actively used in the public API but is provided
/// for potential future extensions where multiple streams might need to be
/// handled together.
pub struct MultiStream {
    /// A collection of duplex streams
    pub streams: Vec<DuplexStream>,
}

impl MultiListener {
    /// Creates a new [`MultiListener`] bound to multiple addresses.
    ///
    /// This method attempts to bind to all provided addresses simultaneously.
    /// If any of the bindings fail, the entire operation fails and returns an error.
    /// All addresses must be successfully bound for this method to succeed.
    ///
    /// # Arguments
    ///
    /// * `addresses` - An iterable collection of addresses that implement [`ToDuplexAddr`]
    ///
    /// # Returns
    ///
    /// Returns a [`MultiListener`] bound to all specified addresses, or an error
    /// if any binding fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # tokio_test::block_on(async {
    /// use axum_listener::multi::MultiListener;
    ///
    /// // Bind to multiple TCP ports
    /// let tcp_addresses = ["127.0.0.1:8080", "127.0.0.1:8081", "127.0.0.1:8082"];
    /// let listener = MultiListener::bind(tcp_addresses).await.unwrap();
    /// # });
    /// ```
    ///
    /// ```rust,no_run
    /// # tokio_test::block_on(async {
    /// use axum_listener::multi::MultiListener;
    ///
    /// // Mix TCP and Unix Domain Socket addresses
    /// # #[cfg(unix)] {
    /// let mixed_addresses = ["localhost:8080", "unix:/tmp/app.sock"];
    /// let listener = MultiListener::bind(mixed_addresses).await.unwrap();
    /// # }
    /// # });
    /// ```
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - Any address format is invalid
    /// - Any address is already in use
    /// - Permission is denied for any requested address
    /// - Unix Domain Sockets are not supported on the current platform
    /// - The provided iterator is empty (no addresses to bind to)
    pub async fn bind<I: IntoIterator<Item = A>, A: ToDuplexAddr>(
        addresses: I,
    ) -> Result<Self, std::io::Error> {
        let listeners = futures::future::join_all(addresses.into_iter().map(DuplexListener::bind))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MultiListener { listeners })
    }

    /// Accepts a new incoming connection from any of the underlying listeners.
    ///
    /// This method waits for a connection to be established on any of the bound
    /// listeners and returns the first one that becomes available. The selection
    /// is non-deterministic when multiple listeners are ready simultaneously.
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
    /// use axum_listener::multi::MultiListener;
    ///
    /// let addresses = ["127.0.0.1:8080", "127.0.0.1:8081"];
    /// let listener = MultiListener::bind(addresses).await.unwrap();
    ///
    /// // Accept a connection from any of the bound addresses
    /// let (stream, addr) = listener.accept().await.unwrap();
    /// println!("Accepted connection from: {:?}", addr);
    /// # });
    /// ```
    ///
    /// # Errors
    ///
    /// This method can fail if there's an I/O error while accepting a connection
    /// from any of the underlying listeners.
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
