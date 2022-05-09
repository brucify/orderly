use crate::{bitstamp, stdin};
use futures::{SinkExt, StreamExt};
use tungstenite::protocol::Message;

pub async fn run() {
    let mut ws_stream = bitstamp::ws_stream().await;
    let mut rx_stdin = stdin::rx();

    // handle websocket messages
    loop {
        tokio::select! {
            ws_msg = ws_stream.next() => {
                match bitstamp::parse(ws_msg) {
                    Ok(_) => {},
                    Err(e) => {
                        println!("Err: {:?}", e);
                        break
                    }
                }
            },
            stdin_msg = rx_stdin.recv() => {
                match stdin_msg {
                    Some(msg) => {
                        let _ = ws_stream.send(Message::Text(msg)).await;
                    },
                    None => break
                }
            }
        };
    }
    // Gracefully close connection by Close-handshake procedure
    bitstamp::close(&mut ws_stream).await;
}
