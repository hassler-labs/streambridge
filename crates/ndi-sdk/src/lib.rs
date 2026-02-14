#[allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]
pub mod ffi;
pub mod types;

use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Arc;

pub use types::*;

#[derive(Debug, thiserror::Error)]
pub enum NdiError {
    #[error("{0}")]
    DllNotFound(String),
    #[error("NDI initialization failed")]
    InitFailed,
    #[error("failed to create find instance")]
    FindCreateFailed,
    #[error("failed to create receive instance")]
    RecvCreateFailed,
}

/// Top-level NDI library handle. Calls `NDIlib_destroy` on drop.
pub struct NdiInstance {
    api: Arc<ffi::NdiApi>,
}

impl NdiInstance {
    pub fn create_find_instance(&self) -> Result<FindInstance, NdiError> {
        let settings = ffi::NDIlib_find_create_t {
            show_local_sources: true,
            p_groups: ptr::null(),
            p_extra_ips: ptr::null(),
        };
        let handle = unsafe { (self.api.find_create_v2)(&settings) };
        if handle.is_null() {
            return Err(NdiError::FindCreateFailed);
        }
        Ok(FindInstance {
            handle,
            api: Arc::clone(&self.api),
        })
    }

    pub fn create_receive_instance(
        &self,
        bandwidth: RecvBandwidth,
        color_format: RecvColorFormat,
    ) -> Result<ReceiveInstance, NdiError> {
        let settings = ffi::NDIlib_recv_create_v3_t {
            source_to_connect_to: ffi::NDIlib_source_t {
                p_ndi_name: ptr::null(),
                p_url_address: ptr::null(),
            },
            color_format: color_format.to_raw(),
            bandwidth: bandwidth.to_raw(),
            allow_video_fields: true,
            p_ndi_recv_name: ptr::null(),
        };
        let handle = unsafe { (self.api.recv_create_v3)(&settings) };
        if handle.is_null() {
            return Err(NdiError::RecvCreateFailed);
        }
        Ok(ReceiveInstance {
            handle,
            api: Arc::clone(&self.api),
        })
    }

    pub fn version(&self) -> &str {
        unsafe {
            let ptr = (self.api.version)();
            if ptr.is_null() {
                "unknown"
            } else {
                CStr::from_ptr(ptr).to_str().unwrap_or("unknown")
            }
        }
    }
}

impl Drop for NdiInstance {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)() }
    }
}

/// Initialize the NDI library. Returns an `NdiInstance` that must stay alive
/// for the duration of NDI usage.
pub fn load() -> Result<NdiInstance, NdiError> {
    let api = ffi::NdiApi::load().map_err(|e| {
        NdiError::DllNotFound(format!("failed to load NDI runtime DLL: {e}"))
    })?;
    let ok = unsafe { (api.initialize)() };
    if !ok {
        return Err(NdiError::InitFailed);
    }
    Ok(NdiInstance {
        api: Arc::new(api),
    })
}

/// NDI source finder. Discovers sources on the network.
pub struct FindInstance {
    handle: ffi::NDIlib_find_instance_t,
    api: Arc<ffi::NdiApi>,
}

// The NDI SDK states that find instances can be used from any thread.
unsafe impl Send for FindInstance {}
unsafe impl Sync for FindInstance {}

impl FindInstance {
    /// Block up to `timeout_ms` waiting for the source list to change.
    /// Returns `true` if sources changed.
    pub fn wait_for_sources(&self, timeout_ms: u32) -> bool {
        unsafe { (self.api.find_wait_for_sources)(self.handle, timeout_ms) }
    }

