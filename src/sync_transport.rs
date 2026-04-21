use std::sync::mpsc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;

pub enum SyncCommand {
    SendMessage(Vec<u8>),
    Shutdown,
}

pub enum SyncEvent {
    MessageReceived(Vec<u8>),
    Connected,
    Disconnected,
}

pub struct SyncHandle {
    pub command_tx: tokio_mpsc::UnboundedSender<SyncCommand>,
}

pub fn start_sync_thread(
    server_url: String,
    token: String,
    context_id: String,
    event_tx: mpsc::Sender<SyncEvent>,
) -> SyncHandle {
    let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        rt.block_on(sync_loop(
            server_url, token, context_id, command_rx, event_tx,
        ));
    });

    SyncHandle { command_tx }
}

fn to_ws_url(server_url: &str) -> String {
    if let Some(rest) = server_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = server_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        server_url.to_string()
    }
}

async fn sync_loop(
    server_url: String,
    token: String,
    context_id: String,
    mut command_rx: tokio_mpsc::UnboundedReceiver<SyncCommand>,
    event_tx: mpsc::Sender<SyncEvent>,
) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(60);

    let ws_base = to_ws_url(&server_url);

    loop {
        if let Ok(SyncCommand::Shutdown) = command_rx.try_recv() {
            return;
        }

        let ws_url = format!("{}/sync/{}", ws_base, context_id);
        let request = match ws_url.as_str().into_client_request() {
            Ok(mut req) => {
                req.headers_mut().insert(
                    "Authorization",
                    format!("Bearer {}", token).parse().unwrap(),
                );
                req
            }
            Err(_) => {
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
                continue;
            }
        };

        match tokio_tungstenite::connect_async(request).await {
            Ok((ws_stream, _)) => {
                backoff = Duration::from_secs(1);
                let _ = event_tx.send(SyncEvent::Connected);

                let shutdown = run_session(ws_stream, &mut command_rx, &event_tx).await;

                let _ = event_tx.send(SyncEvent::Disconnected);

                if shutdown {
                    return;
                }
            }
            Err(_) => {
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

async fn run_session<S>(
    ws_stream: tokio_tungstenite::WebSocketStream<S>,
    command_rx: &mut tokio_mpsc::UnboundedReceiver<SyncCommand>,
    event_tx: &mpsc::Sender<SyncEvent>,
) -> bool
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut write, mut read) = ws_stream.split();

    loop {
        tokio::select! {
            cmd = command_rx.recv() => {
                match cmd {
                    Some(SyncCommand::SendMessage(bytes)) => {
                        if write.send(Message::Binary(bytes.into())).await.is_err() {
                            return false;
                        }
                    }
                    Some(SyncCommand::Shutdown) => {
                        let _ = write.send(Message::Close(None)).await;
                        return true;
                    }
                    None => return true,
                }
            }
            frame = read.next() => {
                match frame {
                    Some(Ok(Message::Binary(bytes))) => {
                        let _ = event_tx.send(SyncEvent::MessageReceived(bytes.to_vec()));
                    }
                    Some(Ok(Message::Close(_))) => return false,
                    Some(Err(_)) => return false,
                    None => return false,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

#[allow(dead_code)]
fn url_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_host() {
        assert_eq!(
            url_host("wss://sync.todui.com/sync/default"),
            "sync.todui.com"
        );
        assert_eq!(
            url_host("ws://localhost:8787/sync/default"),
            "localhost:8787"
        );
    }
}
