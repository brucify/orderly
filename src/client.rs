use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use proto::orderbook_aggregator_client::OrderbookAggregatorClient;

mod proto {
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

    println!(
        "Receiving updates from gRPC server...",
    );

    let mut response = client.book_summary(request).await?.into_inner();

    // setting up indicatif
    let m = MultiProgress::new();
    let spinner_style = ProgressStyle::default_spinner()
        .template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

    let bid0 = m.add(ProgressBar::new(100));
    let bid1 = m.add(ProgressBar::new(100));
    let bid2 = m.add(ProgressBar::new(100));
    let bid3 = m.add(ProgressBar::new(100));
    let bid4 = m.add(ProgressBar::new(100));
    let bid5 = m.add(ProgressBar::new(100));
    let bid6 = m.add(ProgressBar::new(100));
    let bid7 = m.add(ProgressBar::new(100));
    let bid8 = m.add(ProgressBar::new(100));
    let bid9 = m.add(ProgressBar::new(100));

    let pb_spread = m.add(ProgressBar::new(100));

    let ask0 = m.add(ProgressBar::new(100));
    let ask1 = m.add(ProgressBar::new(100));
    let ask2 = m.add(ProgressBar::new(100));
    let ask3 = m.add(ProgressBar::new(100));
    let ask4 = m.add(ProgressBar::new(100));
    let ask5 = m.add(ProgressBar::new(100));
    let ask6 = m.add(ProgressBar::new(100));
    let ask7 = m.add(ProgressBar::new(100));
    let ask8 = m.add(ProgressBar::new(100));
    let ask9 = m.add(ProgressBar::new(100));

    let pb_bids = vec![
        bid0, bid1, bid2, bid3, bid4,
        bid5, bid6, bid7, bid8, bid9
    ];
    let pb_asks = vec![
        ask0, ask1, ask2, ask3, ask4,
        ask5, ask6, ask7, ask8, ask9
    ];

    pb_spread.set_prefix(format!("[Spread]"));
    pb_spread.set_style(spinner_style.clone());
    pb_bids.iter()
        .enumerate()
        .for_each(|(i, pb)| {
            pb.set_prefix(format!("[Bid {}]", i.abs_diff(9)));
            pb.set_style(spinner_style.clone());
        });
    pb_asks.iter()
        .enumerate()
        .for_each(|(i, pb)| {
            pb.set_prefix(format!("[Ask {}]", i));
            pb.set_style(spinner_style.clone());
        });

    tokio::spawn(async move { let _ = m.join_and_clear(); });

    // listening to stream
    while let Some(res) = response.message().await? {
        let proto::Summary{spread, bids, asks} = res;

        bids.iter().rev()
            .enumerate()
            .for_each(|(i, b)| {
                pb_bids[i].set_message(format!("{:?}", b));
            });

        pb_spread.set_message(format!("{:?}", spread.to_string()));

        asks.iter()
            .enumerate()
            .for_each(|(i, a)| {
                pb_asks[i].set_message(format!("{:?}", a));
            });
    }

    Ok(())
}
