use futures::{SinkExt, StreamExt};
use tokio::io::AsyncBufReadExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::Message;
use url::Url;

#[tokio::main]
async fn main() {
    let url = Url::parse("wss://ws.bitstamp.net").unwrap();

    let mut ws_stream = ws_stream(url).await;

    let mut rx_stdin = rx_stdin();

    // handle websocket messages
    loop {
        tokio::select! {
            ws_msg = ws_stream.next() => {
                match ws_msg {
                    Some(msg) => match msg {
                        Ok(msg) => match msg {
                            Message::Binary(x) => println!("binary {:?}", x),
                            Message::Text(x) => println!("{}", x),
                            Message::Ping(x) => println!("Ping {:?}", x),
                            Message::Pong(x) => println!("Pong {:?}", x),
                            Message::Close(x) => println!("Close {:?}", x),
                            Message::Frame(x) => println!("Frame {:?}", x),
                        },
                        Err(_) => {println!("server went away"); break;}
                    },
                    None => {println!("no message"); break;},
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
    let _ = ws_stream.send(Message::Close(None)).await;
    let close = ws_stream.next().await;
    println!("server close msg: {:?}", close);
    assert!(ws_stream.next().await.is_none());
    let _ = ws_stream.close(None).await;
}

async fn ws_stream(url: Url) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    ws_stream
}

fn rx_stdin() -> Receiver<String> {
    let (tx_stdin, rx_stdin) = mpsc::channel::<String>(10);
    // read from stdin
    let stdin_loop = async move {
        loop {
            let mut buf_stdin = tokio::io::BufReader::new(tokio::io::stdin());
            let mut line = String::new();
            buf_stdin.read_line(&mut line).await.unwrap();
            tx_stdin.send(line.trim().to_string()).await.unwrap();
            if line.trim() == "/exit" {
                break;
            }
        }
    };
    tokio::task::spawn(stdin_loop);
    rx_stdin
}
