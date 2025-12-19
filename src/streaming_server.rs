use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

use crate::stream_manager::StreamManager;

pub struct StreamingServer {
    host: String,
    port: u16,
    stream_manager: Arc<RwLock<StreamManager>>,
}

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
            .route("/stream/:id/hls/playlist.m3u8", get(stream_hls_playlist))
            .route("/stream/:id/hls/:segment", get(stream_hls_segment))
            .layer(CorsLayer::permissive())
            .with_state(self.stream_manager);

        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        
        info!("Server listening on http://{}", addr);
        info!("API endpoints:");
        info!("  POST /api/stream/:id/start?rtsp_url=<url> - Start a stream");
        info!("  POST /api/stream/:id/stop - Stop a stream");
        info!("  GET /api/streams - List all streams");
        info!("  GET /stream/:id/mpegts - Get MPEG-TS stream");
        info!("  GET /stream/:id/hls/playlist.m3u8 - Get HLS playlist");

        axum::serve(listener, app).await?;

        Ok(())
    }
}

async fn root_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "RTSP Proxy Server",
        "version": "0.1.0",
        "endpoints": {
            "start_stream": "POST /api/stream/:id/start?rtsp_url=<url>",
            "stop_stream": "POST /api/stream/:id/stop",
            "list_streams": "GET /api/streams",
            "mpegts_stream": "GET /stream/:id/mpegts",
            "hls_playlist": "GET /stream/:id/hls/playlist.m3u8"
        }
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
    Query(params): Query<StartStreamRequest>,
    State(manager): State<Arc<RwLock<StreamManager>>>,
) -> impl IntoResponse {
    info!("Received request to start stream {}", id);

    let mut manager = manager.write().await;
    match manager.start_stream(id.clone(), params.rtsp_url).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Stream {} started", id),
            }),
        ),
        Err(e) => {
            error!("Failed to start stream {}: {}", id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: format!("Failed to start stream: {}", e),
                }),
            )
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
