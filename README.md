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

Below is a concise list of available routes. See detailed sections further down for usage and examples.

**Core Endpoints:**
- GET `/` — Server info and endpoint hints
- GET `/player?rtsp_url=...` — Browser player (HLS)
- GET `/stream?rtsp_url=...` — Direct MPEG-TS stream from RTSP URL
- GET `/stream/hls?rtsp_url=...` — Direct HLS playlist from RTSP URL

**Managed Streams:**
- POST `/api/stream/{id}/start?rtsp_url=...` — Start managed stream
- POST `/api/stream/{id}/stop` — Stop managed stream
- GET `/api/streams` — List managed streams
- GET `/stream/{id}/mpegts` — Managed MPEG-TS stream
- GET `/stream/{id}/hls/playlist.m3u8` — Managed HLS playlist

**Hikvision NVR (ISAPI):**
- GET `/proxy/cameras?ip=...&port=...&username=...&password=...` — List cameras on NVR
- GET `/proxy/rtsp?ip=...&channel=...&stream_number=...&username=...&password=...` — Hikvision MJPEG stream
- GET `/proxyhl/rtsp?ip=...&channel=...&stream_number=...&username=...&password=...&port=...` — Hikvision HLS playlist
- GET `/proxyhl/sessions` — List active HLS sessions with idle time
- GET `/proxyhl/segment/{id}/{file}` — HLS segment file (auto-served)

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

Alternative (form body): you can also send `application/x-www-form-urlencoded` with `rtsp_url=<encoded_rtsp_url>` in the POST body if you prefer not to pass it in the query string.

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

#### 10. List Active HLS Sessions
```bash
GET /proxyhl/sessions
```

Lists all active HLS sessions with their idle time in seconds. Sessions auto-expire after 60s of inactivity.

Example:
```bash
curl "http://localhost:5000/proxyhl/sessions"
```

Response:
```json
{
  "sessions": [
    {
      "id": "abc123...",
      "last_access_secs": 5
    },
    {
      "id": "def456...",
      "last_access_secs": 45
    }
  ]
}
```

#### 11. Hikvision NVR - List Cameras
```bash
GET /proxy/cameras?ip=<nvr_ip>&port=<optional_port>&username=<optional_user>&password=<optional_pass>
```

Discover available cameras on a Hikvision NVR.

Parameters:
- `ip` (required): NVR or camera IP
- `port` (optional): defaults to `554`
- `username` (optional): defaults to `admin`
- `password` (optional): defaults to empty

Behavior:
- Builds `http://{username}:{password}@{ip}:{port}/ISAPI/Streaming/channels` (credentials URL-encoded)
- Sends `Accept: application/json, application/xml`
- If JSON is returned, it is passed through unchanged
- If XML is returned, it is normalized to `{ "channels": [{ "id": "...", "name": "..." }] }`

Error responses:
- `502 Bad Gateway` if unreachable or non-success status from NVR
- `{"success": false, "message": "..."}` payload on errors

Example:
```bash
curl "http://localhost:5000/proxy/cameras?ip=192.168.1.64&username=admin&password=yourpass"
```

#### 12. Hikvision NVR - Stream Camera (MJPEG)
```bash
GET /proxy/rtsp?ip=<nvr_ip>&channel=<channel_id>&stream_number=<stream_idx>&username=<user>&password=<pass>&port=<optional_port>
```

Stream a specific camera channel from Hikvision NVR as MJPEG. **Note:** MJPEG is not directly playable in HTML `<video>` tags; use VLC, ffplay, or `<img>` tags for real-time preview.

Parameters:
- `ip` (required): NVR IP
- `channel` (optional): defaults to `1`
- `stream_number` (optional): defaults to `1` (1 → `01` main stream, 2 → `02` substream, etc.)
- `username` (optional): defaults to `admin`
- `password` (optional): defaults to empty
- `port` (optional): defaults to `554`

Response:
- Content-Type: `multipart/x-mixed-replace; boundary=ffserver`
- MJPEG frames suitable for simple previewing in media players

Example:
```bash
# View in VLC
vlc "http://localhost:5000/proxy/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass"

# View in ffplay
ffplay "http://localhost:5000/proxy/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass"
```

#### 13. Hikvision NVR - HLS Playlist
```bash
GET /proxyhl/rtsp?ip=<nvr_ip>&channel=<channel_id>&stream_number=<stream_idx>&username=<user>&password=<pass>&port=<optional_port>
```

Generate an HLS playlist for a Hikvision channel. Use with `hls.js` in browsers. **Sessions auto-expire after 60 seconds of inactivity.**

Parameters:
- `ip` (required)
- `channel` (optional, default `1`)
- `stream_number` (optional, default `1` → `01` main; `2` → `02` sub)
- `username` (optional, default `admin`)
- `password` (optional)
- `port` (optional, default `554`)

Behavior:
- Returns a 302 redirect to `/proxyhl/segment/{id}/playlist.m3u8`
- Spawns FFmpeg in background to generate HLS segments under `/tmp/hls-proxyhl-{id}/`
- Polls for playlist readiness (up to ~20s)
- Returns `502 Bad Gateway` if RTSP source is unreachable or credentials are invalid

Example:
```bash
# Get playlist URL (follows redirect)
curl -L "http://localhost:5000/proxyhl/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass"

# Browser/hls.js example
http://localhost:5000/proxyhl/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass
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

**Special Characters in Credentials:** If your credentials contain special characters (e.g., `$`, `@`, `%`), URL-encode them:
- `$` → `%24`
- `@` → `%40`
- `%` → `%25`

Example:
```bash
# Password is "pass$word123"
# URL-encoded: pass%24word123

curl "http://localhost:5000/proxyhl/rtsp?ip=192.168.1.64&username=admin&password=pass%24word123"
```

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

2. Stream a specific camera via MJPEG (preview):
```bash
# Open in media player
ffplay "http://localhost:5000/proxy/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass"
```

3. Stream a specific camera via HLS (browser):
```bash
# Open in browser (with HLS support)
http://localhost:5000/proxyhl/rtsp?ip=192.168.1.64&channel=1&username=admin&password=yourpass
```

4. Check active HLS sessions:
```bash
curl "http://localhost:5000/proxyhl/sessions"
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

### HLS Playlist Not Found (502 Error)
- RTSP source is unreachable or not responding
- Verify credentials are correct (especially special characters — they must be URL-encoded)
- Check that port `554` (or specified port) is accessible from the server
- Ensure the stream path matches (e.g., channel numbers for Hikvision)

### Session Inactivity Timeout
HLS sessions auto-expire after 60 seconds without any playlist or segment requests. To check active sessions:
```bash
curl "http://localhost:5000/proxyhl/sessions"
```

To keep a session alive, regularly request the playlist or segments.

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
