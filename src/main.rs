use clap::Parser;
use orderly::orderly;

/// Pulls order depths for the given currency pair from the WebSocket feeds of multiple exchanges.
/// Publishes a merged order book as a gRPC stream.
#[derive(Parser)]
struct Cli {
    #[clap(short, long, help = "(Optional) Currency pair to subscribe to. Default: ETH/BTC")]
    symbol: Option<String>,

    #[clap(short, long, help = "(Optional) Port number on which the the gRPC server will be hosted. Default: 50051")]
    port: Option<usize>,

}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Cli::parse();
    let symbol: String = args.symbol.unwrap_or("ETH/BTC".to_string());
    let port: usize = args.port.unwrap_or(50051);

    orderly::run(&symbol, port).await.unwrap();
}

