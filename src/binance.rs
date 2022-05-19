use crate::error::Error;
use crate::orderbook::{self, Exchange, InTick, ToLevel, ToTick};
use crate::websocket;
use log::{debug, info};
use rust_decimal::Decimal;
use serde::Deserialize;
use tungstenite::Message;

const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";

#[derive(Debug, Deserialize, PartialEq)]
struct Event {
    #[serde(rename = "lastUpdateId")]
    last_update_id: usize,
    bids: Vec<Level>,
    asks: Vec<Level>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Level {
    price: Decimal,
    amount: Decimal,
}

impl ToLevel for Level {
    /// Converts a `binance::Level` into a `orderbook::Level`.
    fn to_level(&self) -> orderbook::Level {
        orderbook::Level::new(self.price, self.amount, Exchange::Binance)
    }
}

fn to_levels(levels: &Vec<Level>, depth: usize) -> Vec<orderbook::Level> {
    let levels = match levels.len() > depth {
        true => levels.split_at(depth).0.to_vec(), // only keep 10
        false => levels.clone(),
    };

    levels.into_iter()
        .map(|l| l.to_level())
        .collect()
}

impl ToTick for Event {
    /// Converts the `Event` into a `Option<InTick>`. Only keep the top ten levels of bids and asks.
    fn maybe_to_tick(&self) -> Option<InTick> {
        let depth = 10;
        let bids = to_levels(&self.bids, depth);
        let asks = to_levels(&self.asks, depth);

        Some(InTick { exchange: Exchange::Binance, bids, asks })
    }
}

pub(crate) async fn connect(symbol: &String) -> Result<websocket::WsStream, Error> {
    let depth = 10;
    let url = format!("{}/{}@depth{}@100ms", BINANCE_WS_URL, symbol.to_lowercase(), depth);
    Ok(websocket::connect(url.as_str()).await?)
}

pub(crate) fn parse(msg: Message) -> Result<Option<InTick>, Error> {
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
                       bids: vec![
                           Level { price: dec!(0.06900300), amount: dec!(14.80480000) },
                           Level { price: dec!(0.06900100), amount: dec!(0.85230000) },
                       ],
                       asks: vec![
                           Level { price: dec!(0.06900400), amount: dec!(12.04200000) },
                           Level { price: dec!(0.06900500), amount: dec!(2.85830000) },
                       ]
                   }
        );
        Ok(())
    }
}