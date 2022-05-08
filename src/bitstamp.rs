use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Error;
use tungstenite::protocol::Message;
use url::Url;

pub(crate) async fn ws_stream() -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let url = Url::parse("wss://ws.bitstamp.net").unwrap();
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    ws_stream
}

pub(crate) fn parse(ws_msg: Option<Result<Message, Error>>) -> Result<(), Error> {
    let msg = ws_msg.unwrap_or_else(|| {println!("no message"); Err(tungstenite::Error::ConnectionClosed)})?;
    match msg {
        Message::Binary(x) => println!("binary {:?}", x),
        Message::Text(x) => println!("{}", x),
        Message::Ping(x) => println!("Ping {:?}", x),
        Message::Pong(x) => println!("Pong {:?}", x),
        Message::Close(x) => println!("Close {:?}", x),
        Message::Frame(x) => println!("Frame {:?}", x),
    }
    Ok(())
}

pub(crate) async fn close(ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) {
    let _ = ws_stream.send(Message::Close(None)).await;
    let close = ws_stream.next().await;
    println!("server close msg: {:?}", close);
    assert!(ws_stream.next().await.is_none());
    let _ = ws_stream.close(None).await;
}
