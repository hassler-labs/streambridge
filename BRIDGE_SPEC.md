# streambridge — Bridge Server Spec

Convert NDI® streams to MJPEG for web consumption. Standalone Rust CLI that SightLine connects to for live video.

## CLI

```
streambridge.exe <COMMAND>

Commands:
  list   Discover and list available NDI sources on the network
  serve  Start MJPEG server — streams are created on-demand
  help   Print this message or the help of the given subcommand(s)
```

### `streambridge list`

One-shot discovery: prints NDI sources found on the network, then exits. Useful for verifying NDI is working before starting the server.

### `streambridge serve`

Starts the HTTP/WebSocket server. Streams are created on-demand when a client connects.

| Flag              | Default | Description                  |
|-------------------|---------|------------------------------|
| `--port`          | `9550`  | HTTP/WS listen port          |
| `--max-fps`       | `25`    | Max frames per second        |
| `--jpeg-quality`  | `75`    | TurboJPEG quality (1-100)    |
| `--log-interval`  | `20`    | Stats log interval (seconds) |

Port 9550 is IANA-unassigned (range 9537–9554 is free).

## Endpoints

### `GET /sources`

Returns JSON array of discovered NDI source names.

```json
["DESKTOP-H6VD0VJ (film)", "DESKTOP-H6VD0VJ (proj 1)"]
```

- NDI discovery runs on a background thread, continuously updated
- Response is the current snapshot of discovered names
- SightLine polls this every 5 seconds

### `WS /ws?source=<name>`

Opens a WebSocket connection that streams JPEG frames for the requested NDI source.

**Query parameters:**
| Param    | Required | Description                     |
|----------|----------|---------------------------------|
| `source` | yes      | NDI source name (URL-encoded)   |

**Frame format:** Each WebSocket message is a single JPEG image sent as a **binary message** (ArrayBuffer). No JSON wrapping, no base64 — raw JPEG bytes.

**Connection lifecycle:**
1. Server resolves `source` to an NDI receiver
2. If the source doesn't exist or can't connect, close the socket with code `4404` and reason `"source not found"`
3. On success, begin sending frames
4. When the client disconnects, tear down the NDI receiver (unless shared — see below)

### `GET /test`

Built-in test page for verifying the bridge without SightLine.

- Title: **streambridge test**
- Lists discovered NDI sources (fetched from `/sources`), each clickable to open a live preview
- Clicking a source opens a WebSocket connection to `/ws?source=<name>` and renders incoming JPEG frames to an `<img>` tag
- **Clear All** button — disconnects all active previews
- **Refresh Sources** button — re-fetches `/sources` and updates the list
- Multiple sources can be previewed simultaneously
- Page is self-contained: inline HTML/CSS/JS, no external dependencies. Served directly by the bridge.

## NDI → JPEG Pipeline

```
NDI Receiver  →  YUV frame (UYVY or NV12)  →  TurboJPEG compress  →  WebSocket binary message
```

1. **NDI receive** — Use the NDI SDK to create a receiver for the requested source. NDI delivers frames in a YUV format (typically UYVY for full-frame, NV12 for some sources).
2. **TurboJPEG encode** — Pass the YUV buffer directly to TurboJPEG's `tjCompressFromYUV` (or the appropriate variant for the pixel format). This avoids an intermediate RGB conversion.
   - JPEG quality: **75** (configurable via `--jpeg-quality`)
   - Chroma subsampling: `TJSAMP_420`
3. **Send** — Write the compressed JPEG bytes as a binary WebSocket message.

### Receiver Sharing

If multiple WebSocket clients request the same source, share a single NDI receiver. Maintain a refcount per source — only destroy the receiver when the last client disconnects.

## Max FPS

Limit the outbound frame rate to avoid flooding the browser.

- **Default:** 25 fps (configurable via `--max-fps`)
- Implementation: after encoding each frame, check elapsed time since last send. If less than `1000 / maxFps` ms, skip the frame. Don't sleep/block — just drop it and wait for the next NDI callback.

### Future: Adaptive Performance

