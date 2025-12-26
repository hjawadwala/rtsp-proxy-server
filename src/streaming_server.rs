use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::stream::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;
use tracing::{error, info};
use uuid::Uuid;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::Value;
use reqwest::Client;

use crate::stream_manager::StreamManager;

pub struct StreamingServer {
    host: String,
    port: u16,
    stream_manager: Arc<RwLock<StreamManager>>,
}

// --- HLS session tracking for inactivity timeout ---
struct HlsSession {
    tmp_dir: String,
    rtsp_url: String,
    last_access: Instant,
    shutdown: mpsc::Sender<()>,
}

static HLS_SESSIONS: Lazy<Arc<RwLock<HashMap<String, HlsSession>>>> = Lazy::new(|| {
    Arc::new(RwLock::new(HashMap::new()))
});

const HLS_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct StartStreamRequest {
    rtsp_url: String,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct StreamListResponse {
    streams: Vec<String>,
}

#[derive(Deserialize)]
struct ProxyCamerasQuery {
    ip: String,
    port: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Deserialize)]
struct ProxyRtspQuery {
    ip: String,
    port: Option<String>,
    username: Option<String>,
    password: Option<String>,
    channel: Option<String>,
    stream_number: Option<String>,
}

#[derive(Deserialize)]
struct ProxyHlsRtspQuery {
    ip: String,
    port: Option<String>,
    username: Option<String>,
    password: Option<String>,
    channel: Option<String>,
    stream_number: Option<String>,
}

#[derive(Serialize)]
struct ChannelInfo {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct ChannelListResponse {
    channels: Vec<ChannelInfo>,
}

impl StreamingServer {
    pub fn new(host: String, port: u16, stream_manager: Arc<RwLock<StreamManager>>) -> Self {
        Self {
            host,
            port,
            stream_manager,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/", get(root_handler))
            .route("/api/streams", get(list_streams))
            .route("/api/stream/:id/start", post(start_stream))
            .route("/api/stream/:id/stop", post(stop_stream))
            .route("/stream/:id/mpegts", get(stream_mpegts))
            .route("/stream", get(direct_stream))
            .route("/stream/hls", get(stream_hls_direct))
            .route("/stream/hls/:id/playlist.m3u8", get(stream_hls_session_playlist))
            .route("/stream/hls/:id/:file", get(stream_hls_session_segment))
            .route("/player", get(player_page))
            .route("/stream/:id/hls/playlist.m3u8", get(stream_hls_playlist))
            .route("/stream/:id/hls/:segment", get(stream_hls_segment))
            .route("/proxy/cameras", get(proxy_cameras))
            .route("/proxy/rtsp", get(proxy_rtsp))
            .route("/proxyhl/rtsp", get(proxy_hls_rtsp))
            .route("/proxyhl/sessions", get(list_proxyhl_sessions))
            .route("/proxyhl/segment/:id/:file", get(proxy_hls_segment))
            .layer(CorsLayer::permissive())
            .with_state(self.stream_manager);

        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        
        info!("Server listening on http://{}", addr);
        info!("API endpoints:");
        info!("  GET /player?rtsp_url=<url> - Play stream in browser");
        info!("  GET /stream?rtsp_url=<url> - Stream directly from RTSP URL (for VLC/ffplay)");
        info!("  POST /api/stream/:id/start - Start a stream (form: rtsp_url)");
        info!("  POST /api/stream/:id/stop - Stop a stream");
        info!("  GET /api/streams - List all streams");
        info!("  GET /stream/:id/mpegts - Get MPEG-TS stream");
        info!("  GET /stream/:id/hls/playlist.m3u8 - Get HLS playlist");
        info!("  GET /proxyhl/rtsp - HLS playlist from Hikvision RTSP");
        info!("  GET /proxyhl/sessions - List active HLS sessions");

        axum::serve(listener, app).await?;

        Ok(())
    }
}

async fn root_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "RTSP Proxy Server",
        "version": "0.1.0",
        "endpoints": {
            "player": "GET /player?rtsp_url=<url> - Play in browser",
            "direct_stream": "GET /stream?rtsp_url=<url> - Stream directly from RTSP",
            "start_stream": "POST /api/stream/:id/start?rtsp_url=<url>",
            "stop_stream": "POST /api/stream/:id/stop",
            "list_streams": "GET /api/streams",
            "mpegts_stream": "GET /stream/:id/mpegts",
            "hls_playlist": "GET /stream/:id/hls/playlist.m3u8"
        },
        "browser_example": "http://localhost:8080/player?rtsp_url=rtsp://user:pass@camera-ip:554/stream",
        "vlc_example": "vlc http://localhost:8080/stream?rtsp_url=rtsp://user:pass@camera-ip:554/stream"
    }))
}

