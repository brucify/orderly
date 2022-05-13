use chrono::{DateTime, Utc};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) struct Tick {
    pub(crate) exchange: Exchange,
    pub(crate) channel: String,
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) microtimestamp: DateTime<Utc>,
    pub(crate) bids: Vec<Bid>,
    pub(crate) asks: Vec<Ask>,
}

pub(crate) trait ToTick {
    fn maybe_to_tick(&self) -> Option<Tick>;
}

pub(crate) enum Exchange {
    Bitstamp,
    Binance,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub(crate) struct Bid {
    price: Decimal,
    amount: Decimal
}

impl Bid {
    pub(crate) fn new(price: Decimal, amount: Decimal) -> Bid {
        Bid{price, amount}
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub(crate) struct Ask {
    price: Decimal,
    amount: Decimal
}

impl Ask {
    pub(crate) fn new(price: Decimal, amount: Decimal) -> Ask {
        Ask{price, amount}
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct MergedOrderBook {
    bitstamp: OrderBook,
    binance: OrderBook,
}

impl MergedOrderBook {
    pub(crate) fn new() -> MergedOrderBook {
        MergedOrderBook {
            bitstamp: OrderBook::new(),
            binance: OrderBook::new(),
        }
    }

    /// Extracts the bids and asks from the `Tick`, then adds into its corresponding
    /// orderbook of the exchange.
    pub(crate) fn add(&mut self, t: Tick) {
        let bids = t.bids.iter()
            .map(|b| (b.price, b.amount))
            .collect::<BTreeMap<Decimal, Decimal>>();
        let asks = t.asks.iter()
            .map(|b| (b.price, b.amount))
            .collect::<BTreeMap<Decimal, Decimal>>();

        match t.exchange {
            Exchange::Bitstamp => {
                self.bitstamp.bids = bids;
                self.bitstamp.asks = asks;
            }
            Exchange::Binance => {
                self.binance.bids = bids;
                self.binance.asks = asks;
            }
        }
    }

    /// Returns a new `OrderBook` containing the merge bids and asks from both orderbooks.
    pub(crate) fn merged(&self) -> OrderBook {
        let mut bids = self.bitstamp.bids.clone();
        bids.merge_and_keep(self.binance.bids.clone(), 10);
        let mut asks = self.bitstamp.asks.clone();
        asks.merge_and_keep(self.binance.asks.clone(), 10);
        OrderBook{ bids, asks }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct OrderBook {
    pub(crate) bids: BTreeMap<Decimal, Decimal>,
    pub(crate) asks: BTreeMap<Decimal, Decimal>,
}

impl OrderBook {
    fn new() -> OrderBook {
        OrderBook{
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }
}

trait Merge {
    fn merge(
        &mut self,
        other: BTreeMap<Decimal, Decimal>,
    );

    fn merge_and_keep(
        &mut self,
        other: BTreeMap<Decimal, Decimal>,
        index: usize,
    );
}

impl Merge for BTreeMap<Decimal, Decimal> {
    /// Same as `BTreeMap::extend` but increments the value instead of replacing it
    /// in case of a duplicate key.
    fn merge(&mut self, other: BTreeMap<Decimal, Decimal>) {
        other.into_iter().for_each(move |(k, v)| {
            match self.get_mut(&k) {
                None => { self.insert(k, v); },
                Some(x) => *x += v, // increment instead of replace
            }
        });
    }

    /// Merges two `BTreeMap`. Returns everything before the given index.
    fn merge_and_keep(&mut self, other: BTreeMap<Decimal, Decimal>, i: usize) {
        self.merge(other);
        if self.len() > i {
            let key = self.keys().collect::<Vec<&Decimal>>()[i].clone();
            self.split_off(&key);
        }
    }
}

pub(crate) fn channel() -> (UnboundedSender<Tick>, UnboundedReceiver<Tick>) {
    futures::channel::mpsc::unbounded()
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use chrono::{DateTime, TimeZone, Utc};
    use rust_decimal_macros::dec;
    use crate::orderbook::{Ask, Bid, Exchange, MergedOrderBook, OrderBook, Tick};

    #[test]
    fn should_add_bitstamp_tick_to_empty() {
        /*
         * Given
         */
        let mut book = MergedOrderBook::new();
        let t = Tick{
            exchange: Exchange::Bitstamp,
            channel: "order_book_ethbtc".to_string(),
            timestamp: Utc.timestamp(1652103479, 0),
            microtimestamp: Utc.timestamp_nanos(1652103479857383000),
            bids: vec![
                Bid::new(dec!(0.07358322), dec!(0.46500000)),
                Bid::new(dec!(0.07357954), dec!(8.50000000)),
                Bid::new(dec!(0.07357942), dec!(0.46500000)),
                Bid::new(dec!(0.07357869), dec!(16.31857550)),
                Bid::new(dec!(0.07357533), dec!(2.17483368)),
                Bid::new(dec!(0.07354592), dec!(10.22442936)),
                Bid::new(dec!(0.07354227), dec!(4.34696532)),
                Bid::new(dec!(0.07352810), dec!(20.01159075)),
                Bid::new(dec!(0.07350019), dec!(21.73733228)),
                Bid::new(dec!(0.07348180), dec!(1.85000000)),
            ],
            asks: vec![
                Ask::new(dec!(0.07366569), dec!(0.46500000)),
                Ask::new(dec!(0.07368584), dec!(16.30832712)),
                Ask::new(dec!(0.07371456), dec!(2.17501178)),
                Ask::new(dec!(0.07373077), dec!(4.35024244)),
                Ask::new(dec!(0.07373618), dec!(8.50000000)),
                Ask::new(dec!(0.07374400), dec!(1.85000000)),
                Ask::new(dec!(0.07375536), dec!(11.31202728)),
                Ask::new(dec!(0.07375625), dec!(6.96131361)),
                Ask::new(dec!(0.07375736), dec!(0.00275804)),
                Ask::new(dec!(0.07377938), dec!(0.00275807)),
            ]
        };

        /*
         * When
         */
        book.add(t);

        /*
         * Then
         */
        assert_eq!(book, MergedOrderBook{
            bitstamp: OrderBook{
                bids: BTreeMap::from([
                    (dec!(0.07358322), dec!(0.46500000)),
                    (dec!(0.07357954), dec!(8.50000000)),
                    (dec!(0.07357942), dec!(0.46500000)),
                    (dec!(0.07357869), dec!(16.31857550)),
                    (dec!(0.07357533), dec!(2.17483368)),
                    (dec!(0.07354592), dec!(10.22442936)),
                    (dec!(0.07354227), dec!(4.34696532)),
                    (dec!(0.07352810), dec!(20.01159075)),
                    (dec!(0.07350019), dec!(21.73733228)),
                    (dec!(0.07348180), dec!(1.85000000)),
                ]),
                asks: BTreeMap::from([
                    (dec!(0.07366569), dec!(0.46500000)),
                    (dec!(0.07368584), dec!(16.30832712)),
                    (dec!(0.07371456), dec!(2.17501178)),
                    (dec!(0.07373077), dec!(4.35024244)),
                    (dec!(0.07373618), dec!(8.50000000)),
                    (dec!(0.07374400), dec!(1.85000000)),
                    (dec!(0.07375536), dec!(11.31202728)),
                    (dec!(0.07375625), dec!(6.96131361)),
                    (dec!(0.07375736), dec!(0.00275804)),
                    (dec!(0.07377938), dec!(0.00275807)),
                ]),
            },
            binance: OrderBook::new(),
        });
    }
}