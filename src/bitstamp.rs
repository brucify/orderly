use chrono::{DateTime, Utc};
use crate::error::Error;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::Message;
use url::Url;
use crate::tick::{Ask, Bid, Tick, ToTick};

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
    fn maybe_to_tick(&self) -> Option<Tick> {
        match self {
            Event::Data { data, channel } =>
                Some(Tick {
                    exchange: "bitstamp".to_string(),
                    channel: channel.clone(),
                    timestamp: data.timestamp,
                    microtimestamp: data.microtimestamp,
                    bids: data.bids.split_at(10).0.to_vec(),
                    asks: data.asks.split_at(10).0.to_vec(),
                }),
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

type Channel = String;

pub(crate) async fn ws_stream() -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let url = Url::parse(BITSTAMP_WS_URL).unwrap();
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    ws_stream
}

pub(crate) fn parse(ws_msg: Option<Result<Message, tungstenite::Error>>) -> Result<Option<Tick>, Error> {
    let msg = ws_msg.unwrap_or_else(|| {
        println!("no message");
        Err(tungstenite::Error::ConnectionClosed)
    })?;
    let e = match msg {
        Message::Binary(x) => { println!("binary {:?}", x); None },
        Message::Text(x) => {
            let e= deserialize(x)?;
            if let Event::Data{..} = e {} else { println!("{:?}", e); }
            Some(e)
        },
        Message::Ping(x) => { println!("Ping {:?}", x); None },
        Message::Pong(x) => { println!("Pong {:?}", x); None },
        Message::Close(x) => { println!("Close {:?}", x); None },
        Message::Frame(x) => { println!("Frame {:?}", x); None },
    };
    Ok(e.map(|e| e.maybe_to_tick()).flatten())
}

fn deserialize(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}

pub(crate) async fn close(ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) {
    let _ = ws_stream.send(Message::Close(None)).await;
    let close = ws_stream.next().await;
    println!("server close msg: {:?}", close);
    assert!(ws_stream.next().await.is_none());
    let _ = ws_stream.close(None).await;
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
                           bids: vec![vec![dec!(0.07295794), dec!(0.46500000)], vec![dec!(0.07295284), dec!(0.60423006)]],
                           asks: vec![vec![dec!(0.07301587), dec!(0.46500000)], vec![dec!(0.07301952), dec!(7.74449027)]]
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
}

