//! Multi-Listener Example
//!
//! This example demonstrates how to create an HTTP server that binds to multiple
//! addresses simultaneously using axum-listener's MultiListener. The server will
//! accept connections on all bound addresses concurrently.
//!
//! Run with: `cargo run --example multi_listener`

use axum::{
    Router,
    extract::{ConnectInfo, Path},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
};
use axum_listener::{DualAddr, MultiAddr, MultiListener};
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
        .route("/echo/{message}", get(echo_handler))
        .route("/api/data", get(get_data))
        .route("/api/data", post(post_data))
        .route("/connection-info", get(connection_info_handler));

    // Define multiple addresses to bind to
    let mut addresses = vec![
        "127.0.0.1:3000", // TCP on port 3000
        "127.0.0.1:3001", // TCP on port 3001
        "0.0.0.0:8080",   // TCP on all interfaces, port 8080
    ];

    // Add Unix Domain Socket if on Unix system
    #[cfg(unix)]
    {
        // Clean up any existing socket files
        let _ = std::fs::remove_file("/tmp/axum-multi-1.sock");
        let _ = std::fs::remove_file("/tmp/axum-multi-2.sock");

        addresses.push("unix:/tmp/axum-multi-1.sock");
        addresses.push("/tmp/axum-multi-2.sock"); // Alternative UDS syntax
    }

    // Create MultiListener bound to all addresses
    let multi_listener = MultiListener::bind(addresses.clone()).await?;

    // Display information about bound addresses
    println!("🚀 Multi-Listener Server started!");
    println!("Listening on {} addresses:", addresses.len());

    for (i, addr) in addresses.iter().enumerate() {
        match addr {
            addr if addr.starts_with("unix:") || (!addr.contains(':') && addr.starts_with('/')) => {
                println!("  {}. Unix Domain Socket: {}", i + 1, addr);
                #[cfg(unix)]
                println!(
                    "     Test with: curl --unix-socket {} http://localhost/",
                    addr.strip_prefix("unix:").unwrap_or(addr)
                );
            }
            _ => {
                println!("  {}. TCP: http://{}", i + 1, addr);
                println!("     Test with: curl http://{}/", addr);
            }
        }
    }

    println!("\nTry these endpoints on any address:");
    println!("  • GET  /health");
    println!("  • GET  /echo/<message>");
    println!("  • GET  /api/data");
    println!("  • POST /api/data");
    println!("  • GET  /connection-info");

    // Start the server
    axum::serve(
        multi_listener,
        app.into_make_service_with_connect_info::<MultiAddr>(),
    )
    .await?;

    // Clean up socket files on exit
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file("/tmp/axum-multi-1.sock");
        let _ = std::fs::remove_file("/tmp/axum-multi-2.sock");
    }

    Ok(())
}

// Handler for the home page
async fn home_handler() -> Html<&'static str> {
    Html(
        r#"
        <h1>Welcome to the axum-listener Multi-Listener Server!</h1>
        <p>This server is powered by <strong>axum-listener</strong>'s MultiListener.</p>
        <p>It's accepting connections on multiple addresses simultaneously!</p>
        <ul>
            <li><a href="/health">Health Check</a></li>
            <li><a href="/echo/world">Echo "world"</a></li>
            <li><a href="/api/data">Get JSON Data</a></li>
            <li><a href="/connection-info">Connection Information</a></li>
        </ul>
        <h2>Architecture Benefits</h2>
        <ul>
            <li><strong>High Availability:</strong> Multiple entry points</li>
            <li><strong>Load Distribution:</strong> Spread across different addresses</li>
            <li><strong>Protocol Flexibility:</strong> TCP and Unix Domain Sockets</li>
            <li><strong>Zero Downtime:</strong> Add/remove listeners dynamically</li>
        </ul>
        "#,
    )
}

// Health check endpoint with multi-listener info
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "listener": "MultiListener",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "tcp_support": true,
            "unix_domain_socket_support": cfg!(unix),
            "concurrent_listeners": "multiple"
        },
        "advantages": [
            "Accepts connections from multiple addresses simultaneously",
            "Non-blocking listener selection",
            "Unified interface for heterogeneous protocols"
        ]
    }))
}

// Connection info handler that shows which address was used
async fn connection_info_handler(ConnectInfo(addr): ConnectInfo<MultiAddr>) -> Json<Value> {
    let connection_type = match &addr.addrs[0] {
        DualAddr::Tcp(_) => "TCP",
        #[cfg(unix)]
        DualAddr::Uds(_) => "Unix Domain Socket",
    };

    Json(json!({
        "connection_info": {
            "address": format!("{:?}", addr),
            "type": connection_type,
            "listener": "MultiListener",
            "timestamp": chrono::Utc::now().to_rfc3339()
        },
        "message": format!("You connected via {}", connection_type),
        "note": "MultiListener automatically selected this connection from multiple available listeners"
    }))
}

// Echo handler that returns the path parameter
async fn echo_handler(
    Path(message): Path<String>,
    ConnectInfo(addr): ConnectInfo<MultiAddr>,
) -> Json<Value> {
    let connection_type = match &addr.addrs[0] {
        DualAddr::Tcp(_) => "TCP",
        #[cfg(unix)]
        DualAddr::Uds(_) => "Unix Domain Socket",
    };

    Json(json!({
        "echo": message,
        "length": message.len(),
        "uppercase": message.to_uppercase(),
        "connection": {
            "type": connection_type,
            "address": format!("{:?}", addr)
        },
        "multi_listener_info": "This response came through one of multiple concurrent listeners"
    }))
}

// GET endpoint for retrieving data
async fn get_data(ConnectInfo(addr): ConnectInfo<MultiAddr>) -> Json<Value> {
    let addr = addr.addrs[0].clone();
    let mut data = HashMap::new();
    data.insert("message", "Hello from Multi-Listener server!");
    data.insert("type", "GET response");
    data.insert("listener", "MultiListener");

    let connection_type = match &addr {
        DualAddr::Tcp(_) => "TCP",
        #[cfg(unix)]
        DualAddr::Uds(_) => "Unix Domain Socket",
    };

    Json(json!({
        "data": data,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "connection": {
            "type": connection_type,
            "address": format!("{:?}", addr)
        },
        "multi_listener_features": {
            "concurrent_addresses": "Multiple addresses can accept connections simultaneously",
            "protocol_agnostic": "Handles both TCP and Unix Domain Sockets uniformly",
            "efficient_polling": "Uses futures::select_all for optimal performance"
        }
    }))
}

// POST endpoint for receiving data
async fn post_data(
    ConnectInfo(addr): ConnectInfo<MultiAddr>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let connection_type = match &addr.addrs[0] {
        DualAddr::Tcp(_) => "TCP",
        #[cfg(unix)]
        DualAddr::Uds(_) => "Unix Domain Socket",
    };

    // Echo back the received data with additional metadata
    Ok(Json(json!({
        "received": payload,
        "processed_at": chrono::Utc::now().to_rfc3339(),
        "message": format!("Data received successfully via {}", connection_type),
        "listener_type": "MultiListener",
        "connection": {
            "type": connection_type,
            "address": format!("{:?}", addr)
        },
        "architecture_benefits": {
            "scalability": "Can handle connections from multiple entry points",
            "reliability": "Redundant access paths increase availability",
            "flexibility": "Different protocols for different use cases",
            "performance": "Efficient concurrent listener management"
        }
    })))
}