async fn list_streams(
    State(manager): State<Arc<RwLock<StreamManager>>>,
) -> impl IntoResponse {
    let manager = manager.read().await;
    let streams = manager.list_streams();
    
    Json(StreamListResponse { streams })
}

async fn start_stream(
    Path(id): Path<String>,
    maybe_query: Option<Query<StartStreamRequest>>, 
    State(manager): State<Arc<RwLock<StreamManager>>>,
    body: String,
) -> impl IntoResponse {
    info!("Received request to start stream {}", id);

    // Prefer query param if present, fallback to urlencoded form body
    let rtsp_url = if let Some(Query(params)) = maybe_query {
        params.rtsp_url
    } else {
        let s = body;
        let mut rtsp_url: Option<String> = None;
        for pair in s.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let Some(key) = parts.next() {
                if key == "rtsp_url" {
                    let val = parts.next().unwrap_or("");
                    match urlencoding::decode(val) {
                        Ok(decoded) => {
                            rtsp_url = Some(decoded.into_owned());
                            break;
                        }
                        Err(_) => {
                            rtsp_url = Some(val.to_string());
                            break;
                        }
                    }
                }
            }
        }
        match rtsp_url {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse {
                        success: false,
                        message: "Missing rtsp_url in query or form body".to_string(),
                    }),
                ).into_response();
            }
        }
    };

    let mut manager = manager.write().await;
    match manager.start_stream(id.clone(), rtsp_url).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Stream {} started", id),
            }),
        ).into_response(),
        Err(e) => {
            error!("Failed to start stream {}: {}", id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: format!("Failed to start stream: {}", e),
                }),
            ).into_response()
        }
    }
}

async fn stop_stream(
    Path(id): Path<String>,
    State(manager): State<Arc<RwLock<StreamManager>>>,
) -> impl IntoResponse {
    info!("Received request to stop stream {}", id);

    let mut manager = manager.write().await;
    match manager.stop_stream(&id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Stream {} stopped", id),
            }),
        ),
        Err(e) => {
            error!("Failed to stop stream {}: {}", id, e);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    message: format!("Failed to stop stream: {}", e),
                }),
            )
        }
    }
}

async fn stream_mpegts(
    Path(id): Path<String>,
    State(manager): State<Arc<RwLock<StreamManager>>>,
) -> Response {
    info!("MPEG-TS stream requested for {}", id);

    let manager = manager.read().await;
    let stream_info = match manager.get_stream(&id) {
        Some(info) => info,
        None => {
            return (
                StatusCode::NOT_FOUND,
                "Stream not found",
            ).into_response();
        }
    };

    // Get data receiver from the client
    let client = stream_info.client.read().await;
    let receiver = match client.get_data_receiver().await {
        Some(rx) => rx,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get stream receiver",
            ).into_response();
        }
    };
    drop(client);
    drop(manager);

    // Create streaming response
    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver)
        .map(|chunk| Ok::<_, std::io::Error>(chunk));
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp2t")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap()
}

async fn stream_hls_playlist(
    Path(id): Path<String>,
    State(manager): State<Arc<RwLock<StreamManager>>>,
) -> Response {
    info!("HLS playlist requested for {}", id);

    let manager = manager.read().await;
    if manager.get_stream(&id).is_none() {
        return (StatusCode::NOT_FOUND, "Stream not found").into_response();
    }

    // Generate a simple HLS playlist
    let playlist = format!(
        "#EXTM3U\n\
         #EXT-X-VERSION:3\n\
         #EXT-X-TARGETDURATION:10\n\
         #EXT-X-MEDIA-SEQUENCE:0\n\
         #EXTINF:10.0,\n\
         /stream/{}/mpegts\n",
        id
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(playlist))
        .unwrap()
}

async fn stream_hls_segment(
    Path((id, segment)): Path<(String, String)>,
    State(manager): State<Arc<RwLock<StreamManager>>>,
) -> Response {
    info!("HLS segment {} requested for stream {}", segment, id);
    
    // For simplicity, redirect to MPEG-TS stream
    // In production, you'd want proper HLS segmentation
    stream_mpegts(Path(id), State(manager)).await
}

