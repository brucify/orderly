use crate::error::Error;
use futures::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::Message;
use url::Url;

#[derive(Debug, Deserialize, PartialEq)]
struct InEvent {
    event: EventType,
    data: InData,
    channel: String,
}

#[derive(Debug, Serialize)]
struct OutEvent {
    event: EventType,
    data: OutData,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
enum EventType {
    #[serde(rename = "data")]
    Data,

    #[serde(rename = "bts:subscribe")]
    Subscribe,

    #[serde(rename = "bts:unsubscribe")]
    Unsubscribe,

    #[serde(rename = "bts:subscription_succeeded")]
    SubscriptionSucceeded,

    #[serde(rename = "bts:unsubscription_succeeded")]
    UnsubscriptionSucceeded,

    #[serde(rename = "bts:error")]
    Error,
}

#[derive(Debug, Serialize)]
struct OutData {
    channel: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct InData {
    timestamp: Option<String>,
    microtimestamp: Option<String>,
    bids: Option<Vec<Bid>>,
    asks: Option<Vec<Ask>>,
}

type Bid = Vec<Decimal>;
type Ask = Vec<Decimal>;

pub(crate) async fn ws_stream() -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let url = Url::parse("wss://ws.bitstamp.net").unwrap();
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    ws_stream
}

pub(crate) fn parse(ws_msg: Option<Result<Message, tungstenite::Error>>) -> Result<(), Error> {
    let msg = ws_msg.unwrap_or_else(|| {
        println!("no message");
        Err(tungstenite::Error::ConnectionClosed)
    })?;
    match msg {
        Message::Binary(x) => println!("binary {:?}", x),
        Message::Text(x) => {
            // let x = deserialize(x).map_err(Error::BadData)?;
            println!("{:?}", x)
        },
        Message::Ping(x) => println!("Ping {:?}", x),
        Message::Pong(x) => println!("Pong {:?}", x),
        Message::Close(x) => println!("Close {:?}", x),
        Message::Frame(x) => println!("Frame {:?}", x),
    }
    Ok(())
}

fn deserialize(s: String) -> serde_json::Result<InEvent> {
    Ok(serde_json::from_str(&s)?)
}

pub(crate) async fn close(ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) {
    let _ = ws_stream.send(Message::Close(None)).await;
    let close = ws_stream.next().await;
    println!("server close msg: {:?}", close);
    assert!(ws_stream.next().await.is_none());
    let _ = ws_stream.close(None).await;
}

#[cfg(test)]
mod test {
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
                   InEvent{
                       event: EventType::Data,
                       data: InData {
                           timestamp: Some("1652103479".to_string()),
                           microtimestamp: Some("1652103479857383".to_string()),
                           bids: Some(vec![vec![dec!(0.07295794), dec!(0.46500000)], vec![dec!(0.07295284), dec!(0.60423006)]]),
                           asks: Some(vec![vec![dec!(0.07301587), dec!(0.46500000)], vec![dec!(0.07301952), dec!(7.74449027)]])
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
                   InEvent{
                       event: EventType::SubscriptionSucceeded,
                       data: InData { timestamp: None, microtimestamp: None, bids: None, asks: None },
                       channel: "order_book_ethbtc".to_string(),
                   });
        Ok(())
    }
    //
    // #[test]
    // fn should_deserialize_subscription_succeeded() -> Result<(), Error> {
    //     assert_eq!(deserialize("{\
    //                    \"\event\":\"bts:error\",\
    //                    \"channel\":\"\",\
    //                    \"data\":{\
    //                        \"code\":null,\
    //                        \"message\":\"Incorrect JSON format.\"\
    //                    }\
    //                }".to_string())?,
    //                InEvent{
    //                    event: EventType::Error,
    //                    data: InData { timestamp: None, microtimestamp: None, bids: None, asks: None },
    //                    channel: None,
    //                });
    //     Ok(())
    // }


}

