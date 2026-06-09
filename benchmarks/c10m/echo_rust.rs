// =============================================================================
// C10M Echo Server — Rust/Tokio
//
// Async echo server using Tokio. Each connection spawns a task that reads
// data, case-swaps the first byte, and echoes it back.
//
// Build:  cargo build --release
// Run:    ./target/release/echo_rust <port>
// =============================================================================

use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::args().nth(1).unwrap_or_else(|| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    
    println!("[Rust/Tokio] Echo server listening on {}", addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => break, // Connection closed
                    Ok(n) => {
                        buf[0] ^= 0x20; // Case swap
                        if socket.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}
