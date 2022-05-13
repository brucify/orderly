use crate::tick::MergedOrderBook;
use crate::{bitstamp, stdin, tick};
use futures::{SinkExt, StreamExt};
use futures_executor::ThreadPool;
use tungstenite::protocol::Message;

pub async fn run() {
    let mut ws_stream = bitstamp::ws_stream().await;
    let mut rx_stdin = stdin::rx();
    let pool = ThreadPool::new().expect("Failed to build pool");
    let (tx_ticks, mut rx_ticks) = tick::channel();
    let mut book = MergedOrderBook::new();

    // handle websocket messages
    loop {
        tokio::select! {
            ws_msg = ws_stream.next() => {
                match bitstamp::parse(ws_msg) {
                    Ok(t) => {
                        let tx = tx_ticks.clone();
                        pool.spawn_ok(async move {
                            t.map(|x|tx.unbounded_send(x).expect("Failed to send"));
                        });
                    },
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
            },
            tick = rx_ticks.next() => {
                match tick {
                    Some(t) => {
                        println!("{:?}", t);
                        book.add(t);
                        println!("{:?}", book);
                        let merged = book.merged();
                        println!("{:?}", merged);
                    },
                    _ => {}
                }
            }
        };
    }
    // Gracefully close connection by Close-handshake procedure
    bitstamp::close(&mut ws_stream).await;
}
