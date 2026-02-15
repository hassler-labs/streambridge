use bytes::Bytes;
use crate::encode::{self, EncodeBuffers};
use crate::stats::SourceStats;
use crate::ndi::{FourCCVideoType, FrameType, NdiInstance, RecvBandwidth, RecvColorFormat, Source};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// A JPEG frame ready to send over WebSocket.
#[derive(Clone)]
pub struct JpegFrame {
    pub data: Bytes,
}

/// A shared receiver for a single NDI source. Broadcasts JPEG frames to subscribers.
pub struct SharedReceiver {
    pub source_name: String,
    pub stats: Arc<SourceStats>,
    tx: broadcast::Sender<JpegFrame>,
    /// Signals the capture thread to stop.
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl SharedReceiver {
    pub fn subscribe(&self) -> broadcast::Receiver<JpegFrame> {
        self.stats.clients.fetch_add(1, Ordering::Relaxed);
        self.tx.subscribe()
    }

    pub fn unsubscribe(&self) {
        self.stats.clients.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn client_count(&self) -> u64 {
        self.stats.clients.load(Ordering::Relaxed)
    }
}

impl Drop for SharedReceiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        debug!("SharedReceiver dropped for {}", self.source_name);
    }
}

/// Manages shared NDI receivers. Creates on first subscriber, destroys on last unsubscribe.
pub struct ReceiverManager {
    receivers: Mutex<HashMap<String, Arc<SharedReceiver>>>,
    ndi: Arc<NdiInstance>,
    jpeg_quality: i32,
    max_fps: u32,
}

impl ReceiverManager {
    pub fn new(ndi: Arc<NdiInstance>, jpeg_quality: i32, max_fps: u32) -> Arc<Self> {
        Arc::new(Self {
            receivers: Mutex::new(HashMap::new()),
            ndi,
            jpeg_quality,
            max_fps,
        })
    }

    /// Get or create a shared receiver for the given source.
    /// Returns the SharedReceiver or an error if the source can't be connected.
    pub fn get_or_create(
        self: &Arc<Self>,
        source: &Source,
    ) -> Result<Arc<SharedReceiver>, String> {
        let mut receivers = self.receivers.lock().unwrap();

        if let Some(existing) = receivers.get(&source.name) {
            return Ok(existing.clone());
        }

        let recv = self
            .ndi
            .create_receive_instance(RecvBandwidth::Highest, RecvColorFormat::Fastest)
            .map_err(|e| format!("failed to create receiver: {e}"))?;

        recv.connect(source);

        let (tx, _) = broadcast::channel::<JpegFrame>(4);
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stats = SourceStats::new();

        let shared = Arc::new(SharedReceiver {
            source_name: source.name.clone(),
            stats: stats.clone(),
            tx: tx.clone(),
            stop: stop.clone(),
        });

        let source_name = source.name.clone();
        let quality = self.jpeg_quality;
        let max_fps = self.max_fps;
        let manager = Arc::clone(self);
        let source_name_thread = source_name.clone();

        std::thread::Builder::new()
            .name(format!("ndi-recv-{}", &source_name))
            .spawn(move || {
                info!("capture thread started for \"{}\"", source_name_thread);
                let mut buffers = EncodeBuffers::new();
                let mut video_frame = crate::ndi::ffi::NDIlib_video_frame_v2_t::default();
                let min_frame_interval_ms = if max_fps > 0 { 1000 / max_fps as u64 } else { 0 };
                let mut last_send = Instant::now();

                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }

                    // If no subscribers, check periodically
                    if tx.receiver_count() == 0 && stats.clients.load(Ordering::Relaxed) == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        // Check again and exit if still no clients
                        if tx.receiver_count() == 0 && stats.clients.load(Ordering::Relaxed) == 0 {
                            break;
                        }
                    }

                    let frame_type = recv.capture_video(&mut video_frame, 1000);

                    match frame_type {
                        FrameType::Video => {
                            stats.frames_in.fetch_add(1, Ordering::Relaxed);

                            // FPS cap: skip if too soon
                            let elapsed = last_send.elapsed().as_millis() as u64;
                            if elapsed < min_frame_interval_ms {
                                stats.dropped.fetch_add(1, Ordering::Relaxed);
                                recv.free_video(&video_frame);
                                continue;
                            }

                            let w = video_frame.xres as usize;
                            let h = video_frame.yres as usize;
                            let fourcc = FourCCVideoType::from(video_frame.four_cc);
                            let stride = if video_frame.line_stride_in_bytes > 0 {
                                video_frame.line_stride_in_bytes as usize
                            } else {
                                match fourcc {
                                    FourCCVideoType::UYVY | FourCCVideoType::UYVA => w * 2,
                                    _ => w * 4,
                                }
                            };

                            if let Some(data) = recv.video_data(&video_frame) {
                                let encode_start = Instant::now();
                                match encode::encode_frame(data, w, h, stride, fourcc, quality, &mut buffers) {
                                    Ok(jpeg) => {
                                        let encode_us = encode_start.elapsed().as_micros() as u64;
                                        stats.encode_time_us.fetch_add(encode_us, Ordering::Relaxed);
                                        stats.encode_count.fetch_add(1, Ordering::Relaxed);
                                        stats.bytes_out.fetch_add(jpeg.len() as u64, Ordering::Relaxed);
                                        stats.frames_out.fetch_add(1, Ordering::Relaxed);
                                        last_send = Instant::now();

                                        let _ = tx.send(JpegFrame {
                                            data: Bytes::from(jpeg),
                                        });
                                    }
                                    Err(e) => {
                                        error!("encode error for \"{}\": {}", source_name_thread, e);
                                        stats.dropped.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }

                            recv.free_video(&video_frame);
                        }
                        FrameType::Error => {
                            warn!("NDI connection error for \"{}\"", source_name_thread);
                            break;
                        }
                        FrameType::None => {
                            // Timeout, no data — loop
                        }
                        _ => {
                            // Audio, metadata, status change — ignore
                        }
                    }
                }

                info!("capture thread stopped for \"{}\"", source_name_thread);
                // Clean up from manager
                let mut receivers = manager.receivers.lock().unwrap();
                receivers.remove(&source_name_thread);
            })
            .map_err(|e| format!("failed to spawn capture thread: {e}"))?;

        receivers.insert(source_name, shared.clone());
        Ok(shared)
    }

    /// Returns (source_name, stats) for all active receivers.
    pub fn active_stats(&self) -> Vec<(String, Arc<crate::stats::SourceStats>)> {
        let receivers = self.receivers.lock().unwrap();
        receivers
            .iter()
            .map(|(name, r)| (name.clone(), r.stats.clone()))
            .collect()
    }

    /// Remove a receiver if it has no more clients.
    pub fn maybe_remove(&self, source_name: &str) {
        let mut receivers = self.receivers.lock().unwrap();
        if let Some(recv) = receivers.get(source_name) {
            if recv.client_count() == 0 {
                receivers.remove(source_name);
                // The SharedReceiver drop will signal the thread to stop
            }
        }
    }
}
