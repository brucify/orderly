use log::info;
use proto::order_book_client::OrderBookClient;

pub mod proto {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut client = OrderBookClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(proto::OrderBookCheckRequest {});

    // let response = client.check(request).await?;
    // info!("RESPONSE={:?}", response);

    let mut response = client.watch(request).await?.into_inner();
    // listening to stream
    while let Some(res) = response.message().await? {
        info!("RESPONSE = {:?}", res);
    }

    Ok(())
}
