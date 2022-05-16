use crate::error::Error;
use crate::orderbook::OrderBook;
use log::info;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};

pub mod proto {
    tonic::include_proto!("orderbook");
}

pub struct OrderBookService {
    order_book: Arc<RwLock<OrderBook>>
}

impl OrderBookService {
    pub(crate) fn new(order_book: Arc<RwLock<OrderBook>>) -> OrderBookService {
        OrderBookService { order_book }
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
}

#[tonic::async_trait]
impl proto::order_book_server::OrderBook for OrderBookService {
    async fn check(
        &self,
        request: Request<proto::OrderBookCheckRequest>
    ) -> Result<Response<proto::OrderBookCheckResponse>, Status> {
        info!("Got a request: {:?}", request);

        let _req = request.into_inner();

        let order_book = self.order_book.read().await;

        let spread = order_book.spread.to_string();

        let bids = order_book.bids.iter()
            .map(|b| proto::Bid{
                price: b.price.to_string(),
                amount: b.amount.to_string(),
                exchange: b.exchange.to_string(),
            })
            .collect::<Vec<proto::Bid>>();

        let asks = order_book.asks.iter()
            .map(|a| proto::Ask{
                price: a.price.to_string(),
                amount: a.amount.to_string(),
                exchange: a.exchange.to_string(),
            })
            .collect::<Vec<proto::Ask>>();

        let reply = proto::OrderBookCheckResponse{ spread, bids, asks };

        Ok(Response::new(reply))
    }
}
