use crate::error::Error;
use crate::orderbook::{self, OutTick};
use crate::orderly::OutTickPair;
use futures::Stream;
use log::info;
use rust_decimal::prelude::ToPrimitive;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};

pub mod proto {
    tonic::include_proto!("orderbook");
}

pub struct OrderBookService {
    out_ticks: Arc<RwLock<OutTickPair>>
}

impl OrderBookService {
    pub(crate) fn new(out_ticks: Arc<RwLock<OutTickPair>>) -> Self {
        OrderBookService { out_ticks }
    }

    pub(crate) async fn serve(self, port: usize) -> Result<(), Error>{
        let addr = format!("[::1]:{}", port);
        let addr = addr.parse()?;

        info!("Serving grpc at {}", addr);

        Server::builder()
            .add_service(proto::orderbook_aggregator_server::OrderbookAggregatorServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    async fn out_tick(&self) -> OutTick {
        let reader = self.out_ticks.read().await;
        let out_tick = reader.1.borrow().clone();
        out_tick
    }
}

impl From<OutTick> for proto::Summary {
    fn from(out_tick: OutTick) -> Self {
        let spread = out_tick.spread.to_f64().unwrap();
        let bids: Vec<proto::Level> = to_levels(&out_tick.bids);
        let asks: Vec<proto::Level> = to_levels(&out_tick.asks);

        proto::Summary{ spread, bids, asks }
    }
}

fn to_levels(levels: &Vec<orderbook::Level>) -> Vec<proto::Level> {
    levels.iter()
        .map(|l|
            proto::Level{
                exchange: l.exchange.to_string(),
                price: l.price.to_f64().unwrap(),
                amount: l.amount.to_f64().unwrap(),
            })
        .collect()
}

#[tonic::async_trait]
impl proto::orderbook_aggregator_server::OrderbookAggregator for OrderBookService {
    async fn check(
        &self,
        request: Request<proto::Empty>
    ) -> Result<Response<proto::Summary>, Status> {
        info!("Got a request: {:?}", request);

        let _req = request.into_inner();

        let out_tick = self.out_tick().await;

        let reply = proto::Summary::from(out_tick);

        Ok(Response::new(reply))
    }

    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<proto::Summary, Status>> + Send + 'static>>;

    async fn book_summary(
        &self,
        request: Request<proto::Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        info!("Got a request: {:?}", request);

        let _req = request.into_inner();

        let mut rx_out_ticks = self.out_ticks.read().await.1.clone();

        let output = async_stream::try_stream! {
            // yield the current value
            let out_tick = rx_out_ticks.borrow().clone();
            yield proto::Summary::from(out_tick);

            while let Ok(_) = rx_out_ticks.changed().await {
                let out_tick = rx_out_ticks.borrow().clone();
                yield proto::Summary::from(out_tick);
            }
        };

        Ok(Response::new(Box::pin(output) as Self::BookSummaryStream))
    }
}

#[cfg(test)]
mod test {
    use rust_decimal_macros::dec;
    use crate::grpc::proto;
    use crate::orderbook::{Exchange, Level, OutTick};

    #[test]
    fn should_convert_to_summary() {
        /*
         * Given
         */
        let out_tick = OutTick {
            spread: dec!(0.00000010), 
            bids: vec![
                Level { price: dec!(0.00018688), amount: dec!(610014.67000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018687), amount: dec!(2205276.09000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018686), amount: dec!(4959229.21000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018685), amount: dec!(13520849.56000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018683), amount: dec!(2697439.72000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018682), amount: dec!(1575744.75000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018681), amount: dec!(6302978.66000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018680), amount: dec!(5954547.05000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018679), amount: dec!(10776354.35000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018678), amount: dec!(15388083.16000000), exchange: Exchange::Binance },
            ],
            asks: vec![
                Level { price: dec!(0.00018698), amount: dec!(595429.87000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018699), amount: dec!(123707.71000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018700), amount: dec!(44033903.92000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018705), amount: dec!(4278646.87000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018706), amount: dec!(12777847.03000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018707), amount: dec!(11137472.05000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018708), amount: dec!(380833.80000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018710), amount: dec!(2938703.50000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018711), amount: dec!(73753.41000000), exchange: Exchange::Binance },
                Level { price: dec!(0.00018712), amount: dec!(566911.25000000), exchange: Exchange::Binance },
            ],
        };
        
        /*
         * When
         */
        let summary = proto::Summary::from(out_tick);

        /*
         * Then
         */
        assert_eq!(summary, proto::Summary{
            spread: 0.0000001,
            bids: vec![
                proto::Level { price: 0.00018688, amount: 610014.67, exchange: "binance".to_string() },
                proto::Level { price: 0.00018687, amount: 2205276.09, exchange: "binance".to_string() },
                proto::Level { price: 0.00018686, amount: 4959229.21, exchange: "binance".to_string() },
                proto::Level { price: 0.00018685, amount: 13520849.56, exchange: "binance".to_string() },
                proto::Level { price: 0.00018683, amount: 2697439.72, exchange: "binance".to_string() },
                proto::Level { price: 0.00018682, amount: 1575744.75, exchange: "binance".to_string() },
                proto::Level { price: 0.00018681, amount: 6302978.66, exchange: "binance".to_string() },
                proto::Level { price: 0.0001868, amount: 5954547.05, exchange: "binance".to_string() },
                proto::Level { price: 0.00018679, amount: 10776354.35, exchange: "binance".to_string() },
                proto::Level { price: 0.00018678, amount: 15388083.16, exchange: "binance".to_string() },
            ],
            asks: vec![
                proto::Level { price: 0.00018698, amount: 595429.87, exchange: "binance".to_string() },
                proto::Level { price: 0.00018699, amount: 123707.71, exchange: "binance".to_string() },
                proto::Level { price: 0.000187, amount: 44033903.92, exchange: "binance".to_string() },
                proto::Level { price: 0.00018705, amount: 4278646.87, exchange: "binance".to_string() },
                proto::Level { price: 0.00018706, amount: 12777847.03, exchange: "binance".to_string() },
                proto::Level { price: 0.00018707, amount: 11137472.05, exchange: "binance".to_string() },
                proto::Level { price: 0.00018708, amount: 380833.80, exchange: "binance".to_string() },
                proto::Level { price: 0.0001871, amount: 2938703.50, exchange: "binance".to_string() },
                proto::Level { price: 0.00018711, amount: 73753.41, exchange: "binance".to_string() },
                proto::Level { price: 0.00018712, amount: 566911.25, exchange: "binance".to_string() },
            ],
        });
    }
}