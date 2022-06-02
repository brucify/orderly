use crate::error::Error;
use crate::orderbook::{Exchange, InTick, ToLevel, ToLevels, ToTick};
use crate::{orderbook, websocket};
use futures::SinkExt;
use log::{debug, info};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tungstenite::protocol::Message;

const KRAKEN_WS_URL: &str = "wss://ws.kraken.com";

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum Event {
    GeneralMessage(GeneralMessage),

    PublicMessage(PublicMessage),
}

impl ToTick for Event {
    /// Converts the `Event` into a `Option<InTick>`. Only keep the top ten levels of bids and asks.
    fn maybe_to_tick(&self) -> Option<InTick> {
        match self {
            Event::PublicMessage(
                PublicMessage::SinglePayload(
                    SinglePayload{
                        payload: Payload::Book(Book::Snapshot {bids, asks}),
                        ..
                    })) => {
                let bids = bids.to_levels(orderbook::Side::Bid, 10);
                let asks = asks.to_levels(orderbook::Side::Ask, 10);
                Some(InTick { exchange: Exchange::Kraken, bids, asks })
            },
            Event::PublicMessage(
                PublicMessage::SinglePayload(
                    SinglePayload{
                        payload: Payload::Book(Book::Update {bids, asks, .. }),
                        ..
                    })) => {
                let mut tick = InTick{ exchange: Exchange::Kraken, bids: vec![], asks: vec![] };
                bids.as_ref().map(|bids| tick.bids = bids.to_levels(orderbook::Side::Bid, 10) );
                asks.as_ref().map(|asks| tick.asks = asks.to_levels(orderbook::Side::Ask, 10) );
                Some(tick)
            },
            Event::PublicMessage(
                PublicMessage::DoublePayload(
                    DoublePayload{
                        payload1: Payload::Book(Book::Update {bids: b1, asks: a1, ..}),
                        payload2: Payload::Book(Book::Update {bids: b2, asks: a2, ..}),
                        ..
                    })) => {
                let mut tick = InTick{ exchange: Exchange::Kraken, bids: vec![], asks: vec![] };
                b1.as_ref().map(|bids| tick.bids = bids.to_levels(orderbook::Side::Bid, 10) );
                b2.as_ref().map(|bids| tick.bids = bids.to_levels(orderbook::Side::Bid, 10) );
                a1.as_ref().map(|asks| tick.asks = asks.to_levels(orderbook::Side::Ask, 10) );
                a2.as_ref().map(|asks| tick.asks = asks.to_levels(orderbook::Side::Ask, 10) );
                Some(tick)
            },
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "event", rename_all = "camelCase")]
enum GeneralMessage {
    /// Request. Client can ping server to determine whether connection is alive,
    /// server responds with pong. This is an application level ping as opposed to
    /// default ping in websockets standard which is server initiated
    ///
    /// **Example of payload**
    /// ```json
    /// {
    ///   "event": "ping",
    ///   "reqid": 42
    /// }
    /// ```
    Ping{
        /// Optional - client originated ID reflected in response message
        reqid: Option<usize>
    },

    /// Response. Server pong response to a ping to determine whether connection is alive. This is an application level pong as opposed to default pong in websockets standard which is sent by client in response to a ping
    ///
    /// **Example of payload**
    /// ```json
    /// {
    ///   "event": "pong",
    ///   "reqid": 42
    /// }
    ///```
    Pong{
        /// Optional - matching client originated request ID
        reqid: Option<usize>
    },

    Heartbeat{},

    /// Publication: Status sent on connection or system status changes.
    ///
    /// **Example of payload**
    ///
    /// ```json
    /// {
    ///   "connectionID": 8628615390848610000,
    ///   "event": "systemStatus",
    ///   "status": "online",
    ///   "version": "1.0.0"
    /// }
    /// ```
    SystemStatus{
        /// Optional - Connection ID (will appear only in initial connection status message)
        #[serde(rename = "connectionID")]
        connection_id: Option<usize>,

        /// online|maintenance|cancel_only|limit_only|post_only
        status: Status,
        version: String,
    },

