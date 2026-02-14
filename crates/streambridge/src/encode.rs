use ndi_sdk::FourCCVideoType;

/// Reusable encoding buffers to avoid per-frame allocation.
pub struct EncodeBuffers {
    pub y_plane: Vec<u8>,
    pub u_plane: Vec<u8>,
    pub v_plane: Vec<u8>,
    /// Contiguous YUV buffer for turbojpeg: [Y][U][V]
    pub yuv_buf: Vec<u8>,
    compressor: turbojpeg::Compressor,
    last_w: usize,
    last_h: usize,
    last_quality: i32,
}

impl EncodeBuffers {
    pub fn new() -> Self {
        Self {
            y_plane: Vec::new(),
            u_plane: Vec::new(),
            v_plane: Vec::new(),
            yuv_buf: Vec::new(),
            compressor: turbojpeg::Compressor::new().expect("failed to create turbojpeg compressor"),
            last_w: 0,
            last_h: 0,
            last_quality: -1,
        }
    }

    /// Ensure buffers are sized for the given dimensions (4:2:0).
    fn ensure_capacity(&mut self, w: usize, h: usize) {
        if w != self.last_w || h != self.last_h {
            self.y_plane.resize(w * h, 0);
            self.u_plane.resize((w / 2) * (h / 2), 0);
            self.v_plane.resize((w / 2) * (h / 2), 0);
            self.yuv_buf.resize(w * h + (w / 2) * (h / 2) * 2, 0);
            self.last_w = w;
            self.last_h = h;
        }
    }

    fn set_quality(&mut self, quality: i32) {
        if quality != self.last_quality {
            self.compressor.set_quality(quality).expect("failed to set quality");
            self.last_quality = quality;
        }
    }
}

/// Convert UYVY packed 4:2:2 to planar YUV 4:2:0 (averaging chroma vertically).
pub fn uyvy_to_yuv420_planar(
    uyvy: &[u8],
    stride: usize,
    w: usize,
    h: usize,
    y: &mut [u8],
    u: &mut [u8],
    v: &mut [u8],
) {
    for row in 0..h {
        let src = &uyvy[row * stride..];
        let y_off = row * w;
        for col in (0..w).step_by(2) {
            let i = col * 2;
            y[y_off + col] = src[i + 1];
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

/// Encode a video frame to JPEG. Returns the JPEG bytes or an error message.
pub fn encode_frame(
    data: &[u8],
    w: usize,
    h: usize,
    stride: usize,
    fourcc: FourCCVideoType,
    quality: i32,
    buffers: &mut EncodeBuffers,
) -> Result<Vec<u8>, String> {
    buffers.set_quality(quality);

    match fourcc {
        FourCCVideoType::UYVY => {
            buffers.ensure_capacity(w, h);

            uyvy_to_yuv420_planar(
                data, stride, w, h,
                &mut buffers.y_plane,
                &mut buffers.u_plane,
                &mut buffers.v_plane,
            );

            // Pack into contiguous [Y][U][V] buffer
            let y_size = w * h;
            let uv_size = (w / 2) * (h / 2);
            buffers.yuv_buf[..y_size].copy_from_slice(&buffers.y_plane[..y_size]);
            buffers.yuv_buf[y_size..y_size + uv_size].copy_from_slice(&buffers.u_plane[..uv_size]);
            buffers.yuv_buf[y_size + uv_size..y_size + uv_size * 2].copy_from_slice(&buffers.v_plane[..uv_size]);

            let yuv_image = turbojpeg::YuvImage {
                pixels: &buffers.yuv_buf[..y_size + uv_size * 2],
                width: w,
                align: 1,
                height: h,
                subsamp: turbojpeg::Subsamp::Sub2x2,
            };

            buffers.compressor
                .compress_yuv_to_vec(yuv_image)
                .map_err(|e| format!("turbojpeg compress error: {e}"))
        }
        FourCCVideoType::BGRA | FourCCVideoType::BGRX => {
            let image = turbojpeg::Image {
                pixels: data,
                width: w,
                pitch: stride,
                height: h,
                format: turbojpeg::PixelFormat::BGRA,
            };
            buffers.compressor
                .compress_to_vec(image)
                .map_err(|e| format!("turbojpeg compress error: {e}"))
        }
        FourCCVideoType::RGBA | FourCCVideoType::RGBX => {
            let image = turbojpeg::Image {
                pixels: data,
                width: w,
                pitch: stride,
                height: h,
                format: turbojpeg::PixelFormat::RGBA,
            };
            buffers.compressor
                .compress_to_vec(image)
                .map_err(|e| format!("turbojpeg compress error: {e}"))
        }
        other => Err(format!("unsupported FourCC: {other:?}")),
    }
}
