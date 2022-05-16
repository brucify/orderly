use crate::error::Error;
use crate::orderbook::OutTick;
use crate::orderly::OutTickPair;
use log::info;
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

    pub(crate) async fn serve(self) -> Result<(), Error>{
        let addr = "[::1]:50051".parse()?;

        info!("Serving grpc at {}", addr);

        Server::builder()
            .add_service(proto::order_book_server::OrderBookServer::new(self))
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

impl From<OutTick> for proto::OrderBookCheckResponse {
    fn from(out_tick: OutTick) -> Self {
        let spread = out_tick.spread.to_string();

        let bids: Vec<proto::Bid> = out_tick.bids.iter()
            .map(|b| proto::Bid{
                price: b.price.to_string(),
                amount: b.amount.to_string(),
                exchange: b.exchange.to_string(),
            })
            .collect();

        let asks: Vec<proto::Ask> = out_tick.asks.iter()
            .map(|a| proto::Ask{
                price: a.price.to_string(),
                amount: a.amount.to_string(),
                exchange: a.exchange.to_string(),
            })
            .collect();

        proto::OrderBookCheckResponse{ spread, bids, asks }
    }
}

#[tonic::async_trait]
impl proto::order_book_server::OrderBook for OrderBookService {
    async fn check(
        &self,
        request: Request<proto::OrderBookCheckRequest>
    ) -> Result<Response<proto::OrderBookCheckResponse>, Status> {
        info!("Got a request: {:?}", request);

        let _req = request.into_inner();

        let out_tick = self.out_tick().await;

        let reply = proto::OrderBookCheckResponse::from(out_tick);

        Ok(Response::new(reply))
    }
}
