use crate::error::Error;
use crate::grpc::OrderBookService;
use crate::orderbook::{Exchanges, OutTick};
use crate::{bitstamp, stdin, orderbook, binance, websocket};
use futures::{SinkExt, StreamExt};
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::{RwLock, watch};
use tungstenite::protocol::Message;

pub async fn run(symbol: &String, port: usize) -> Result<(), Error> {
    let connector = Connector::new();
    let service = OrderBookService::new(connector.out_ticks.clone());

    tokio::spawn(async move {
        service.serve(port).await.expect("Failed to serve grpc");
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
        let mut ws_bitstamp = bitstamp::connect(symbol).await?;
        let mut ws_binance = binance::connect(symbol).await?;
        let mut rx_stdin = stdin::rx();
        let (tx_in_ticks, mut rx_in_ticks) = orderbook::channel();

        let mut exchanges = Exchanges::new();

        // handle websocket messages
        loop {
            tokio::select! {
                ws_msg = ws_bitstamp.next() => {
                    let tx = tx_in_ticks.clone();

                    let res = handle(ws_msg).and_then(|msg| {
                        bitstamp::parse_and_send(msg, tx)
                    });

                    if let Err(e) = res {
                        info!("Err: {:?}", e);
                        break
                    }
                },
                ws_msg = ws_binance.next() => {
                    let tx = tx_in_ticks.clone();

                    let res = handle(ws_msg).and_then(|msg| {
                        binance::parse_and_send(msg, tx)
                    });

                    if let Err(e) = res {
                        info!("Err: {:?}", e);
                        break
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

fn handle(
    ws_msg: Option<Result<Message, tungstenite::Error>>,
) -> Result<Message, Error>
{
    let msg = ws_msg.unwrap_or_else(|| {
        info!("no message");
        Err(tungstenite::Error::ConnectionClosed)
    })?;

    Ok(msg)
}