    /// Request. Subscribe to a topic on a single or multiple currency pairs.
    ///
    /// **Example of payload**
    ///
    ///```json
    ///{
    ///  "event": "subscribe",
    ///  "pair": [
    ///    "XBT/USD",
    ///    "XBT/EUR"
    ///  ],
    ///  "subscription": {
    ///    "name": "ticker"
    ///  }
    ///}
    ///
    ///{
    ///  "event": "subscribe",
    ///  "pair": [
    ///    "XBT/EUR"
    ///  ],
    ///  "subscription": {
    ///    "interval": 5,
    ///    "name": "ohlc"
    ///  }
    ///}
    ///
    ///{
    ///  "event": "subscribe",
    ///  "subscription": {
    ///    "name": "ownTrades",
    ///    "token": "WW91ciBhdXRoZW50aWNhdGlvbiB0b2tlbiBnb2VzIGhlcmUu"
    ///  }
    ///}
    ///```
    Subscribe{
        /// Optional - client originated ID reflected in response message
        #[serde(skip_serializing_if = "Option::is_none")]
        reqid: Option<usize>,

        /// Optional - Array of currency pairs. Format of each pair is "A/B", where A and B are ISO 4217-A3 for standardized assets and popular unique symbol if not standardized.
        pair: Vec<String>,

        subscription: Subscription,
    },

    Unsubscribe{
        /// Optional - client originated ID reflected in response message
        #[serde(skip_serializing_if = "Option::is_none")]
        reqid: Option<usize>,

        /// Optional - Array of currency pairs. Format of each pair is "A/B", where A and B are ISO 4217-A3 for standardized assets and popular unique symbol if not standardized.
        pair: Vec<String>,

        subscription: Unsubscription,
    },

    /// Response. Subscription status response to subscribe, unsubscribe or exchange initiated unsubscribe.
    ///
    /// **Example of payload**
    ///
    /// ```json
    ///{
    ///  "channelID": 10001,
    ///  "channelName": "ticker",
    ///  "event": "subscriptionStatus",
    ///  "pair": "XBT/EUR",
    ///  "status": "subscribed",
    ///  "subscription": {
    ///    "name": "ticker"
    ///  }
    ///}
    ///
    ///{
    ///  "channelID": 10001,
    ///  "channelName": "ohlc-5",
    ///  "event": "subscriptionStatus",
    ///  "pair": "XBT/EUR",
    ///  "reqid": 42,
    ///  "status": "unsubscribed",
    ///  "subscription": {
    ///    "interval": 5,
    ///    "name": "ohlc"
    ///  }
    ///}
    ///
    ///{
    ///  "channelName": "ownTrades",
    ///  "event": "subscriptionStatus",
    ///  "status": "subscribed",
    ///  "subscription": {
    ///    "name": "ownTrades"
    ///  }
    ///}
    ///
    ///{
    ///  "errorMessage": "Subscription depth not supported",
    ///  "event": "subscriptionStatus",
    ///  "pair": "XBT/USD",
    ///  "status": "error",
    ///  "subscription": {
    ///    "depth": 42,
    ///    "name": "book"
    ///  }
    ///}
    ///```
    SubscriptionStatus{
        /// Channel Name on successful subscription. For payloads 'ohlc' and 'book', respective interval or depth will be added as suffix.
        #[serde(rename = "channelName")]
        channel_name: Option<String>,

        /// Optional - matching client originated request ID
        reqid: Option<usize>,

        /// Optional - Currency pair, applicable to public messages only
        pair: Option<String>,

        /// Status of subscription
        status: String,

        subscription: Option<SubscriptionStatus>,

        /// Error message
        #[serde(rename = "errorMessage")]
        error_message: Option<String>,

        /// Channel ID on successful subscription, applicable to public messages only - deprecated, use channelName and pair
        #[serde(rename = "channelID")]
        channel_id: Option<usize>,
    },

