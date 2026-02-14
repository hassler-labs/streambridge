# NDI to TurboJPEG Recipe

Reference for receiving NDI video frames and encoding them to JPEG via turbojpeg.

## 1. The `ndi-sdk-dt` Crate (3pp/ndi)

A Rust wrapper around the NDI SDK v6 (`Processing.NDI.Lib.x64.dll`), originally forked from [julusian/rust-ndi](https://github.com/julusian/rust-ndi).

### Crate name & features
- **Package**: `ndi-sdk-dt`
- **Feature `dynamic-link`**: loads `Processing.NDI.Lib.x64.dll` at runtime via `libloading` instead of static linking. Without it, the build links `Processing.NDI.Lib.x64` statically via `build.rs`.

### Bindings generation
Bindings in `src/sdk.rs` are auto-generated with bindgen from `Processing.NDI.Lib.h`:
```bash
bindgen Processing.NDI.Lib.h -o src/sdk.rs \
  --no-layout-tests \
  --allowlist-function NDIlib_v3_load \
  --allowlist-type ".*" --allowlist-var ".*"
```

### Architecture

```
NDIInstance            -- entry point, owns Arc<NDIHandle>
  ├── create_find_instance()   -> FindInstance
  └── create_receive_instance() -> Arc<ReceiveInstance>

FindInstance           -- discovers NDI sources on the network
  ├── wait_for_sources(timeout_ms) -> bool
  └── get_current_sources() -> Vec<FindSource { name, url }>

ReceiveInstance         -- receives frames from a connected source
  ├── connect(source)  -> bool
  └── receive_capture(video, audio, metadata, timeout)
       -> Result<ReceiveCaptureResult, ReceiveCaptureError>

ReceiveCaptureResult::Video(VideoFrame)
  ├── width, height: i32
  ├── four_cc_type: FourCCType
  ├── frame_format_type: FrameFormatType
  ├── timecode, timestamp: i64
  └── lock_data() -> Option<VideoFrameData<'_>>   // &[u8] of pixel data
```

### Initialization

```rust
use ndi_sdk_dt::{
    NDIInstance,
    receive::{ReceiveBandwidth, ReceiveColorFormat, ReceiveCaptureResult, ReceiveInstanceExt},
};

let instance: NDIInstance = ndi_sdk_dt::load()?;
```

With `dynamic-link`, the DLL is found via:
1. `./Processing.NDI.Lib.x64.dll` (current dir)
2. `$NDI_RUNTIME_DIR_V3/libndi.so` (env var)
3. System library path

Without `dynamic-link`, it links statically and calls `NDIlib_v3_load()` directly.

### Finding sources — how NDI discovery works

NDI uses mDNS/Bonjour for zero-config discovery on the local subnet. When you create a `FindInstance`, it starts a background thread that listens for NDI source announcements via multicast. Sources outside the local subnet are not discovered automatically — you must provide their IPs via `extra_ips`.

**Discovery flow:**

```
1. create_find_instance(show_local, extra_ips)
   └── NDIlib_find_create_v2()
       - starts mDNS listener on the network
       - if extra_ips provided, also unicast-queries those IPs
       - if show_local_sources=true, includes sources on this machine

2. wait_for_sources(timeout_ms) -> bool
   └── NDIlib_find_wait_for_sources()
       - blocks up to timeout_ms waiting for the source list to change
       - returns true if sources were added/removed since last call
       - returns false on timeout (no change)
       - first call almost always returns true (initial population)

3. get_current_sources() -> Vec<FindSource>
   └── NDIlib_find_get_current_sources()
       - returns snapshot of all currently visible sources
       - memory is owned by the SDK, freed on next call or destroy
       - the Rust wrapper copies into Vec<FindSource> immediately
```

**FindSource struct:**
```rust
pub struct FindSource {
    pub name: String,   // e.g. "MY-PC (OBS)" or "MACHINE (Source 1)"
    pub url: Option<String>,  // e.g. "192.168.1.50:5961" — used for direct connect
}
```

The `name` format is always `"MACHINE_NAME (SOURCE_NAME)"`. The URL is the TCP address the receiver connects to for the actual video stream.

**Usage pattern — poll loop:**
```rust
let finder = instance.create_find_instance(true, &extra_ips)?;

// Typical: wait a bit for initial discovery, then grab sources
finder.wait_for_sources(5000);  // block up to 5s for first results
let sources = finder.get_current_sources();

// Or poll continuously for changes:
loop {
    if finder.wait_for_sources(2000) {
        let sources = finder.get_current_sources();
        // source list changed — update UI, reconnect, etc.
    }
}
```

**`extra_ips` parameter:**
A `&[String]` of additional IPs/hostnames to probe. Joined with `,` internally and passed to `NDIlib_find_create_t.p_extra_ips`. Use this for sources on different subnets or behind firewalls:
```rust
let extra_ips = vec!["192.168.2.100".to_string(), "10.0.0.50".to_string()];
let finder = instance.create_find_instance(true, &extra_ips)?;
```

**Connecting to a found source:**
```rust
// Find by name
let source = sources.iter().find(|s| s.name == "MY-PC (OBS)").unwrap();

// Pass to receiver.connect() — internally converts to NDIlib_source_t
// via util::to_ndi_source() which maps name/url to CStrings
receiver.connect(Some(&source));
```

**Lifetime:** The `FindInstance` stops discovery when dropped (`NDIlib_find_destroy`). Sources found before dropping remain valid as `FindSource` (Rust-owned strings).

### Receiving frames

```rust
let receiver = instance.create_receive_instance(
    ReceiveBandwidth::Highest,   // or Lowest for proxy
    ReceiveColorFormat::Fastest, // SDK picks fastest format (usually UYVY)
)?;

receiver.connect(Some(&source));

match receiver.receive_capture(true, false, false, 5000)? {
    ReceiveCaptureResult::Video(video) => {
        // video.width, video.height, video.four_cc_type
        if let Some(data) = video.lock_data() {
            // data: &[u8] -- raw pixel bytes
            // data.len() depends on FourCC:
            //   UYVY: stride * height  (stride >= width * 2)
            //   UYVA: stride * height + width * height (extra alpha plane)
        }
    }
    _ => {}
}
```

### ReceiveColorFormat options

| Enum variant | No alpha | With alpha |
|---|---|---|
| `Fastest`  | UYVY | UYVA |
| `UyvyBgra` | UYVY | BGRA |
| `UyvyRgba` | UYVY | RGBA |
| `BgrxBgra` | BGRX | BGRA |
| `RgbxRgba` | RGBX | RGBA |

### ReceiveBandwidth options

| Variant | Description |
|---|---|
| `Highest` | Full quality |
| `Lowest` | Low quality proxy |
| `AudioOnly` | Audio only |
| `MetadataOnly` | Metadata only |

### FourCCType (all pixel formats the SDK can deliver)

| FourCC | Layout | Description |
|---|---|---|
| `UYVY` | Packed `[U Y V Y]` per 2 pixels | 4:2:2 YUV, 16 bits/pixel |
| `UYVA` | UYVY data + separate alpha plane | 4:2:2 YUV + 8-bit alpha |
| `I420` | Planar Y, U, V (4:2:0) | 12 bits/pixel |
| `NV12` | Planar Y, interleaved UV (4:2:0) | 12 bits/pixel |
| `YV12` | Planar Y, V, U (4:2:0) | 12 bits/pixel (like I420 but U/V swapped) |
| `BGRA` | Packed B,G,R,A | 32 bits/pixel |
| `BGRX` | Packed B,G,R,X | 32 bits/pixel, alpha ignored |
| `RGBA` | Packed R,G,B,A | 32 bits/pixel |
| `RGBX` | Packed R,G,B,X | 32 bits/pixel, alpha ignored |

### VideoFrame data layout

```
NDIlib_video_frame_v2_t:
  xres, yres          -- dimensions
  FourCC              -- pixel format (see above)
  frame_rate_N/D      -- framerate as fraction
  frame_format_type   -- progressive/interlaced/field0/field1
  p_data              -- pointer to pixel data
  line_stride_in_bytes -- row stride (may differ from width * bpp)
  timecode, timestamp
```

The `lock_data()` method returns a `&[u8]` slice of length:
- **UYVY**: `stride * height`
- **UYVA**: `stride * height + width * height` (alpha plane appended)
- **I420/YV12**: `stride * height * 3/2` (Y + half-size U + half-size V)
- **NV12**: `stride * height * 3/2` (Y + interleaved UV)
- **BGRA/RGBA/etc**: `stride * height`

### Frame lifetime

`VideoFrame` holds a `Weak<ReceiveInstance>`. On drop, it calls `NDIlib_recv_free_video_v2` to release the frame back to NDI. The data pointer is only valid while `lock_data()` guard is alive.

---

## 2. How stagerenderer Uses NDI (ndi.rs)

The `stagerenderer` crate receives NDI as UYVY and converts to RGBA for GPU upload:

```rust
// Creates receiver with Fastest (-> UYVY) and Lowest bandwidth (proxy quality)
instance.create_receive_instance(ReceiveBandwidth::Lowest, ReceiveColorFormat::Fastest)

// On each frame:
video.lock_data()  // -> &[u8] of UYVY packed data
convert_to_rgba(&video, data, &mut rgba)  // UYVY -> RGBA via `yuv` crate
uploader.create_image(R8G8B8A8_UNORM, ...)  // upload RGBA to Vulkan texture
```

### YUV matrix selection (BT.601 / BT.709 / BT.2020)

The matrix is chosen by resolution or explicit `NDIColorSpace` config:
- User override: `Rec601`, `Rec709`, `Rec2020`
- Auto (by resolution):
  - `> 1920x1080` -> BT.2020
  - `> 720x576`   -> BT.709
  - else          -> BT.601

---

## 3. NDI UYVY to TurboJPEG Pipeline

### Why avoid UYVY -> RGBA -> JPEG

JPEG uses YCbCr internally. Going UYVY -> RGBA -> JPEG means:
1. YUV to RGB conversion (CPU work)
2. RGB to YCbCr conversion inside turbojpeg (more CPU work)
3. Precision loss from two color space conversions

### Recommended: UYVY -> planar YUV -> turbojpeg

**Step 1: Receive UYVY from NDI**
```rust
let receiver = instance.create_receive_instance(
    ReceiveBandwidth::Highest,
    ReceiveColorFormat::Fastest,  // gives UYVY
)?;
```

**Step 2: Convert UYVY (packed 4:2:2) to planar YUV**

Option A - Keep 4:2:2 (no chroma loss):
```rust
// UYVY layout: [U0 Y0 V0 Y1] [U2 Y2 V2 Y3] ...
// -> Y plane: [Y0 Y1 Y2 Y3 ...]          (width * height bytes)
// -> U plane: [U0 U2 ...]                 (width/2 * height bytes)
// -> V plane: [V0 V2 ...]                 (width/2 * height bytes)
fn uyvy_to_yuv422_planar(uyvy: &[u8], stride: usize, w: usize, h: usize,
                          y: &mut [u8], u: &mut [u8], v: &mut [u8]) {
    for row in 0..h {
        let src = &uyvy[row * stride..];
        let y_off = row * w;
        let uv_off = row * (w / 2);
        for col in (0..w).step_by(2) {
            let i = col * 2;
            u[uv_off + col / 2] = src[i];
            y[y_off + col]      = src[i + 1];
            v[uv_off + col / 2] = src[i + 2];
            y[y_off + col + 1]  = src[i + 3];
        }
    }
}
```

Option B - Downsample to 4:2:0 (smaller JPEG, standard for photos):
```rust
// Same as above but average U/V vertically for each pair of rows
fn uyvy_to_yuv420_planar(uyvy: &[u8], stride: usize, w: usize, h: usize,
                          y: &mut [u8], u: &mut [u8], v: &mut [u8]) {
    for row in 0..h {
        let src = &uyvy[row * stride..];
        let y_off = row * w;
        for col in (0..w).step_by(2) {
            let i = col * 2;
            y[y_off + col]     = src[i + 1];
            y[y_off + col + 1] = src[i + 3];

            let uv_row = row / 2;
            let uv_off = uv_row * (w / 2) + col / 2;
            if row % 2 == 0 {
                u[uv_off] = src[i];
                v[uv_off] = src[i + 2];
            } else {
                u[uv_off] = ((u[uv_off] as u16 + src[i] as u16) / 2) as u8;
                v[uv_off] = ((v[uv_off] as u16 + src[i + 2] as u16) / 2) as u8;
            }
        }
    }
}
```

**Step 3: Feed planar YUV into turbojpeg**

Using the `turbojpeg` C API (or a Rust wrapper like the `turbojpeg` crate):

```rust
// For 4:2:2:
tj3Set(handle, TJPARAM_SUBSAMP, TJSAMP_422);
tj3Set(handle, TJPARAM_QUALITY, 85);
// planes must be contiguous: [Y][U][V]
tj3CompressFromYUV8(handle, yuv_buf.as_ptr(), width, /*pad=*/1, height,
                    &mut jpeg_buf, &mut jpeg_size);

// For 4:2:0:
tj3Set(handle, TJPARAM_SUBSAMP, TJSAMP_420);
// same call
```

### Alternative: RGBA path (simpler, slower)

If you prefer simplicity or need RGBA for other reasons:
```rust
// 1. Convert UYVY to RGBA (use the `yuv` crate like stagerenderer does)
yuv::uyvy422_to_rgba(&uyvy_img, &mut rgba, stride, YuvRange::Limited, matrix);

// 2. Feed RGBA into turbojpeg
tj3Set(handle, TJPARAM_QUALITY, 85);
tj3Compress8(handle, rgba.as_ptr(), width, /*pitch=*/width*4, height,
             TJPF_RGBA, &mut jpeg_buf, &mut jpeg_size);
```

### Buffer sizes

| Format | Buffer size |
|---|---|
| UYVY input | `stride * height` (stride >= width * 2) |
| Y plane (any) | `width * height` |
| U plane (4:2:2) | `width/2 * height` |
| V plane (4:2:2) | `width/2 * height` |
| U plane (4:2:0) | `width/2 * height/2` |
| V plane (4:2:0) | `width/2 * height/2` |
| RGBA | `width * height * 4` |
| JPEG output | use `tj3JPEGBufSize(width, height, subsamp)` to allocate |

### Color matrix note

- JPEG (JFIF) specifies BT.601 YCbCr.
- NDI at HD resolution typically uses BT.709.
- turbojpeg's `CompressFromYUV` assumes the YUV data is already in the correct color space and does **no matrix conversion** -- it just DCT-compresses the planes as-is.
- This means if NDI delivers BT.709 UYVY and you feed it directly, decoders will interpret it as BT.601. For preview/thumbnail purposes this is usually fine (slight color shift). For color-accurate work you'd need an explicit matrix conversion.

---

## 4. Minimal Working Example Skeleton

```rust
use ndi_sdk_dt::{
    NDIInstance,
    receive::{ReceiveBandwidth, ReceiveColorFormat, ReceiveCaptureResult, ReceiveInstanceExt},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load NDI
    let instance = ndi_sdk_dt::load()?;

    // 2. Find a source
    let finder = instance.create_find_instance(true, &[])?;
    let source = loop {
        finder.wait_for_sources(5000);
        let sources = finder.get_current_sources();
        if let Some(s) = sources.into_iter().next() { break s; }
    };

    // 3. Create receiver
    let receiver = instance.create_receive_instance(
        ReceiveBandwidth::Highest,
        ReceiveColorFormat::Fastest,
    )?;
    receiver.connect(Some(&source));

    // 4. Capture loop
    loop {
        match receiver.receive_capture(true, false, false, 5000)? {
            ReceiveCaptureResult::Video(video) => {
                if let Some(data) = video.lock_data() {
                    let w = video.width as usize;
                    let h = video.height as usize;
                    // stride from NDI (may be > w*2)
                    let stride = data.len() / h;

                    // Allocate planar buffers
                    let mut y_plane = vec![0u8; w * h];
                    let mut u_plane = vec![0u8; (w / 2) * h];
                    let mut v_plane = vec![0u8; (w / 2) * h];

                    // Convert UYVY -> planar 4:2:2
                    uyvy_to_yuv422_planar(
                        &data, stride, w, h,
                        &mut y_plane, &mut u_plane, &mut v_plane,
                    );

                    // Feed into turbojpeg...
                    // let jpeg = turbojpeg_compress_yuv(w, h, &y_plane, &u_plane, &v_plane);
                }
            }
            _ => {}
        }
    }
}
```
