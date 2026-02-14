use std::os::raw::{c_char, c_int};

// Opaque handle types
pub enum NDIlib_find_instance_type {}
pub type NDIlib_find_instance_t = *mut NDIlib_find_instance_type;

pub enum NDIlib_recv_instance_type {}
pub type NDIlib_recv_instance_t = *mut NDIlib_recv_instance_type;

// Frame type returned by recv_capture
pub type NDIlib_frame_type_e = i32;
pub const NDIlib_frame_type_none: NDIlib_frame_type_e = 0;
pub const NDIlib_frame_type_video: NDIlib_frame_type_e = 1;
pub const NDIlib_frame_type_audio: NDIlib_frame_type_e = 2;
pub const NDIlib_frame_type_metadata: NDIlib_frame_type_e = 3;
pub const NDIlib_frame_type_error: NDIlib_frame_type_e = 4;
pub const NDIlib_frame_type_status_change: NDIlib_frame_type_e = 100;

// FourCC video types
pub type NDIlib_FourCC_video_type_e = u32;

macro_rules! fourcc {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        ($a as u32) | (($b as u32) << 8) | (($c as u32) << 16) | (($d as u32) << 24)
    };
}

pub const NDIlib_FourCC_video_type_UYVY: NDIlib_FourCC_video_type_e =
    fourcc!(b'U', b'Y', b'V', b'Y');
pub const NDIlib_FourCC_video_type_UYVA: NDIlib_FourCC_video_type_e =
    fourcc!(b'U', b'Y', b'V', b'A');
pub const NDIlib_FourCC_video_type_I420: NDIlib_FourCC_video_type_e =
    fourcc!(b'I', b'4', b'2', b'0');
pub const NDIlib_FourCC_video_type_NV12: NDIlib_FourCC_video_type_e =
    fourcc!(b'N', b'V', b'1', b'2');
pub const NDIlib_FourCC_video_type_YV12: NDIlib_FourCC_video_type_e =
    fourcc!(b'Y', b'V', b'1', b'2');
pub const NDIlib_FourCC_video_type_BGRA: NDIlib_FourCC_video_type_e =
    fourcc!(b'B', b'G', b'R', b'A');
pub const NDIlib_FourCC_video_type_BGRX: NDIlib_FourCC_video_type_e =
    fourcc!(b'B', b'G', b'R', b'X');
pub const NDIlib_FourCC_video_type_RGBA: NDIlib_FourCC_video_type_e =
    fourcc!(b'R', b'G', b'B', b'A');
pub const NDIlib_FourCC_video_type_RGBX: NDIlib_FourCC_video_type_e =
    fourcc!(b'R', b'G', b'B', b'X');

// Frame format type
pub type NDIlib_frame_format_type_e = i32;
pub const NDIlib_frame_format_type_progressive: NDIlib_frame_format_type_e = 1;
pub const NDIlib_frame_format_type_interleaved: NDIlib_frame_format_type_e = 0;
pub const NDIlib_frame_format_type_field_0: NDIlib_frame_format_type_e = 2;
pub const NDIlib_frame_format_type_field_1: NDIlib_frame_format_type_e = 3;

// Recv bandwidth
pub type NDIlib_recv_bandwidth_e = i32;
pub const NDIlib_recv_bandwidth_metadata_only: NDIlib_recv_bandwidth_e = -10;
pub const NDIlib_recv_bandwidth_audio_only: NDIlib_recv_bandwidth_e = 10;
pub const NDIlib_recv_bandwidth_lowest: NDIlib_recv_bandwidth_e = 0;
pub const NDIlib_recv_bandwidth_highest: NDIlib_recv_bandwidth_e = 100;

// Recv color format
pub type NDIlib_recv_color_format_e = i32;
pub const NDIlib_recv_color_format_BGRX_BGRA: NDIlib_recv_color_format_e = 0;
pub const NDIlib_recv_color_format_UYVY_BGRA: NDIlib_recv_color_format_e = 1;
pub const NDIlib_recv_color_format_RGBX_RGBA: NDIlib_recv_color_format_e = 2;
pub const NDIlib_recv_color_format_UYVY_RGBA: NDIlib_recv_color_format_e = 3;
pub const NDIlib_recv_color_format_fastest: NDIlib_recv_color_format_e = 100;
pub const NDIlib_recv_color_format_best: NDIlib_recv_color_format_e = 101;

// Source descriptor
#[repr(C)]
pub struct NDIlib_source_t {
    pub p_ndi_name: *const c_char,
    pub p_url_address: *const c_char,
}

// Find creation settings
#[repr(C)]
pub struct NDIlib_find_create_t {
    pub show_local_sources: bool,
    pub p_groups: *const c_char,
    pub p_extra_ips: *const c_char,
}

// Recv creation settings
#[repr(C)]
pub struct NDIlib_recv_create_v3_t {
    pub source_to_connect_to: NDIlib_source_t,
    pub color_format: NDIlib_recv_color_format_e,
    pub bandwidth: NDIlib_recv_bandwidth_e,
    pub allow_video_fields: bool,
    pub p_ndi_recv_name: *const c_char,
}