_Nice-to-have for later._ When the server is under load (slow encode, backpressure from WebSocket sends), it could automatically lower quality or skip more aggressively. Not needed for v1 — manual `--max-fps` and `--jpeg-quality` flags are sufficient.

## Statistics Logging

Log a summary line to stdout every **20 seconds** (configurable via `--log-interval`) per active source:

```
[stats] "DESKTOP-H6VD0VJ (film)" — 2 clients, 28.4 fps out, 30.0 fps in, 1.2 ms encode avg, 847 KB/s, 3 dropped
```

Fields:
| Field           | Description                                                  |
|-----------------|--------------------------------------------------------------|
| clients         | Number of connected WebSocket clients for this source        |
| fps out         | Actual frames sent per second (after FPS cap and drops)      |
| fps in          | Frames received from NDI per second                          |
| encode avg      | Average TurboJPEG encode time in ms                          |
| KB/s            | Average outbound bandwidth in kilobytes per second           |
| dropped         | Frames skipped due to FPS cap or slow clients in this period |

Reset counters after each log interval.

## Error Handling

- NDI source disappears mid-stream → send WebSocket close `4410` with reason `"source lost"`, tear down receiver
- TurboJPEG encode fails → log error, skip frame, don't kill the connection
- Client sends a text message → ignore it (server is send-only)

## CORS

The HTTP endpoint must set:
```
Access-Control-Allow-Origin: *
```
WebSocket connections don't need CORS headers but should accept any origin.

## SightLine Client-Side Performance

Notes for the SightLine web client (`streaming.js`, `projector.js`) that consumes streambridge.

### WebSocket Connection Sharing

Currently each LED wall segment and each projector opens its own WebSocket — even when pointing at the same source URL. 3 segments + 2 projectors on the same source = 5 connections, 5 identical JPEG decodes per frame.

**Fix:** Client-side connection pool keyed by URL. One WebSocket per unique URL, one JPEG decode, shared source canvas. Each consumer references the shared canvas for its own masking/texturing.

### Frame Decoding

Current path: `ArrayBuffer → new Blob() → createObjectURL() → new Image() → img.onload`.

**Fix:** Use `createImageBitmap(blob)` instead. Decodes off the main thread, avoids Image element overhead, returns a bitmap drawable directly to canvas.

### Per-Frame Work That Should Be Cached

- **Tile clipping path** — The BigInt bitmask loop in `updateMaskedTexture()` rebuilds the clip region every frame, but `paintedTilesData` only changes on user edit. Cache as a `Path2D` object, rebuild only when tiles change.
- **Grid overlay** — Static grid lines are stroked every frame. Draw once to a separate canvas, composite over the masked output.

### Misc

- `console.log('Frame received...')` fires every frame (~25/s) — remove or gate behind a debug flag.
- `applyLedWallStreamToMesh()` replaces `segMesh.material` without disposing the old one — minor leak on first frame.

## Distribution & NDI Licensing

The NDI SDK license requires the following for distribution:

### Trademark Notice

Include in `--help` output and README:

> NDI® is a registered trademark of Vizrt Group.

### Binary Naming

The executable must **not** start with "NDI". Name: `streambridge.exe`.

### NDI Runtime DLL

Two distribution strategies:

1. **App-local** — Ship `Processing.NDI.Lib.x64.dll` alongside `streambridge.exe` (permitted by SDK license). Works out of the box.
2. **System install** — Require users to install the NDI Runtime from https://ndi.video/. Smaller download, always up-to-date.

**Open question:** How to load `Processing.NDI.Lib.x64.dll` at runtime (dynamic linking) instead of compile-time linking. Options to investigate:
- Rust `libloading` crate — `Library::new()` to load DLL, then look up function pointers
- Windows `LoadLibrary` / `GetProcAddress` via the `windows` crate
- NDI SDK may provide a loader header/pattern (check SDK `examples/` folder)
- Check if the existing Rust NDI bindings crate already handles this

### If Charging Money

Register for a Vendor ID at https://ndi.video/ before selling.
