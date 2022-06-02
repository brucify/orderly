use chrono::{DateTime, Utc};
use futures::SinkExt;
use crate::error::Error;
use crate::orderbook::{self, Exchange, InTick, ToLevel, ToLevels, ToTick};
use crate::websocket;
use log::{debug, info};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tungstenite::Message;

const COINBASE_WS_URL: &str = "wss://ws-feed.exchange.coinbase.com";

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Event {
    /// Most failure cases will cause an error message (a message with the type "error") to be emitted.
    /// This can be helpful for implementing a client or debugging issues.
    ///
    /// ```json
    /// {
    ///   "type": "error",
    ///   "message": "error message"
    ///   /* ... */
    /// }
    /// ```
    Error {
        message: String,
    },

    /// To begin receiving feed messages, you must first send a subscribe message to the server indicating which channels and products to receive. This message is mandatory—you are disconnected if no subscribe has been received within 5 seconds.
    ///
    /// Every subscribe request must also be signed; see the section below on signing for more information.
    ///
    /// ```json
    /// // Request
    /// // Subscribe to ETH-USD and ETH-EUR with the level2, heartbeat and ticker channels,
    /// // plus receive the ticker entries for ETH-BTC and ETH-USD
    /// {
    ///     "type": "subscribe",
    ///     "product_ids": [
    ///         "ETH-USD",
    ///         "ETH-EUR"
    ///     ],
    ///     "channels": [
    ///         "level2",
    ///         "heartbeat",
    ///         {
    ///             "name": "ticker",
    ///             "product_ids": [
    ///                 "ETH-BTC",
    ///                 "ETH-USD"
    ///             ]
    ///         }
    ///     ]
    /// }
    /// ```
    Subscribe {
        #[serde(skip_serializing_if = "Option::is_none")]
        product_ids: Option<Vec<String>>,

        channels: Vec<Channel>,
    },

    /// If you want to unsubscribe from channel/product pairs, send an `unsubscribe` message. The structure is equivalent to `subscribe` messages. As a shorthand you can also provide no product IDs for a channel, which unsubscribes you from the channel entirely.
    ///
    /// ```json
    /// // Request
    /// {
    ///     "type": "unsubscribe",
    ///     "product_ids": [
    ///         "ETH-USD",
    ///         "ETH-EUR"
    ///     ],
    ///     "channels": ["ticker"]
    /// }
    /// ```
    Unsubscribe {
        #[serde(skip_serializing_if = "Option::is_none")]
        product_ids: Option<Vec<String>>,

        channels: Vec<Channel>,
    },

    /// Once a `subscribe` message is received, the server responds with a `subscriptions` message that lists all channels you are subscribed to. Subsequent subscribe messages add to the list of subscriptions. If you already subscribed to a channel without being authenticated, you will remain in the unauthenticated channel.
    ///
    /// ```json
    /// // Response
    /// {
    ///     "type": "subscriptions",
    ///     "channels": [
    ///         {
    ///             "name": "level2",
    ///             "product_ids": [
    ///                 "ETH-USD",
    ///                 "ETH-EUR"
    ///             ],
    ///         },
    ///         {
    ///             "name": "heartbeat",
    ///             "product_ids": [
    ///                 "ETH-USD",
    ///                 "ETH-EUR"
    ///             ],
    ///         },
    ///         {
    ///             "name": "ticker",
    ///             "product_ids": [
    ///                 "ETH-USD",
    ///                 "ETH-EUR",
    ///                 "ETH-BTC"
    ///             ]
    ///         }
    ///     ]
    /// }
    /// ```
    Subscriptions {
        channels: Vec<Channel>,
    },

    /// To receive heartbeat messages for specific products every second, subscribe to the heartbeat channel. Heartbeats include sequence numbers and last trade IDs that can be used to verify that no messages were missed.
    ///
    /// ```json
    /// // Heartbeat message
    /// {
    ///   "type": "heartbeat",
    ///   "sequence": 90,
    ///   "last_trade_id": 20,
    ///   "product_id": "BTC-USD",
    ///   "time": "2014-11-07T08:19:28.464459Z"
    /// }
    /// ```
    Heartbeat {
        sequence: usize,
        last_trade_id: usize,
        product_id: String,

        #[serde(with = "timestamp")]
        time: DateTime<Utc>
    },

    /// The status channel sends all products and currencies on a preset interval.
    ///
    /// ```json
    /// // Status Message
    /// {
    ///   "type": "status",
    ///   "products": [
    ///     {
    ///       "id": "BTC-USD",
    ///       "base_currency": "BTC",
    ///       "quote_currency": "USD",
    ///       "base_min_size": "0.001",
    ///       "base_max_size": "70",
    ///       "base_increment": "0.00000001",
    ///       "quote_increment": "0.01",
    ///       "display_name": "BTC/USD",
    ///       "status": "online",
    ///       "status_message": null,
    ///       "min_market_funds": "10",
    ///       "max_market_funds": "1000000",
    ///       "post_only": false,
    ///       "limit_only": false,
    ///       "cancel_only": false,
    ///       "fx_stablecoin": false
    ///     }
    ///   ],
    ///   "currencies": [
    ///     {
    ///       "id": "USD",
    ///       "name": "United States Dollar",
    ///       "min_size": "0.01000000",
    ///       "status": "online",
    ///       "status_message": null,
    ///       "max_precision": "0.01",
    ///       "convertible_to": ["USDC"],
    ///       "details": {}
    ///     },
    ///     {
    ///       "id": "USDC",
    ///       "name": "USD Coin",
    ///       "min_size": "0.00000100",
    ///       "status": "online",
    ///       "status_message": null,
    ///       "max_precision": "0.000001",
    ///       "convertible_to": ["USD"],
    ///       "details": {}
    ///     },
    ///     {
    ///       "id": "BTC",
    ///       "name": "Bitcoin",
    ///       "min_size":" 0.00000001",
    ///       "status": "online",
    ///       "status_message": null,
    ///       "max_precision": "0.00000001",
    ///       "convertible_to": [],
    ///       "details": {}
    ///     }
    ///   ]
    /// }
    /// ```
    Status {
        products: Vec<Product>,
        currencies: Vec<Currency>,
    },

    /// The `ticker` channel provides real-time price updates every time a match happens. It batches updates in case of cascading matches, greatly reducing bandwidth requirements.
    /// ```json
    /// // Ticker messsage
    /// {
    ///    "type":"ticker",
    ///    "sequence":29912240,
    ///    "product_id":"BTC-USD",
    ///    "price":"40552.26",
    ///    "open_24h":"40552.26",
    ///    "volume_24h":"0.43526841",
    ///    "low_24h":"40552.26",
    ///    "high_24h":"40662.06",
    ///    "volume_30d":"160.65999711",
    ///    "best_bid":"40552.26",
    ///    "best_ask":"40553.84",
    ///    "side":"sell",
    ///    "time":"2022-03-16T18:42:08.145773Z",
    ///    "trade_id":131414,
    ///    "last_size":"0.00002465"
    /// }
    /// ```
    Ticker {
        sequence: usize, // 29912240,
        product_id: String, // "BTC-USD",
        price: Decimal, // 40552.26",
        open_24h: Decimal, // 40552.26",
        volume_24h: Decimal, // 0.43526841",
        low_24h: Decimal, // 40552.26",
        high_24h: Decimal, // 40662.06",
        volume_30d: Decimal, // 160.65999711",
        best_bid: Decimal, // 40552.26",
        best_ask: Decimal, // 40553.84",
        side: Side, // "sell",

        #[serde(with = "timestamp")]
        time: DateTime<Utc>, // "2022-03-16T18:42:08.145773Z",
        trade_id: usize, // 131414,
        last_size: Decimal, // "0.00002465"
    },

    /// The level2 channel sends a message with the type `snapshot` and the corresponding `product_id`. The properties `bids` and `asks` are arrays of `[price, size]` tuples and represent the entire order book.
    /// ```json
    /// {
    ///   "type": "snapshot",
    ///   "product_id": "BTC-USD",
    ///   "bids": [["10101.10", "0.45054140"]],
    ///   "asks": [["10102.55", "0.57753524"]]
    /// }
    /// ```
    Snapshot {
        product_id: String, // "BTC-USD",
        bids: Vec<Level>,
        asks: Vec<Level>,
    },

    /// Subsequent updates have the type `l2update`. The changes property of `l2updates` is an array with `[side, price, size]` tuples. The `time` property of `l2update` is the time of the event as recorded by our trading engine.
    ///
    /// ```json
    /// {
    ///   "type": "l2update",
    ///   "product_id": "BTC-USD",
    ///   "time": "2019-08-14T20:42:27.265Z",
    ///   "changes": [
    ///     [
    ///       "buy",
    ///       "10101.80000000",
    ///       "0.162567"
    ///     ]
    ///   ]
    /// }
    /// ```
    #[serde(rename = "l2update")]
    L2Update {
        product_id: String, // "BTC-USD",
        #[serde(with = "timestamp")]
        time: DateTime<Utc>, // "2019-08-14T20:42:27.265Z",
        changes: Vec<Change>
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Level {
    price: Decimal,
    amount: Decimal,
}

impl ToLevel for Level {
    /// Converts a `coinbase::Level` into a `orderbook::Level`.
    fn to_level(&self, side: orderbook::Side) -> orderbook::Level {
        orderbook::Level::new(side, self.price, self.amount, Exchange::Coinbase)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
struct Change {
    side: Side,
    price: Decimal,
    amount: Decimal,
}

impl ToLevel for Change {
    /// Converts a `coinbase::Change` into a `orderbook::Level`.
    fn to_level(&self, side: orderbook::Side) -> orderbook::Level {
        orderbook::Level::new(side, self.price, self.amount, Exchange::Coinbase)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
enum Side {
    Buy,
    Sell,
}

/// Specifying Product IDs
///
/// There are two ways to specify the product IDs to listen for within each channel:
///
/// * You can define product IDs for an individual channel.
///
/// * You can define product IDs at the root of the object—this adds them to all the channels you subscribe to.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum Channel {
    /// Product IDs defined at the root of the object—this adds them to all the channels you subscribe to.
    Channel(String),

    /// Product IDs defined for an individual channel.
    Config(ChannelConfig),
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct ChannelConfig {
    name: String,
    product_ids: Vec<String>,
}

/// ```json
///     {
///       "id": "BTC-USD",
///       "base_currency": "BTC",
///       "quote_currency": "USD",
///       "base_min_size": "0.001",
///       "base_max_size": "70",
///       "base_increment": "0.00000001",
///       "quote_increment": "0.01",
///       "display_name": "BTC/USD",
///       "status": "online",
///       "status_message": null,
///       "min_market_funds": "10",
///       "max_market_funds": "1000000",
///       "post_only": false,
///       "limit_only": false,
///       "cancel_only": false,
///       "fx_stablecoin": false
///     }
/// ```
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Product {
    id: String,
    base_currency: String, // "BTC"
    quote_currency: String, // "USD"
    base_min_size: Decimal, // "0.001"
    base_max_size: Decimal, // "70"
    base_increment: Decimal, // "0.00000001"
    quote_increment: Decimal, // "0.01"
    display_name: String, // "BTC/USD"
    status: String, // "online"
    status_message: Option<String>, // null
    min_market_funds: Decimal, // "10"
    max_market_funds: Decimal, // "1000000"
    post_only: bool, // false
    limit_only: bool, // false
    cancel_only: bool, // false
    fx_stablecoin: bool, // false
}

/// ```json
///     {
///       "id": "USD",
///       "name": "United States Dollar",
///       "min_size": "0.01000000",
///       "status": "online",
///       "status_message": null,
///       "max_precision": "0.01",
///       "convertible_to": ["USDC"],
///       "details": {}
///     }
/// ```
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Currency {
    id: String, // "USD"
    name: String, // "United States Dollar"
    min_size: Decimal, // "0.01000000"
    status: String, // "online"
    status_message: Option<String>, // null
    max_precision: Decimal, // "0.01"
    convertible_to: Vec<String>, // ["USDC"]
    details: CurrencyDetails, // {}
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct CurrencyDetails {}

impl ToTick for Event {
    /// Converts the `Event` into a `Option<InTick>`. Only keep the top ten levels of bids and asks.
    fn maybe_to_tick(&self) -> Option<InTick> {
        match self {
            Event::Snapshot { bids, asks, .. } => {
                let bids = bids.to_levels(orderbook::Side::Bid, 10);
                let asks = asks.to_levels(orderbook::Side::Ask, 10);

                Some(InTick { exchange: Exchange::Coinbase, bids, asks })
            }
            Event::L2Update { changes, .. } => {
                let bids = changes.iter()
                    .filter(|c| c.side == Side::Buy)
                    .cloned().collect::<Vec<Change>>()
                    .to_levels(orderbook::Side::Bid, 10);
                let asks = changes.iter()
                    .filter(|c| c.side == Side::Sell)
                    .cloned().collect::<Vec<Change>>()
                    .to_levels(orderbook::Side::Ask, 10);

                Some(InTick { exchange: Exchange::Coinbase, bids, asks })
            }
            _ => None
        }
    }
}

pub(crate) async fn connect(symbol: &String) -> Result<websocket::WsStream, Error> {
    let mut ws_stream = websocket::connect(COINBASE_WS_URL).await?;
    subscribe(&mut ws_stream, symbol).await?;
    Ok(ws_stream)
}

async fn subscribe (
    rx: &mut websocket::WsStream,
    symbol: &String,
) -> Result<(), Error>
{
    let symbol = symbol.to_uppercase().replace("/", "-");
    let sub = Event::Subscribe{
        product_ids: Some(vec![ symbol ]),
        channels: vec![
            Channel::Channel("level2".to_string()),
            Channel::Channel("heartbeat".to_string()),
        ]
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

            let e= deserialize(x)?;
            match e {
                Event::Ticker { .. } => debug!("{:?}", e),
                Event::Snapshot { .. } => debug!("{:?}", e),
                Event::L2Update { .. } => debug!("{:?}", e),
                _ => info!("{:?}", e),
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

fn deserialize(s: String) -> serde_json::Result<Event> {
    Ok(serde_json::from_str(&s)?)
}

fn serialize(e: Event) -> serde_json::Result<String> {
    Ok(serde_json::to_string(&e)?)
}

mod timestamp {
    use chrono::{DateTime, Utc, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_str(date.to_rfc3339().as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let datetime = Utc.datetime_from_str(&s, "%+").map_err(serde::de::Error::custom)?;  // "2014-11-07T08:19:28.464459Z"
        Ok(datetime)
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use rust_decimal_macros::dec;
    use crate::coinbase::*;

    #[test]
    fn should_deserialize_subscriptions() -> Result<(), Error> {
        assert_eq!(deserialize(r#"
        {
            "type": "subscriptions",
            "channels": [
                {
                    "name": "level2",
                    "product_ids": [
                        "ETH-USD",
                        "ETH-EUR"
                    ]
                },
                {
                    "name": "heartbeat",
                    "product_ids": [
                        "ETH-USD",
                        "ETH-EUR"
                    ]
                },
                {
                    "name": "ticker",
                    "product_ids": [
                        "ETH-USD",
                        "ETH-EUR",
                        "ETH-BTC"
                    ]
                }
            ]
        }"#.to_string())?,
                   Event::Subscriptions{
                       channels: vec![
                           Channel::Config(ChannelConfig{
                               name: "level2".to_string(),
                               product_ids: vec![
                                   "ETH-USD".to_string(),
                                   "ETH-EUR".to_string(),
                               ]
                           }),
                           Channel::Config(ChannelConfig{
                               name: "heartbeat".to_string(),
                               product_ids: vec![
                                   "ETH-USD".to_string(),
                                   "ETH-EUR".to_string(),
                               ]
                           }),
                           Channel::Config(ChannelConfig{
                               name: "ticker".to_string(),
                               product_ids: vec![
                                   "ETH-USD".to_string(),
                                   "ETH-EUR".to_string(),
                                   "ETH-BTC".to_string(),
                               ]
                           }),
                       ],
                   }
        );
        Ok(())
    }

    #[test]
    fn should_deserialize_heartbeat() -> Result<(), Error> {
        assert_eq!(deserialize(r#"
        {
            "type": "heartbeat",
            "sequence": 90,
            "last_trade_id": 20,
            "product_id": "BTC-USD",
            "time": "2014-11-07T08:19:28.464459Z"
        }"#.to_string())?,
                   Event::Heartbeat{
                       sequence: 90,
                       last_trade_id: 20,
                       product_id: "BTC-USD".to_string(),
                       time: DateTime::from_str("2014-11-07T08:19:28.464459Z").unwrap(),
                   }
        );
        Ok(())
    }

    #[test]
    fn should_deserialize_snapshot() -> Result<(), Error> {
        assert_eq!(deserialize(r#"
        {
            "type": "snapshot",
            "product_id": "BTC-USD",
            "bids": [["10101.10", "0.45054140"]],
            "asks": [["10102.55", "0.57753524"]]
        }"#.to_string())?,
                   Event::Snapshot{
                       product_id: "BTC-USD".to_string(),
                       bids: vec![
                           Level{price: dec!(10101.10), amount: dec!(0.45054140), },
                       ],
                       asks: vec![
                           Level{price: dec!(10102.55), amount: dec!(0.57753524), },
                       ]
                   }
        );
        Ok(())
    }

    #[test]
    fn should_deserialize_l2udpate() -> Result<(), Error> {
        assert_eq!(deserialize(r#"
        {
            "type": "l2update",
            "product_id": "BTC-USD",
            "time": "2019-08-14T20:42:27.265Z",
            "changes": [
                [
                    "buy",
                    "10101.80000000",
                    "0.162567"
                ]
            ]
        }"#.to_string())?,
                   Event::L2Update {
                       product_id: "BTC-USD".to_string(),
                       time: DateTime::from_str("2019-08-14T20:42:27.265Z").unwrap(),
                       changes: vec![
                           Change{
                               side: Side::Buy,
                               price: dec!(10101.80000000),
                               amount: dec!(0.162567),
                           }
                       ]
                   }
        );
        Ok(())
    }

    #[test]
    fn should_serialize() -> Result<(), Error> {
        let mut serialized = r#"
        {
            "type": "subscribe",
            "product_ids": [
                "ETH-USD",
                "ETH-EUR"
            ],
            "channels": [
                "level2",
                "heartbeat",
                {
                    "name": "ticker",
                    "product_ids": [
                        "ETH-BTC",
                        "ETH-USD"
                    ]
                }
            ]
        }
        "#.to_string();
        serialized.retain(|c| !c.is_whitespace());

        assert_eq!(serialize(Event::Subscribe{
            product_ids: Some(vec![
                "ETH-USD".to_string(),
                "ETH-EUR".to_string(),
            ]),
            channels: vec![
                Channel::Channel("level2".to_string()),
                Channel::Channel("heartbeat".to_string()),
                Channel::Config(ChannelConfig{
                    name: "ticker".to_string(),
                    product_ids: vec![
                        "ETH-BTC".to_string(),
                        "ETH-USD".to_string(),
                    ],
                }),
            ]
        })?, serialized);

        Ok(())
    }

    #[test]
    fn should_convert_to_tick() -> Result<(), Error> {
        /*
         * Given
         */
        let e = Event::Snapshot {
            product_id: "BTC-USD".to_string(),
            bids: vec![
                Level { price: dec!(0.067990), amount: dec!(29.35934962) },
                Level { price: dec!(0.067980), amount: dec!(48.72763614) },
                Level { price: dec!(0.067970), amount: dec!(25.55979457) },
                Level { price: dec!(0.067960), amount: dec!(48.91046225) },
                Level { price: dec!(0.067950), amount: dec!(17.83261805) },
                Level { price: dec!(0.067930), amount: dec!(2.11301052) },
                Level { price: dec!(0.067920), amount: dec!(48.92972805) },
                Level { price: dec!(0.067900), amount: dec!(53.93281284) },
                Level { price: dec!(0.067880), amount: dec!(15.00000000) },
                Level { price: dec!(0.067870), amount: dec!(2.84944758) },
            ],
            asks: vec![
                Level { price: dec!(0.068010), amount: dec!(2.61547960) },
                Level { price: dec!(0.068020), amount: dec!(2.80351225) },
                Level { price: dec!(0.068040), amount: dec!(24.45938572) },
                Level { price: dec!(0.068050), amount: dec!(24.45938596) },
                Level { price: dec!(0.068060), amount: dec!(14.63500000) },
                Level { price: dec!(0.068070), amount: dec!(48.92440377) },
                Level { price: dec!(0.068080), amount: dec!(4.00000000) },
                Level { price: dec!(0.068090), amount: dec!(50.90608702) },
                Level { price: dec!(0.068110), amount: dec!(18.43030000) },
                Level { price: dec!(0.068120), amount: dec!(59.24322805) },
            ],
        };

        /*
         * When
         */
        let tick = e.maybe_to_tick();

        /*
         * Then
         */
        assert_eq!(tick, Some(InTick{
            exchange: Exchange::Coinbase,
            bids: vec![
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067990), dec!(29.35934962), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067980), dec!(48.72763614), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067970), dec!(25.55979457), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067960), dec!(48.91046225), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067950), dec!(17.83261805), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067930), dec!(2.11301052), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067920), dec!(48.92972805), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067900), dec!(53.93281284), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067880), dec!(15.00000000), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Bid, dec!(0.067870), dec!(2.84944758), Exchange::Coinbase),
            ],
            asks: vec![
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068010), dec!(2.61547960), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068020), dec!(2.80351225), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068040), dec!(24.45938572), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068050), dec!(24.45938596), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068060), dec!(14.63500000), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068070), dec!(48.92440377), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068080), dec!(4.00000000), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068090), dec!(50.90608702), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068110), dec!(18.43030000), Exchange::Coinbase),
                orderbook::Level::new(orderbook::Side::Ask, dec!(0.068120), dec!(59.24322805), Exchange::Coinbase),
            ],
        }));

        Ok(())
    }
}