    /// **Examples of payload**
    ///
    ///```json
    /// {
    ///   "errorMessage": "Malformed request",
    ///   "event": "error"
    /// }
    ///
    /// {
    ///   "errorMessage":"Exceeded msg rate",
    ///   "event": "error",
    ///   "reqid": 42
    /// }
    ///```
    Error{
        /// Error detail message.
        #[serde(rename = "errorMessage")]
        error_message: String,

        /// Optional - client originated ID reflected in response message
        reqid: Option<usize>,
    },

}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Online,
    Maintenance,
    CancelOnly,
    LimitOnly,
    PostOnly,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Subscription {
    /// Optional - depth associated with book subscription in number of levels each side, default 10. Valid Options are: 10, 25, 100, 500, 1000
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<usize>,

    /// Optional - Time interval associated with ohlc subscription in minutes. Default 1. Valid Interval values: 1|5|15|30|60|240|1440|10080|21600
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<usize>,

    /// book|ohlc|openOrders|ownTrades|spread|ticker|trade|*, * for all available channels depending on the connected environment
    name: SubscriptionType,

    /// Optional - whether to send rate-limit counter in updates (supported only for openOrders subscriptions; default = false)
    #[serde(skip_serializing_if = "Option::is_none")]
    ratecounter: Option<bool>,

    /// Optional - whether to send historical feed data snapshot upon subscription (supported only for ownTrades subscriptions; default = true)
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot: Option<bool>,

    /// Optional - base64-encoded authentication token for private-data endpoints
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Unsubscription {
    /// Optional - depth associated with book subscription in number of levels each side, default 10. Valid Options are: 10, 25, 100, 500, 1000
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<usize>,

    /// Optional - Time interval associated with ohlc subscription in minutes. Default 1. Valid Interval values: 1|5|15|30|60|240|1440|10080|21600
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<usize>,

    /// book|ohlc|openOrders|ownTrades|spread|ticker|trade|*, * for all available channels depending on the connected environment
    name: SubscriptionType,

    /// Optional - base64-encoded authentication token for private-data endpoints
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct SubscriptionStatus {
    /// Optional - depth associated with book subscription in number of levels each side, default 10. Valid Options are: 10, 25, 100, 500, 1000
    depth: Option<usize>,

    /// Optional - Time interval associated with ohlc subscription in minutes. Default 1. Valid Interval values: 1|5|15|30|60|240|1440|10080|21600
    interval: Option<usize>,

    /// Optional - max rate-limit budget. Compare to the ratecounter field in the openOrders updates to check whether you are approaching the rate limit.
    maxratecount: Option<usize>,

    /// book|ohlc|openOrders|ownTrades|spread|ticker|trade|*, * for all available channels depending on the connected environment
    name: SubscriptionType,

    /// Optional - base64-encoded authentication token for private-data endpoints
    token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum PublicMessage {
    SinglePayload(SinglePayload),
    DoublePayload(DoublePayload),
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct SinglePayload {
    /// Channel ID of subscription - deprecated, use channelName and pair
    channel_id: usize,

    payload: Payload,

    /// Channel Name of subscription
    channel_name: String,

    /// Asset pair
    pair: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct DoublePayload {
    /// Channel ID of subscription - deprecated, use channelName and pair
    channel_id: usize,

    payload1: Payload,

    payload2: Payload,

    /// Channel Name of subscription
    channel_name: String,

    /// Asset pair
    pair: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum Payload {
    Book(Book),
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum Book {
    /// Publication: Order book levels. On subscription, a snapshot will be published at the specified depth, following the snapshot, level updates will be published
    ///
    /// **Example of snapshot payload**
    ///
    ///```json
    /// [
    ///   0,
    ///   {
    ///     "as": [
    ///       [
    ///         "5541.30000",
    ///         "2.50700000",
    ///         "1534614248.123678"
    ///       ],
    ///       [
    ///         "5541.80000",
    ///         "0.33000000",
    ///         "1534614098.345543"
    ///       ],
    ///       [
    ///         "5542.70000",
    ///         "0.64700000",
    ///         "1534614244.654432"
    ///       ]
    ///     ],
    ///     "bs": [
    ///       [
    ///         "5541.20000",
    ///         "1.52900000",
    ///         "1534614248.765567"
    ///       ],
    ///       [
    ///         "5539.90000",
    ///         "0.30000000",
    ///         "1534614241.769870"
    ///       ],
    ///       [
    ///         "5539.50000",
    ///         "5.00000000",
    ///         "1534613831.243486"
    ///       ]
    ///     ]
    ///   },
    ///   "book-100",
    ///   "XBT/USD"
    /// ]
    ///```
    Snapshot {
        /// Array of price levels, ascending from best ask
        #[serde(rename = "as")]
        asks: Vec<Level>,

        /// Array of price levels, descending from best bid
        #[serde(rename = "bs")]
        bids: Vec<Level>,
    },

    /// Publication: Order book levels. On subscription, a snapshot will be published at the specified depth, following the snapshot, level updates will be published
    ///
    /// **Example of update payload**
    ///
    /// ```json
    /// [
    ///  1234,
    ///  {
    ///    "a": [
    ///      [
    ///        "5541.30000",
    ///        "2.50700000",
    ///        "1534614248.456738"
    ///      ],
    ///      [
    ///        "5542.50000",
    ///        "0.40100000",
    ///        "1534614248.456738"
    ///      ]
    ///    ],
    ///    "c": "974942666"
    ///  },
    ///  "book-10",
    ///  "XBT/USD"
    ///]
    ///
    ///[
    ///  1234,
    ///  {
    ///    "b": [
    ///      [
    ///        "5541.30000",
    ///        "0.00000000",
    ///        "1534614335.345903"
    ///      ]
    ///    ],
    ///    "c": "974942666"
    ///  },
    ///  "book-10",
    ///  "XBT/USD"
    ///]
    ///
    ///[
    ///  1234,
    ///  {
    ///    "a": [
    ///      [
    ///        "5541.30000",
    ///        "2.50700000",
    ///        "1534614248.456738"
    ///      ],
    ///      [
    ///        "5542.50000",
    ///        "0.40100000",
    ///        "1534614248.456738"
    ///      ]
    ///    ]
    ///  },
    ///  {
    ///    "b": [
    ///      [
    ///        "5541.30000",
    ///        "0.00000000",
    ///        "1534614335.345903"
    ///      ]
    ///    ],
    ///    "c": "974942666"
    ///  },
    ///  "book-10",
    ///  "XBT/USD"
    /// ]
    /// ```
    ///
    /// **Example of republish payload**
    ///
    /// ```json
    /// [
    ///  1234,
    ///  {
    ///    "a": [
    ///      [
    ///        "5541.30000",
    ///        "2.50700000",
    ///        "1534614248.456738",
    ///        "r"
    ///      ],
    ///      [
    ///        "5542.50000",
    ///        "0.40100000",
    ///        "1534614248.456738",
    ///        "r"
    ///      ]
    ///    ],
    ///    "c": "974942666"
    ///  },
    ///  "book-25",
    ///  "XBT/USD"
    ///]
    /// ```
    Update {
        /// Ask array of level updates
        #[serde(default)]
        #[serde(rename = "a")]
        asks: Option<Vec<Level>>,

        /// Bid array of level updates
        #[serde(default)]
        #[serde(rename = "b")]
        bids: Option<Vec<Level>>,

        /// Optional - Book checksum as a quoted unsigned 32-bit integer, present only within the last update container in the message. See calculation details.
        #[serde(default)]
        #[serde(rename = "c")]
        checksum: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Level {
    /// Price level
    price: Decimal,

    /// Price level volume, for updates volume = 0 for level removal/deletion
    volume: Decimal,

    /// Price level last updated, seconds since epoch
    timestamp: Decimal,

    #[serde(default)]
    /// Optional - "r" in case update is a republished update
    update_type: Option<String>
}

impl ToLevel for Level {
    /// Converts a `kraken::Level` into a `orderbook::Level`.
    fn to_level(&self, side: orderbook::Side) -> orderbook::Level {
        orderbook::Level::new(side, self.price, self.volume, Exchange::Kraken)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum SubscriptionType {
    Book,
    Ohlc,
    OpenOrders,
    OwnTrades,
    Spread,
    Ticker,
    Trade,
    #[serde(rename = "*")]
    AllAvailable,
}

pub(crate) async fn connect(symbol: &String) -> Result<websocket::WsStream, Error> {
    let mut ws_stream = websocket::connect(KRAKEN_WS_URL).await?;
    subscribe(&mut ws_stream, symbol).await?;
    Ok(ws_stream)
}

async fn subscribe (
    rx: &mut websocket::WsStream,
    symbol: &String,
) -> Result<(), Error>
{
    let pair = symbol.to_uppercase();
    let sub = GeneralMessage::Subscribe{
        reqid: None,
        pair: vec![pair],
        subscription: Subscription {
            depth: Some(10),
            name: SubscriptionType::Book,
            interval: None,
            ratecounter: None,
            snapshot: None,
            token: None,
        },
    };
    let msg = serialize(sub)?;
    rx.send(Message::Text(msg)).await?;
    Ok(())
}

pub(crate) fn parse(msg: Message) -> Result<Option<InTick>, Error> {
    let e = match msg {
        Message::Binary(x) => { info!("binary {:?}", x); None },
        Message::Text(x) => {
            debug!("{:?}", x);

            let e = deserialize_event(x)?;
            match e {
                Event::GeneralMessage(_) => info!("{:?}", e),
                Event::PublicMessage(_) => debug!("{:?}", e),
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

fn deserialize_event(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}

fn serialize(msg: GeneralMessage) -> serde_json::Result<String> {
    Ok(serde_json::to_string(&Event::GeneralMessage(msg))?)
}

#[cfg(test)]
mod test {
    use rust_decimal_macros::dec;
    use crate::kraken::*;

    #[test]
    fn should_deserialize_book_snapshot() -> Result<(), Error> {
        assert_eq!(deserialize_event(r#"
        [
            640,
            {
                "as": [
                    ["0.068010","2.61547960","1652817781.572052"],
                    ["0.068020","2.80351225","1652817780.290886"],
                    ["0.068040","24.45938572","1652817780.453451"],
                    ["0.068050","24.45938596","1652817780.339826"],
                    ["0.068060","14.63500000","1652817759.528227"],
                    ["0.068070","48.92440377","1652817779.227643"],
                    ["0.068080","4.00000000","1652817780.668774"],
                    ["0.068090","50.90608702","1652817765.593309"],
                    ["0.068110","18.43030000","1652817774.974343"],
                    ["0.068120","59.24322805","1652817779.215020"]
                ],
                "bs":[
                    ["0.067990","29.35934962","1652817780.853167"],
                    ["0.067980","48.72763614","1652817781.487388"],
                    ["0.067970","25.55979457","1652817781.624545"],
                    ["0.067960","48.91046225","1652817780.502996"],
                    ["0.067950","17.83261805","1652817779.124903"],
                    ["0.067930","2.11301052","1652817779.101854"],
                    ["0.067920","48.92972805","1652817779.207823"],
                    ["0.067900","53.93281284","1652817781.478333"],
                    ["0.067880","15.00000000","1652817781.574921"],
                    ["0.067870","2.84944758","1652817779.146792"]
                ]
            },
            "book-10",
            "ETH/XBT"
        ]"#.to_string())?, Event::PublicMessage(PublicMessage::SinglePayload(SinglePayload {
            channel_id: 640,
                payload: Payload::Book(Book::Snapshot {
                    bids: vec![
                        Level { price: dec!(0.067990), volume: dec!(29.35934962), timestamp: dec!(1652817780.853167), update_type: None },
                        Level { price: dec!(0.067980), volume: dec!(48.72763614), timestamp: dec!(1652817781.487388), update_type: None },
                        Level { price: dec!(0.067970), volume: dec!(25.55979457), timestamp: dec!(1652817781.624545), update_type: None },
                        Level { price: dec!(0.067960), volume: dec!(48.91046225), timestamp: dec!(1652817780.502996), update_type: None },
                        Level { price: dec!(0.067950), volume: dec!(17.83261805), timestamp: dec!(1652817779.124903), update_type: None },
                        Level { price: dec!(0.067930), volume: dec!(2.11301052), timestamp: dec!(1652817779.101854), update_type: None },
                        Level { price: dec!(0.067920), volume: dec!(48.92972805), timestamp: dec!(1652817779.207823), update_type: None },
                        Level { price: dec!(0.067900), volume: dec!(53.93281284), timestamp: dec!(1652817781.478333), update_type: None },
                        Level { price: dec!(0.067880), volume: dec!(15.00000000), timestamp: dec!(1652817781.574921), update_type: None },
                        Level { price: dec!(0.067870), volume: dec!(2.84944758), timestamp: dec!(1652817779.146792), update_type: None },
                    ],
                    asks: vec![
                        Level { price: dec!(0.068010), volume: dec!(2.61547960), timestamp: dec!(1652817781.572052), update_type: None },
                        Level { price: dec!(0.068020), volume: dec!(2.80351225), timestamp: dec!(1652817780.290886), update_type: None },
                        Level { price: dec!(0.068040), volume: dec!(24.45938572), timestamp: dec!(1652817780.453451), update_type: None },
                        Level { price: dec!(0.068050), volume: dec!(24.45938596), timestamp: dec!(1652817780.339826), update_type: None },
                        Level { price: dec!(0.068060), volume: dec!(14.63500000), timestamp: dec!(1652817759.528227), update_type: None },
                        Level { price: dec!(0.068070), volume: dec!(48.92440377), timestamp: dec!(1652817779.227643), update_type: None },
                        Level { price: dec!(0.068080), volume: dec!(4.00000000), timestamp: dec!(1652817780.668774), update_type: None },
                        Level { price: dec!(0.068090), volume: dec!(50.90608702), timestamp: dec!(1652817765.593309), update_type: None },
                        Level { price: dec!(0.068110), volume: dec!(18.43030000), timestamp: dec!(1652817774.974343), update_type: None },
                        Level { price: dec!(0.068120), volume: dec!(59.24322805), timestamp: dec!(1652817779.215020), update_type: None },
                    ],
                }),
                channel_name: "book-10".to_string(),
                pair: "ETH/XBT".to_string(),
            }))
        );
        Ok(())
    }

    #[test]
    fn should_deserialize_book_update_single_payload() -> Result<(), Error> {
        assert_eq!(deserialize_event(r#"
        [
            640,
            {
                "b":[
                    ["0.067670","30.32313249","1652895615.219798"]
                ],
                "c":"1980194141"
            },
            "book-10",
            "ETH/XBT"
        ]"#.to_string())?, Event::PublicMessage(PublicMessage::SinglePayload(SinglePayload {
            channel_id: 640,
            payload: Payload::Book(Book::Update {
                asks: None,
                bids: Some(vec![
                    Level { price: dec!(0.067670), volume: dec!(30.32313249), timestamp: dec!(1652895615.219798), update_type: None },
                ]),
                checksum: Some("1980194141".to_string()),
            }),
            channel_name: "book-10".to_string(),
            pair: "ETH/XBT".to_string(),
        })));
        Ok(())
    }

    #[test]
    fn should_deserialize_book_update_double_payload() -> Result<(), Error> {
        assert_eq!(deserialize_event(r#"
        [
            640,
            {
                "a": [
                    [
                    "0.067390",
                    "31.09081272",
                    "1652905268.998332"
                    ]
                ]
            },
            {
                "b": [
                    [
                    "0.067290",
                    "53.27428999",
                    "1652905268.998444"
                    ]
                ],
                "c": "201829889"
            },
            "book-10",
            "ETH/XBT"
        ]"#.to_string())?, Event::PublicMessage(PublicMessage::DoublePayload(DoublePayload {
            channel_id: 640,
            payload1: Payload::Book(Book::Update {
                asks: Some(vec![
                    Level { price: dec!(0.067390), volume: dec!(31.09081272), timestamp: dec!(1652905268.998332), update_type: None },
                ]),
                bids: None,
                checksum: None,
            }),
            payload2: Payload::Book(Book::Update {
                asks: None,
                bids: Some(vec![
                    Level { price: dec!(0.067290), volume: dec!(53.27428999), timestamp: dec!(1652905268.998444), update_type: None },
                ]),
                checksum: Some("201829889".to_string()),
            }),
            channel_name: "book-10".to_string(),
            pair: "ETH/XBT".to_string(),
        })));
        Ok(())
    }

    #[test]
    fn should_deserialize_subscription() -> Result<(), Error> {
        assert_eq!(deserialize_event(r#"
        {
            "channelID":640,
            "channelName":"book-10",
            "event":"subscriptionStatus",
            "pair":"ETH/XBT",
            "status":"subscribed",
            "subscription":{
                "depth":10,
                "name":"book"
            }
        }"#.to_string())?, Event::GeneralMessage(GeneralMessage::SubscriptionStatus{
            channel_name: Some("book-10".to_string()),
            reqid: None, 
            pair: Some("ETH/XBT".to_string()),
            status: "subscribed".to_string(),
            subscription: Some(SubscriptionStatus { 
                depth: Some(10), 
                interval: None, 
                maxratecount: None, 
                name: SubscriptionType::Book,
                token: None,
            }), 
            error_message: None, 
            channel_id: Some(640),
        }));

        Ok(())
    }

    #[test]
    fn should_deserialize_subscription_error() -> Result<(), Error> {
        assert_eq!(deserialize_event(r#"
        {
            "errorMessage": "Event(s) not found",
            "event": "subscriptionStatus",
            "status": "error"
        }"#.to_string())?, Event::GeneralMessage(GeneralMessage::SubscriptionStatus{
            channel_name: None,
            reqid: None,
            pair: None,
            status: "error".to_string(),
            subscription: None,
            error_message: Some("Event(s) not found".to_string()),
            channel_id: None
        }));

        Ok(())
    }

    #[test]
    fn should_serialize_subscription() -> Result<(), Error> {
        let mut serialized = r#"
        {
            "event": "subscribe",
            "pair": [
                "ETH/BTC"
            ],
            "subscription": {
                "depth": 10,
                "name": "book"
            }
        }"#.to_string();
        serialized.retain(|c| !c.is_whitespace());

        assert_eq!(serialize(GeneralMessage::Subscribe{
            reqid: None,
            pair: vec!["ETH/BTC".to_string()],
            subscription: Subscription {
                depth: Some(10),
                name: SubscriptionType::Book,
                interval: None,
                ratecounter: None,
                snapshot: None,
                token: None,
            },
        })?, serialized);

        Ok(())
    }

    #[test]
    fn should_convert_to_tick() -> Result<(), Error> {
        /*
         * Given
         */
        let e = Event::PublicMessage(PublicMessage::SinglePayload(SinglePayload {
            channel_id: 640,
            payload: Payload::Book(Book::Snapshot {
                bids: vec![
                    Level { price: dec!(0.067990), volume: dec!(29.35934962), timestamp: dec!(1652817780.853167), update_type: None },
                    Level { price: dec!(0.067980), volume: dec!(48.72763614), timestamp: dec!(1652817781.487388), update_type: None },
                    Level { price: dec!(0.067970), volume: dec!(25.55979457), timestamp: dec!(1652817781.624545), update_type: None },
                    Level { price: dec!(0.067960), volume: dec!(48.91046225), timestamp: dec!(1652817780.502996), update_type: None },
                    Level { price: dec!(0.067950), volume: dec!(17.83261805), timestamp: dec!(1652817779.124903), update_type: None },
                    Level { price: dec!(0.067930), volume: dec!(2.11301052), timestamp: dec!(1652817779.101854), update_type: None },
                    Level { price: dec!(0.067920), volume: dec!(48.92972805), timestamp: dec!(1652817779.207823), update_type: None },
                    Level { price: dec!(0.067900), volume: dec!(53.93281284), timestamp: dec!(1652817781.478333), update_type: None },
                    Level { price: dec!(0.067880), volume: dec!(15.00000000), timestamp: dec!(1652817781.574921), update_type: None },
                    Level { price: dec!(0.067870), volume: dec!(2.84944758), timestamp: dec!(1652817779.146792), update_type: None },
                ],
                asks: vec![
                    Level { price: dec!(0.068010), volume: dec!(2.61547960), timestamp: dec!(1652817781.572052), update_type: None },
                    Level { price: dec!(0.068020), volume: dec!(2.80351225), timestamp: dec!(1652817780.290886), update_type: None },
                    Level { price: dec!(0.068040), volume: dec!(24.45938572), timestamp: dec!(1652817780.453451), update_type: None },
                    Level { price: dec!(0.068050), volume: dec!(24.45938596), timestamp: dec!(1652817780.339826), update_type: None },
                    Level { price: dec!(0.068060), volume: dec!(14.63500000), timestamp: dec!(1652817759.528227), update_type: None },
                    Level { price: dec!(0.068070), volume: dec!(48.92440377), timestamp: dec!(1652817779.227643), update_type: None },
                    Level { price: dec!(0.068080), volume: dec!(4.00000000), timestamp: dec!(1652817780.668774), update_type: None },
                    Level { price: dec!(0.068090), volume: dec!(50.90608702), timestamp: dec!(1652817765.593309), update_type: None },
                    Level { price: dec!(0.068110), volume: dec!(18.43030000), timestamp: dec!(1652817774.974343), update_type: None },
                    Level { price: dec!(0.068120), volume: dec!(59.24322805), timestamp: dec!(1652817779.215020), update_type: None },
                ],
            }),
            channel_name: "book-10".to_string(),
            pair: "ETH/XBT".to_string(),
        }));

        /*
         * When
         */
        let tick = e.maybe_to_tick();

        /*
         * Then
         */
        assert_eq!(tick, Some(InTick{
            exchange: Exchange::Kraken,
            bids: vec![
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067990), dec!(29.35934962), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067980), dec!(48.72763614), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067970), dec!(25.55979457), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067960), dec!(48.91046225), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067950), dec!(17.83261805), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067930), dec!(2.11301052), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067920), dec!(48.92972805), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067900), dec!(53.93281284), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067880), dec!(15.00000000), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067870), dec!(2.84944758), Exchange::Kraken),
            ],
            asks: vec![
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068010), dec!(2.61547960), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068020), dec!(2.80351225), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068040), dec!(24.45938572), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068050), dec!(24.45938596), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068060), dec!(14.63500000), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068070), dec!(48.92440377), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068080), dec!(4.00000000), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068090), dec!(50.90608702), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068110), dec!(18.43030000), Exchange::Kraken),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068120), dec!(59.24322805), Exchange::Kraken),
            ],
        }));

        Ok(())
    }

}

