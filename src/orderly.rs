use crate::error::Error;
use crate::orderbook::MainOrderBook;
use crate::{bitstamp, stdin, orderbook, binance, websocket};
use futures::{SinkExt, StreamExt};
use futures_executor::ThreadPool;
use log::{debug, info};
use tungstenite::protocol::Message;

pub async fn run(symbol: &String) -> Result<(), Error> {
    let mut ws_bitstamp = bitstamp::connect(symbol).await?;
    let mut ws_binance = binance::connect(symbol).await?;
    let mut rx_stdin = stdin::rx();
    let pool = ThreadPool::new()?;
    let (tx_ticks, mut rx_ticks) = orderbook::channel();
    let mut book = MainOrderBook::new();

    // handle websocket messages
    loop {
        tokio::select! {
            ws_msg = ws_bitstamp.next() => {
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
            ws_msg = ws_binance.next() => {
                match binance::parse(ws_msg) {
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
            }
            stdin_msg = rx_stdin.recv() => {
                match stdin_msg {
                    Some(msg) => {
                        info!("Sent to bitstamp: {:?}", msg);
                        let _ = ws_bitstamp.send(Message::Text(msg)).await;
                    },
                    None => break
                }
            },
            tick = rx_ticks.next() => {
                match tick {
                    Some(t) => {
                        debug!("{:?}", t);
                        book.add(t);
                        let merged = book.merged();
                        info!("{:?}", merged);
                    },
                    _ => {}
                }
            }
        };
    }

    // Gracefully close connection by Close-handshake procedure
    websocket::close(&mut ws_bitstamp).await;
    websocket::close(&mut ws_binance).await;
    Ok(())
}