#[derive(Deserialize)]
struct DirectStreamQuery {
    rtsp_url: String,
}

async fn direct_stream(
    Query(params): Query<DirectStreamQuery>,
) -> Response {
    use std::process::Stdio;
    use tokio::process::Command;
    use tokio::io::AsyncReadExt;
    
    info!("Direct stream requested for {}", params.rtsp_url);

    // Start FFmpeg process directly
    let mut child = match Command::new("ffmpeg")
        .args(&[
            "-rtsp_transport", "tcp",
            "-i", &params.rtsp_url,
            "-f", "mpegts",
            "-codec:v", "libx264",
            "-preset", "ultrafast",
            "-codec:a", "aac",
            "-ar", "44100",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            error!("Failed to start FFmpeg: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to start FFmpeg: {}. Make sure FFmpeg is installed and in PATH.", e),
            ).into_response();
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to capture FFmpeg stdout",
            ).into_response();
        }
    };

    // Create async stream from FFmpeg stdout
    let stream = async_stream::stream! {
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut buffer = vec![0u8; 188 * 7]; // MPEG-TS packets are 188 bytes
        
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    info!("FFmpeg stream ended");
                    break;
                }
                Ok(n) => {
                    yield Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(&buffer[..n]));
                }
                Err(e) => {
                    error!("Error reading from FFmpeg: {}", e);
                    break;
                }
            }
        }
    };

    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp2t")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap()
}

async fn stream_hls_direct(Query(params): Query<DirectStreamQuery>) -> Response {
    info!("Direct HLS stream requested for {}", params.rtsp_url);

    // Create a temporary directory for HLS segments
    let id = Uuid::new_v4().to_string();
    let tmp_dir = format!("/tmp/hls-stream-{}", id);
    let playlist_path = format!("{}/playlist.m3u8", tmp_dir);
    let segment_pattern = format!("{}/segment%03d.ts", tmp_dir);
    let base_url = format!("/stream/hls/{}/", id);

    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        error!("Failed to create temp directory: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create temp directory: {}", e),
        )
            .into_response();
    }

    let playlist_path_clone = playlist_path.clone();
    let rtsp_url_clone = params.rtsp_url.clone();

    // Create shutdown channel and register session
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    {
        let mut map = HLS_SESSIONS.write().await;
        map.insert(
            id.clone(),
            HlsSession {
                tmp_dir: tmp_dir.clone(),
                rtsp_url: params.rtsp_url.clone(),
                last_access: Instant::now(),
                shutdown: shutdown_tx.clone(),
            },
        );
    }

    // Spawn FFmpeg in background to generate HLS segments
    let id_clone_for_ffmpeg = id.clone();
    let tmp_dir_for_ffmpeg = tmp_dir.clone();
    let sessions_for_ffmpeg = HLS_SESSIONS.clone();
    tokio::spawn(async move {
        let mut child = match Command::new("ffmpeg")
            .args(&[
                "-rtsp_transport", "tcp",
                "-i", &rtsp_url_clone,
                "-f", "hls",
                "-hls_time", "2",
                "-hls_list_size", "5",
                "-hls_flags", "delete_segments+independent_segments",
                "-hls_segment_filename", &segment_pattern,
                "-hls_base_url", &base_url,
                "-codec:v", "libx264",
                "-preset", "ultrafast",
                "-tune", "zerolatency",
                "-g", "50",
                "-keyint_min", "25",
                "-sc_threshold", "0",
                "-b:v", "2000k",
                "-codec:a", "aac",
                "-ar", "44100",
                "-b:a", "128k",
                &playlist_path_clone,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                error!("Failed to start FFmpeg for HLS: {}", e);
                // Remove session if we failed to start
                let mut map = sessions_for_ffmpeg.write().await;
                map.remove(&id_clone_for_ffmpeg);
                return;
            }
        };

        // Wait for shutdown or process exit
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutting down HLS session {} due to inactivity or explicit stop", id_clone_for_ffmpeg);
                let _ = child.kill().await;
            }
            _ = child.wait() => {
                info!("HLS ffmpeg process exited for session {}", id_clone_for_ffmpeg);
            }
        }
        let _ = std::fs::remove_dir_all(&tmp_dir_for_ffmpeg);
        let mut map = sessions_for_ffmpeg.write().await;
        map.remove(&id_clone_for_ffmpeg);
    });

    // Spawn inactivity monitor
    let id_for_monitor = id.clone();
    let sessions_for_monitor = HLS_SESSIONS.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let should_shutdown = {
                let map = sessions_for_monitor.read().await;
                if let Some(sess) = map.get(&id_for_monitor) {
                    sess.last_access.elapsed() > HLS_IDLE_TIMEOUT
                } else {
                    // Session already gone
                    false
                }
            };
            if should_shutdown {
                info!("HLS session {} idle timeout reached; requesting shutdown", id_for_monitor);
                let mut map = sessions_for_monitor.write().await;
                if let Some(sess) = map.get(&id_for_monitor) {
                    let _ = sess.shutdown.try_send(());
                }
                break;
            }
        }
    });

    // Poll for playlist existence (up to ~20s), then redirect to it
    let playlist_rel_url = format!("/stream/hls/{}/playlist.m3u8", id);
    let mut ready = false;
    for _ in 0..80 {
        if let Ok(meta) = std::fs::metadata(&playlist_path) {
            if meta.len() > 0 {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }

    if !ready {
        error!("Failed to find playlist after waiting: {}", playlist_path);
        return (
            StatusCode::BAD_GATEWAY,
            "HLS playlist not available; source may be unreachable",
        )
            .into_response();
    }

    // Update last access
    {
        let mut map = HLS_SESSIONS.write().await;
        if let Some(sess) = map.get_mut(&id) {
            sess.last_access = Instant::now();
        }
    }

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, playlist_rel_url)
        .body(Body::empty())
        .unwrap()
}

