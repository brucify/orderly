use orderly::connector;

#[tokio::main]
async fn main() {
    connector::run().await;
}

