use clap::Parser;
use orderly::orderly;

/// Connects to WebSocket feeds of exchanges. Pulls order book for the given currency pair
/// Publishes a merged order book as a stream.
#[derive(Parser)]
struct Cli {
    #[clap(short, long, help = "Currency pair to subscribe to. Default: ethbtc")]
    symbol: Option<String>,

}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Cli::parse();
    let symbol: String = args.symbol.unwrap_or("ethbtc".to_string());

    orderly::run(&symbol).await.unwrap();
}

