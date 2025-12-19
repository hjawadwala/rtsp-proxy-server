# Quick Start Guide

## Step 1: Install FFmpeg

### Windows
1. Download FFmpeg from: https://www.gyan.dev/ffmpeg/builds/
   - Click on "ffmpeg-release-essentials.7z" or "ffmpeg-release-essentials.zip"
   
2. Extract the archive to a location like `C:\ffmpeg`

3. Add FFmpeg to your PATH:
   ```powershell
   # Temporary (current session only)
   $env:Path += ";C:\ffmpeg\bin"
   
   # Permanent (requires admin)
   [Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\ffmpeg\bin", "Machine")
   ```

4. Verify installation (restart terminal if needed):
   ```powershell
   ffmpeg -version
   ```

## Step 2: Run the Proxy Server

```powershell
cargo run --release
# or
.\target\release\rtsp-proxy.exe
```

The server will start on http://localhost:8080

## Step 3: Test with a Stream

### Option A: Using curl

Start a stream:
```powershell
curl -X POST "http://localhost:8080/api/stream/test1/start?rtsp_url=rtsp://your-camera-ip:554/stream"
```

List streams:
```powershell
curl http://localhost:8080/api/streams
```

### Option B: Using the Web Interface

1. Open `viewer.html` in your web browser
2. Enter a stream ID (e.g., "camera1")
3. Enter your RTSP URL (e.g., "rtsp://admin:password@192.168.1.100:554/stream")
4. Click "Start Stream"

### Option C: Using VLC or ffplay

After starting a stream via the API, view it with:
```powershell
# VLC
vlc http://localhost:8080/stream/test1/mpegts

# ffplay (comes with ffmpeg)
ffplay http://localhost:8080/stream/test1/mpegts
```

## Testing Without a Real Camera

If you don't have an RTSP camera, you can create a test stream using FFmpeg:

### Create a Test RTSP Server
```powershell
# Install MediaMTX (lightweight RTSP server)
# Download from: https://github.com/bluenviron/mediamtx/releases
# Extract and run mediamtx.exe

# Or use FFmpeg to create a test stream file
ffmpeg -f lavfi -i testsrc=size=1280x720:rate=25 -t 30 test.mp4
```

Then stream a video file as RTSP (requires additional tools like MediaMTX or VLC streaming).

## Common RTSP URL Formats

- `rtsp://192.168.1.100:554/stream`
- `rtsp://admin:password@192.168.1.100:554/stream1`
- `rtsp://username:password@camera.local/live/main`
- `rtsp://10.0.0.50/h264`

## API Reference

### Start Stream
```
POST /api/stream/{stream_id}/start?rtsp_url={encoded_url}
```

### Stop Stream
```
POST /api/stream/{stream_id}/stop
```

### List Streams
```
GET /api/streams
```

### Watch Stream (MPEG-TS)
```
GET /stream/{stream_id}/mpegts
```

### HLS Playlist
```
GET /stream/{stream_id}/hls/playlist.m3u8
```

## Troubleshooting

### FFmpeg not found
- Ensure FFmpeg is installed: `ffmpeg -version`
- Check PATH includes FFmpeg bin directory
- Restart terminal after changing PATH

### Stream won't start
- Test RTSP URL with VLC first
- Check firewall settings
- Verify camera credentials
- Check server logs for detailed errors

### Can't play stream in browser
- Browsers have limited codec support for MPEG-TS
- Use VLC, ffplay, or MPV player instead
- Or use HLS playlist (browser support varies)