// Video frame
#[repr(C)]
pub struct NDIlib_video_frame_v2_t {
    pub xres: c_int,
    pub yres: c_int,
    pub four_cc: NDIlib_FourCC_video_type_e,
    pub frame_rate_n: c_int,
    pub frame_rate_d: c_int,
    pub picture_aspect_ratio: f32,
    pub frame_format_type: NDIlib_frame_format_type_e,
    pub timecode: i64,
    pub p_data: *mut u8,
    pub line_stride_in_bytes: c_int,
    pub p_metadata: *const c_char,
    pub timestamp: i64,
}

// Audio frame v3
#[repr(C)]
pub struct NDIlib_audio_frame_v3_t {
    pub sample_rate: c_int,
    pub no_channels: c_int,
    pub no_samples: c_int,
    pub timecode: i64,
    pub four_cc: u32,
    pub p_data: *mut u8,
    pub channel_stride_in_bytes: c_int,
    pub p_metadata: *const c_char,
    pub timestamp: i64,
}

// Metadata frame
#[repr(C)]
pub struct NDIlib_metadata_frame_t {
    pub length: c_int,
    pub timecode: i64,
    pub p_data: *mut c_char,
}

unsafe impl Send for NDIlib_video_frame_v2_t {}

impl Default for NDIlib_video_frame_v2_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for NDIlib_audio_frame_v3_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for NDIlib_metadata_frame_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

const DLL_NAME: &str = "Processing.NDI.Lib.x64.dll";

pub struct NdiApi {
    // Hold the library so it stays loaded for the lifetime of this struct.
    _lib: libloading::Library,

    pub initialize: unsafe extern "C" fn() -> bool,
    pub destroy: unsafe extern "C" fn(),
    pub version: unsafe extern "C" fn() -> *const c_char,

    pub find_create_v2:
        unsafe extern "C" fn(*const NDIlib_find_create_t) -> NDIlib_find_instance_t,
    pub find_destroy: unsafe extern "C" fn(NDIlib_find_instance_t),
    pub find_wait_for_sources: unsafe extern "C" fn(NDIlib_find_instance_t, u32) -> bool,
    pub find_get_current_sources:
        unsafe extern "C" fn(NDIlib_find_instance_t, *mut u32) -> *const NDIlib_source_t,

    pub recv_create_v3:
        unsafe extern "C" fn(*const NDIlib_recv_create_v3_t) -> NDIlib_recv_instance_t,
    pub recv_destroy: unsafe extern "C" fn(NDIlib_recv_instance_t),
    pub recv_connect: unsafe extern "C" fn(NDIlib_recv_instance_t, *const NDIlib_source_t),
    pub recv_capture_v3: unsafe extern "C" fn(
        NDIlib_recv_instance_t,
        *mut NDIlib_video_frame_v2_t,
        *mut NDIlib_audio_frame_v3_t,
        *mut NDIlib_metadata_frame_t,
        u32,
    ) -> NDIlib_frame_type_e,
    pub recv_free_video_v2:
        unsafe extern "C" fn(NDIlib_recv_instance_t, *const NDIlib_video_frame_v2_t),
}

// Safety: the NDI SDK documentation states all functions are thread-safe.
unsafe impl Send for NdiApi {}
unsafe impl Sync for NdiApi {}

impl NdiApi {
    /// Try to load the NDI runtime DLL.
    ///
    /// Search order:
    /// 1. System default (exe dir, PATH, etc.)
    /// 2. `%NDI_RUNTIME_DIR_V6%\Processing.NDI.Lib.x64.dll`
    pub fn load() -> Result<Self, libloading::Error> {
        let lib = unsafe { libloading::Library::new(DLL_NAME) }.or_else(|first_err| {
            if let Ok(dir) = std::env::var("NDI_RUNTIME_DIR_V6") {
                let mut path = std::path::PathBuf::from(dir);
                path.push(DLL_NAME);
                unsafe { libloading::Library::new(&path) }
            } else {
                Err(first_err)
            }
        })?;

        unsafe {
            Ok(Self {
                initialize: *lib.get(b"NDIlib_initialize\0")?,
                destroy: *lib.get(b"NDIlib_destroy\0")?,
                version: *lib.get(b"NDIlib_version\0")?,
                find_create_v2: *lib.get(b"NDIlib_find_create_v2\0")?,
                find_destroy: *lib.get(b"NDIlib_find_destroy\0")?,
                find_wait_for_sources: *lib.get(b"NDIlib_find_wait_for_sources\0")?,
                find_get_current_sources: *lib.get(b"NDIlib_find_get_current_sources\0")?,
                recv_create_v3: *lib.get(b"NDIlib_recv_create_v3\0")?,
                recv_destroy: *lib.get(b"NDIlib_recv_destroy\0")?,
                recv_connect: *lib.get(b"NDIlib_recv_connect\0")?,
                recv_capture_v3: *lib.get(b"NDIlib_recv_capture_v3\0")?,
                recv_free_video_v2: *lib.get(b"NDIlib_recv_free_video_v2\0")?,
                _lib: lib,
            })
        }
    }
}