async fn stream_hls_session_playlist(Path(id): Path<String>) -> Response {
    let path = format!("/tmp/hls-stream-{}/playlist.m3u8", id);

    // Update session last access
    {
        let mut map = HLS_SESSIONS.write().await;
        if let Some(sess) = map.get_mut(&id) {
            sess.last_access = Instant::now();
        }
    }

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) => {
            error!("Playlist read error for {}: {}", path, e);
            (
                StatusCode::NOT_FOUND,
                "Playlist not found",
            )
                .into_response()
        }
    }
}

async fn stream_hls_session_segment(Path((id, file)): Path<(String, String)>) -> Response {
    // Prevent path traversal
    if file.contains("..") || file.contains('/') || file.contains('\\') {
        return (
            StatusCode::BAD_REQUEST,
            "Invalid segment path",
        )
            .into_response();
    }

    // Update session last access
    {
        let mut map = HLS_SESSIONS.write().await;
        if let Some(sess) = map.get_mut(&id) {
            sess.last_access = Instant::now();
        }
    }

    let path = format!("/tmp/hls-stream-{}/{}", id, file);

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            // Basic content-type guess
            let ctype = if file.ends_with(".ts") {
                "video/mp2t"
            } else if file.ends_with(".m3u8") {
                "application/vnd.apple.mpegurl"
            } else {
                "application/octet-stream"
            };

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, ctype)
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) => {
            error!("Segment read error for {}: {}", path, e);
            (
                StatusCode::NOT_FOUND,
                "Segment not found",
            )
                .into_response()
        }
    }
}

