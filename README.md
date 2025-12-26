# RTSP Proxy

A high-performance RTSP to MPEG-TS/HLS proxy server written in Rust. This server allows you to stream RTSP video feeds over HTTP as MPEG-TS or HLS streams.

## Features

- ✅ Converts RTSP streams to MPEG-TS format
- ✅ Provides HLS playlist support
- ✅ RESTful API for stream management
- ✅ Support for authenticated RTSP URLs
- ✅ Built with Rust for high performance and reliability
- ✅ Uses FFmpeg for robust media handling
- ✅ Direct streaming without pre-starting streams
- ✅ Built-in web player for browser playback
- ✅ Hikvision NVR camera discovery support

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
cargo run --release -- --port 3000 --host 0.0.0.0
```

Options:
- `--port, -p`: HTTP server port (default: 5000)
- `--host`: Host to bind to (default: 0.0.0.0)

### API Endpoints

#### 1. Get Server Info
```bash
GET /
```

Returns server information and available endpoints.

Example:
```bash
curl http://localhost:5000/
```

#### 2. Direct Stream (No Pre-Start Required)
```bash
GET /stream?rtsp_url=<encoded_rtsp_url>
```

Stream directly from an RTSP URL without starting a managed stream. Perfect for VLC/ffplay.

Example:
```bash
# View in VLC
vlc "http://localhost:5000/stream?rtsp_url=rtsp://username:password@192.168.1.100:554/stream"

# View in ffplay
ffplay "http://localhost:5000/stream?rtsp_url=rtsp://admin:admin@192.168.1.100:554/stream1"
```

#### 3. Browser Player
```bash
GET /player?rtsp_url=<encoded_rtsp_url>
```

Opens a web page with HLS player for browser viewing.

Example:
```bash
# Open in browser
http://localhost:5000/player?rtsp_url=rtsp://username:password@192.168.1.100:554/stream
```

#### 4. Start a Managed Stream
```bash
POST /api/stream/{stream_id}/start?rtsp_url=<encoded_rtsp_url>
```

Example:
```bash
curl -X POST "http://localhost:5000/api/stream/camera1/start?rtsp_url=rtsp://username:password@192.168.1.100:554/stream"
```

#### 5. Stop a Managed Stream
```bash
POST /api/stream/{stream_id}/stop
```

Example:
```bash
curl -X POST http://localhost:5000/api/stream/camera1/stop
```

#### 6. List All Managed Streams
```bash
GET /api/streams
```

Example:
```bash
curl http://localhost:5000/api/streams
```

#### 7. Access MPEG-TS Stream (Managed)
```bash
GET /stream/{stream_id}/mpegts
```

Example - View in VLC or ffplay:
```bash
vlc http://localhost:5000/stream/camera1/mpegts
# or
ffplay http://localhost:5000/stream/camera1/mpegts
```

#### 8. Access HLS Playlist (Managed)
```bash
GET /stream/{stream_id}/hls/playlist.m3u8
```

Example:
```bash
vlc http://localhost:5000/stream/camera1/hls/playlist.m3u8
```

#### 9. Direct HLS Stream
```bash
GET /stream/hls?rtsp_url=<encoded_rtsp_url>
```

Get HLS playlist directly without managing a stream.

Example:
```bash
curl "http://localhost:5000/stream/hls?rtsp_url=rtsp://admin:pass@192.168.1.100:554/stream"
```

#### 10. Hikvision NVR - List Cameras
```bash
GET /proxy/cameras?ip=<nvr_ip>&port=<optional_port>&username=<optional_user>&password=<optional_pass>
```

Discover available cameras on a Hikvision NVR.

Example:
```bash
curl "http://localhost:5000/proxy/cameras?ip=192.168.1.64&username=admin&password=yourpass"
```

#### 11. Hikvision NVR - Stream Camera
```bash
GET /proxy/rtsp?ip=<nvr_ip>&channel=<channel_id>&username=<user>&password=<pass>
```

Stream a specific camera channel from Hikvision NVR as MJPEG.

Example:
```bash
# View in browser or VLC
http://localhost:5000/proxy/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass
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

### Example Workflows

#### Quick Start - Direct Streaming (Easiest)

1. Start the server:
```bash
cargo run --release
```

2. Open in your browser:
```
http://localhost:5000/player?rtsp_url=rtsp://admin:admin123@192.168.1.50:554/stream1
```

Or view directly in VLC:
```bash
vlc "http://localhost:5000/stream?rtsp_url=rtsp://admin:admin123@192.168.1.50:554/stream1"
```

#### Managed Streams Workflow

1. Start the server:
```bash
cargo run --release
```

2. Start streaming from an RTSP camera:
```bash
curl -X POST "http://localhost:5000/api/stream/frontdoor/start?rtsp_url=rtsp://admin:admin123@192.168.1.50:554/stream1"
```

3. Watch the stream:
```bash
# Using VLC
vlc http://localhost:5000/stream/frontdoor/mpegts

# Using ffplay
ffplay http://localhost:5000/stream/frontdoor/mpegts

# In a web browser (with HLS support)
# Open: http://localhost:5000/stream/frontdoor/hls/playlist.m3u8
```

4. Stop the stream when done:
```bash
curl -X POST http://localhost:5000/api/stream/frontdoor/stop
```

#### Hikvision NVR Workflow

1. List available cameras:
```bash
curl "http://localhost:5000/proxy/cameras?ip=192.168.1.64&username=admin&password=yourpass"
```

2. Stream a specific camera:
```bash
# Open in browser
http://localhost:5000/proxy/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass
```

## HTML Client Examples

### Simple Direct Stream Player

```html
<!DOCTYPE html>
<html>
<head>
    <title>RTSP Direct Player</title>
</head>
<body>
    <h1>RTSP Direct Stream Player</h1>
    
    <div>
        <input type="text" id="rtspUrl" placeholder="RTSP URL" value="rtsp://192.168.1.100/stream" style="width: 500px;">
        <button onclick="playStream()">Play Stream</button>
    </div>
    
    <div id="playerContainer"></div>

    <script>
        function playStream() {
            const rtspUrl = document.getElementById('rtspUrl').value;
            const encodedUrl = encodeURIComponent(rtspUrl);
            
            // Use built-in player page
            window.location.href = `http://localhost:5000/player?rtsp_url=${encodedUrl}`;
        }
    </script>
</body>
</html>
```

### Managed Stream Client

```html
<!DOCTYPE html>
<html>
<head>
    <title>RTSP Proxy Viewer</title>
</head>
<body>
    <h1>RTSP Stream Viewer (Managed)</h1>
    
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
                `http://localhost:5000/api/stream/${streamId}/start?rtsp_url=${encodeURIComponent(rtspUrl)}`,
                { method: 'POST' }
            );
            
            const result = await response.json();
            alert(result.message);
            
            if (result.success) {
                // Load HLS stream
                const video = document.getElementById('player');
                video.src = `http://localhost:5000/stream/${streamId}/hls/playlist.m3u8`;
            }
        }
        
        async function stopStream() {
            const streamId = document.getElementById('streamId').value;
            
            const response = await fetch(
                `http://localhost:5000/api/stream/${streamId}/stop`,
                { method: 'POST' }
            );
            
            const result = await response.json();
            alert(result.message);
        }
    </script>
</body>
</html>
```

For a complete web interface, see [viewer.html](viewer.html) included in the repository.

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
