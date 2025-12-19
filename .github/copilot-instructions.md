# RTSP Proxy Server - AI Agent Instructions

## Project Overview
An async Rust server that proxies RTSP camera streams to HTTP-accessible MPEG-TS/HLS streams using FFmpeg. Built with Tokio for async runtime and Axum for HTTP serving.

## Architecture

### Three-Layer Design
1. **[src/main.rs](../src/main.rs)** - Entry point: CLI parsing (clap), tracing setup, server initialization
2. **[src/stream_manager.rs](../src/stream_manager.rs)** - Manages stream lifecycle and tracks active streams in HashMap
3. **[src/rtsp_client.rs](../src/rtsp_client.rs)** - Spawns FFmpeg child processes and streams output via mpsc channels
4. **[src/streaming_server.rs](../src/streaming_server.rs)** - Axum HTTP server with REST API and streaming endpoints

### Data Flow
```
RTSP URL → FFmpeg process → stdout pipe → tokio channel → HTTP response stream
```

Key pattern: `RtspClient` uses `mpsc::UnboundedReceiver<Bytes>` wrapped in `Arc<Mutex<Option<T>>>` so it can be taken once per HTTP connection. Multiple HTTP clients cannot share the same receiver - each needs to start/stop streams independently.

## Critical FFmpeg Dependency

**FFmpeg must be installed and in PATH.** The server shells out to `ffmpeg` binary (not using Rust bindings):
```rust
Command::new("ffmpeg")
    .args(&["-rtsp_transport", "tcp", "-i", &rtsp_url, "-f", "mpegts", ...])
```

Standard FFmpeg flags used:
- `-rtsp_transport tcp` - More reliable than UDP for proxying
- `-preset ultrafast -tune zerolatency` - Real-time streaming optimizations
- `-b:v 2000k -b:a 128k` - Bitrate caps
- Buffer size: `188 * 7` bytes (MPEG-TS packets are 188 bytes)

## Development Workflow

### Building and Running
```bash
cargo build --release           # Optimized build
cargo run --release             # Default: 0.0.0.0:8080
cargo run -- --port 3000        # Custom port
./target/release/rtsp-proxy     # Direct binary execution
```

### Testing Streams
No automated tests. Manual testing workflow:
1. Start server: `cargo run --release`
2. Start stream via curl:
   ```bash
   curl -X POST "http://localhost:8080/api/stream/cam1/start?rtsp_url=rtsp://..."
   ```
3. View in VLC/ffplay: `vlc http://localhost:8080/stream/cam1/mpegts`
4. Or use [viewer.html](../viewer.html) in browser (static HTML with fetch API)

### Debugging Streams
- FFmpeg stderr is redirected to `/dev/null` - to debug FFmpeg issues, change `Stdio::null()` to `Stdio::inherit()` in [src/rtsp_client.rs](../src/rtsp_client.rs#L56)
- Check logs with `RUST_LOG=debug cargo run`
- Process cleanup: FFmpeg processes are `kill_on_drop(true)` - they terminate when RtspClient drops

## Code Conventions

### Async/Await Patterns
- Everything is async with Tokio runtime (`#[tokio::main]`)
- Shared state uses `Arc<RwLock<T>>` (not Mutex) for concurrent reads
- Channel pattern: `mpsc::unbounded_channel()` for FFmpeg output streaming

### Error Handling
- Use `anyhow::Result` for application errors
- API responses return custom `ApiResponse` struct with `success: bool` field
- HTTP status codes: 200 (OK), 404 (NOT_FOUND), 500 (INTERNAL_SERVER_ERROR)

### Logging
- `tracing` crate with `info!`, `warn!`, `error!` macros
- Default level: INFO (set in main.rs)
- Log key lifecycle events: stream start/stop, HTTP requests, errors

## Common Pitfalls

1. **Stream ID Conflicts**: Starting a stream with existing ID returns error - must stop first
2. **Receiver Consumption**: `get_data_receiver()` uses `.take()` - calling twice returns None. Each HTTP connection needs fresh stream start.
3. **FFmpeg Not Found**: Error message explicitly mentions PATH - installation instructions in [README.md](../README.md#prerequisites)
4. **CORS**: Uses `CorsLayer::permissive()` - safe for development, review for production
5. **HLS Playlist**: Currently a stub that redirects to MPEG-TS - not proper HLS segmentation

## Key Files
- **[Cargo.toml](../Cargo.toml)** - Dependencies: tokio (async), axum (HTTP), clap (CLI)
- **[viewer.html](../viewer.html)** - Standalone test client (no build step required)
- **[QUICKSTART.md](../QUICKSTART.md)** - User-facing setup guide
- **target/release/** - Build output (gitignored)

## Extending the Server

### Adding New Endpoints
Add routes in `StreamingServer::run()`:
```rust
.route("/api/new-endpoint", get(handler_fn))
```

### Modifying FFmpeg Parameters
Edit args array in `RtspClient::start()` - common changes: resolution, codec, bitrate

### Supporting New Formats
Add new handlers similar to `stream_mpegts()` - change Content-Type header and FFmpeg output format (`-f`)