async fn proxy_cameras(Query(params): Query<ProxyCamerasQuery>) -> Response {
    let port = params.port.unwrap_or_else(|| "554".to_string());
    let username = params.username.unwrap_or_else(|| "admin".to_string());
    let password = params.password.unwrap_or_default();

    let encoded_user = urlencoding::encode(&username);
    let encoded_pass = urlencoding::encode(&password);
    let isapi_url = format!(
        "http://{}:{}@{}:{}/ISAPI/Streaming/channels",
        encoded_user, encoded_pass, params.ip, port
    );

    info!("Fetching cameras from {}:{}", params.ip, port);

    let client = Client::new();
    let response = match client
        .get(&isapi_url)
        .header("Accept", "application/json, application/xml")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Camera fetch error: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse {
                    success: false,
                    message: format!("Failed to contact NVR: {}", e),
                }),
            )
                .into_response();
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        error!("NVR returned HTTP {}", status);
        return (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse {
                success: false,
                message: format!("NVR responded with {}", status),
            }),
        )
            .into_response();
    }

    let body = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            error!("Failed reading NVR response: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse {
                    success: false,
                    message: format!("Failed to read NVR response: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Try JSON first
    if let Ok(value) = serde_json::from_str::<Value>(&body) {
        return (
            StatusCode::OK,
            Json(value),
        )
            .into_response();
    }

    // Fallback: parse XML for channel list
    let channels = parse_channels_xml(&body);
    let response = ChannelListResponse { channels };

    (
        StatusCode::OK,
        Json(response),
    )
        .into_response()
}

fn parse_channels_xml(xml: &str) -> Vec<ChannelInfo> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut channels = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut in_channel = false;
    let mut current_tag: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "StreamingChannel" {
                    in_channel = true;
                    current_id = None;
                    current_name = None;
                } else if in_channel {
                    current_tag = Some(name);
                }
            }
            Ok(Event::Text(e)) => {
                if in_channel {
                    if let Some(tag) = &current_tag {
                        let text = e.unescape().unwrap_or_default().to_string();
                        if tag == "id" {
                            current_id = Some(text);
                        } else if tag == "name" {
                            current_name = Some(text);
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "StreamingChannel" {
                    let id = current_id.clone().unwrap_or_else(|| format!("{}", channels.len() + 1));
                    let name_val = current_name.clone().unwrap_or_else(|| format!("Camera {}", channels.len() + 1));
                    channels.push(ChannelInfo { id, name: name_val });
                    in_channel = false;
                    current_tag = None;
                } else if in_channel {
                    current_tag = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    channels
}

async fn proxy_rtsp(Query(params): Query<ProxyRtspQuery>) -> Response {
    let port = params.port.unwrap_or_else(|| "554".to_string());
    let username = params.username.unwrap_or_else(|| "admin".to_string());
    let password = params.password.unwrap_or_default();
    let channel = params.channel.unwrap_or_else(|| "1".to_string());
    let stream_number = params.stream_number.unwrap_or_else(|| "1".to_string()); // 1=main (01), 2=sub (02), etc.

    // Hikvision convention: channels/{channel}{stream:02d}
    let suffix = format!("{}{:02}", channel, stream_number.parse::<u32>().unwrap_or(1));

    let encoded_user = urlencoding::encode(&username);
    let encoded_pass = urlencoding::encode(&password);
    let rtsp_url = format!(
        "rtsp://{}:{}@{}:{}/ISAPI/Streaming/channels/{}",
        encoded_user, encoded_pass, params.ip, port, suffix
    );

    info!("Proxying RTSP channel {} from {}", channel, params.ip);

    let mut child = match Command::new("ffmpeg")
        .args(&[
            "-rtsp_transport", "tcp",
            "-i", &rtsp_url,
            "-vf", "scale=640:480",
            "-q:v", "5",
            "-f", "mjpeg",
            "-fflags", "flush_packets",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            error!("Failed to start FFmpeg: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to start FFmpeg: {}. Ensure ffmpeg is installed and in PATH.", e),
            )
                .into_response();
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to capture FFmpeg stdout",
            )
                .into_response();
        }
    };

    let stream = async_stream::stream! {
        let mut child = child;
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut buffer = vec![0u8; 8192];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    info!("FFmpeg stream ended");
                    break;
                }
                Ok(n) => {
                    yield Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(&buffer[..n]));
                }
                Err(e) => {
                    error!("Error reading from FFmpeg: {}", e);
                    break;
                }
            }
        }

        let _ = child.wait().await;
    };

    // Ensure ffmpeg terminates when client disconnects
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "multipart/x-mixed-replace; boundary=ffserver")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header("Pragma", "no-cache")
        .header("Expires", "0")
        .body(body)
        .unwrap()
}

