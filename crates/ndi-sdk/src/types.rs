use crate::ffi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FourCCVideoType {
    UYVY,
    UYVA,
    I420,
    NV12,
    YV12,
    BGRA,
    BGRX,
    RGBA,
    RGBX,
    Unknown(u32),
}

impl From<ffi::NDIlib_FourCC_video_type_e> for FourCCVideoType {
    fn from(v: ffi::NDIlib_FourCC_video_type_e) -> Self {
        match v {
            ffi::NDIlib_FourCC_video_type_UYVY => Self::UYVY,
            ffi::NDIlib_FourCC_video_type_UYVA => Self::UYVA,
            ffi::NDIlib_FourCC_video_type_I420 => Self::I420,
            ffi::NDIlib_FourCC_video_type_NV12 => Self::NV12,
            ffi::NDIlib_FourCC_video_type_YV12 => Self::YV12,
            ffi::NDIlib_FourCC_video_type_BGRA => Self::BGRA,
            ffi::NDIlib_FourCC_video_type_BGRX => Self::BGRX,
            ffi::NDIlib_FourCC_video_type_RGBA => Self::RGBA,
            ffi::NDIlib_FourCC_video_type_RGBX => Self::RGBX,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvBandwidth {
    MetadataOnly,
    AudioOnly,
    Lowest,
    Highest,
}

impl RecvBandwidth {
    pub fn to_raw(self) -> ffi::NDIlib_recv_bandwidth_e {
        match self {
            Self::MetadataOnly => ffi::NDIlib_recv_bandwidth_metadata_only,
            Self::AudioOnly => ffi::NDIlib_recv_bandwidth_audio_only,
            Self::Lowest => ffi::NDIlib_recv_bandwidth_lowest,
            Self::Highest => ffi::NDIlib_recv_bandwidth_highest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvColorFormat {
    BgrxBgra,
    UyvyBgra,
    RgbxRgba,
    UyvyRgba,
    Fastest,
    Best,
}

impl RecvColorFormat {
    pub fn to_raw(self) -> ffi::NDIlib_recv_color_format_e {
        match self {
            Self::BgrxBgra => ffi::NDIlib_recv_color_format_BGRX_BGRA,
            Self::UyvyBgra => ffi::NDIlib_recv_color_format_UYVY_BGRA,
            Self::RgbxRgba => ffi::NDIlib_recv_color_format_RGBX_RGBA,
            Self::UyvyRgba => ffi::NDIlib_recv_color_format_UYVY_RGBA,
            Self::Fastest => ffi::NDIlib_recv_color_format_fastest,
            Self::Best => ffi::NDIlib_recv_color_format_best,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    None,
    Video,
    Audio,
    Metadata,
    Error,
    StatusChange,
    Unknown(i32),
}

impl From<ffi::NDIlib_frame_type_e> for FrameType {
    fn from(v: ffi::NDIlib_frame_type_e) -> Self {
        match v {
            ffi::NDIlib_frame_type_none => Self::None,
            ffi::NDIlib_frame_type_video => Self::Video,
            ffi::NDIlib_frame_type_audio => Self::Audio,
            ffi::NDIlib_frame_type_metadata => Self::Metadata,
            ffi::NDIlib_frame_type_error => Self::Error,
            ffi::NDIlib_frame_type_status_change => Self::StatusChange,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Source {
    pub name: String,
    pub url: Option<String>,
}
