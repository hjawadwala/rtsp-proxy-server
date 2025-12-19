# RTSP Proxy

A high-performance RTSP to MPEG-TS/HLS proxy server written in Rust. This server allows you to stream RTSP video feeds over HTTP as MPEG-TS or HLS streams.

## Features

- ✅ Converts RTSP streams to MPEG-TS format
- ✅ Provides HLS playlist support
- ✅ RESTful API for stream management
- ✅ Support for authenticated RTSP URLs
- ✅ Built with Rust for high performance and reliability
- ✅ Uses GStreamer for robust media handling

## Prerequisites

### FFmpeg Installation

This proxy uses FFmpeg to handle RTSP streams and convert them to MPEG-TS format.

#### Windows
1. Download FFmpeg from https://www.gyan.dev/ffmpeg/builds/
2. Extract the archive (e.g., `ffmpeg-release-essentials.zip`)
3. Add the `bin` folder to your PATH:
   ```powershell
   # Add to PATH (replace with your actual path)
   $env:Path += ";C:\ffmpeg\bin"
   ```
4. Verify installation:
   ```powershell
   ffmpeg -version
   ```

#### Linux
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install ffmpeg

# Fedora
sudo dnf install ffmpeg

# Arch
sudo pacman -S ffmpeg
```

#### macOS
```bash
brew install ffmpeg
```

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd rtsp-proxy
```

2. Build the project:
```bash
cargo build --release
```

## Usage

### Starting the Server

```bash
cargo run --release
```

Or with custom options:
```bash
cargo run --release -- --port 8080 --host 0.0.0.0
```

Options:
- `--port, -p`: HTTP server port (default: 8080)
- `--host`: Host to bind to (default: 0.0.0.0)

### API Endpoints

#### 1. Start a Stream
```bash
POST /api/stream/{stream_id}/start?rtsp_url=<encoded_rtsp_url>
```

Example:
```bash
curl -X POST "http://localhost:8080/api/stream/camera1/start?rtsp_url=rtsp://username:password@192.168.1.100:554/stream"
```

#### 2. Stop a Stream
```bash
POST /api/stream/{stream_id}/stop
```

Example:
```bash
curl -X POST http://localhost:8080/api/stream/camera1/stop
```

#### 3. List All Streams
```bash
GET /api/streams
```

Example:
```bash
curl http://localhost:8080/api/streams
```

#### 4. Access MPEG-TS Stream
```bash
GET /stream/{stream_id}/mpegts
```

Example - View in VLC or ffplay:
```bash
vlc http://localhost:8080/stream/camera1/mpegts
# or
ffplay http://localhost:8080/stream/camera1/mpegts
```

#### 5. Access HLS Playlist
```bash
GET /stream/{stream_id}/hls/playlist.m3u8
```

Example:
```bash
vlc http://localhost:8080/stream/camera1/hls/playlist.m3u8
```

### RTSP URL Format

The proxy supports various RTSP URL formats:

```
rtsp://[username]:[password]@[ip]:[port]/[path]
rtsp://[username]:[password]@[ip]/[path]
rtsp://[ip]:[port]/[path]
rtsp://[ip]/[path]
```

Examples:
- `rtsp://admin:password123@192.168.1.100:554/stream1`
- `rtsp://192.168.1.100/live/main`
- `rtsp://user:pass@camera.local:8554/h264`

**Important:** When passing the RTSP URL as a query parameter, make sure to URL-encode it properly, especially if it contains special characters.

### Example Workflow

1. Start the server:
```bash
cargo run --release
```

2. Start streaming from an RTSP camera:
```bash
curl -X POST "http://localhost:8080/api/stream/frontdoor/start?rtsp_url=rtsp://admin:admin123@192.168.1.50:554/stream1"
```

3. Watch the stream:
```bash
# Using VLC
vlc http://localhost:8080/stream/frontdoor/mpegts

# Using ffplay
ffplay http://localhost:8080/stream/frontdoor/mpegts

# In a web browser (with HLS support)
# Open: http://localhost:8080/stream/frontdoor/hls/playlist.m3u8
```

4. Stop the stream when done:
```bash
curl -X POST http://localhost:8080/api/stream/frontdoor/stop
```

## HTML Client Example

```html
<!DOCTYPE html>
<html>
<head>
    <title>RTSP Proxy Viewer</title>
</head>
<body>
    <h1>RTSP Stream Viewer</h1>
    
    <div>
        <input type="text" id="streamId" placeholder="Stream ID" value="camera1">
        <input type="text" id="rtspUrl" placeholder="RTSP URL" value="rtsp://192.168.1.100/stream">
        <button onclick="startStream()">Start Stream</button>
        <button onclick="stopStream()">Stop Stream</button>
    </div>
    
    <div>
        <video id="player" controls autoplay width="800"></video>
    </div>

    <script>
        async function startStream() {
            const streamId = document.getElementById('streamId').value;
            const rtspUrl = document.getElementById('rtspUrl').value;
            
            const response = await fetch(
                `http://localhost:8080/api/stream/${streamId}/start?rtsp_url=${encodeURIComponent(rtspUrl)}`,
                { method: 'POST' }
            );
            
            const result = await response.json();
            alert(result.message);
            
            if (result.success) {
                // Load HLS stream
                const video = document.getElementById('player');
                video.src = `http://localhost:8080/stream/${streamId}/hls/playlist.m3u8`;
            }
        }
        
        async function stopStream() {
            const streamId = document.getElementById('streamId').value;
            
            const response = await fetch(
                `http://localhost:8080/api/stream/${streamId}/stop`,
                { method: 'POST' }
            );
            
            const result = await response.json();
            alert(result.message);
        }
    </script>
</body>
</html>
```

## Troubleshooting

### FFmpeg Not Found
If you get errors about FFmpeg not being found:
- Ensure FFmpeg is installed: run `ffmpeg -version`
- Verify FFmpeg is in your PATH
- On Windows, restart your terminal after adding FFmpeg to PATH

### Stream Not Playing
- Verify the RTSP URL is accessible using VLC or another RTSP client first
- Test with FFmpeg directly: `ffmpeg -i rtsp://your-url -f mpegts output.ts`
- Check network connectivity to the RTSP source
- Ensure credentials are correct if authentication is required
- Check server logs for detailed error messages

### Performance Issues
You can adjust FFmpeg parameters in `rtsp_client.rs`:
- Reduce bitrate: change `-b:v 2000k` to a lower value (e.g., `1000k`)
- Change encoding preset: modify `-preset ultrafast` to `superfast`, `veryfast`, or `fast`
## Architecture

The proxy consists of several key components:

1. **RTSP Client** (`rtsp_client.rs`): Connects to RTSP sources using FFmpeg and converts video to MPEG-TS
2. **Stream Manager** (`stream_manager.rs`): Manages multiple concurrent streams
3. **Streaming Server** (`streaming_server.rs`): HTTP server that provides RESTful API and stream endpoints
4. **Main** (`main.rs`): Application entry point and initialization

The proxy uses FFmpeg as a subprocess to handle the complex RTSP protocol and video transcoding, making it simple to build and deploy across platforms.rrent streams
3. **Streaming Server** (`streaming_server.rs`): HTTP server that provides RESTful API and stream endpoints
4. **Main** (`main.rs`): Application entry point and initialization

## License

MIT License - See LICENSE file for details

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
