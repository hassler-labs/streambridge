mod discovery;
mod encode;
mod receiver;
mod server;
mod stats;
mod test_page;

use clap::{Parser, Subcommand};
use receiver::ReceiverManager;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Parser)]
#[command(
    name = "streambridge",
    about = "Bridge NDI streams to MJPEG over WebSocket",
    after_help = "NDI\u{00ae} is a registered trademark of Vizrt Group."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover and list available NDI sources on the network
    List,
    /// Start MJPEG server — streams are created on-demand
    Serve {
        /// HTTP/WS listen port
        #[arg(long, default_value_t = 9550)]
        port: u16,

        /// Max frames per second
        #[arg(long, default_value_t = 25)]
        max_fps: u32,

        /// TurboJPEG quality (1-100)
        #[arg(long, default_value_t = 75)]
        jpeg_quality: i32,

        /// Stats log interval in seconds
        #[arg(long, default_value_t = 20)]
        log_interval: u64,
    },
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::List => cmd_list(),
        Commands::Serve {
            port,
            max_fps,
            jpeg_quality,
            log_interval,
        } => cmd_serve(port, max_fps, jpeg_quality, log_interval),
    }
}

fn cmd_list() {
    let ndi = match ndi_sdk::load() {
        Ok(n) => n,
        Err(ndi_sdk::NdiError::DllNotFound(_)) => {
            eprintln!("Error: NDI runtime not found.\n");
            eprintln!("Download and install it from: https://ndi.video/tools/");
            std::process::exit(1);
        }
        Err(e) => {
            error!("Failed to initialize NDI: {}", e);
            std::process::exit(1);
        }
    };

    info!("NDI version: {}", ndi.version());
    let finder = ndi.create_find_instance().expect("failed to create finder");

    println!("Searching for NDI sources...");
    finder.wait_for_sources(5000);
    let sources = finder.get_current_sources();

    if sources.is_empty() {
        println!("No NDI sources found.");
    } else {
        println!("Found {} source(s):", sources.len());
        for s in &sources {
            println!(
                "  {}{}",
                s.name,
                s.url.as_deref().map_or(String::new(), |u| format!(" ({})", u))
            );
        }
    }
}

fn cmd_serve(port: u16, max_fps: u32, jpeg_quality: i32, log_interval: u64) {
    let ndi = match ndi_sdk::load() {
        Ok(n) => n,
        Err(ndi_sdk::NdiError::DllNotFound(_)) => {
            eprintln!("Error: NDI runtime not found.\n");
            eprintln!("Download and install it from: https://ndi.video/tools/");
            std::process::exit(1);
        }
        Err(e) => {
            error!("Failed to initialize NDI: {}", e);
            std::process::exit(1);
        }
    };

    info!("NDI version: {}", ndi.version());

    let ndi = Arc::new(ndi);
    let finder = ndi.create_find_instance().expect("failed to create finder");
    let sources = discovery::start_discovery(finder);
    let receiver_manager = ReceiverManager::new(Arc::clone(&ndi), jpeg_quality, max_fps);

    let state = server::AppState {
        sources: sources.clone(),
        receiver_manager: receiver_manager.clone(),
    };

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        // Stats logging task
        if log_interval > 0 {
            let _manager = receiver_manager.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(log_interval));
                loop {
                    interval.tick().await;
                    // Stats are logged per-source from the receiver manager
                    // For now, a placeholder that could iterate active receivers
                }
            });
        }

        let router = server::create_router(state);
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        info!("streambridge server listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("failed to bind");
        axum::serve(listener, router)
            .await
            .expect("server error");
    });
}
