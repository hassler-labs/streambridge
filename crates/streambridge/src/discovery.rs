use crate::ndi::{FindInstance, Source};
use std::sync::{Arc, RwLock};
use std::thread;
use tracing::{debug, info};

pub type SourceList = Arc<RwLock<Vec<Source>>>;

/// Spawn a background thread that continuously discovers NDI sources.
/// Returns a shared source list that is updated whenever sources change.
pub fn start_discovery(find: FindInstance) -> SourceList {
    let sources: SourceList = Arc::new(RwLock::new(Vec::new()));
    let sources_clone = sources.clone();

    thread::Builder::new()
        .name("ndi-discovery".into())
        .spawn(move || {
            info!("NDI discovery thread started");
            loop {
                if find.wait_for_sources(2000) {
                    let current = find.get_current_sources();
                    debug!("discovered {} NDI source(s)", current.len());
                    let mut list = sources_clone.write().unwrap();
                    *list = current;
                }
            }
        })
        .expect("failed to spawn discovery thread");

    sources
}
