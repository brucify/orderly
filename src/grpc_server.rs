use crate::error::Error;
use crate::orderbook::OrderBook;
use events::orderly_server::{Orderly, OrderlyServer};
use events::{OrderlyOrderBookRequest, OrderlyOrderBookResponse, Bid, Ask};
use log::info;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub mod events {
    tonic::include_proto!("events");
}

pub struct OrderlyService {
    order_book: Arc<RwLock<OrderBook>>
}

impl OrderlyService {
    pub(crate) fn new(order_book: Arc<RwLock<OrderBook>>) -> OrderlyService {
        OrderlyService{ order_book }
    }

    pub(crate) async fn serve(self) -> Result<(), Error>{
        let addr = "[::1]:50051".parse()?;

        info!("Serving grpc at {}", addr);

        Server::builder()
            .add_service(OrderlyServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait()]
impl Orderly for OrderlyService {
    async fn order_book(
        &self,
        request: Request<OrderlyOrderBookRequest>
    ) -> Result<Response<OrderlyOrderBookResponse>, Status> {
        info!("Got a request: {:?}", request);

        let _req = request.into_inner();

        let order_book = self.order_book.read().await;

        let spread = order_book.spread.to_string();

        let bids = order_book.bids.iter()
            .map(|b| Bid{
                price: b.price.to_string(),
                amount: b.amount.to_string(),
                exchange: b.exchange.to_string(),
            })
            .collect::<Vec<Bid>>();

        let asks = order_book.asks.iter()
            .map(|a| Ask{
                price: a.price.to_string(),
                amount: a.amount.to_string(),
                exchange: a.exchange.to_string(),
            })
            .collect::<Vec<Ask>>();

        let reply = OrderlyOrderBookResponse{ spread, bids, asks };

        Ok(Response::new(reply))
    }
}
