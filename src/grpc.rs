use crate::error::Error;
use crate::orderbook::OutTick;
use crate::orderly::OutTickPair;
use futures::Stream;
use log::info;
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
        let spread = out_tick.spread.to_string();

        let bids: Vec<proto::Level> = out_tick.bids.iter()
            .map(|b| proto::Level{
                price: b.price.to_string(),
                amount: b.amount.to_string(),
                exchange: b.exchange.to_string(),
            })
            .collect();

        let asks: Vec<proto::Level> = out_tick.asks.iter()
            .map(|a| proto::Level{
                price: a.price.to_string(),
                amount: a.amount.to_string(),
                exchange: a.exchange.to_string(),
            })
            .collect();

        proto::Summary{ spread, bids, asks }
    }
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