async fn proxy_hls_rtsp(Query(params): Query<ProxyHlsRtspQuery>) -> Response {
    info!("Direct HLS stream requested for Hikvision channel");

    // Build RTSP URL similar to proxy_rtsp
    let port = params.port.unwrap_or_else(|| "554".to_string());
    let username = params.username.unwrap_or_else(|| "admin".to_string());
    let password = params.password.unwrap_or_default();
    let channel = params.channel.unwrap_or_else(|| "1".to_string());
    let stream_number = params.stream_number.unwrap_or_else(|| "1".to_string());

    let suffix = format!("{}{:02}", channel, stream_number.parse::<u32>().unwrap_or(1));

    let encoded_user = urlencoding::encode(&username);
    let encoded_pass = urlencoding::encode(&password);
    let rtsp_url = format!(
        "rtsp://{}:{}@{}:{}/ISAPI/Streaming/channels/{}",
        encoded_user, encoded_pass, params.ip, port, suffix
    );

    // Create a temporary directory for HLS segments
    let id = Uuid::new_v4().to_string();
    let tmp_dir = format!("/tmp/hls-proxyhl-{}", id);
    let playlist_path = format!("{}/playlist.m3u8", tmp_dir);
    let segment_pattern = format!("{}/segment%03d.ts", tmp_dir);
    let base_url = format!("/proxyhl/segment/{}/", id);

    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        error!("Failed to create temp directory: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create temp directory: {}", e),
        )
            .into_response();
    }

    let playlist_path_clone = playlist_path.clone();
    let rtsp_url_clone = rtsp_url.clone();

    // Create shutdown channel and register session
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    {
        let mut map = HLS_SESSIONS.write().await;
        map.insert(
            id.clone(),
            HlsSession {
                tmp_dir: tmp_dir.clone(),
                rtsp_url: rtsp_url.clone(),
                last_access: Instant::now(),
                shutdown: shutdown_tx.clone(),
            },
        );
    }

    // Spawn FFmpeg in background to generate HLS segments
    let id_clone_for_ffmpeg = id.clone();
    let tmp_dir_for_ffmpeg = tmp_dir.clone();
    let sessions_for_ffmpeg = HLS_SESSIONS.clone();
    tokio::spawn(async move {
        let mut child = match Command::new("ffmpeg")
            .args(&[
                "-rtsp_transport", "tcp",
                "-i", &rtsp_url_clone,
                "-f", "hls",
                "-hls_time", "2",
                "-hls_list_size", "5",
                "-hls_flags", "delete_segments+independent_segments",
                "-hls_segment_filename", &segment_pattern,
                "-hls_base_url", &base_url,
                "-codec:v", "libx264",
                "-preset", "ultrafast",
                "-tune", "zerolatency",
                "-g", "50",
                "-keyint_min", "25",
                "-sc_threshold", "0",
                "-b:v", "2000k",
                "-codec:a", "aac",
                "-ar", "44100",
                "-b:a", "128k",
                &playlist_path_clone,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                error!("Failed to start FFmpeg for HLS: {}", e);
                // Remove session if we failed to start
                let mut map = sessions_for_ffmpeg.write().await;
                map.remove(&id_clone_for_ffmpeg);
                return;
            }
        };

        // Wait for shutdown or process exit
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutting down HLS session {} due to inactivity or explicit stop", id_clone_for_ffmpeg);
                let _ = child.kill().await;
            }
            _ = child.wait() => {
                info!("HLS ffmpeg process exited for session {}", id_clone_for_ffmpeg);
            }
        }
        let _ = std::fs::remove_dir_all(&tmp_dir_for_ffmpeg);
        let mut map = sessions_for_ffmpeg.write().await;
        map.remove(&id_clone_for_ffmpeg);
    });

    // Spawn inactivity monitor
    let id_for_monitor = id.clone();
    let sessions_for_monitor = HLS_SESSIONS.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let should_shutdown = {
                let map = sessions_for_monitor.read().await;
                if let Some(sess) = map.get(&id_for_monitor) {
                    sess.last_access.elapsed() > HLS_IDLE_TIMEOUT
                } else {
                    // Session already gone
                    false
                }
            };
            if should_shutdown {
                info!("HLS session {} idle timeout reached; requesting shutdown", id_for_monitor);
                let mut map = sessions_for_monitor.write().await;
                if let Some(sess) = map.get(&id_for_monitor) {
                    let _ = sess.shutdown.try_send(());
                }
                break;
            }
        }
    });

    // Poll for playlist existence (up to ~20s), then redirect to it
    let playlist_rel_url = format!("/proxyhl/segment/{}/playlist.m3u8", id);
    let mut ready = false;
    for _ in 0..80 {
        if let Ok(meta) = std::fs::metadata(&playlist_path) {
            if meta.len() > 0 {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }

    if !ready {
        error!("Failed to find playlist after waiting: {}", playlist_path);
        return (
            StatusCode::BAD_GATEWAY,
            "HLS playlist not available; source may be unreachable or credentials invalid",
        )
            .into_response();
    }

    // Update last access
    {
        let mut map = HLS_SESSIONS.write().await;
        if let Some(sess) = map.get_mut(&id) {
            sess.last_access = Instant::now();
        }
    }

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, playlist_rel_url)
        .body(Body::empty())
        .unwrap()
}

