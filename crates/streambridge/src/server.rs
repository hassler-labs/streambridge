use crate::discovery::SourceList;
use crate::receiver::{JpegFrame, ReceiverManager};
use crate::test_page::TEST_PAGE_HTML;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub sources: SourceList,
    pub receiver_manager: Arc<ReceiverManager>,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new().allow_origin(Any);

    Router::new()
        .route("/sources", get(get_sources))
        .route("/ws", get(ws_handler))
        .route("/", get(test_page))
        .layer(cors)
        .with_state(state)
}

async fn get_sources(State(state): State<AppState>) -> impl IntoResponse {
    let sources = state.sources.read().unwrap();
    let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
    let json = serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string());
    ([(header::CONTENT_TYPE, "application/json")], json)
}

#[derive(Deserialize)]
pub struct WsQuery {
    source: String,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> Response {
    let source_name = query.source;
    ws.on_upgrade(move |socket| handle_ws(socket, source_name, state))
}

async fn send_close(socket: &mut WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.to_string().into(),
        })))
        .await;
}

async fn handle_ws(mut socket: WebSocket, source_name: String, state: AppState) {
    // Find the source in our discovery list
    let source = {
        let sources = state.sources.read().unwrap();
        sources.iter().find(|s| s.name == source_name).cloned()
    };

    let source = match source {
        Some(s) => s,
        None => {
            warn!("WS: source not found: \"{}\"", source_name);
            send_close(&mut socket, 4404, "source not found").await;
            return;
        }
    };

    // Get or create shared receiver
    let shared = match state.receiver_manager.get_or_create(&source) {
        Ok(s) => s,
        Err(e) => {
            warn!("WS: failed to create receiver for \"{}\": {}", source_name, e);
            send_close(&mut socket, 4404, "source not found").await;
            return;
        }
    };

    info!("WS: client connected for \"{}\"", source_name);
    let mut rx = shared.subscribe();

    loop {
        match rx.recv().await {
            Ok(JpegFrame { data }) => {
                if socket
                    .send(Message::Binary(data.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("WS: client lagged {} frames for \"{}\"", n, source_name);
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                warn!("WS: source lost for \"{}\"", source_name);
                send_close(&mut socket, 4410, "source lost").await;
                break;
            }
        }
    }

    shared.unsubscribe();
    state.receiver_manager.maybe_remove(&source_name);
    info!("WS: client disconnected from \"{}\"", source_name);
}

async fn test_page() -> Html<&'static str> {
    Html(TEST_PAGE_HTML)
}