    /// Get the current snapshot of discovered sources.
    pub fn get_current_sources(&self) -> Vec<Source> {
        let mut count: u32 = 0;
        let ptr = unsafe { (self.api.find_get_current_sources)(self.handle, &mut count) };
        if ptr.is_null() || count == 0 {
            return Vec::new();
        }
        let sources = unsafe { std::slice::from_raw_parts(ptr, count as usize) };
        sources
            .iter()
            .map(|s| {
                let name = if s.p_ndi_name.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(s.p_ndi_name) }
                        .to_string_lossy()
                        .into_owned()
                };
                let url = if s.p_url_address.is_null() {
                    None
                } else {
                    let u = unsafe { CStr::from_ptr(s.p_url_address) }
                        .to_string_lossy()
                        .into_owned();
                    if u.is_empty() { None } else { Some(u) }
                };
                Source { name, url }
            })
            .collect()
    }
}

impl Drop for FindInstance {
    fn drop(&mut self) {
        unsafe { (self.api.find_destroy)(self.handle) }
    }
}

/// NDI receiver. Receives frames from a connected source.
pub struct ReceiveInstance {
    handle: ffi::NDIlib_recv_instance_t,
    api: Arc<ffi::NdiApi>,
}

unsafe impl Send for ReceiveInstance {}
unsafe impl Sync for ReceiveInstance {}

impl ReceiveInstance {
    /// Connect to a source. Pass the source's name and optional URL.
    pub fn connect(&self, source: &Source) {
        let name_c = CString::new(source.name.as_str()).unwrap();
        let url_c = source.url.as_ref().map(|u| CString::new(u.as_str()).unwrap());
        let ndi_src = ffi::NDIlib_source_t {
            p_ndi_name: name_c.as_ptr(),
            p_url_address: url_c.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
        };
        unsafe { (self.api.recv_connect)(self.handle, &ndi_src) }
    }

    /// Disconnect from the current source.
    pub fn disconnect(&self) {
        unsafe { (self.api.recv_connect)(self.handle, ptr::null()) }
    }

    /// Attempt to capture a video frame. Returns the frame type and fills `video_frame`.
    /// The caller must call `free_video` when done with the frame data.
    pub fn capture_video(&self, video_frame: &mut ffi::NDIlib_video_frame_v2_t, timeout_ms: u32) -> FrameType {
        let frame_type = unsafe {
            (self.api.recv_capture_v3)(
                self.handle,
                video_frame,
                ptr::null_mut(),
                ptr::null_mut(),
                timeout_ms,
            )
        };
        FrameType::from(frame_type)
    }

    /// Free a video frame previously captured.
    pub fn free_video(&self, video_frame: &ffi::NDIlib_video_frame_v2_t) {
        unsafe { (self.api.recv_free_video_v2)(self.handle, video_frame) }
    }

    /// Get the raw video data as a byte slice from a captured frame.
    /// Returns `None` if `p_data` is null.
    pub fn video_data<'a>(&self, frame: &'a ffi::NDIlib_video_frame_v2_t) -> Option<&'a [u8]> {
        if frame.p_data.is_null() {
            return None;
        }
        let fourcc = FourCCVideoType::from(frame.four_cc);
        let w = frame.xres as usize;
        let h = frame.yres as usize;
        let stride = frame.line_stride_in_bytes as usize;
        let stride = if stride == 0 {
            match fourcc {
                FourCCVideoType::UYVY | FourCCVideoType::UYVA => w * 2,
                FourCCVideoType::BGRA | FourCCVideoType::BGRX |
                FourCCVideoType::RGBA | FourCCVideoType::RGBX => w * 4,
                _ => w * 2,
            }
        } else {
            stride
        };

        let len = match fourcc {
            FourCCVideoType::UYVY => stride * h,
            FourCCVideoType::UYVA => stride * h + w * h,
            FourCCVideoType::I420 | FourCCVideoType::YV12 | FourCCVideoType::NV12 => {
                stride * h * 3 / 2
            }
            FourCCVideoType::BGRA | FourCCVideoType::BGRX |
            FourCCVideoType::RGBA | FourCCVideoType::RGBX => stride * h,
            FourCCVideoType::Unknown(_) => stride * h,
        };
        Some(unsafe { std::slice::from_raw_parts(frame.p_data, len) })
    }
}

impl Drop for ReceiveInstance {
    fn drop(&mut self) {
        unsafe { (self.api.recv_destroy)(self.handle) }
    }
}
