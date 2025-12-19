use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::rtsp_client::RtspClient;

pub struct StreamInfo {
    pub rtsp_url: String,
    pub client: Arc<RwLock<RtspClient>>,
    pub active: bool,
}

pub struct StreamManager {
    streams: HashMap<String, StreamInfo>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub async fn start_stream(&mut self, stream_id: String, rtsp_url: String) -> Result<()> {
        info!("Starting stream {} from {}", stream_id, rtsp_url);

        // Check if stream already exists
        if self.streams.contains_key(&stream_id) {
            return Err(anyhow!("Stream {} already exists", stream_id));
        }

        // Create RTSP client
        let client = RtspClient::new(rtsp_url.clone())?;
        let client = Arc::new(RwLock::new(client));

        // Start the RTSP client
        {
            let mut c = client.write().await;
            c.start().await?;
        }

        // Store stream info
        self.streams.insert(
            stream_id.clone(),
            StreamInfo {
                rtsp_url,
                client,
                active: true,
            },
        );

        info!("Stream {} started successfully", stream_id);
        Ok(())
    }

    pub async fn stop_stream(&mut self, stream_id: &str) -> Result<()> {
        info!("Stopping stream {}", stream_id);

        if let Some(stream_info) = self.streams.get_mut(stream_id) {
            let mut client = stream_info.client.write().await;
            client.stop().await?;
            stream_info.active = false;
            info!("Stream {} stopped", stream_id);
            Ok(())
        } else {
            Err(anyhow!("Stream {} not found", stream_id))
        }
    }

    pub fn get_stream(&self, stream_id: &str) -> Option<&StreamInfo> {
        self.streams.get(stream_id)
    }

    pub fn list_streams(&self) -> Vec<String> {
        self.streams.keys().cloned().collect()
    }
}
