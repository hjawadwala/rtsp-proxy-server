use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, Level};

mod rtsp_client;
mod streaming_server;
mod stream_manager;

use stream_manager::StreamManager;
use streaming_server::StreamingServer;

#[derive(Parser, Debug)]
#[command(name = "rtsp-proxy")]
#[command(about = "RTSP to HLS/MPEG-TS proxy server", long_about = None)]
struct Args {
    /// HTTP server port
    #[arg(short, long, default_value = "5000")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .init();

    let args = Args::parse();

    info!("Starting RTSP Proxy Server");
    info!("Server will listen on {}:{}", args.host, args.port);

    // Create stream manager
    let stream_manager = Arc::new(RwLock::new(StreamManager::new()));

    // Start HTTP server
    let server = StreamingServer::new(args.host, args.port, stream_manager);
    server.run().await?;

    Ok(())
}
