use std::cmp::Ordering;
use std::collections::BTreeMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[derive(Debug, PartialEq)]
pub(crate) struct InTick {
    pub(crate) exchange: Exchange,
    pub(crate) bids: Vec<Level>,
    pub(crate) asks: Vec<Level>,
}

pub(crate) trait ToTick {
    fn maybe_to_tick(&self) -> Option<InTick>;
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct OutTick {
    pub(crate) spread: Decimal,
    pub(crate) bids: Vec<Level>,
    pub(crate) asks: Vec<Level>,
}

impl OutTick {
    pub(crate) fn new() -> OutTick {
        OutTick {
            spread: Default::default(),
            bids: vec![],
            asks: vec![],
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Exchange {
    Bitstamp,
    Binance,
    Kraken,
    Coinbase,
}

impl ToString for Exchange {
    fn to_string(&self) -> String {
        match self {
            Exchange::Bitstamp => "bitstamp".to_string(),
            Exchange::Binance => "binance".to_string(),
            Exchange::Kraken => "kraken".to_string(),
            Exchange::Coinbase => "coinbase".to_string(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Level {
    pub(crate) side: Side,
    pub(crate) price: Decimal,
    pub(crate) amount: Decimal,
    pub(crate) exchange: Exchange,
}

impl Level {
    pub(crate) fn new(side: Side, price: Decimal, amount: Decimal, exchange: Exchange) -> Level {
        Level{side, price, amount, exchange}
    }
}

impl Ord for Level {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.price.cmp(&other.price), &self.side) {
            (Ordering::Equal, Side::Bid) => self.amount.cmp(&other.amount),
            (Ordering::Equal, Side::Ask) => self.amount.cmp(&other.amount).reverse(),
            (ord, _) => ord,
        }
    }
}

impl PartialOrd for Level {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.price.partial_cmp(&other.price), &self.side) {
            (Some(Ordering::Equal), Side::Bid) => self.amount.partial_cmp(&other.amount),
            (Some(Ordering::Equal), Side::Ask) => self.amount.partial_cmp(&other.amount).map(Ordering::reverse),
            (ord, _) => ord,
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Side {
    Bid,
    Ask,
}

pub(crate) trait ToLevel {
    fn to_level(&self, side: Side) -> Level;
}

pub(crate) trait ToLevels {
    fn to_levels(&self, side: Side, depth: usize) -> Vec<Level>;
}

impl<T> ToLevels for Vec<T>
    where T: ToLevel + Clone
{
    fn to_levels(&self, side: Side, depth: usize) -> Vec<Level> {
        let levels = match self.len() > depth {
            true => self.split_at(depth).0.to_vec(), // only keep 10
            false => self.clone(),
        };

        levels.into_iter()
            .map(|l| l.to_level(side.clone()))
            .collect()
    }
}

trait Merge {
    fn merge(self, other: Vec<Level>) -> Vec<Level>;
    fn merge_map(self, other: LevelsMap) -> Vec<Level>;
}

impl Merge for Vec<Level> {
    fn merge(self, other: Vec<Level>) -> Vec<Level> {
        let mut levels: Vec<Level> =
            self.into_iter()
                .chain(other)
                .collect();
        levels.sort_unstable();
        levels
    }

    fn merge_map(self, other: LevelsMap) -> Vec<Level> {
        let levels: Vec<Level> = other.values().cloned().collect();
        self.merge(levels)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Exchanges {
    bitstamp: OrderDepths,
    binance: OrderDepths,
    kraken: OrderDepthsMap,
    coinbase: OrderDepthsMap,
}

impl Exchanges {
    pub(crate) fn new() -> Exchanges {
        Exchanges {
            bitstamp: OrderDepths::new(),
            binance: OrderDepths::new(),
            kraken: OrderDepthsMap::new(),
            coinbase: OrderDepthsMap::new(),
        }
    }

    /// Extracts the bids and asks from the `InTick`, then adds into its corresponding
    /// orderbook of the exchange.
    pub(crate) fn update(&mut self, t: InTick) {
        match t.exchange {
            Exchange::Bitstamp => {
                self.bitstamp.bids = t.bids;
                self.bitstamp.asks = t.asks;
            },
            Exchange::Binance => {
                self.binance.bids = t.bids;
                self.binance.asks = t.asks;
            },
            Exchange::Kraken => {
                let bids = t.bids.into_iter()
                    .map(|l| (l.price, l))
                    .collect::<LevelsMap>();
                let asks = t.asks.into_iter()
                    .map(|l| (l.price, l))
                    .collect::<LevelsMap>();

                self.kraken.bids.extend_and_keep(bids, 10);
                self.kraken.asks.extend_and_keep(asks, 10);
            },
            Exchange::Coinbase => {
                let bids = t.bids.into_iter()
                    .map(|l| (l.price, l))
                    .collect::<LevelsMap>();
                let asks = t.asks.into_iter()
                    .map(|l| (l.price, l))
                    .collect::<LevelsMap>();

                self.coinbase.bids.extend_and_keep(bids, 10);
                self.coinbase.asks.extend_and_keep(asks, 10);
            }
        }
    }

    /// Returns a new `OutTick` containing the merge bids and asks from both orderbooks.
    pub(crate) fn to_tick(&self) -> OutTick {
        let bids: Vec<Level> =
            self.bitstamp.bids.clone()
                .merge(self.binance.bids.clone())
                .merge_map(self.kraken.bids.clone())
                .merge_map(self.coinbase.bids.clone())
                .into_iter().rev().take(10)
                .collect();

        let asks: Vec<Level> =
            self.bitstamp.asks.clone()
                .merge(self.binance.asks.clone())
                .merge_map(self.kraken.asks.clone())
                .merge_map(self.coinbase.asks.clone())
                .into_iter().take(10)
                .collect();

        let spread = match (bids.first(), asks.first()) {
            (Some(b), Some(a)) => a.price - b.price,
            (_, _) => dec!(0),
        };

        OutTick { spread, bids, asks }
    }
}

#[derive(Debug, PartialEq)]
struct OrderDepths {
    bids: Vec<Level>,
    asks: Vec<Level>,
}

impl OrderDepths {
    fn new() -> Self {
        OrderDepths {
            bids: vec![],
            asks: vec![],
        }
    }
}

type LevelsMap = BTreeMap<Decimal, Level>;

#[derive(Debug, PartialEq)]
struct OrderDepthsMap {
    bids: LevelsMap,
    asks: LevelsMap,
}

impl OrderDepthsMap {
    fn new() -> Self {
        OrderDepthsMap {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }
}

trait ExtendAndKeep {
    fn extend_and_keep(
        &mut self,
        other: LevelsMap,
        index: usize,
    );
}

impl ExtendAndKeep for LevelsMap {
    /// Merges two `BTreeMap`. Returns everything before the given index.
    fn extend_and_keep(&mut self, other: LevelsMap, i: usize) {
        self.extend(other);
        self.retain(|_k, v| !v.amount.eq(&dec!(0))); // remove where volume is 0
        if self.len() > i {
            let key = self.keys().collect::<Vec<&Decimal>>()[i].clone();
            self.split_off(&key);
        }
    }
}

#[cfg(test)]
mod test {
    use crate::orderbook::*;
    use rust_decimal_macros::dec;

    #[test]
    fn should_add_bitstamp_tick_to_empty() {
        /*
         * Given
         */
        let mut exchanges = Exchanges::new();
        let t = InTick {
            exchange: Exchange::Bitstamp,
            bids: vec![
                Level::new(Side::Bid, dec!(0.07358322), dec!(0.46500000), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07357954), dec!(8.50000000), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07357942), dec!(0.46500000), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07357869), dec!(16.31857550), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07357533), dec!(2.17483368), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07354592), dec!(10.22442936), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07354227), dec!(4.34696532), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07352810), dec!(20.01159075), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07350019), dec!(21.73733228), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(0.07348180), dec!(1.85000000), Exchange::Bitstamp),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(0.07366569), dec!(0.46500000), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07368584), dec!(16.30832712), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07371456), dec!(2.17501178), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07373077), dec!(4.35024244), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07373618), dec!(8.50000000), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07374400), dec!(1.85000000), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07375536), dec!(11.31202728), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07375625), dec!(6.96131361), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07375736), dec!(0.00275804), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(0.07377938), dec!(0.00275807), Exchange::Bitstamp),
            ],
        };

        /*
         * When
         */
        exchanges.update(t);

        /*
         * Then
         */
        assert_eq!(exchanges, Exchanges {
            bitstamp: OrderDepths {
                bids: vec![
                    Level::new(Side::Bid, dec!(0.07358322), dec!(0.46500000), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07357954), dec!(8.50000000), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07357942), dec!(0.46500000), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07357869), dec!(16.31857550), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07357533), dec!(2.17483368), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07354592), dec!(10.22442936), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07354227), dec!(4.34696532), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07352810), dec!(20.01159075), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07350019), dec!(21.73733228), Exchange::Bitstamp),
                    Level::new(Side::Bid, dec!(0.07348180), dec!(1.85000000), Exchange::Bitstamp),
                ],
                asks: vec![
                    Level::new(Side::Ask, dec!(0.07366569), dec!(0.46500000), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07368584), dec!(16.30832712), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07371456), dec!(2.17501178), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07373077), dec!(4.35024244), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07373618), dec!(8.50000000), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07374400), dec!(1.85000000), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07375536), dec!(11.31202728), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07375625), dec!(6.96131361), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07375736), dec!(0.00275804), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(0.07377938), dec!(0.00275807), Exchange::Bitstamp),
                ],
            },
            binance: OrderDepths::new(),
            kraken: OrderDepthsMap::new(),
            coinbase: OrderDepthsMap::new(),
        });
    }

    #[test]
    fn should_merge() {
        /*
         * Given
         */
        let mut exchanges = Exchanges::new();
        let t1 = InTick {
            exchange: Exchange::Bitstamp,
            bids: vec![
                Level::new(Side::Bid, dec!(10), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(9), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(8), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(7), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(6), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(5), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(4), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(3), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(2), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(1), dec!(1), Exchange::Bitstamp),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(12), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(13), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(14), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(15), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(16), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(17), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(18), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(19), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(20), dec!(1), Exchange::Bitstamp),
            ],
        };
        let t2 = InTick {
            exchange: Exchange::Binance,
            bids: vec![
                Level::new(Side::Bid, dec!(10.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(9.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(8.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(7.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(6.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(5.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(4.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(3.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(2.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(1.5), dec!(2), Exchange::Binance),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(12.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(13.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(14.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(15.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(16.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(17.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(18.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(19.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(20.5), dec!(2), Exchange::Binance),
            ],
        };
        let t3 = InTick {
            exchange: Exchange::Kraken,
            bids: vec![
                Level::new(Side::Bid, dec!(10.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(9.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(8.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(7.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(6.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(5.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(4.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(3.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(2.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(1.75), dec!(3), Exchange::Kraken),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(12.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(13.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(14.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(15.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(16.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(17.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(18.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(19.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(20.75), dec!(3), Exchange::Kraken),
            ],
        };
        let t4 = InTick {
            exchange: Exchange::Coinbase,
            bids: vec![
                Level::new(Side::Bid, dec!(10.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(9.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(8.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(7.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(6.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(5.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(4.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(3.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(2.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(1.85), dec!(4), Exchange::Coinbase),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(12.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(13.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(14.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(15.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(16.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(17.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(18.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(19.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(20.85), dec!(4), Exchange::Coinbase),
            ],
        };
        exchanges.update(t1);
        exchanges.update(t2);
        exchanges.update(t3);
        exchanges.update(t4);

        /*
         * When
         */
        let out_tick = exchanges.to_tick();

        /*
         * Then
         */
        assert_eq!(out_tick, OutTick {
            spread: dec!(0.15),
            bids:vec![
                Level::new(Side::Bid, dec!(10.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(10.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(10.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(10), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(9.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(9.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(9.5), dec!(2), Exchange::Binance),
                Level::new(Side::Bid, dec!(9), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Bid, dec!(8.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Bid, dec!(8.75), dec!(3), Exchange::Kraken),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(11.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(11.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(11.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(12), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(12.5), dec!(2), Exchange::Binance),
                Level::new(Side::Ask, dec!(12.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(12.85), dec!(4), Exchange::Coinbase),
                Level::new(Side::Ask, dec!(13), dec!(1), Exchange::Bitstamp),
                Level::new(Side::Ask, dec!(13.5), dec!(2), Exchange::Binance),
            ],
        });
    }

    #[test]
    fn should_remove_kraken_volumes() {
        /*
         * Given
         */
        let mut exchanges = Exchanges::new();
        let t1 = InTick {
            exchange: Exchange::Kraken,
            bids: vec![
                Level::new(Side::Bid, dec!(10.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(9.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(8.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(7.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(6.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(5.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(4.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(3.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(2.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(1.75), dec!(3), Exchange::Kraken),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(12.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(13.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(14.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(15.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(16.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(17.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(18.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(19.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(20.75), dec!(3), Exchange::Kraken),
            ],
        };
        exchanges.update(t1);

        /*
         * When
         */
        let t2 = InTick {
            exchange: Exchange::Kraken,
            bids: vec![
                Level::new(Side::Bid, dec!(10.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Bid, dec!(9.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Bid, dec!(8.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Bid, dec!(7.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Bid, dec!(6.75), dec!(0), Exchange::Kraken),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Ask, dec!(12.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Ask, dec!(13.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Ask, dec!(14.75), dec!(0), Exchange::Kraken),
                Level::new(Side::Ask, dec!(15.75), dec!(0), Exchange::Kraken),
            ],
        };
        exchanges.update(t2);


        /*
         * Then
         */
        let out_tick = exchanges.to_tick();
        assert_eq!(out_tick, OutTick {
            spread: dec!(11),
            bids:vec![
                Level::new(Side::Bid, dec!(5.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(4.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(3.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(2.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Bid, dec!(1.75), dec!(3), Exchange::Kraken),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(16.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(17.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(18.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(19.75), dec!(3), Exchange::Kraken),
                Level::new(Side::Ask, dec!(20.75), dec!(3), Exchange::Kraken),
            ],
        });
    }

    #[test]
    fn should_merge_simple() {
        /*
         * Given
         */
        let mut exchanges = Exchanges::new();

        let t1 = InTick {
            exchange: Exchange::Bitstamp,
            bids: vec![
                Level::new(Side::Bid, dec!(10), dec!(1), Exchange::Bitstamp),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11), dec!(1), Exchange::Bitstamp),
            ],
        };
        let t2 = InTick {
            exchange: Exchange::Binance,
            bids: vec![
                Level::new(Side::Bid, dec!(10.5), dec!(2), Exchange::Binance),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.75), dec!(2), Exchange::Binance),
            ],
        };
        let t3 = InTick {
            exchange: Exchange::Kraken,
            bids: vec![
                Level::new(Side::Bid, dec!(10.5), dec!(3), Exchange::Kraken),
            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.75), dec!(3), Exchange::Kraken),
            ],
        };
        let t4 = InTick {
            exchange: Exchange::Coinbase,
            bids: vec![
                Level::new(Side::Bid, dec!(10.85), dec!(4), Exchange::Coinbase),

            ],
            asks: vec![
                Level::new(Side::Ask, dec!(11.85), dec!(4), Exchange::Coinbase),
            ],
        };
        exchanges.update(t1);
        exchanges.update(t2);
        exchanges.update(t3);
        exchanges.update(t4);

        /*
         * When
         */
        let out_tick = exchanges.to_tick();

        /*
         * Then
         */
        assert_eq!(
            out_tick,
            OutTick {
                spread: dec!(0.15),
                bids: vec![
                    Level::new(Side::Bid, dec!(10.85), dec!(4), Exchange::Coinbase),
                    Level::new(Side::Bid, dec!(10.5), dec!(3), Exchange::Kraken),
                    Level::new(Side::Bid, dec!(10.5), dec!(2), Exchange::Binance),
                    Level::new(Side::Bid, dec!(10), dec!(1), Exchange::Bitstamp),
                ],
                asks: vec![
                    Level::new(Side::Ask, dec!(11), dec!(1), Exchange::Bitstamp),
                    Level::new(Side::Ask, dec!(11.75), dec!(3), Exchange::Kraken),
                    Level::new(Side::Ask, dec!(11.75), dec!(2), Exchange::Binance),
                    Level::new(Side::Ask, dec!(11.85), dec!(4), Exchange::Coinbase),
                ],
            }
        );
    }
}