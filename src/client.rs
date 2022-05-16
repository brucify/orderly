use clap::Parser;
use log::info;
use proto::orderbook_aggregator_client::OrderbookAggregatorClient;

pub mod proto {
    tonic::include_proto!("orderbook");
}

/// Connects to the gRPC server and streams the orderbook summary.
#[derive(Parser)]
struct Cli {
    #[clap(short, long, help = "(Optional) Port number of the gRPC server. Default: 50051")]
    port: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Cli::parse();
    let port: usize = args.port.unwrap_or(50051);
    let addr = format!("http://[::1]:{}", port);

    let mut client = OrderbookAggregatorClient::connect(addr).await?;

    let request = tonic::Request::new(proto::Empty {});

    // let response = client.check(request).await?;
    // info!("{:?}", response);

    let mut response = client.book_summary(request).await?.into_inner();
    // listening to stream
    while let Some(res) = response.message().await? {
        info!("{:?}", res);
    }

    Ok(())
}
