use crate::error::Error;
use crate::orderbook::{Ask, Bid, Exchange, Tick, ToTick};
use crate::websocket;
use log::{debug, info};
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message;

const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";

#[derive(Debug, Deserialize, PartialEq)]
struct Event {
    #[serde(rename = "lastUpdateId")]
    last_update_id: usize,
    bids: Vec<Bid>,
    asks: Vec<Ask>,
}

impl ToTick for Event {
    /// Converts the `Event` into a `Option<Tick>`. Only keep the top ten levels of bids and asks.
    fn maybe_to_tick(&self) -> Option<Tick> {
        let depth = 10;
        let bids = match self.bids.len() > depth {
            true => self.bids.split_at(depth).0.to_vec(), // only keep 10
            false => self.bids.clone(),
        };
        let asks = match self.asks.len() > depth {
            true => self.asks.split_at(depth).0.to_vec(), // only keep 10
            false => self.asks.clone(),
        };

        Some(Tick {
            exchange: Exchange::Binance,
            bids,
            asks,
        })
    }
}

pub(crate) async fn connect(symbol: &String) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Error> {
    let depth = 10;
    let url = format!("{}/{}@depth{}@100ms", BINANCE_WS_URL, symbol.to_lowercase(), depth);
    Ok(websocket::connect(url.as_str()).await?)
}

pub(crate) fn parse(ws_msg: Option<Result<Message, tungstenite::Error>>) -> Result<Option<Tick>, Error> {
    let msg = ws_msg.unwrap_or_else(|| {
        info!("no message");
        Err(tungstenite::Error::ConnectionClosed)
    })?;
    let e = match msg {
        Message::Binary(x) => { info!("binary {:?}", x); None },
        Message::Text(x) => {
            let e= deserialize(x)?;
            debug!("{:?}", e);
            Some(e)
        },
        Message::Ping(x) => { info!("Ping {:?}", x); None },
        Message::Pong(x) => { info!("Pong {:?}", x); None },
        Message::Close(x) => { info!("Close {:?}", x); None },
        Message::Frame(x) => { info!("Frame {:?}", x); None },
    };
    Ok(e.map(|e| e.maybe_to_tick()).flatten())
}

fn deserialize(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}

#[cfg(test)]
mod test {
    use rust_decimal_macros::dec;
    use crate::binance::*;

    #[test]
    fn should_deserialize_event() -> Result<(), Error> {
        assert_eq!(deserialize("{\
                       \"lastUpdateId\":5244166729,\
                       \"bids\":[[\"0.06900300\",\"14.80480000\"],[\"0.06900100\",\"0.85230000\"]],\
                       \"asks\":[[\"0.06900400\",\"12.04200000\"],[\"0.06900500\",\"2.85830000\"]]\
                   }".to_string())?,
                   Event{
                       last_update_id: 5244166729,
                       bids: vec![Bid::new(dec!(0.06900300), dec!(14.80480000)), Bid::new(dec!(0.06900100), dec!(0.85230000))],
                       asks: vec![Ask::new(dec!(0.06900400), dec!(12.04200000)), Ask::new(dec!(0.06900500), dec!(2.85830000))]
                   }
        );
        Ok(())
    }
}