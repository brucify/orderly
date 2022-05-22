
# orderly

A Rust CLI WebSocket client for crypto exchanges. 
Connects to the WebSocket feeds of multiple exchanges. 
Subscribes to the live order book for the given currency pair.
Publishes a merged order book as a gRPC stream.

<img src="https://user-images.githubusercontent.com/1086619/169712976-fa2639a3-f083-4a33-8edb-4aade0cf8ae6.gif" />

Currently supports: 

* Bitstamp WebSocket: `wss://ws.bitstamp.net`
* Binance WebSocket: `wss://stream.binance.com:9443/ws`
* Kraken WebSocket: `wss://ws.kraken.com`
* Coinbase WebSocket: `wss://ws-feed.exchange.coinbase.com`

```
USAGE:
    orderly-server [OPTIONS]

OPTIONS:
    -h, --help               Print help information
        --no-binance         (Optional) Don't show Binance in gRPC stream. Default: false
        --no-bitstamp        (Optional) Don't show Bitstamp in gRPC stream. Default: false
        --no-kraken          (Optional) Don't show Kraken in gRPC stream. Default: false
        --no-coinbase        (Optional) Don't show Coinbase in gRPC stream. Default: false
    -p, --port <PORT>        (Optional) Port number on which the the gRPC server will be hosted.
                             Default: 50051
    -s, --symbol <SYMBOL>    (Optional) Currency pair to subscribe to. Default: ETH/BTC
```

Run gRPC server:

```
cargo run --bin orderly-server
```
or with logs and options:
```
env RUST_LOG=info cargo run --bin orderly-server -- --symbol ETH/BTC --port 50051
```
Exclude certain exchanges:

```
cargo run --bin orderly-server -- --no-binance --no-bitstamp
```

Client
-----

Connects to the gRPC server and streams the orderbook summary.

<img src="https://user-images.githubusercontent.com/1086619/169551698-3d59df5d-73db-47a3-a84d-cb0d2d0dd678.jpg" width="700"/>

```
USAGE:
    orderly-client [OPTIONS]

OPTIONS:
    -h, --help           Print help information
    -p, --port <PORT>    (Optional) Port number of the gRPC server. Default: 50051
```

Run gRPC client:

```
cargo run --bin orderly-client
```
or with logs and options:

```
env RUST_LOG=info cargo run --bin orderly-client -- --port 50051
```

