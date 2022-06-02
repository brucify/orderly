use crate::error::Error;
use futures::{SinkExt, StreamExt};
use log::info;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message;
use url::Url;

pub(crate) type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(crate) async fn connect(s: &str) -> Result<WsStream, Error> {
    let url = Url::parse(s).unwrap();
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(url).await?;
    info!("Successfully connected to {}", s);
    Ok(ws_stream)
}

pub(crate) async fn close(ws_stream: &mut WsStream) {
    let _ = ws_stream.send(Message::Close(None)).await;
    let close = ws_stream.next().await;
    info!("server close msg: {:?}", close);
    assert!(ws_stream.next().await.is_none());
    let _ = ws_stream.close(None).await;
}