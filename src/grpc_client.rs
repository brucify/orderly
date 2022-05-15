use events::OrderlyOrderBookRequest;
use events::orderly_client::OrderlyClient;
use log::info;

pub mod events {
    tonic::include_proto!("events");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut client = OrderlyClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(OrderlyOrderBookRequest {});

    let response = client.order_book(request).await?;

    info!("RESPONSE={:?}", response);

    Ok(())
}
