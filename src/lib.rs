//! # Some extra listeners for ease of use for axum
//!
//! [listener]: This adds a duplex listener that can accept both TCP and UDS connections.  
//! [multi]: This adds a multi listener that can accept connections from multiple listeners at the
//! same time.
//!
//! ## How to use
//! ```rust,no_run
//! # tokio_test::block_on(async {
//! use axum::{Router, routing::get};
//! use axum_listener::listener::DuplexListener;
//! use axum_listener::multi::MultiListener;
//!
//! let router: Router<()> = Router::new().route("/", get(|| async { "Hello, World!" }));
//! let multi_listener = MultiListener::bind(["localhost:8080", "unix:/tmp/socket.sock"]).await.unwrap();
//! axum::serve(multi_listener, router).await.unwrap();
//! # });
//! ```
pub mod listener;
pub mod multi;
