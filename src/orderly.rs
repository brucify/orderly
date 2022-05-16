use crate::error::Error;
use crate::grpc::OrderBookService;
use crate::orderbook::{Exchanges, OutTick};
use crate::{bitstamp, stdin, orderbook, binance, websocket};
use futures::{SinkExt, StreamExt};
use futures_executor::ThreadPool;
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::{RwLock, watch};
use tungstenite::protocol::Message;

pub async fn run(symbol: &String) -> Result<(), Error> {
    let connector = Connector::new();
    let service = OrderBookService::new(connector.out_ticks.clone());

    tokio::spawn(async move {
        service.serve().await.expect("Failed to serve grpc");
    });

    connector.run(symbol).await?;

    Ok(())
}

pub(crate) type OutTickPair = (watch::Sender<OutTick>, watch::Receiver<OutTick>);

struct Connector {
    out_ticks: Arc<RwLock<OutTickPair>>,
}

impl Connector {
    fn new() -> Connector {
        let out_ticks = Arc::new(RwLock::new(watch::channel(OutTick::new())));
        Connector { out_ticks }
    }

    async fn run(&self, symbol: &String) -> Result<(), Error> {
        let pool = ThreadPool::new()?;

        let mut ws_bitstamp = bitstamp::connect(symbol).await?;
        let mut ws_binance = binance::connect(symbol).await?;
        let mut rx_stdin = stdin::rx();
        let (tx_in_ticks, mut rx_in_ticks) = orderbook::channel();

        let mut exchanges = Exchanges::new();

        // handle websocket messages
        loop {
            tokio::select! {
                ws_msg = ws_bitstamp.next() => {
                    match bitstamp::parse(ws_msg) {
                        Ok(t) => {
                            let tx = tx_in_ticks.clone();
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
                            let tx = tx_in_ticks.clone();
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
                in_tick = rx_in_ticks.next() => {
                    match in_tick {
                        Some(t) => {
                            debug!("{:?}", t);
                            exchanges.update(t);

                            let out_tick = exchanges.to_tick();
                            info!("{:?}", out_tick);

                            let writer = self.out_ticks.write().await;
                            let tx = &writer.0;

                            tx.send(out_tick).expect("channel should not be closed");
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
}