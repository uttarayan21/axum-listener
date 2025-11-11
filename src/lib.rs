//! # Axum Listener Extensions
//!
//! This crate provides enhanced listeners for the [Axum](https://github.com/tokio-rs/axum) web framework,
//! enabling unified handling of TCP and Unix Domain Socket (UDS) connections.
//!
//! ## Features
//!
//! - **DualListener**: A unified listener that can bind to either TCP or Unix Domain Socket addresses
//! - **MultiListener**: A listener that can simultaneously accept connections from multiple underlying listeners
//! - **Cross-platform support**: Works on Unix-like systems with UDS support and all platforms for TCP
//! - **Axum integration**: Implements the `axum::serve::Listener` trait for seamless integration
//!
//! ## Cargo Features
//!
//! - `http1` (default): Enables HTTP/1 support in axum
//! - `http2`: Enables HTTP/2 support in axum
//! - `remove-on-drop` (default): Automatically removes UDS socket files when the address is dropped
//!
//! ## Quick Start
//!
//! ### Using DualListener for a single address
//!
//! ```rust,no_run
//! # tokio_test::block_on(async {
//! use axum::{Router, routing::get};
//! use axum_listener::listener::DualListener;
//!
//! let router = Router::new().route("/", get(|| async { "Hello, World!" }));
//!
//! // Bind to a TCP address
//! let listener = DualListener::bind("localhost:8080").await.unwrap();
//! axum::serve(listener, router.clone()).await.unwrap();
//!
//! // Or bind to a Unix Domain Socket (on Unix systems)
//! # #[cfg(unix)] {
//! let listener = DualListener::bind("unix:/tmp/app.sock").await.unwrap();
//! axum::serve(listener, router).await.unwrap();
//! # }
//! # });
//! ```
//!
//! ### Using MultiListener for multiple addresses
//!
//! ```rust,no_run
//! # tokio_test::block_on(async {
//! use axum::{Router, routing::get};
//! use axum_listener::multi::MultiListener;
//!
//! let router = Router::new().route("/", get(|| async { "Hello, World!" }));
//!
//! // Bind to multiple TCP and UDS simultaneously
//! let addresses = ["localhost:8080", "localhost:9090", "unix:/tmp/app.sock"];
//! let multi_listener = MultiListener::bind(addresses).await.unwrap();
//! axum::serve(multi_listener, router).await.unwrap();
//! # });
//! ```
//!
//! ## Address Formats
//!
//! The following address formats are supported:
//!
//! - **TCP addresses**: `"localhost:8080"`, `"127.0.0.1:3000"`, `"[::1]:8080"`
//! - **Unix Domain Sockets** (Unix only): `"/path/to/socket"`, `"unix:/path/to/socket"`
//!
//! ## Platform Support
//!
//! - TCP listeners work on all platforms
//! - Unix Domain Socket listeners are only available on Unix-like systems (Linux, macOS, BSD, etc.)
//! - The crate gracefully handles platform differences with conditional compilation
pub mod listener;
pub mod multi;
#[doc(inline)]
pub use listener::*;
#[doc(inline)]
pub use multi::*;
