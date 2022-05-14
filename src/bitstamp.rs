use chrono::{DateTime, Utc};
use crate::error::Error;
use crate::orderbook::{Exchange, Tick, ToTick};
use futures::SinkExt;
use log::{debug, info};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::Message;
use crate::{orderbook, websocket};

const BITSTAMP_WS_URL: &str = "wss://ws.bitstamp.net";

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "event")]
enum Event {
    #[serde(rename = "data")]
    Data{data: InData, channel: Channel},

    #[serde(rename = "bts:subscribe")]
    Subscribe{data: OutSubscription},

    #[serde(rename = "bts:unsubscribe")]
    Unsubscribe{data: OutSubscription},

    #[serde(rename = "bts:subscription_succeeded")]
    SubscriptionSucceeded{data: InSubscription, channel: Channel},

    #[serde(rename = "bts:unsubscription_succeeded")]
    UnsubscriptionSucceeded{data: InSubscription, channel: Channel},

    #[serde(rename = "bts:error")]
    Error{data: InError, channel: Channel},
}

impl ToTick for Event {
    /// Converts the `Event` into a `Option<Tick>`. Only keep the top ten levels of bids and asks.
    fn maybe_to_tick(&self) -> Option<Tick> {
        match self {
            Event::Data { data, .. } => {
                let depth = 10;
                let bids = match data.bids.len() > depth {
                    true => data.bids.split_at(depth).0.to_vec(), // only keep 10
                    false => data.bids.clone(),
                };
                let asks = match data.asks.len() > depth {
                    true => data.asks.split_at(depth).0.to_vec(), // only keep 10
                    false => data.asks.clone(),
                };

                let bids = bids.into_iter()
                    .map(|b| orderbook::Bid::new(b.price, b.amount, Exchange::Bitstamp))
                    .collect();
                let asks = asks.into_iter()
                    .map(|b| orderbook::Ask::new(b.price, b.amount, Exchange::Bitstamp))
                    .collect();

                Some(Tick {
                    exchange: Exchange::Bitstamp,
                    bids,
                    asks,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct OutSubscription {
    channel: Channel,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct InData {
    #[serde(with = "timestamp")]
    timestamp: DateTime<Utc>,

    #[serde(with = "microtimestamp")]
    microtimestamp: DateTime<Utc>,

    bids: Vec<Bid>,
    asks: Vec<Ask>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct InSubscription {}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct InError {
    code: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Bid {
    price: Decimal,
    amount: Decimal
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Ask {
    price: Decimal,
    amount: Decimal
}

type Channel = String;

pub(crate) async fn connect(symbol: &String) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Error> {
    let mut ws_stream = websocket::connect(BITSTAMP_WS_URL).await?;
    subscribe(&mut ws_stream, symbol).await?;
    Ok(ws_stream)
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
            if let Event::Data{..} = e {
                debug!("{:?}", e);
            } else {
                info!("{:?}", e);
            }
            Some(e)
        },
        Message::Ping(x) => { info!("Ping {:?}", x); None },
        Message::Pong(x) => { info!("Pong {:?}", x); None },
        Message::Close(x) => { info!("Close {:?}", x); None },
        Message::Frame(x) => { info!("Frame {:?}", x); None },
    };
    Ok(e.map(|e| e.maybe_to_tick()).flatten())
}

async fn subscribe (
    rx: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    symbol: &String,
) -> Result<(), Error>
{
    let channel = format!("order_book_{}", symbol.to_lowercase());
    let msg = serialize(Event::Subscribe{ data: OutSubscription { channel } })?;
    rx.send(Message::Text(msg)).await?;
    Ok(())
}

fn deserialize(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}

fn serialize(e: Event) -> serde_json::Result<String> {
    Ok(serde_json::to_string(&e)?)
}

mod timestamp {
    use std::str::FromStr;
    use chrono::{DateTime, Utc, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_i64(date.timestamp())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let datetime = Utc.timestamp(i64::from_str(&s).map_err(serde::de::Error::custom)?, 0);
        Ok(datetime)
    }
}

mod microtimestamp {
    use std::str::FromStr;
    use chrono::{DateTime, Utc, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_i64(date.timestamp_nanos()/1000)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let datetime = Utc.timestamp_nanos(i64::from_str(&s).map_err(serde::de::Error::custom)?*1000);
        Ok(datetime)
    }
}

#[cfg(test)]
mod test {
    use chrono::TimeZone;
    use rust_decimal_macros::dec;
    use crate::bitstamp::*;

    #[test]
    fn should_deserialize_event_data() -> Result<(), Error> {
        assert_eq!(deserialize("{\
                       \"data\":{\
                           \"timestamp\":\"1652103479\",\
                           \"microtimestamp\":\"1652103479857383\",\
                           \"bids\":[[\"0.07295794\",\"0.46500000\"],[\"0.07295284\",\"0.60423006\"]],\
                           \"asks\":[[\"0.07301587\",\"0.46500000\"],[\"0.07301952\",\"7.74449027\"]]\
                       },\
                       \"channel\":\"order_book_ethbtc\",\
                       \"event\":\"data\"\
                   }".to_string())?,
                   Event::Data{
                       data: InData {
                           timestamp: Utc.timestamp(1652103479, 0),
                           microtimestamp: Utc.timestamp_nanos(1652103479857383000),
                           bids: vec![ Bid { price: dec!(0.07295794), amount: dec!(0.46500000) }
                                     , Bid { price: dec!(0.07295284), amount: dec!(0.60423006) }
                                     ],
                           asks: vec![ Ask { price: dec!(0.07301587), amount: dec!(0.46500000) }
                                     , Ask { price: dec!(0.07301952), amount: dec!(7.74449027) }
                                     ]
                       },
                       channel: "order_book_ethbtc".to_string(),
                   });
        Ok(())
    }

    #[test]
    fn should_deserialize_subscription_succeeded() -> Result<(), Error> {
        assert_eq!(deserialize("{\
                       \"data\":{},\
                       \"channel\":\"order_book_ethbtc\",\
                       \"event\":\"bts:subscription_succeeded\"
                   }".to_string())?,
                   Event::SubscriptionSucceeded{
                       data: InSubscription{},
                       channel: "order_book_ethbtc".to_string(),
                   });
        Ok(())
    }

    #[test]
    fn should_deserialize_error() -> Result<(), Error> {
        assert_eq!(deserialize("{\
                       \"event\":\"bts:error\",\
                       \"channel\":\"\",\
                       \"data\":{\
                           \"code\":null,\
                           \"message\":\"Incorrect JSON format.\"\
                       }\
                   }".to_string())?,
                   Event::Error{
                       data: InError{ code: None, message: "Incorrect JSON format.".to_string() },
                       channel: "".to_string(),
                   });
        Ok(())
    }

    #[test]
    fn should_serialize_subscribe() -> Result<(), Error> {
        assert_eq!(serialize(Event::Subscribe{
            data: OutSubscription { channel: "order_book_ethbtc".to_string() }
        })?,
        "{\"event\":\"bts:subscribe\",\"data\":{\"channel\":\"order_book_ethbtc\"}}".to_string()
        );
        Ok(())
    }
}

