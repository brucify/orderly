use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug)]
pub(crate) struct Tick {
    pub(crate) exchange: String,
    pub(crate) channel: String,
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) microtimestamp: DateTime<Utc>,
    pub(crate) bids: Vec<Bid>,
    pub(crate) asks: Vec<Ask>,
}

pub(crate) trait ToTick {
    fn maybe_to_tick(&self) -> Option<Tick>;
}

pub(crate) type Bid = Vec<Decimal>;
pub(crate) type Ask = Vec<Decimal>;