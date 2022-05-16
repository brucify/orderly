use proto::order_book_client::OrderBookClient;
use log::info;

pub mod proto {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut client = OrderBookClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(proto::OrderBookCheckRequest {});

    let response = client.check(request).await?;

    info!("RESPONSE={:?}", response);

    Ok(())
}
