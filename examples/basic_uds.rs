//! Basic Unix Domain Socket Server Example
//!
//! This example demonstrates how to create a simple HTTP server using
//! axum-listener's DualListener to bind to a Unix Domain Socket.
//!
//! Note: This example only works on Unix-like systems (Linux, macOS, BSD).
//!
//! Run with: `cargo run --example basic_uds`

use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
};
use axum_listener::DualListener;
use serde_json::{Value, json};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only run on Unix systems
    #[cfg(not(unix))]
    {
        eprintln!("This example requires a Unix-like system for Unix Domain Socket support");
        std::process::exit(1);
    }

    #[cfg(unix)]
    {
        // Initialize tracing for logging
        tracing_subscriber::fmt::init();

        // Create the Axum router with various routes
        let app = Router::new()
            .route("/", get(home_handler))
            .route("/health", get(health_check))
            .route("/echo/{message}", get(echo_handler))
            .route("/api/data", get(get_data))
            .route("/api/data", post(post_data));

        // Define the socket path
        let socket_path = "/tmp/axum-listener-example.sock";

        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(socket_path);

        // Bind to a Unix Domain Socket using DualListener
        let listener = DualListener::bind(format!("unix:{}", socket_path)).await?;

        println!("🚀 Server running on Unix Domain Socket: {}", socket_path);
        println!("Try these commands with curl:");
        println!("  • curl --unix-socket {} http://localhost/", socket_path);
        println!(
            "  • curl --unix-socket {} http://localhost/health",
            socket_path
        );
        println!(
            "  • curl --unix-socket {} http://localhost/echo/hello",
            socket_path
        );
        println!(
            "  • curl --unix-socket {} http://localhost/api/data",
            socket_path
        );
        println!(
            "  • curl --unix-socket {} -X POST -H 'Content-Type: application/json' -d '{{\"test\":\"data\"}}' http://localhost/api/data",
            socket_path
        );

        // Start the server
        axum::serve(listener, app).await?;

        // Clean up socket file on exit
        let _ = std::fs::remove_file(socket_path);
    }

    Ok(())
}

#[cfg(unix)]
async fn home_handler() -> Html<&'static str> {
    Html(
        r#"
        <h1>Welcome to the axum-listener Unix Domain Socket Server!</h1>
        <p>This server is powered by <strong>axum-listener</strong>'s DualListener via UDS.</p>
        <p>You're connected through a Unix Domain Socket!</p>
        <ul>
            <li><a href="/health">Health Check</a></li>
            <li><a href="/echo/world">Echo "world"</a></li>
            <li><a href="/api/data">Get JSON Data</a></li>
        </ul>
        <p><em>Note: Links above won't work in browser - use curl with --unix-socket</em></p>
        "#,
    )
}

#[cfg(unix)]
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "listener": "DualListener (Unix Domain Socket mode)",
        "version": env!("CARGO_PKG_VERSION"),
        "connection_type": "unix_domain_socket"
    }))
}

#[cfg(unix)]
async fn echo_handler(Path(message): Path<String>) -> Json<Value> {
    Json(json!({
        "echo": message,
        "length": message.len(),
        "uppercase": message.to_uppercase(),
        "via": "Unix Domain Socket"
    }))
}

#[cfg(unix)]
async fn get_data() -> Json<Value> {
    let mut data = HashMap::new();
    data.insert("message", "Hello from Unix Domain Socket server!");
    data.insert("type", "GET response");
    data.insert("listener", "DualListener (UDS)");
    data.insert("socket_type", "unix_domain_socket");

    Json(json!({
        "data": data,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "performance_note": "UDS typically has lower latency than TCP for local communication"
    }))
}

#[cfg(unix)]
async fn post_data(Json(payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    // Echo back the received data with additional metadata
    Ok(Json(json!({
        "received": payload,
        "processed_at": chrono::Utc::now().to_rfc3339(),
        "message": "Data received successfully via Unix Domain Socket",
        "listener_type": "DualListener (UDS)",
        "advantages": [
            "Lower latency than TCP for local communication",
            "No network stack overhead",
            "Filesystem-based access control"
        ]
    })))
}
