//! Basic TCP Server Example
//!
//! This example demonstrates how to create a simple HTTP server using
//! axum-listener's DualListener to bind to a TCP address.
//!
//! Run with: `cargo run --example basic_tcp`

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
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Create the Axum router with various routes
    let app = Router::new()
        .route("/", get(home_handler))
        .route("/health", get(health_check))
        .route("/echo/:message", get(echo_handler))
        .route("/api/data", get(get_data))
        .route("/api/data", post(post_data));

    // Bind to a TCP address using DualListener
    let listener = DualListener::bind("127.0.0.1:3000").await?;

    println!("🚀 Server running at http://127.0.0.1:3000");
    println!("Try these endpoints:");
    println!("  • GET  http://127.0.0.1:3000/");
    println!("  • GET  http://127.0.0.1:3000/health");
    println!("  • GET  http://127.0.0.1:3000/echo/hello");
    println!("  • GET  http://127.0.0.1:3000/api/data");
    println!("  • POST http://127.0.0.1:3000/api/data");

    // Start the server
    axum::serve(listener, app).await?;

    Ok(())
}

// Handler for the home page
async fn home_handler() -> Html<&'static str> {
    Html(
        r#"
        <h1>Welcome to the axum-listener TCP Server!</h1>
        <p>This server is powered by <strong>axum-listener</strong>'s DualListener.</p>
        <ul>
            <li><a href="/health">Health Check</a></li>
            <li><a href="/echo/world">Echo "world"</a></li>
            <li><a href="/api/data">Get JSON Data</a></li>
        </ul>
        "#,
    )
}

// Health check endpoint
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "listener": "DualListener (TCP mode)",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// Echo handler that returns the path parameter
async fn echo_handler(Path(message): Path<String>) -> Json<Value> {
    Json(json!({
        "echo": message,
        "length": message.len(),
        "uppercase": message.to_uppercase()
    }))
}

// GET endpoint for retrieving data
async fn get_data() -> Json<Value> {
    let mut data = HashMap::new();
    data.insert("message", "Hello from TCP server!");
    data.insert("type", "GET response");
    data.insert("listener", "DualListener");

    Json(json!({
        "data": data,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// POST endpoint for receiving data
async fn post_data(Json(payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    // Echo back the received data with additional metadata
    Ok(Json(json!({
        "received": payload,
        "processed_at": chrono::Utc::now().to_rfc3339(),
        "message": "Data received successfully via TCP",
        "listener_type": "DualListener (TCP)"
    })))
}
