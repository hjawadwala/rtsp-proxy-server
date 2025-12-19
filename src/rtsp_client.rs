use anyhow::{anyhow, Result};
use bytes::Bytes;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct RtspClient {
    rtsp_url: String,
    ffmpeg_process: Option<Child>,
    data_sender: Option<mpsc::UnboundedSender<Bytes>>,
    data_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<Bytes>>>>,
}

impl RtspClient {
    pub fn new(rtsp_url: String) -> Result<Self> {
        Ok(Self {
            rtsp_url,
            ffmpeg_process: None,
            data_sender: None,
            data_receiver: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting RTSP client for {}", self.rtsp_url);

        // Create channel for data
        let (tx, rx) = mpsc::unbounded_channel();
        self.data_sender = Some(tx.clone());
        *self.data_receiver.lock().await = Some(rx);

        // Start FFmpeg process to convert RTSP to MPEG-TS
        // FFmpeg command: ffmpeg -i rtsp://... -f mpegts -codec:v libx264 -preset ultrafast -tune zerolatency -b:v 2000k -codec:a aac pipe:1
        let mut child = Command::new("ffmpeg")
            .args(&[
                "-rtsp_transport", "tcp",
                "-i", &self.rtsp_url,
                "-f", "mpegts",
                "-codec:v", "libx264",
                "-preset", "ultrafast",
                "-tune", "zerolatency",
                "-b:v", "2000k",
                "-codec:a", "aac",
                "-b:a", "128k",
                "-avoid_negative_ts", "make_zero",
                "-fflags", "+genpts",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow!("Failed to start FFmpeg. Make sure FFmpeg is installed and in PATH: {}", e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture FFmpeg stdout"))?;

        // Spawn a task to read from FFmpeg stdout and send to channel
        let sender = tx.clone();
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut buffer = vec![0u8; 188 * 7]; // MPEG-TS packets are 188 bytes, read multiple at once

            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        info!("FFmpeg stream ended");
                        break;
                    }
                    Ok(n) => {
                        let data = Bytes::copy_from_slice(&buffer[..n]);
                        if sender.send(data).is_err() {
                            warn!("Failed to send data to channel, receiver dropped");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error reading from FFmpeg: {}", e);
                        break;
                    }
                }
            }
        });

        self.ffmpeg_process = Some(child);

        info!("RTSP client started successfully");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping RTSP client");

        if let Some(mut process) = self.ffmpeg_process.take() {
            let _ = process.kill().await;
        }

        self.data_sender = None;

        Ok(())
    }

    pub async fn get_data_receiver(&self) -> Option<mpsc::UnboundedReceiver<Bytes>> {
        self.data_receiver.lock().await.take()
    }

    pub fn is_active(&self) -> bool {
        self.ffmpeg_process.is_some()
    }
}

impl Drop for RtspClient {
    fn drop(&mut self) {
        if let Some(mut process) = self.ffmpeg_process.take() {
            let _ = process.start_kill();
        }
    }
}
