use crate::orderbook::MergedOrderBook;
use crate::{bitstamp, stdin, orderbook};
use futures::{SinkExt, StreamExt};
use futures_executor::ThreadPool;
use log::{debug, info};
use tungstenite::protocol::Message;

pub async fn run(pair: &String) {
    let mut ws_stream = bitstamp::ws_stream().await;
    let mut rx_stdin = stdin::rx();
    let pool = ThreadPool::new().expect("Failed to build pool");
    let (tx_ticks, mut rx_ticks) = orderbook::channel();
    let mut book = MergedOrderBook::new();
    bitstamp::subscribe(&mut ws_stream, pair).await.expect("Failed to subscribe to Bitstamp");

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
                        info!("Err: {:?}", e);
                        break
                    }
                }
            },
            stdin_msg = rx_stdin.recv() => {
                match stdin_msg {
                    Some(msg) => {
                        info!("Sent: {:?}", msg);
                        let _ = ws_stream.send(Message::Text(msg)).await;
                    },
                    None => break
                }
            },
            tick = rx_ticks.next() => {
                match tick {
                    Some(t) => {
                        book.add(t);
                        let merged = book.merged();
                        debug!("{:?}", merged);
                    },
                    _ => {}
                }
            }
        };
    }
    // Gracefully close connection by Close-handshake procedure
    bitstamp::close(&mut ws_stream).await;
}