async fn proxy_hls_segment(Path((id, file)): Path<(String, String)>) -> Response {
    // Prevent path traversal
    if file.contains("..") || file.contains('/') || file.contains('\\') {
        return (
            StatusCode::BAD_REQUEST,
            "Invalid segment path",
        )
            .into_response();
    }

    // Update session last access
    {
        let mut map = HLS_SESSIONS.write().await;
        if let Some(sess) = map.get_mut(&id) {
            sess.last_access = Instant::now();
        }
    }

    let path = format!("/tmp/hls-proxyhl-{}/{}", id, file);

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            // Basic content-type guess
            let ctype = if file.ends_with(".ts") {
                "video/mp2t"
            } else if file.ends_with(".m3u8") {
                "application/vnd.apple.mpegurl"
            } else {
                "application/octet-stream"
            };

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, ctype)
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) => {
            error!("Segment read error for {}: {}", path, e);
            (
                StatusCode::NOT_FOUND,
                "Segment not found",
            )
                .into_response()
        }
    }
}

#[derive(Serialize)]
struct HlsSessionView {
    id: String,
    rtsp_url: String,
    last_access_secs: u64,
}

#[derive(Serialize)]
struct HlsSessionsListResponse {
    sessions: Vec<HlsSessionView>,
}

async fn list_proxyhl_sessions() -> impl IntoResponse {
    let map = HLS_SESSIONS.read().await;
    let mut sessions: Vec<HlsSessionView> = Vec::new();
    for (id, sess) in map.iter() {
        sessions.push(HlsSessionView {
            id: id.clone(),
            rtsp_url: sess.rtsp_url.clone(),
            last_access_secs: sess.last_access.elapsed().as_secs(),
        });
    }
    // Sort by most recently accessed first
    sessions.sort_by_key(|s| std::cmp::Reverse(s.last_access_secs));
    Json(HlsSessionsListResponse { sessions })
}

async fn player_page(Query(params): Query<DirectStreamQuery>) -> Response {
    let hls_url = format!("/stream/hls?rtsp_url={}", urlencoding::encode(&params.rtsp_url));
    let html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>RTSP Stream Player</title>
    <script src="https://cdn.jsdelivr.net/npm/hls.js@latest"></script>
    <style>
        body {{
            margin: 0;
            padding: 20px;
            font-family: Arial, sans-serif;
            background: #1a1a1a;
            color: #fff;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        h1 {{
            text-align: center;
            margin-bottom: 20px;
        }}
        .video-wrapper {{
            background: #000;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
        }}
        video {{
            width: 100%;
            max-width: 1000px;
            height: auto;
            border-radius: 4px;
        }}
        .info {{
            margin-top: 20px;
            padding: 15px;
            background: #2a2a2a;
            border-radius: 4px;
        }}
        .status {{
            padding: 10px;
            margin-top: 10px;
            background: #334455;
            border-radius: 4px;
            font-size: 12px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎥 RTSP Stream Player</h1>
        <div class="video-wrapper">
            <video id="player" controls autoplay width="800" height="600"></video>
        </div>
        <div class="info">
            <strong>Stream URL:</strong><br>
            <code>{}</code>
            <div class="status" id="status">Loading...</div>
        </div>
    </div>
    <script>
        const videoElement = document.getElementById('player');
        const statusDiv = document.getElementById('status');
        const hls = new Hls();
        
        hls.loadSource('{}');
        hls.attachMedia(videoElement);
        
        hls.on(Hls.Events.MANIFEST_PARSED, function() {{
            statusDiv.innerHTML = '✅ Stream loaded successfully. Playing...';
            videoElement.play().catch(e => {{
                statusDiv.innerHTML = '⚠️ Autoplay blocked: ' + e.message;
            }});
        }});
        
        hls.on(Hls.Events.ERROR, function(event, data) {{
            if (data.fatal) {{
                statusDiv.innerHTML = '❌ Stream error: ' + data.response?.statusText || data.details;
            }}
        }});
    </script>
</body>
</html>"#, 
        params.rtsp_url,
        hls_url
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}
