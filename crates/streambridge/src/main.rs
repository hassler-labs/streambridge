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
    about = "Bridge NDI\u{00ae} streams to MJPEG over HTTP",
    after_help = "NDI\u{00ae} is a registered trademark of Vizrt NDI AB.\nhttps://ndi.video"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// HTTP/WS listen port
    #[arg(long, default_value_t = 9550, global = true)]
    port: u16,

    /// Max frames per second
    #[arg(long, default_value_t = 25, global = true)]
    max_fps: u32,

    /// TurboJPEG quality (1-100)
    #[arg(long, default_value_t = 75, global = true)]
    jpeg_quality: i32,

    /// Stats log interval in seconds
    #[arg(long, default_value_t = 20, global = true)]
    log_interval: u64,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover and list available NDI sources on the network
    List,
    /// Start MJPEG server — streams are created on-demand
    Serve,
}

fn print_banner(port: u16) {
    eprintln!();
    eprintln!("  StreamBridge v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("  Powered by NDI\u{00ae} \u{2014} https://ndi.video");
    eprintln!("  NDI\u{00ae} is a registered trademark of Vizrt NDI AB.");
    eprintln!();
    eprintln!("  Server: http://localhost:{}", port);
    eprintln!("  Close this window to stop.");
    eprintln!();
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List) => cmd_list(),
        Some(Commands::Serve) | None => {
            cmd_serve(cli.port, cli.max_fps, cli.jpeg_quality, cli.log_interval)
        }
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
    print_banner(port);

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
            let manager = receiver_manager.clone();
            let interval_secs = log_interval as f64;
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(log_interval));
                loop {
                    tick.tick().await;
                    for (name, stats) in manager.active_stats() {
                        let snap = stats.snapshot_and_reset(interval_secs);
                        if snap.clients > 0 || snap.fps_out > 0.0 {
                            info!("[{}] {}", name, snap);
                        }
                    }
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
