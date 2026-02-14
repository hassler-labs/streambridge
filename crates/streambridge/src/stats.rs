use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Per-source statistics counters.
pub struct SourceStats {
    pub frames_in: AtomicU64,
    pub frames_out: AtomicU64,
    pub encode_time_us: AtomicU64,
    pub encode_count: AtomicU64,
    pub bytes_out: AtomicU64,
    pub dropped: AtomicU64,
    pub clients: AtomicU64,
}

impl SourceStats {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            frames_in: AtomicU64::new(0),
            frames_out: AtomicU64::new(0),
            encode_time_us: AtomicU64::new(0),
            encode_count: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            clients: AtomicU64::new(0),
        })
    }

    /// Snapshot and reset counters. Returns (frames_in, frames_out, avg_encode_ms, kb_per_sec, dropped, clients).
    pub fn snapshot_and_reset(&self, interval_secs: f64) -> StatsSnapshot {
        let fi = self.frames_in.swap(0, Ordering::Relaxed);
        let fo = self.frames_out.swap(0, Ordering::Relaxed);
        let et = self.encode_time_us.swap(0, Ordering::Relaxed);
        let ec = self.encode_count.swap(0, Ordering::Relaxed);
        let bo = self.bytes_out.swap(0, Ordering::Relaxed);
        let dr = self.dropped.swap(0, Ordering::Relaxed);
        let cl = self.clients.load(Ordering::Relaxed);

        let fps_in = fi as f64 / interval_secs;
        let fps_out = fo as f64 / interval_secs;
        let avg_encode_ms = if ec > 0 {
            (et as f64 / ec as f64) / 1000.0
        } else {
            0.0
        };
        let kb_per_sec = (bo as f64 / 1024.0) / interval_secs;

        StatsSnapshot {
            clients: cl,
            fps_in,
            fps_out,
            avg_encode_ms,
            kb_per_sec,
            dropped: dr,
        }
    }
}

pub struct StatsSnapshot {
    pub clients: u64,
    pub fps_in: f64,
    pub fps_out: f64,
    pub avg_encode_ms: f64,
    pub kb_per_sec: f64,
    pub dropped: u64,
}

impl std::fmt::Display for StatsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} clients, {:.1} fps out, {:.1} fps in, {:.1} ms encode avg, {:.0} KB/s, {} dropped",
            self.clients, self.fps_out, self.fps_in, self.avg_encode_ms, self.kb_per_sec, self.dropped,
        )
    }
}
